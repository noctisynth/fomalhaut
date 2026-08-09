use std::{
    error::Error,
    fmt,
    io::{BufReader, BufWriter},
    process::{Child, ChildStdin, Command, Stdio},
    sync::mpsc::{self, Receiver, RecvTimeoutError},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use fomalhaut_core::{
    AuthConversation, AuthState, BackendError, ConversationBackend, MessageLevel, PromptId,
    PromptKind, ReauthBackend, Secret,
};

use crate::{
    CurrentUserIdentity, PAM_WORKER_ARGUMENT,
    ipc::{
        IpcError, MAX_TRANSACTION_FRAMES, ParentMessage, WorkerMessage, WorkerMessageLevel,
        WorkerPromptKind, read_worker_message, write_parent_message,
    },
};

const WORKER_CHANNEL_CAPACITY: usize = 8;
const WORKER_STEP_TIMEOUT: Duration = Duration::from_secs(30);
const WORKER_EXIT_TIMEOUT: Duration = Duration::from_secs(5);
const WORKER_EXIT_POLL_INTERVAL: Duration = Duration::from_millis(10);

/// Failure while preparing the first isolated PAM worker before acquiring the session lock.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PamBackendError {
    /// The one-shot worker process could not be started.
    Spawn,
    /// The worker did not become ready through the bounded startup protocol.
    NotReady,
}

impl fmt::Display for PamBackendError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Spawn => "the isolated PAM worker could not be started",
            Self::NotReady => "the isolated PAM worker did not become ready",
        })
    }
}

impl Error for PamBackendError {}

/// Current-user reauthentication backend backed by one process per PAM transaction.
pub struct PamReauthBackend {
    identity: CurrentUserIdentity,
    conversation: AuthConversation,
    factory: Box<dyn WorkerFactory>,
    worker: Option<Box<dyn WorkerTransport>>,
    worker_prompt: Option<u64>,
    received_frames: usize,
}

impl PamReauthBackend {
    /// Prepares the first one-shot PAM worker before the native host requests a session lock.
    pub fn connect(identity: CurrentUserIdentity) -> Result<Self, PamBackendError> {
        Self::connect_with_factory(identity, Box::new(ProcessWorkerFactory))
    }

    fn connect_with_factory(
        identity: CurrentUserIdentity,
        factory: Box<dyn WorkerFactory>,
    ) -> Result<Self, PamBackendError> {
        let worker = factory.spawn(&identity).map_err(|failure| match failure {
            WorkerFailure::Spawn => PamBackendError::Spawn,
            WorkerFailure::Timeout
            | WorkerFailure::Disconnected
            | WorkerFailure::Protocol
            | WorkerFailure::ExitTimeout
            | WorkerFailure::ExitStatus
            | WorkerFailure::Reader => PamBackendError::NotReady,
        })?;
        Ok(Self {
            identity,
            conversation: AuthConversation::new(),
            factory,
            worker: Some(worker),
            worker_prompt: None,
            received_frames: 0,
        })
    }

    fn begin_transaction(&mut self) -> Result<(), BackendError> {
        self.conversation.begin()?;
        self.worker_prompt = None;
        self.received_frames = 0;
        if self.worker.is_none() {
            self.worker = Some(self.factory.spawn(&self.identity).map_err(|_| {
                self.conversation.fail();
                BackendError::Unavailable
            })?);
        }
        let Some(worker) = self.worker.as_mut() else {
            self.conversation.fail();
            return Err(BackendError::Unavailable);
        };
        if worker.send_begin().is_err() {
            self.fail_worker();
            return Err(BackendError::Unavailable);
        }
        self.drive_worker()
    }

    fn answer_prompt(&mut self, prompt: PromptId, response: Secret) -> Result<(), BackendError> {
        self.conversation.begin_response(prompt)?;
        let Some(worker_prompt) = self.worker_prompt.take() else {
            self.fail_worker();
            return Err(BackendError::Protocol);
        };
        let response = response.into_inner().into_bytes();
        let result = self
            .worker
            .as_mut()
            .ok_or(WorkerFailure::Disconnected)
            .and_then(|worker| worker.send_answer(worker_prompt, response));
        if let Err(failure) = result {
            if let WorkerFailure::Protocol = failure {
                self.protocol_failure();
                return Err(BackendError::Protocol);
            }
            self.fail_worker();
            return Err(BackendError::Unavailable);
        }
        self.drive_worker()
    }

