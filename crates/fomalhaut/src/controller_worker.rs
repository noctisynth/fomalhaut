//! Dedicated async worker that owns the greetd client outside the GTK main thread.

use std::{
    cell::RefCell,
    error::Error,
    fmt,
    path::PathBuf,
    sync::mpsc::{self, Receiver, SyncSender, TrySendError},
    thread::{self, JoinHandle},
};

use fomalhaut_config::{PowerConfig, UserDiscoveryConfig};
use fomalhaut_greetd::GreeterClient;
use fomalhaut_gtk::{BridgeController, ControllerBatch, ControllerOutput, SubmitError};
use fomalhaut_logind::LogindPowerControl;
use fomalhaut_user::discover_users;
use fomalhaut_web::{
    controller::{GreeterController, TrustedSession},
    protocol::RequestEnvelope,
};

const CHANNEL_CAPACITY: usize = 8;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GreeterAction {
    SessionStarted,
}

pub type WorkerOutput = ControllerOutput<GreeterAction>;

enum WorkerCommand {
    Request {
        epoch: u64,
        request: RequestEnvelope,
    },
    CancelForPage,
    Shutdown,
}

#[derive(Debug)]
pub struct WorkerSpawnError(std::io::Error);

impl fmt::Display for WorkerSpawnError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("the authentication worker thread could not be started")
    }
}

impl Error for WorkerSpawnError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.0)
    }
}

pub struct WorkerHandle {
    sender: SyncSender<WorkerCommand>,
    thread: RefCell<Option<JoinHandle<()>>>,
}

impl WorkerHandle {
    pub fn spawn(
        socket_path: PathBuf,
        sessions: Vec<TrustedSession>,
        users: UserDiscoveryConfig,
        power: PowerConfig,
    ) -> Result<(Self, Receiver<WorkerOutput>), WorkerSpawnError> {
        let (command_sender, command_receiver) = mpsc::sync_channel(CHANNEL_CAPACITY);
        let (output_sender, output_receiver) = mpsc::sync_channel(CHANNEL_CAPACITY);
        let thread = thread::Builder::new()
            .name("fomalhaut-auth-controller".to_owned())
            .spawn(move || {
                run_worker(
                    socket_path,
                    sessions,
                    users,
                    power,
                    command_receiver,
                    output_sender,
                );
            })
            .map_err(WorkerSpawnError)?;

        Ok((
            Self {
                sender: command_sender,
                thread: RefCell::new(Some(thread)),
            },
            output_receiver,
        ))
    }

    pub fn submit(&self, epoch: u64, request: RequestEnvelope) -> Result<(), SubmitError> {
        self.try_send(WorkerCommand::Request { epoch, request })
    }

    pub fn cancel_for_page(&self) -> Result<(), SubmitError> {
        self.try_send(WorkerCommand::CancelForPage)
    }

    pub fn shutdown(&self) {
        let Some(thread) = self.thread.borrow_mut().take() else {
            return;
        };
        let _ = self.sender.send(WorkerCommand::Shutdown);
        if thread.join().is_err() {
            eprintln!("Fomalhaut authentication worker terminated unexpectedly");
        }
    }

    fn try_send(&self, command: WorkerCommand) -> Result<(), SubmitError> {
        match self.sender.try_send(command) {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(_)) => Err(SubmitError::Busy),
            Err(TrySendError::Disconnected(_)) => Err(SubmitError::Stopped),
        }
    }
}

impl Drop for WorkerHandle {
    fn drop(&mut self) {
        self.shutdown();
    }
}

impl BridgeController for WorkerHandle {
    fn submit(&self, epoch: u64, request: RequestEnvelope) -> Result<(), SubmitError> {
        Self::submit(self, epoch, request)
    }

    fn cancel_for_page(&self) -> Result<(), SubmitError> {
        Self::cancel_for_page(self)
    }

    fn shutdown(&self) {
        Self::shutdown(self);
    }
}

