//! Dedicated locker controller thread shared by every monitor WebView.

use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    error::Error,
    fmt,
    rc::Rc,
    sync::mpsc::{self, Receiver, SyncSender, TrySendError},
    thread::{self, JoinHandle},
};

use fomalhaut_config::PowerConfig;
use fomalhaut_core::ReauthBackend;
use fomalhaut_gtk::{
    BridgeController, ControllerBatch, ControllerOutput, ResourceAsset, SubmitError,
};
use fomalhaut_logind::LogindPowerControl;
use fomalhaut_pam::{CurrentUserIdentity, PamReauthBackend};
use fomalhaut_user::discover_current_avatar;
use fomalhaut_web::{
    bridge::event_dispatch_script,
    controller::LockerController,
    protocol::{IdentitySummary, RequestEnvelope},
};

const COMMAND_CAPACITY: usize = 16;
const VIEW_OUTPUT_CAPACITY: usize = 8;
const NATIVE_OUTPUT_CAPACITY: usize = 8;

/// Locker view actions are deliberately empty; unlock authority uses the native-only channel.
pub enum LockerViewAction {}

/// Native lifecycle notification that is never exposed to JavaScript.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeEvent {
    /// The initial PAM worker is ready, so requesting a compositor lock is safe.
    BackendReady,
    /// The controller recorded compositor lock acquisition.
    LockAcquired,
    /// Native lock acquisition failed before a usable lock was established.
    LockFailed,
    /// Reauthentication was accepted and the native host may call session-lock unlock.
    Unlock,
    /// The compositor confirmed release after an authorized unlock.
    Released,
    /// A sanitized controller or native lifecycle invariant failed.
    Fatal(&'static str),
}

/// Failure to create the dedicated locker controller thread.
#[derive(Debug)]
pub struct WorkerSpawnError(std::io::Error);

impl fmt::Display for WorkerSpawnError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("the locker controller thread could not be started")
    }
}

impl Error for WorkerSpawnError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.0)
    }
}

/// Failure to register a bounded output channel for one monitor view.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ViewRegistrationError {
    /// The monotonically increasing internal view identifier was exhausted.
    IdentifierExhausted,
    /// The controller command queue is full or disconnected.
    ControllerUnavailable,
}

impl fmt::Display for ViewRegistrationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::IdentifierExhausted => "the locker view identifier space was exhausted",
            Self::ControllerUnavailable => "the locker controller is unavailable",
        })
    }
}

impl Error for ViewRegistrationError {}

enum WorkerCommand {
    Register {
        view: u64,
        output: SyncSender<ControllerOutput<LockerViewAction>>,
    },
    Detach {
        view: u64,
    },
    Request {
        view: u64,
        epoch: u64,
        request: RequestEnvelope,
    },
    CancelForPage,
    LockAcquired,
    LockFailed,
    LockReleased,
    Shutdown,
}

/// Main-thread handle for the shared locker controller and its registered views.
pub struct WorkerHandle {
    sender: SyncSender<WorkerCommand>,
    thread: RefCell<Option<JoinHandle<()>>>,
    next_view: Cell<u64>,
}

impl WorkerHandle {
    /// Starts the controller thread and prepares its first one-shot PAM worker.
    pub fn spawn(
        identity: CurrentUserIdentity,
        power: PowerConfig,
    ) -> Result<(Rc<Self>, Receiver<NativeEvent>), WorkerSpawnError> {
        let (command_sender, command_receiver) = mpsc::sync_channel(COMMAND_CAPACITY);
        let (native_sender, native_receiver) = mpsc::sync_channel(NATIVE_OUTPUT_CAPACITY);
        let thread = thread::Builder::new()
            .name("fomalhaut-lock-controller".to_owned())
            .spawn(move || run_worker(identity, power, command_receiver, native_sender))
            .map_err(WorkerSpawnError)?;
        Ok((
            Rc::new(Self {
                sender: command_sender,
                thread: RefCell::new(Some(thread)),
                next_view: Cell::new(1),
            }),
            native_receiver,
        ))
    }