    fn drive_worker(&mut self) -> Result<(), BackendError> {
        loop {
            self.received_frames = self.received_frames.checked_add(1).ok_or_else(|| {
                self.protocol_failure();
                BackendError::Protocol
            })?;
            if self.received_frames > MAX_TRANSACTION_FRAMES {
                self.protocol_failure();
                return Err(BackendError::Protocol);
            }
            let message = match self
                .worker
                .as_mut()
                .ok_or(WorkerFailure::Disconnected)
                .and_then(|worker| worker.receive())
            {
                Ok(message) => message,
                Err(WorkerFailure::Protocol) => {
                    self.protocol_failure();
                    return Err(BackendError::Protocol);
                }
                Err(
                    WorkerFailure::Spawn
                    | WorkerFailure::Timeout
                    | WorkerFailure::Disconnected
                    | WorkerFailure::ExitTimeout
                    | WorkerFailure::ExitStatus
                    | WorkerFailure::Reader,
                ) => {
                    self.fail_worker();
                    return Err(BackendError::Unavailable);
                }
            };
            match message {
                WorkerMessage::Ready => {
                    self.protocol_failure();
                    return Err(BackendError::Protocol);
                }
                WorkerMessage::Prompt {
                    prompt,
                    kind,
                    message,
                } => {
                    if self.worker_prompt.is_some() {
                        self.protocol_failure();
                        return Err(BackendError::Protocol);
                    }
                    let kind = match kind {
                        WorkerPromptKind::Secret => PromptKind::Secret,
                        WorkerPromptKind::Visible => PromptKind::Visible,
                    };
                    if self.conversation.emit_prompt(kind, message).is_err() {
                        self.protocol_failure();
                        return Err(BackendError::Protocol);
                    }
                    self.worker_prompt = Some(prompt);
                    return Ok(());
                }
                WorkerMessage::Message { level, text } => {
                    let level = match level {
                        WorkerMessageLevel::Info => MessageLevel::Info,
                        WorkerMessageLevel::Error => MessageLevel::Error,
                    };
                    if self.conversation.emit_message(level, text).is_err() {
                        self.protocol_failure();
                        return Err(BackendError::Protocol);
                    }
                }
                WorkerMessage::Authenticated => {
                    self.finish_worker()?;
                    let identity = self.identity.authenticated_identity()?;
                    self.conversation.authenticated(identity)?;
                    return Ok(());
                }
                WorkerMessage::Rejected => {
                    self.finish_worker()?;
                    self.conversation.authentication_failed()?;
                    return Ok(());
                }
                WorkerMessage::Fatal => {
                    self.fail_worker();
                    return Err(BackendError::Service);
                }
            }
        }
    }

    fn cancel_transaction(&mut self) -> Result<(), BackendError> {
        self.conversation.begin_cancel()?;
        self.terminate_worker();
        self.worker_prompt = None;
        self.conversation.cancelled()?;
        Ok(())
    }

    fn finish_worker(&mut self) -> Result<(), BackendError> {
        let Some(mut worker) = self.worker.take() else {
            self.conversation.disconnect();
            return Err(BackendError::Unavailable);
        };
        if let Err(failure) = worker.finish() {
            eprintln!(
                "Fomalhaut PAM worker terminal cleanup failed: {}",
                failure.diagnostic()
            );
            worker.terminate();
            self.conversation.disconnect();
            return Err(BackendError::Unavailable);
        }
        self.worker_prompt = None;
        Ok(())
    }

    fn protocol_failure(&mut self) {
        self.terminate_worker();
        self.worker_prompt = None;
        self.conversation.fail();
    }

    fn fail_worker(&mut self) {
        self.terminate_worker();
        self.worker_prompt = None;
        self.conversation.fail();
    }

    fn terminate_worker(&mut self) {
        if let Some(mut worker) = self.worker.take() {
            worker.terminate();
        }
    }
}

impl ConversationBackend for PamReauthBackend {
    fn state(&self) -> AuthState {
        self.conversation.state()
    }

    fn needs_cancel(&self) -> bool {
        self.conversation.needs_cancel()
    }

    async fn respond(&mut self, prompt: PromptId, response: Secret) -> Result<(), BackendError> {
        self.answer_prompt(prompt, response)
    }