fn run_worker(
    socket_path: PathBuf,
    sessions: Vec<TrustedSession>,
    user_config: UserDiscoveryConfig,
    power_config: PowerConfig,
    commands: Receiver<WorkerCommand>,
    outputs: SyncSender<WorkerOutput>,
) {
    let discovered = match discover_users(user_config) {
        Ok(discovered) => discovered,
        Err(_) => {
            eprintln!("Fomalhaut user discovery failed; manual login remains available");
            Default::default()
        }
    };
    let (users, avatars) = discovered.into_parts();
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .build()
    {
        Ok(runtime) => runtime,
        Err(_) => {
            let _ = outputs.send(WorkerOutput::Fatal(
                "the authentication runtime could not be created",
            ));
            return;
        }
    };
    let client = match runtime.block_on(GreeterClient::connect(socket_path)) {
        Ok(client) => client,
        Err(_) => {
            let _ = outputs.send(WorkerOutput::Fatal(
                "the authentication service could not be reached",
            ));
            return;
        }
    };
    let power = LogindPowerControl::discover(&power_config);
    let mut controller = GreeterController::with_power_control(client, sessions, users, power);
    if outputs.send(WorkerOutput::Ready(avatars)).is_err() {
        let _ = runtime.block_on(controller.cancel_for_lifecycle());
        return;
    }

    while let Ok(command) = commands.recv() {
        match command {
            WorkerCommand::Request { epoch, request } => {
                let batch = match runtime.block_on(controller.handle(request)) {
                    Ok(batch) => batch,
                    Err(_) => {
                        send_fatal(
                            &outputs,
                            "the authentication controller could not maintain public state",
                        );
                        break;
                    }
                };
                let session_started = batch.session_started();
                let (response, event_scripts) = match batch.into_bridge_parts() {
                    Ok(parts) => parts,
                    Err(_) => {
                        send_fatal(
                            &outputs,
                            "the authentication controller could not encode its output",
                        );
                        break;
                    }
                };
                if outputs
                    .send(WorkerOutput::Batch(ControllerBatch {
                        epoch,
                        response,
                        event_scripts,
                        terminal: session_started.then_some(GreeterAction::SessionStarted),
                    }))
                    .is_err()
                {
                    break;
                }
            }
            WorkerCommand::CancelForPage => {
                if runtime.block_on(controller.cancel_for_lifecycle()).is_err() {
                    send_fatal(
                        &outputs,
                        "the authentication controller could not cancel a stale page",
                    );
                    break;
                }
            }
            WorkerCommand::Shutdown => {
                if runtime.block_on(controller.cancel_for_lifecycle()).is_err() {
                    send_fatal(
                        &outputs,
                        "the authentication controller could not cancel during shutdown",
                    );
                }
                return;
            }
        }
    }

    let _ = runtime.block_on(controller.cancel_for_lifecycle());
}

fn send_fatal(outputs: &SyncSender<WorkerOutput>, message: &'static str) {
    let _ = outputs.send(WorkerOutput::Fatal(message));
}

#[cfg(test)]
mod tests {
    use std::{
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
        time::Duration,
    };

    use super::{GreeterAction, WorkerHandle, WorkerOutput};
    use fomalhaut_config::{PowerConfig, UserDiscoveryConfig};
    use fomalhaut_core::SessionCommand;
    use fomalhaut_web::{
        controller::TrustedSession,
        protocol::{SessionKind, SessionSummary, decode_request},
    };
    use greetd_ipc::{AuthMessageType, ErrorType, Request, Response, codec::TokioCodec};