    /// Registers one monitor WebView with its own bounded output channel.
    pub fn register_view(
        self: &Rc<Self>,
    ) -> Result<
        (
            Rc<ViewController>,
            Receiver<ControllerOutput<LockerViewAction>>,
        ),
        ViewRegistrationError,
    > {
        let view = self.next_view.get();
        let next = view
            .checked_add(1)
            .ok_or(ViewRegistrationError::IdentifierExhausted)?;
        let (output, receiver) = mpsc::sync_channel(VIEW_OUTPUT_CAPACITY);
        self.try_send(WorkerCommand::Register { view, output })
            .map_err(|_| ViewRegistrationError::ControllerUnavailable)?;
        self.next_view.set(next);
        Ok((
            Rc::new(ViewController {
                sender: self.sender.clone(),
                view,
                detached: Cell::new(false),
            }),
            receiver,
        ))
    }

    /// Records compositor confirmation that the lock is active.
    pub fn mark_lock_acquired(&self) -> Result<(), SubmitError> {
        self.try_send(WorkerCommand::LockAcquired)
    }

    /// Records native failure to acquire or retain the requested lock.
    pub fn mark_lock_failed(&self) -> Result<(), SubmitError> {
        self.try_send(WorkerCommand::LockFailed)
    }

    /// Records the compositor `unlocked` signal for authorization validation and release.
    pub fn mark_lock_released(&self) -> Result<(), SubmitError> {
        self.try_send(WorkerCommand::LockReleased)
    }

    /// Stops the shared controller and waits for its PAM transaction to be cleaned up.
    pub fn shutdown(&self) {
        let Some(thread) = self.thread.borrow_mut().take() else {
            return;
        };
        let _ = self.sender.send(WorkerCommand::Shutdown);
        if thread.join().is_err() {
            eprintln!("Fomalhaut locker controller terminated unexpectedly");
        }
    }

    fn try_send(&self, command: WorkerCommand) -> Result<(), SubmitError> {
        map_try_send(self.sender.try_send(command))
    }
}

impl Drop for WorkerHandle {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// Per-monitor bridge capability that cannot stop or replace the shared controller.
pub struct ViewController {
    sender: SyncSender<WorkerCommand>,
    view: u64,
    detached: Cell<bool>,
}

impl ViewController {
    fn detach(&self) {
        if self.detached.replace(true) {
            return;
        }
        let _ = self
            .sender
            .try_send(WorkerCommand::Detach { view: self.view });
    }
}

impl BridgeController for ViewController {
    fn submit(&self, epoch: u64, request: RequestEnvelope) -> Result<(), SubmitError> {
        if self.detached.get() {
            return Err(SubmitError::Stopped);
        }
        map_try_send(self.sender.try_send(WorkerCommand::Request {
            view: self.view,
            epoch,
            request,
        }))
    }

    fn cancel_for_page(&self) -> Result<(), SubmitError> {
        if self.detached.get() {
            return Err(SubmitError::Stopped);
        }
        map_try_send(self.sender.try_send(WorkerCommand::CancelForPage))
    }