    async fn cancel(&mut self) -> Result<(), BackendError> {
        self.cancel_transaction()
    }

    async fn next_event(&mut self) -> Result<fomalhaut_core::AuthEvent, BackendError> {
        self.conversation.next_event().map_err(BackendError::from)
    }
}

impl ReauthBackend for PamReauthBackend {
    async fn begin_reauth(&mut self) -> Result<(), BackendError> {
        self.begin_transaction()
    }
}

impl Drop for PamReauthBackend {
    fn drop(&mut self) {
        self.terminate_worker();
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WorkerFailure {
    Spawn,
    Timeout,
    Disconnected,
    Protocol,
    ExitTimeout,
    ExitStatus,
    Reader,
}

impl WorkerFailure {
    const fn diagnostic(self) -> &'static str {
        match self {
            Self::Spawn => "spawn",
            Self::Timeout => "message timeout",
            Self::Disconnected => "IPC disconnected",
            Self::Protocol => "IPC protocol",
            Self::ExitTimeout => "exit timeout",
            Self::ExitStatus => "non-zero or unavailable exit status",
            Self::Reader => "reader thread",
        }
    }
}

trait WorkerFactory: Send {
    fn spawn(
        &self,
        identity: &CurrentUserIdentity,
    ) -> Result<Box<dyn WorkerTransport>, WorkerFailure>;
}

trait WorkerTransport: Send {
    fn send_begin(&mut self) -> Result<(), WorkerFailure>;
    fn send_answer(&mut self, prompt: u64, response: Vec<u8>) -> Result<(), WorkerFailure>;
    fn receive(&mut self) -> Result<WorkerMessage, WorkerFailure>;
    fn finish(&mut self) -> Result<(), WorkerFailure>;
    fn terminate(&mut self);
}

struct ProcessWorkerFactory;

impl WorkerFactory for ProcessWorkerFactory {
    fn spawn(
        &self,
        identity: &CurrentUserIdentity,
    ) -> Result<Box<dyn WorkerTransport>, WorkerFailure> {
        ProcessWorker::spawn(identity).map(|worker| Box::new(worker) as Box<dyn WorkerTransport>)
    }
}

struct ProcessWorker {
    child: Option<Child>,
    input: Option<BufWriter<ChildStdin>>,
    messages: Receiver<Result<WorkerMessage, IpcError>>,
    reader: Option<JoinHandle<()>>,
}

impl ProcessWorker {
    fn spawn(identity: &CurrentUserIdentity) -> Result<Self, WorkerFailure> {
        let executable = std::env::current_exe().map_err(|_| WorkerFailure::Spawn)?;
        let mut child = Command::new(executable)
            .arg(PAM_WORKER_ARGUMENT)
            .env_clear()
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|_| WorkerFailure::Spawn)?;
        let Some(stdin) = child.stdin.take() else {
            terminate_child(&mut child);
            return Err(WorkerFailure::Spawn);
        };
        let Some(stdout) = child.stdout.take() else {
            terminate_child(&mut child);
            return Err(WorkerFailure::Spawn);
        };
        let (sender, messages) = mpsc::sync_channel(WORKER_CHANNEL_CAPACITY);
        let reader = thread::Builder::new()
            .name("fomalhaut-pam-ipc".to_owned())
            .spawn(move || {
                let mut stdout = BufReader::new(stdout);
                loop {
                    let message = read_worker_message(&mut stdout);
                    let terminal = matches!(
                        message,
                        Ok(WorkerMessage::Authenticated
                            | WorkerMessage::Rejected
                            | WorkerMessage::Fatal)
                    );
                    if sender.send(message).is_err() || terminal {
                        return;
                    }
                }
            })
            .map_err(|_| {
                terminate_child(&mut child);
                WorkerFailure::Spawn
            })?;
        let mut worker = Self {
            child: Some(child),
            input: Some(BufWriter::new(stdin)),
            messages,
            reader: Some(reader),
        };
        let prepare = ParentMessage::Prepare(identity.username().to_owned());
        if worker.write(&prepare).is_err() || !matches!(worker.receive(), Ok(WorkerMessage::Ready))
        {
            worker.terminate();
            return Err(WorkerFailure::Protocol);
        }
        Ok(worker)
    }