    static SOCKET_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn socket_path() -> PathBuf {
        let sequence = SOCKET_COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "fomalhaut-worker-{}-{sequence}.sock",
            std::process::id()
        ))
    }

    #[test]
    fn unix_worker_drives_password_authentication_and_shutdown_cancel() {
        let path = socket_path();
        let listener = std::os::unix::net::UnixListener::bind(&path)
            .expect("unique worker test socket can be bound");
        listener
            .set_nonblocking(true)
            .expect("test listener can be made nonblocking");
        let server = std::thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_io()
                .build()
                .expect("test server runtime can be created");
            runtime.block_on(async move {
                let listener = tokio::net::UnixListener::from_std(listener)
                    .expect("test listener enters its runtime");
                let (mut stream, _) = listener.accept().await.expect("worker connects to stub");

                let create = Request::read_from(&mut stream)
                    .await
                    .expect("stub reads create request");
                assert!(matches!(
                    create,
                    Request::CreateSession { username } if username == "alice"
                ));
                Response::AuthMessage {
                    auth_message_type: AuthMessageType::Secret,
                    auth_message: "Password:".to_owned(),
                }
                .write_to(&mut stream)
                .await
                .expect("stub writes password prompt");

                let respond = Request::read_from(&mut stream)
                    .await
                    .expect("stub reads password response");
                assert!(matches!(
                    respond,
                    Request::PostAuthMessageResponse { response } if response.as_deref() == Some("incorrect")
                ));
                Response::Error {
                    error_type: ErrorType::AuthError,
                    description: "test authentication rejection".to_owned(),
                }
                    .write_to(&mut stream)
                    .await
                    .expect("stub rejects authentication");

                let failure_cancel = Request::read_from(&mut stream)
                    .await
                    .expect("worker cancels the rejected greetd session");
                assert!(matches!(failure_cancel, Request::CancelSession));
                Response::Success
                    .write_to(&mut stream)
                    .await
                    .expect("stub accepts rejected-session cancellation");

                let retry = Request::read_from(&mut stream)
                    .await
                    .expect("stub reads authentication retry");
                assert!(matches!(
                    retry,
                    Request::CreateSession { username } if username == "alice"
                ));
                Response::AuthMessage {
                    auth_message_type: AuthMessageType::Secret,
                    auth_message: "Password:".to_owned(),
                }
                .write_to(&mut stream)
                .await
                .expect("stub writes retry password prompt");

                let retry_response = Request::read_from(&mut stream)
                    .await
                    .expect("stub reads retry password response");
                assert!(matches!(
                    retry_response,
                    Request::PostAuthMessageResponse { response } if response.as_deref() == Some("correct")
                ));
                Response::Success
                    .write_to(&mut stream)
                    .await
                    .expect("stub accepts retry authentication");

                let cancel = Request::read_from(&mut stream)
                    .await
                    .expect("page change explicitly cancels authenticated session");
                assert!(matches!(cancel, Request::CancelSession));
                Response::Success
                    .write_to(&mut stream)
                    .await
                    .expect("stub accepts page cancellation");

                let page_retry = Request::read_from(&mut stream)
                    .await
                    .expect("stub reads authentication after page cancellation");
                assert!(matches!(
                    page_retry,
                    Request::CreateSession { username } if username == "alice"
                ));
                Response::AuthMessage {
                    auth_message_type: AuthMessageType::Secret,
                    auth_message: "Password:".to_owned(),
                }
                .write_to(&mut stream)
                .await
                .expect("stub writes retry prompt");

                let shutdown_cancel = Request::read_from(&mut stream)
                    .await
                    .expect("shutdown explicitly cancels waiting prompt");
                assert!(matches!(shutdown_cancel, Request::CancelSession));
                Response::Success
                    .write_to(&mut stream)
                    .await
                    .expect("stub accepts shutdown cancellation");
            });
        });

        let (worker, outputs) = WorkerHandle::spawn(
            path.clone(),
            Vec::new(),
            UserDiscoveryConfig::disabled(),
            PowerConfig::default(),
        )
        .expect("worker thread starts");
        assert!(matches!(
            outputs
                .recv_timeout(Duration::from_secs(2))
                .expect("worker reports readiness"),
            WorkerOutput::Ready(avatars) if avatars.is_empty()
        ));

        let begin = decode_request(
            br#"{"protocol":1,"id":1,"method":"auth.begin","params":{"username":"alice"}}"#,
        )
        .expect("begin request fixture is valid");
        worker.submit(7, begin).expect("begin request is queued");
        let WorkerOutput::Batch(begin) = outputs
            .recv_timeout(Duration::from_secs(2))
            .expect("worker returns password prompt")
        else {
            panic!("worker must return a controller batch");
        };
        assert_eq!(begin.epoch, 7);
        assert!(
            begin
                .event_scripts
                .iter()
                .any(|event| event.contains("auth.prompt"))
        );

        let respond = decode_request(
            br#"{"protocol":1,"id":2,"method":"auth.respond","params":{"promptId":1,"response":"incorrect"}}"#,
        )
        .expect("response request fixture is valid");
        worker.submit(7, respond).expect("response is queued");
        let WorkerOutput::Batch(rejected) = outputs
            .recv_timeout(Duration::from_secs(2))
            .expect("worker returns recoverable authentication rejection")
        else {
            panic!("worker must return an authentication batch");
        };
        assert!(
            rejected
                .event_scripts
                .iter()
                .any(|event| event.contains("auth.failed"))
        );
        assert!(rejected.response.contains(r#""ok":true"#));
        assert!(!rejected.response.contains("incorrect"));

        let retry = decode_request(
            br#"{"protocol":1,"id":3,"method":"auth.begin","params":{"username":"alice"}}"#,
        )
        .expect("retry request fixture is valid");
        worker.submit(7, retry).expect("retry request is queued");
        let WorkerOutput::Batch(retry_prompt) = outputs
            .recv_timeout(Duration::from_secs(2))
            .expect("worker returns retry password prompt")
        else {
            panic!("worker must return a retry batch");
        };
        assert!(
            retry_prompt
                .event_scripts
                .iter()
                .any(|event| event.contains("auth.prompt"))
        );

        let retry_response = decode_request(
            br#"{"protocol":1,"id":4,"method":"auth.respond","params":{"promptId":2,"response":"correct"}}"#,
        )
        .expect("retry response request fixture is valid");
        worker
            .submit(7, retry_response)
            .expect("retry response is queued");
        let WorkerOutput::Batch(authenticated) = outputs
            .recv_timeout(Duration::from_secs(2))
            .expect("worker returns retry authentication success")
        else {
            panic!("worker must return an authentication batch");
        };
        assert!(
            authenticated
                .event_scripts
                .iter()
                .any(|event| event.contains("auth.succeeded"))
        );
        assert!(!authenticated.response.contains("correct"));

        worker
            .cancel_for_page()
            .expect("page cancellation is queued");
        let page_retry = decode_request(
            br#"{"protocol":1,"id":5,"method":"auth.begin","params":{"username":"alice"}}"#,
        )
        .expect("page retry request fixture is valid");
        worker
            .submit(8, page_retry)
            .expect("page retry request is queued");
        let WorkerOutput::Batch(page_retry_prompt) = outputs
            .recv_timeout(Duration::from_secs(2))
            .expect("worker returns page retry prompt")
        else {
            panic!("worker must return a retry batch");
        };
        assert_eq!(page_retry_prompt.epoch, 8);
        assert!(
            page_retry_prompt
                .event_scripts
                .iter()
                .any(|event| event.contains("auth.prompt"))
        );

        worker.shutdown();
        server.join().expect("stub server completes");
        std::fs::remove_file(path).expect("worker test socket is removable");
    }

    #[test]
    fn unix_worker_starts_only_the_host_resolved_session() {
        let path = socket_path();
        let listener = std::os::unix::net::UnixListener::bind(&path)
            .expect("unique session-start socket can be bound");
        listener
            .set_nonblocking(true)
            .expect("session-start listener can be made nonblocking");
        let server = std::thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_io()
                .build()
                .expect("session-start server runtime can be created");
            runtime.block_on(async move {
                let listener = tokio::net::UnixListener::from_std(listener)
                    .expect("session-start listener enters its runtime");
                let (mut stream, _) = listener.accept().await.expect("worker connects to stub");

                let create = Request::read_from(&mut stream)
                    .await
                    .expect("stub reads create request");
                assert!(matches!(
                    create,
                    Request::CreateSession { username } if username == "alice"
                ));
                Response::AuthMessage {
                    auth_message_type: AuthMessageType::Secret,
                    auth_message: "Password:".to_owned(),
                }
                .write_to(&mut stream)
                .await
                .expect("stub writes password prompt");

                let respond = Request::read_from(&mut stream)
                    .await
                    .expect("stub reads password response");
                assert!(matches!(
                    respond,
                    Request::PostAuthMessageResponse { response } if response.as_deref() == Some("correct")
                ));
                Response::Success
                    .write_to(&mut stream)
                    .await
                    .expect("stub accepts authentication");

                let start = Request::read_from(&mut stream)
                    .await
                    .expect("stub reads trusted session start");
                assert!(matches!(
                    start,
                    Request::StartSession { cmd, env }
                        if cmd == ["/usr/bin/fomalhaut-test-session", "--safe"]
                            && env == ["XDG_SESSION_TYPE=wayland"]
                ));
                Response::Success
                    .write_to(&mut stream)
                    .await
                    .expect("stub starts trusted session");
            });
        });

        let summary = SessionSummary::new(
            "wayland:fomalhaut-test".to_owned(),
            "Fomalhaut Test".to_owned(),
            SessionKind::Wayland,
        )
        .expect("session summary fixture is frontend-safe");
        let command = SessionCommand::new(
            vec![
                "/usr/bin/fomalhaut-test-session".to_owned(),
                "--safe".to_owned(),
            ],
            vec!["XDG_SESSION_TYPE=wayland".to_owned()],
        )
        .expect("session command fixture is non-empty");
        let sessions = vec![TrustedSession::new(summary, command)];
        let (worker, outputs) = WorkerHandle::spawn(
            path.clone(),
            sessions,
            UserDiscoveryConfig::disabled(),
            PowerConfig::default(),
        )
        .expect("session-start worker starts");
        assert!(matches!(
            outputs
                .recv_timeout(Duration::from_secs(2))
                .expect("session-start worker reports readiness"),
            WorkerOutput::Ready(avatars) if avatars.is_empty()
        ));

        let begin = decode_request(
            br#"{"protocol":1,"id":1,"method":"auth.begin","params":{"username":"alice"}}"#,
        )
        .expect("begin request fixture is valid");
        worker.submit(11, begin).expect("begin request is queued");
        assert!(matches!(
            outputs
                .recv_timeout(Duration::from_secs(2))
                .expect("worker returns password prompt"),
            WorkerOutput::Batch(_)
        ));

        let respond = decode_request(
            br#"{"protocol":1,"id":2,"method":"auth.respond","params":{"promptId":1,"response":"correct"}}"#,
        )
        .expect("response request fixture is valid");
        worker.submit(11, respond).expect("response is queued");
        let WorkerOutput::Batch(started) = outputs
            .recv_timeout(Duration::from_secs(2))
            .expect("worker returns session-start terminal batch")
        else {
            panic!("worker must return a session-start batch");
        };
        assert_eq!(started.terminal, Some(GreeterAction::SessionStarted));
        assert!(
            started
                .event_scripts
                .iter()
                .any(|event| event.contains("session.started"))
        );
        assert!(!started.response.contains("fomalhaut-test-session"));

        worker.shutdown();
        server.join().expect("session-start stub server completes");
        std::fs::remove_file(path).expect("session-start socket is removable");
    }
}