    fn shutdown(&self) {
        self.detach();
    }
}

impl Drop for ViewController {
    fn drop(&mut self) {
        self.detach();
    }
}

fn map_try_send<T>(result: Result<(), TrySendError<T>>) -> Result<(), SubmitError> {
    match result {
        Ok(()) => Ok(()),
        Err(TrySendError::Full(_)) => Err(SubmitError::Busy),
        Err(TrySendError::Disconnected(_)) => Err(SubmitError::Stopped),
    }
}

fn run_worker(
    identity: CurrentUserIdentity,
    power_config: PowerConfig,
    commands: Receiver<WorkerCommand>,
    native: SyncSender<NativeEvent>,
) {
    let runtime = match tokio::runtime::Builder::new_current_thread().build() {
        Ok(runtime) => runtime,
        Err(_) => {
            let _ = native.send(NativeEvent::Fatal(
                "the locker authentication runtime could not be created",
            ));
            return;
        }
    };
    let backend = match PamReauthBackend::connect(identity.clone()) {
        Ok(backend) => backend,
        Err(_) => {
            let _ = native.send(NativeEvent::Fatal(
                "the isolated PAM worker could not be prepared",
            ));
            return;
        }
    };
    let fallback_identity = match IdentitySummary::new(
        identity.username().to_owned(),
        identity.display_name().to_owned(),
        None,
    ) {
        Ok(identity) => identity,
        Err(_) => {
            let _ = native.send(NativeEvent::Fatal(
                "the current account identity is not frontend-safe",
            ));
            return;
        }
    };
    if native.send(NativeEvent::BackendReady).is_err() {
        return;
    }
    let avatar = discover_current_avatar(identity.uid(), identity.username());
    let identity_with_avatar = avatar.as_ref().and_then(|asset| {
        IdentitySummary::new(
            identity.username().to_owned(),
            identity.display_name().to_owned(),
            Some(asset.uri().to_owned()),
        )
        .ok()
    });
    let (identity, resources) = match identity_with_avatar {
        Some(identity) => (identity, avatar.into_iter().collect::<Vec<_>>()),
        None => (fallback_identity, Vec::new()),
    };
    let power = LogindPowerControl::discover(&power_config);
    let mut controller = LockerController::with_power_control(backend, identity, power);
    let mut views = HashMap::new();

    while let Ok(command) = commands.recv() {
        match command {
            WorkerCommand::Register { view, output } => {
                attach_view(view, output, &resources, &mut views);
            }
            WorkerCommand::Detach { view } => {
                views.remove(&view);
            }
            WorkerCommand::Request {
                view,
                epoch,
                request,
            } => {
                if handle_request(
                    &runtime,
                    &mut controller,
                    &mut views,
                    &native,
                    view,
                    epoch,
                    request,
                )
                .is_err()
                {
                    send_fatal(
                        &mut views,
                        &native,
                        "the locker controller could not maintain public state",
                    );
                    return;
                }
            }
            WorkerCommand::CancelForPage => {
                if runtime.block_on(controller.cancel_for_lifecycle()).is_err() {
                    send_fatal(
                        &mut views,
                        &native,
                        "the locker controller could not cancel a stale page",
                    );
                    return;
                }
            }
            WorkerCommand::LockAcquired => {
                let events = match controller.mark_lock_acquired() {
                    Ok(events) => events,
                    Err(_) => {
                        send_fatal(
                            &mut views,
                            &native,
                            "the native lock acquisition order is invalid",
                        );
                        return;
                    }
                };
                if broadcast_events(&mut views, events).is_err()
                    || native.send(NativeEvent::LockAcquired).is_err()
                {
                    return;
                }
            }
            WorkerCommand::LockFailed => {
                let events = match controller.mark_lock_failed() {
                    Ok(events) => events,
                    Err(_) => {
                        send_fatal(
                            &mut views,
                            &native,
                            "the native lock failure order is invalid",
                        );
                        return;
                    }
                };
                let _ = broadcast_events(&mut views, events);
                let _ = native.send(NativeEvent::LockFailed);
                return;
            }
            WorkerCommand::LockReleased => {
                let events = match controller.mark_lock_released() {
                    Ok(events) => events,
                    Err(_) => {
                        send_fatal(
                            &mut views,
                            &native,
                            "the session lock was released without authorization",
                        );
                        return;
                    }
                };
                if broadcast_events(&mut views, events).is_err()
                    || native.send(NativeEvent::Released).is_err()
                {
                    return;
                }
            }
            WorkerCommand::Shutdown => {
                let _ = runtime.block_on(controller.cancel_for_lifecycle());
                return;
            }
        }
    }
    let _ = runtime.block_on(controller.cancel_for_lifecycle());
}

fn attach_view(
    view: u64,
    output: SyncSender<ControllerOutput<LockerViewAction>>,
    resources: &[ResourceAsset],
    views: &mut HashMap<u64, SyncSender<ControllerOutput<LockerViewAction>>>,
) {
    if output
        .send(ControllerOutput::Ready(resources.to_vec()))
        .is_ok()
    {
        views.insert(view, output);
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_request<B: ReauthBackend>(
    runtime: &tokio::runtime::Runtime,
    controller: &mut LockerController<B>,
    views: &mut HashMap<u64, SyncSender<ControllerOutput<LockerViewAction>>>,
    native: &SyncSender<NativeEvent>,
    view: u64,
    epoch: u64,
    request: RequestEnvelope,
) -> Result<(), ()> {
    if !views.contains_key(&view) {
        return Ok(());
    }
    let mut batch = runtime
        .block_on(controller.handle(request))
        .map_err(|_| ())?;
    let trusted_fallback = batch.requires_trusted_fallback();
    let unlock = batch.take_unlock_authorization();
    let unlock_authorized = unlock.is_some();
    if let Some(authorization) = unlock {
        controller.begin_unlock(authorization).map_err(|_| ())?;
    }
    let (response, event_scripts) = batch.into_bridge_parts().map_err(|_| ())?;
    send_to_view(
        views,
        view,
        ControllerOutput::Batch(ControllerBatch {
            epoch,
            response,
            event_scripts: event_scripts.clone(),
            terminal: None,
        }),
    );
    broadcast_serialized_events(views, Some(view), event_scripts);
    if trusted_fallback {
        send_view_fallback(
            views,
            "the isolated authentication worker failed; retry starts a fresh transaction",
        );
    }
    if unlock_authorized {
        native.send(NativeEvent::Unlock).map_err(|_| ())?;
    }
    Ok(())
}

fn broadcast_events(
    views: &mut HashMap<u64, SyncSender<ControllerOutput<LockerViewAction>>>,
    events: Vec<fomalhaut_web::protocol::EventEnvelope>,
) -> Result<(), ()> {
    let scripts = events
        .iter()
        .map(event_dispatch_script)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| ())?;
    broadcast_serialized_events(views, None, scripts);
    Ok(())
}

fn broadcast_serialized_events(
    views: &mut HashMap<u64, SyncSender<ControllerOutput<LockerViewAction>>>,
    excluded: Option<u64>,
    scripts: Vec<String>,
) {
    views.retain(|view, output| {
        if excluded == Some(*view) || scripts.is_empty() {
            return true;
        }
        output
            .try_send(ControllerOutput::Events(scripts.clone()))
            .is_ok()
    });
}

fn send_to_view(
    views: &mut HashMap<u64, SyncSender<ControllerOutput<LockerViewAction>>>,
    view: u64,
    output: ControllerOutput<LockerViewAction>,
) {
    let keep = views
        .get(&view)
        .is_some_and(|sender| sender.try_send(output).is_ok());
    if !keep {
        views.remove(&view);
    }
}

fn send_fatal(
    views: &mut HashMap<u64, SyncSender<ControllerOutput<LockerViewAction>>>,
    native: &SyncSender<NativeEvent>,
    message: &'static str,
) {
    for (_, output) in views.drain() {
        let _ = output.try_send(ControllerOutput::Fatal(message));
    }
    let _ = native.send(NativeEvent::Fatal(message));
}

fn send_view_fallback(
    views: &mut HashMap<u64, SyncSender<ControllerOutput<LockerViewAction>>>,
    message: &'static str,
) {
    for (_, output) in views.drain() {
        let _ = output.try_send(ControllerOutput::Fatal(message));
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, sync::mpsc, time::Duration};

    use fomalhaut_core::{
        AuthConversation, AuthEvent, AuthState, AuthenticatedIdentity, BackendError,
        ConversationBackend, PromptId, PromptKind, ReauthBackend, Secret,
    };
    use fomalhaut_gtk::{ControllerOutput, ResourceAsset};
    use fomalhaut_web::{
        controller::LockerController,
        protocol::{IdentitySummary, RuntimeMode, decode_request_for_mode},
    };

    use super::{LockerViewAction, NativeEvent, attach_view, handle_request};

    struct FakeReauth {
        conversation: AuthConversation,
    }

    impl FakeReauth {
        fn new() -> Self {
            Self {
                conversation: AuthConversation::new(),
            }
        }
    }

    impl ConversationBackend for FakeReauth {
        fn state(&self) -> AuthState {
            self.conversation.state()
        }

        fn needs_cancel(&self) -> bool {
            self.conversation.needs_cancel()
        }

        async fn respond(
            &mut self,
            prompt: PromptId,
            response: Secret,
        ) -> Result<(), BackendError> {
            self.conversation.begin_response(prompt)?;
            let mut response = response.into_inner().into_bytes();
            let disconnected = response == b"disconnect";
            let accepted = response == b"correct";
            response.fill(0);
            if disconnected {
                self.conversation.fail();
                return Err(BackendError::Unavailable);
            }
            if accepted {
                self.conversation
                    .authenticated(AuthenticatedIdentity::new("alice")?)?;
            } else {
                self.conversation.authentication_failed()?;
            }
            Ok(())
        }

        async fn cancel(&mut self) -> Result<(), BackendError> {
            self.conversation.begin_cancel()?;
            self.conversation.cancelled()?;
            Ok(())
        }

        async fn next_event(&mut self) -> Result<AuthEvent, BackendError> {
            self.conversation.next_event().map_err(BackendError::from)
        }
    }

    impl ReauthBackend for FakeReauth {
        async fn begin_reauth(&mut self) -> Result<(), BackendError> {
            self.conversation.begin()?;
            self.conversation
                .emit_prompt(PromptKind::Secret, "Password:".to_owned())?;
            Ok(())
        }
    }

    fn controller() -> LockerController<FakeReauth> {
        let identity = IdentitySummary::new("alice".to_owned(), "Alice".to_owned(), None)
            .expect("test identity is frontend-safe");
        let mut controller = LockerController::new(FakeReauth::new(), identity);
        controller
            .mark_lock_acquired()
            .expect("test lock can be acquired");
        controller
    }

    #[test]
    fn every_monitor_receives_the_same_validated_avatar_resource() {
        let resource = ResourceAsset::avatar(1, b"\x89PNG\r\n\x1a\nfixture".to_vec(), "image/png")
            .expect("the test avatar metadata is valid");
        let resources = vec![resource];
        let mut views = HashMap::new();

        for view in [1, 2] {
            let (sender, receiver) = mpsc::sync_channel(1);
            attach_view(view, sender, &resources, &mut views);
            let ControllerOutput::Ready(received) = receiver
                .recv_timeout(Duration::from_secs(1))
                .expect("the registered view receives initial resources")
            else {
                panic!("the first view output must contain resources");
            };
            assert_eq!(received.len(), 1);
            assert_eq!(received[0].uri(), "fomalhaut://avatar/1");
        }
        assert_eq!(views.len(), 2);
    }

    #[test]
    fn one_transaction_routes_reply_to_origin_and_events_to_every_view() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("test runtime can be created");
        let mut controller = controller();
        let (first_sender, first_receiver) = mpsc::sync_channel(8);
        let (second_sender, second_receiver) = mpsc::sync_channel(8);
        let mut views = HashMap::from([(1, first_sender), (2, second_sender)]);
        let (native_sender, native_receiver) = mpsc::sync_channel(8);

        for (view, receiver) in [(1, &first_receiver), (2, &second_receiver)] {
            let snapshot = decode_request_for_mode(
                br#"{"protocol":1,"id":9,"method":"state.get","params":{}}"#,
                RuntimeMode::Locker,
            )
            .expect("locker snapshot request is valid");
            handle_request(
                &runtime,
                &mut controller,
                &mut views,
                &native_sender,
                view,
                7,
                snapshot,
            )
            .expect("snapshot request is routed");
            let ControllerOutput::Batch(snapshot) = receiver
                .recv_timeout(Duration::from_secs(1))
                .expect("each view receives its own snapshot")
            else {
                panic!("snapshot must be a correlated response");
            };
            assert!(snapshot.response.contains("\"mode\":\"locker\""));
            assert!(snapshot.response.contains("\"sequence\":1"));
            assert!(snapshot.event_scripts.is_empty());
        }

        let begin = decode_request_for_mode(
            br#"{"protocol":1,"id":1,"method":"auth.begin","params":{}}"#,
            RuntimeMode::Locker,
        )
        .expect("locker begin request is valid");
        handle_request(
            &runtime,
            &mut controller,
            &mut views,
            &native_sender,
            1,
            7,
            begin,
        )
        .expect("begin request is routed");

        let ControllerOutput::Batch(first_begin) = first_receiver
            .recv_timeout(Duration::from_secs(1))
            .expect("origin receives a correlated batch")
        else {
            panic!("origin must receive one response batch");
        };
        assert_eq!(first_begin.epoch, 7);
        assert!(
            first_begin
                .event_scripts
                .iter()
                .any(|event| event.contains("auth.prompt"))
        );
        let ControllerOutput::Events(second_begin) = second_receiver
            .recv_timeout(Duration::from_secs(1))
            .expect("secondary view receives broadcast events")
        else {
            panic!("secondary view must not receive the origin response");
        };
        assert!(
            second_begin
                .iter()
                .any(|event| event.contains("auth.prompt"))
        );

        let respond = decode_request_for_mode(
            br#"{"protocol":1,"id":2,"method":"auth.respond","params":{"promptId":1,"response":"correct"}}"#,
            RuntimeMode::Locker,
        )
        .expect("locker response request is valid");
        handle_request(
            &runtime,
            &mut controller,
            &mut views,
            &native_sender,
            1,
            7,
            respond,
        )
        .expect("response request is routed");

        let ControllerOutput::Batch(first_success) = first_receiver
            .recv_timeout(Duration::from_secs(1))
            .expect("origin receives authentication success")
        else {
            panic!("origin must receive the authentication response");
        };
        assert!(first_success.terminal.is_none());
        assert!(
            first_success
                .event_scripts
                .iter()
                .any(|event| event.contains("auth.succeeded"))
        );
        let ControllerOutput::Events(second_success) = second_receiver
            .recv_timeout(Duration::from_secs(1))
            .expect("secondary view receives authentication success")
        else {
            panic!("secondary view must receive broadcast success only");
        };
        assert!(
            second_success
                .iter()
                .any(|event| event.contains("auth.succeeded"))
        );
        assert_eq!(
            native_receiver
                .recv_timeout(Duration::from_secs(1))
                .expect("native host receives unlock authorization"),
            NativeEvent::Unlock
        );
        assert!(native_receiver.try_recv().is_err());
        assert_eq!(views.len(), 2);
    }

    #[test]
    fn full_secondary_channel_is_detached_without_blocking_the_controller() {
        let (sender, receiver) = mpsc::sync_channel::<ControllerOutput<LockerViewAction>>(1);
        sender
            .try_send(ControllerOutput::Events(vec!["occupied".to_owned()]))
            .expect("test channel has one slot");
        let mut views = HashMap::from([(9, sender)]);
        super::broadcast_serialized_events(&mut views, None, vec!["event".to_owned()]);
        assert!(views.is_empty());
        assert!(matches!(
            receiver.recv_timeout(Duration::from_secs(1)),
            Ok(ControllerOutput::Events(events)) if events == ["occupied"]
        ));
    }

    #[test]
    fn pam_worker_failure_moves_every_view_to_trusted_fallback_without_unlocking() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("test runtime can be created");
        let mut controller = controller();
        let (first_sender, first_receiver) = mpsc::sync_channel(8);
        let (second_sender, second_receiver) = mpsc::sync_channel(8);
        let mut views = HashMap::from([(1, first_sender), (2, second_sender)]);
        let (native_sender, native_receiver) = mpsc::sync_channel(8);

        let begin = decode_request_for_mode(
            br#"{"protocol":1,"id":1,"method":"auth.begin","params":{}}"#,
            RuntimeMode::Locker,
        )
        .expect("locker begin request is valid");
        handle_request(
            &runtime,
            &mut controller,
            &mut views,
            &native_sender,
            1,
            7,
            begin,
        )
        .expect("begin request is routed");
        let _ = first_receiver
            .recv_timeout(Duration::from_secs(1))
            .expect("origin receives the prompt batch");
        let _ = second_receiver
            .recv_timeout(Duration::from_secs(1))
            .expect("secondary receives the prompt events");

        let respond = decode_request_for_mode(
            br#"{"protocol":1,"id":2,"method":"auth.respond","params":{"promptId":1,"response":"disconnect"}}"#,
            RuntimeMode::Locker,
        )
        .expect("locker response request is valid");
        handle_request(
            &runtime,
            &mut controller,
            &mut views,
            &native_sender,
            1,
            7,
            respond,
        )
        .expect("worker failure remains a routed controller result");

        assert!(matches!(
            first_receiver.recv_timeout(Duration::from_secs(1)),
            Ok(ControllerOutput::Batch(_))
        ));
        assert!(matches!(
            first_receiver.recv_timeout(Duration::from_secs(1)),
            Ok(ControllerOutput::Fatal(_))
        ));
        assert!(matches!(
            second_receiver.recv_timeout(Duration::from_secs(1)),
            Ok(ControllerOutput::Events(_))
        ));
        assert!(matches!(
            second_receiver.recv_timeout(Duration::from_secs(1)),
            Ok(ControllerOutput::Fatal(_))
        ));
        assert!(views.is_empty());
        assert!(native_receiver.try_recv().is_err());
    }
}