    fn write(&mut self, message: &ParentMessage) -> Result<(), WorkerFailure> {
        let Some(input) = self.input.as_mut() else {
            return Err(WorkerFailure::Disconnected);
        };
        write_parent_message(input, message).map_err(map_ipc_failure)
    }

    fn wait_for_exit(&mut self) -> Result<(), WorkerFailure> {
        self.input.take();
        let deadline = Instant::now() + WORKER_EXIT_TIMEOUT;
        let status = loop {
            let Some(child) = self.child.as_mut() else {
                return Err(WorkerFailure::ExitStatus);
            };
            match child.try_wait() {
                Ok(Some(status)) => break status,
                Ok(None) if Instant::now() < deadline => {
                    thread::sleep(WORKER_EXIT_POLL_INTERVAL);
                }
                Ok(None) => return Err(WorkerFailure::ExitTimeout),
                Err(_) => return Err(WorkerFailure::ExitStatus),
            }
        };
        self.child.take();
        if let Some(reader) = self.reader.take()
            && reader.join().is_err()
        {
            return Err(WorkerFailure::Reader);
        }
        if status.success() {
            Ok(())
        } else {
            Err(WorkerFailure::ExitStatus)
        }
    }
}

impl WorkerTransport for ProcessWorker {
    fn send_begin(&mut self) -> Result<(), WorkerFailure> {
        self.write(&ParentMessage::Begin)
    }

    fn send_answer(&mut self, prompt: u64, response: Vec<u8>) -> Result<(), WorkerFailure> {
        let mut message = ParentMessage::Answer { prompt, response };
        let result = self.write(&message);
        if let ParentMessage::Answer { response, .. } = &mut message {
            response.fill(0);
        }
        result
    }

    fn receive(&mut self) -> Result<WorkerMessage, WorkerFailure> {
        match self.messages.recv_timeout(WORKER_STEP_TIMEOUT) {
            Ok(Ok(message)) => Ok(message),
            Ok(Err(error)) => Err(map_ipc_failure(error)),
            Err(RecvTimeoutError::Timeout) => Err(WorkerFailure::Timeout),
            Err(RecvTimeoutError::Disconnected) => Err(WorkerFailure::Disconnected),
        }
    }

    fn finish(&mut self) -> Result<(), WorkerFailure> {
        self.wait_for_exit()
    }

    fn terminate(&mut self) {
        self.input.take();
        if let Some(mut child) = self.child.take() {
            terminate_child(&mut child);
        }
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
    }
}

impl Drop for ProcessWorker {
    fn drop(&mut self) {
        self.terminate();
    }
}

fn map_ipc_failure(error: IpcError) -> WorkerFailure {
    match error {
        IpcError::Disconnected | IpcError::Io => WorkerFailure::Disconnected,
        IpcError::InvalidFrame | IpcError::LimitExceeded => WorkerFailure::Protocol,
    }
}

fn terminate_child(child: &mut Child) {
    match child.try_wait() {
        Ok(Some(_)) => {}
        Ok(None) | Err(_) => {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::VecDeque,
        future::Future,
        sync::{Arc, Mutex},
        task::{Context, Poll, Waker},
    };

    use fomalhaut_core::{
        AuthEvent, AuthState, ConversationBackend, PromptKind, ReauthBackend, Secret,
    };

    use crate::{
        CurrentUserIdentity,
        ipc::{MAX_TRANSACTION_FRAMES, WorkerMessage, WorkerMessageLevel, WorkerPromptKind},
    };

    use super::{PamReauthBackend, WorkerFactory, WorkerFailure, WorkerTransport};

    fn block_on<F: Future>(future: F) -> F::Output {
        let mut context = Context::from_waker(Waker::noop());
        let mut future = std::pin::pin!(future);
        loop {
            match future.as_mut().poll(&mut context) {
                Poll::Ready(output) => return output,
                Poll::Pending => std::thread::yield_now(),
            }
        }
    }

    #[derive(Default)]
    struct SharedWorkerState {
        answers: Vec<(u64, usize)>,
        begins: usize,
        terminations: usize,
    }

    struct ScriptedFactory {
        scripts: Mutex<VecDeque<VecDeque<Result<WorkerMessage, WorkerFailure>>>>,
        state: Arc<Mutex<SharedWorkerState>>,
    }

    impl ScriptedFactory {
        fn new(scripts: Vec<Vec<WorkerMessage>>) -> (Self, Arc<Mutex<SharedWorkerState>>) {
            Self::with_results(
                scripts
                    .into_iter()
                    .map(|script| script.into_iter().map(Ok).collect())
                    .collect(),
            )
        }

        fn with_results(
            scripts: Vec<Vec<Result<WorkerMessage, WorkerFailure>>>,
        ) -> (Self, Arc<Mutex<SharedWorkerState>>) {
            let state = Arc::new(Mutex::new(SharedWorkerState::default()));
            let scripts = scripts.into_iter().map(VecDeque::from).collect();
            (
                Self {
                    scripts: Mutex::new(scripts),
                    state: Arc::clone(&state),
                },
                state,
            )
        }
    }

    impl WorkerFactory for ScriptedFactory {
        fn spawn(
            &self,
            _identity: &CurrentUserIdentity,
        ) -> Result<Box<dyn WorkerTransport>, WorkerFailure> {
            let mut scripts = self.scripts.lock().map_err(|_| WorkerFailure::Spawn)?;
            let mut messages = scripts.pop_front().ok_or(WorkerFailure::Spawn)?;
            if !matches!(messages.pop_front(), Some(Ok(WorkerMessage::Ready))) {
                return Err(WorkerFailure::Protocol);
            }
            Ok(Box::new(ScriptedWorker {
                messages,
                state: Arc::clone(&self.state),
                terminal_seen: false,
            }))
        }
    }

    struct ScriptedWorker {
        messages: VecDeque<Result<WorkerMessage, WorkerFailure>>,
        state: Arc<Mutex<SharedWorkerState>>,
        terminal_seen: bool,
    }

    impl WorkerTransport for ScriptedWorker {
        fn send_begin(&mut self) -> Result<(), WorkerFailure> {
            self.state
                .lock()
                .map_err(|_| WorkerFailure::Disconnected)?
                .begins += 1;
            Ok(())
        }

        fn send_answer(&mut self, prompt: u64, mut response: Vec<u8>) -> Result<(), WorkerFailure> {
            self.state
                .lock()
                .map_err(|_| WorkerFailure::Disconnected)?
                .answers
                .push((prompt, response.len()));
            response.fill(0);
            Ok(())
        }

        fn receive(&mut self) -> Result<WorkerMessage, WorkerFailure> {
            let message = self
                .messages
                .pop_front()
                .unwrap_or(Err(WorkerFailure::Disconnected))?;
            self.terminal_seen = matches!(
                message,
                WorkerMessage::Authenticated | WorkerMessage::Rejected
            );
            Ok(message)
        }

        fn finish(&mut self) -> Result<(), WorkerFailure> {
            if self.terminal_seen {
                Ok(())
            } else {
                Err(WorkerFailure::ExitStatus)
            }
        }

        fn terminate(&mut self) {
            if let Ok(mut state) = self.state.lock() {
                state.terminations += 1;
            }
        }
    }

    fn identity() -> CurrentUserIdentity {
        CurrentUserIdentity::from_trusted_parts(1000, "alice", "Alice")
            .expect("test identity is valid")
    }

    #[test]
    fn multi_round_conversation_authenticates_fixed_identity() {
        let (factory, state) = ScriptedFactory::new(vec![vec![
            WorkerMessage::Ready,
            WorkerMessage::Prompt {
                prompt: 11,
                kind: WorkerPromptKind::Secret,
                message: "Password:".to_owned(),
            },
            WorkerMessage::Prompt {
                prompt: 12,
                kind: WorkerPromptKind::Visible,
                message: "OTP:".to_owned(),
            },
            WorkerMessage::Authenticated,
        ]]);
        let mut backend = PamReauthBackend::connect_with_factory(identity(), Box::new(factory))
            .expect("prepared worker is ready");

        block_on(backend.begin_reauth()).expect("authentication begins");
        let first = block_on(backend.next_event()).expect("first prompt is queued");
        let first_id = match first {
            AuthEvent::Prompt {
                id,
                kind: PromptKind::Secret,
                ..
            } => id,
            _ => panic!("expected secret prompt"),
        };
        block_on(backend.respond(first_id, Secret::new("password")))
            .expect("first response advances PAM");
        let second = block_on(backend.next_event()).expect("second prompt is queued");
        let second_id = match second {
            AuthEvent::Prompt {
                id,
                kind: PromptKind::Visible,
                ..
            } => id,
            _ => panic!("expected visible prompt"),
        };
        block_on(backend.respond(second_id, Secret::new("123456")))
            .expect("second response completes PAM");
        assert!(matches!(
            block_on(backend.next_event()),
            Ok(AuthEvent::Authenticated(_))
        ));
        assert_eq!(backend.state(), AuthState::Authenticated);
        let state = state.lock().expect("test state mutex is available");
        assert_eq!(state.begins, 1);
        assert_eq!(state.answers, vec![(11, 8), (12, 6)]);
    }

    #[test]
    fn rejection_exits_worker_and_retry_uses_a_fresh_transaction() {
        let (factory, state) = ScriptedFactory::new(vec![
            vec![WorkerMessage::Ready, WorkerMessage::Rejected],
            vec![
                WorkerMessage::Ready,
                WorkerMessage::Prompt {
                    prompt: 1,
                    kind: WorkerPromptKind::Secret,
                    message: "Password:".to_owned(),
                },
            ],
        ]);
        let mut backend = PamReauthBackend::connect_with_factory(identity(), Box::new(factory))
            .expect("prepared worker is ready");
        block_on(backend.begin_reauth()).expect("rejected transaction completes normally");
        assert!(matches!(
            block_on(backend.next_event()),
            Ok(AuthEvent::AuthenticationFailed)
        ));
        assert_eq!(backend.state(), AuthState::Failed);

        block_on(backend.begin_reauth()).expect("explicit retry starts a fresh worker");
        assert!(matches!(
            block_on(backend.next_event()),
            Ok(AuthEvent::Prompt { .. })
        ));
        assert_eq!(state.lock().expect("test mutex is available").begins, 2);
    }

    #[test]
    fn cancellation_terminates_worker_without_unlocking_or_replaying_answers() {
        let (factory, state) = ScriptedFactory::new(vec![vec![
            WorkerMessage::Ready,
            WorkerMessage::Prompt {
                prompt: 3,
                kind: WorkerPromptKind::Secret,
                message: "Password:".to_owned(),
            },
        ]]);
        let mut backend = PamReauthBackend::connect_with_factory(identity(), Box::new(factory))
            .expect("prepared worker is ready");
        block_on(backend.begin_reauth()).expect("authentication begins");
        let _ = block_on(backend.next_event()).expect("prompt is queued");
        block_on(backend.cancel()).expect("active transaction cancels");
        assert!(matches!(
            block_on(backend.next_event()),
            Ok(AuthEvent::Cancelled)
        ));
        assert_eq!(backend.state(), AuthState::Idle);
        let state = state.lock().expect("test mutex is available");
        assert_eq!(state.terminations, 1);
        assert!(state.answers.is_empty());
    }

    #[test]
    fn maps_noninteractive_messages_before_the_next_prompt() {
        let (factory, _) = ScriptedFactory::new(vec![vec![
            WorkerMessage::Ready,
            WorkerMessage::Message {
                level: WorkerMessageLevel::Info,
                text: "Touch your security key".to_owned(),
            },
            WorkerMessage::Message {
                level: WorkerMessageLevel::Error,
                text: "Previous token was rejected".to_owned(),
            },
            WorkerMessage::Prompt {
                prompt: 4,
                kind: WorkerPromptKind::Visible,
                message: "Token:".to_owned(),
            },
        ]]);
        let mut backend = PamReauthBackend::connect_with_factory(identity(), Box::new(factory))
            .expect("prepared worker is ready");
        block_on(backend.begin_reauth()).expect("messages advance to the prompt");
        assert!(matches!(
            block_on(backend.next_event()),
            Ok(AuthEvent::Message {
                level: fomalhaut_core::MessageLevel::Info,
                ..
            })
        ));
        assert!(matches!(
            block_on(backend.next_event()),
            Ok(AuthEvent::Message {
                level: fomalhaut_core::MessageLevel::Error,
                ..
            })
        ));
        assert!(matches!(
            block_on(backend.next_event()),
            Ok(AuthEvent::Prompt {
                kind: PromptKind::Visible,
                ..
            })
        ));
    }

    #[test]
    fn stale_prompt_is_rejected_before_any_answer_reaches_the_worker() {
        let (factory, state) = ScriptedFactory::new(vec![
            vec![
                WorkerMessage::Ready,
                WorkerMessage::Prompt {
                    prompt: 8,
                    kind: WorkerPromptKind::Secret,
                    message: "Password:".to_owned(),
                },
            ],
            vec![
                WorkerMessage::Ready,
                WorkerMessage::Prompt {
                    prompt: 9,
                    kind: WorkerPromptKind::Secret,
                    message: "Password:".to_owned(),
                },
            ],
        ]);
        let mut backend = PamReauthBackend::connect_with_factory(identity(), Box::new(factory))
            .expect("prepared worker is ready");
        block_on(backend.begin_reauth()).expect("authentication begins");
        let stale = match block_on(backend.next_event()).expect("first prompt is queued") {
            AuthEvent::Prompt { id, .. } => id,
            _ => panic!("expected prompt"),
        };
        block_on(backend.cancel()).expect("first transaction cancels");
        let _ = block_on(backend.next_event()).expect("cancellation event is queued");
        block_on(backend.begin_reauth()).expect("second transaction begins");
        let _ = block_on(backend.next_event()).expect("second prompt is queued");
        assert!(block_on(backend.respond(stale, Secret::new("not sent"))).is_err());
        assert!(
            state
                .lock()
                .expect("test state mutex is available")
                .answers
                .is_empty()
        );
        block_on(backend.cancel()).expect("active prompt remains cancellable");
    }

    #[test]
    fn worker_disconnect_fails_closed_and_retry_does_not_replay_the_answer() {
        let (factory, state) = ScriptedFactory::with_results(vec![
            vec![
                Ok(WorkerMessage::Ready),
                Ok(WorkerMessage::Prompt {
                    prompt: 5,
                    kind: WorkerPromptKind::Secret,
                    message: "Password:".to_owned(),
                }),
                Err(WorkerFailure::Disconnected),
            ],
            vec![
                Ok(WorkerMessage::Ready),
                Ok(WorkerMessage::Prompt {
                    prompt: 9,
                    kind: WorkerPromptKind::Secret,
                    message: "Password:".to_owned(),
                }),
            ],
        ]);
        let mut backend = PamReauthBackend::connect_with_factory(identity(), Box::new(factory))
            .expect("prepared worker is ready");
        block_on(backend.begin_reauth()).expect("authentication begins");
        let first = match block_on(backend.next_event()).expect("first prompt is queued") {
            AuthEvent::Prompt { id, .. } => id,
            _ => panic!("expected first prompt"),
        };
        assert!(block_on(backend.respond(first, Secret::new("first secret"))).is_err());
        assert_eq!(backend.state(), AuthState::Failed);

        block_on(backend.begin_reauth()).expect("explicit retry creates a new worker");
        assert!(matches!(
            block_on(backend.next_event()),
            Ok(AuthEvent::Prompt { .. })
        ));
        let state = state.lock().expect("test state mutex is available");
        assert_eq!(state.answers, vec![(5, 12)]);
        assert_eq!(state.begins, 2);
    }

    #[test]
    fn timeout_fails_closed_and_terminates_the_transaction() {
        let (factory, state) = ScriptedFactory::with_results(vec![vec![
            Ok(WorkerMessage::Ready),
            Err(WorkerFailure::Timeout),
        ]]);
        let mut backend = PamReauthBackend::connect_with_factory(identity(), Box::new(factory))
            .expect("prepared worker is ready");
        assert!(block_on(backend.begin_reauth()).is_err());
        assert_eq!(backend.state(), AuthState::Failed);
        assert_eq!(
            state
                .lock()
                .expect("test state mutex is available")
                .terminations,
            1
        );
    }

    #[test]
    fn frame_limit_rejects_unbounded_pam_message_streams() {
        let mut script = vec![WorkerMessage::Ready];
        script.extend(
            (0..=MAX_TRANSACTION_FRAMES).map(|_| WorkerMessage::Message {
                level: WorkerMessageLevel::Info,
                text: "bounded".to_owned(),
            }),
        );
        let (factory, state) = ScriptedFactory::new(vec![script]);
        let mut backend = PamReauthBackend::connect_with_factory(identity(), Box::new(factory))
            .expect("prepared worker is ready");
        assert!(block_on(backend.begin_reauth()).is_err());
        assert_eq!(backend.state(), AuthState::Failed);
        assert_eq!(
            state
                .lock()
                .expect("test state mutex is available")
                .terminations,
            1
        );
    }
}
