//! Toolkit-neutral controller joining frontend protocol requests to the greetd core.

use std::{collections::VecDeque, error::Error, fmt};

use fomalhaut_core::{
    AuthEvent as CoreAuthEvent, AuthState as CoreAuthState, BackendError, CoreError, LoginBackend,
    MessageLevel as CoreMessageLevel, PromptId as CorePromptId, PromptKind as CorePromptKind,
    SessionCommand,
};

use crate::{
    bridge::{event_dispatch_script, response_json},
    protocol::{
        AuthMessage, AuthState, Capabilities, EmptyResult, Event, EventEnvelope, EventSequence,
        FrontendRequest, MAX_AUTH_MESSAGES, MessageLevel, PowerAction, Prompt, PromptId,
        PromptKind, ProtocolErrorBody, ProtocolErrorCode, RequestEnvelope, ResponseEnvelope,
        ResponseResult, SessionSelectedData, SessionSummary, StateChangedData, StateSnapshot,
        UserSummary,
    },
};

/// One response and its ordered frontend events produced by a controller request.
#[derive(Debug)]
pub struct ControllerBatch {
    response: ResponseEnvelope,
    events: Vec<EventEnvelope>,
    session_started: bool,
}

impl ControllerBatch {
    /// Consumes the batch into the correlated response and ordered events.
    #[must_use]
    pub fn into_parts(self) -> (ResponseEnvelope, Vec<EventEnvelope>) {
        (self.response, self.events)
    }

    /// Returns whether this transaction successfully started the trusted user session.
    #[must_use]
    pub const fn session_started(&self) -> bool {
        self.session_started
    }

    /// Serializes the batch into one reply and ordered JavaScript event calls.
    pub fn into_bridge_parts(self) -> Result<(String, Vec<String>), ControllerError> {
        let response = response_json(&self.response)
            .map_err(|_| ControllerError::new("the controller response could not be serialized"))?;
        let events = self
            .events
            .iter()
            .map(event_dispatch_script)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| ControllerError::new("a controller event could not be serialized"))?;
        Ok((response, events))
    }
}

/// Sanitized fatal failure while maintaining public controller state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ControllerError {
    message: &'static str,
}

impl ControllerError {
    const fn new(message: &'static str) -> Self {
        Self { message }
    }
}

impl fmt::Display for ControllerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.message)
    }
}

impl Error for ControllerError {}

/// Frontend-safe metadata paired with a command resolved entirely by the trusted host.
pub struct TrustedSession {
    summary: SessionSummary,
    command: SessionCommand,
}

/// Sanitized failure returned by a trusted host power backend.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PowerControlError;

impl fmt::Display for PowerControlError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("the power backend could not complete the operation")
    }
}

impl Error for PowerControlError {}

/// Trusted host boundary for enumerated power operations.
pub trait PowerControl: Send {
    /// Returns actions allowed by both administrator policy and the active backend.
    fn capabilities(&self) -> Capabilities;

    /// Requests one previously advertised power operation without interactive authorization.
    fn request(&mut self, action: PowerAction) -> Result<(), PowerControlError>;
}

/// Power backend used when the host has no enabled operations.
#[derive(Default)]
pub struct DisabledPowerControl;

impl PowerControl for DisabledPowerControl {
    fn capabilities(&self) -> Capabilities {
        Capabilities::disabled()
    }

    fn request(&mut self, _action: PowerAction) -> Result<(), PowerControlError> {
        Err(PowerControlError)
    }
}

impl TrustedSession {
    /// Pairs public metadata with its trusted launch command.
    #[must_use]
    pub const fn new(summary: SessionSummary, command: SessionCommand) -> Self {
        Self { summary, command }
    }
}

/// Serial authentication controller over an arbitrary core transport.
pub struct HostController<B> {
    client: B,
    authentication: AuthState,
    prompt: Option<Prompt>,
    core_prompt: Option<CorePromptId>,
    messages: VecDeque<AuthMessage>,
    sessions: Vec<TrustedSession>,
    users: Vec<UserSummary>,
    power: Box<dyn PowerControl>,
    selected_session: Option<usize>,
    sequences: EventSequence,
}

impl<B: LoginBackend> HostController<B> {
    /// Wraps a connected core client and initializes its public state.
    #[must_use]
    pub fn new(client: B) -> Self {
        Self::with_sessions(client, Vec::new())
    }

    /// Wraps a connected core client with a host-resolved trusted session catalog.
    #[must_use]
    pub fn with_sessions(client: B, sessions: Vec<TrustedSession>) -> Self {
        Self::with_catalogs(client, sessions, Vec::new())
    }

    /// Wraps a connected core client with trusted session and public user catalogs.
    #[must_use]
    pub fn with_catalogs(
        client: B,
        sessions: Vec<TrustedSession>,
        users: Vec<UserSummary>,
    ) -> Self {
        Self::with_power_control(client, sessions, users, DisabledPowerControl)
    }

    /// Wraps a connected core client with trusted catalogs and a host power backend.
    #[must_use]
    pub fn with_power_control(
        client: B,
        sessions: Vec<TrustedSession>,
        users: Vec<UserSummary>,
        power: impl PowerControl + 'static,
    ) -> Self {
        let authentication = map_state(client.state(), client.session_started());
        let selected_session = (!sessions.is_empty()).then_some(0);
        Self {
            client,
            authentication,
            prompt: None,
            core_prompt: None,
            messages: VecDeque::new(),
            sessions,
            users,
            power: Box::new(power),
            selected_session,
            sequences: EventSequence::default(),
        }
    }

    /// Returns a bounded, frontend-safe snapshot of the current controller state.
    pub fn snapshot(&self) -> Result<StateSnapshot, ControllerError> {
        StateSnapshot::new(
            self.authentication,
            self.prompt.clone(),
            self.messages.iter().cloned().collect(),
            self.users.clone(),
            self.sessions
                .iter()
                .map(|session| session.summary.clone())
                .collect(),
            self.selected_session
                .and_then(|index| self.sessions.get(index))
                .map(|session| session.summary.id().to_owned()),
            self.power.capabilities(),
        )
        .map_err(|_| ControllerError::new("the controller public state is invalid"))
    }
}

impl<B: LoginBackend> HostController<B> {
    /// Handles one strictly decoded frontend request as a serial transaction.
    pub async fn handle(
        &mut self,
        request: RequestEnvelope,
    ) -> Result<ControllerBatch, ControllerError> {
        let (id, request) = request.into_parts();
        if matches!(request, FrontendRequest::StateGet(_)) {
            let snapshot = self.snapshot()?;
            return Ok(ControllerBatch {
                response: ResponseEnvelope::success(id, ResponseResult::State(snapshot)),
                events: Vec::new(),
                session_started: self.client.session_started(),
            });
        }

        let previous_state = self.authentication;
        let mut operation = self.execute(request).await;
        let mut detail_events = match operation.as_mut() {
            Ok(events) => std::mem::take(events),
            Err(_) => Vec::new(),
        };
        let core_events = match self.drain_core_events().await {
            Ok(events) => events,
            Err(error) => {
                self.cancel_after_internal_failure().await;
                return Err(error);
            }
        };
        detail_events.extend(core_events);

        if operation.is_ok()
            && self.client.state() == CoreAuthState::Authenticated
            && !self.sessions.is_empty()
        {
            let Some(selected) = self.selected_session else {
                return Err(ControllerError::new(
                    "the non-empty session catalog has no selected session",
                ));
            };
            let command = self
                .sessions
                .get(selected)
                .ok_or_else(|| {
                    ControllerError::new("the selected session index is outside the catalog")
                })?
                .command
                .clone();
            match self.client.start_session(command).await {
                Ok(()) => detail_events.push(Event::SessionStarted(EmptyResult {})),
                Err(error) => operation = Err(protocol_error(error)),
            }
            let session_events = match self.drain_core_events().await {
                Ok(events) => events,
                Err(error) => {
                    self.cancel_after_internal_failure().await;
                    return Err(error);
                }
            };
            detail_events.extend(session_events);
        }

        self.authentication = map_state(self.client.state(), self.client.session_started());

        self.finish_batch(id, previous_state, operation, detail_events)
    }

    fn finish_batch(
        &mut self,
        id: crate::protocol::RequestId,
        previous_state: AuthState,
        operation: Result<Vec<Event>, ProtocolErrorBody>,
        detail_events: Vec<Event>,
    ) -> Result<ControllerBatch, ControllerError> {
        let mut events = Vec::with_capacity(detail_events.len().saturating_add(1));
        if previous_state != self.authentication {
            events.push(Event::StateChanged(StateChangedData::new(
                self.authentication,
            )));
        }
        events.extend(detail_events);
        let events = self.envelope_events(events)?;

        let response = match operation {
            Ok(_) => ResponseEnvelope::success(id, ResponseResult::Empty(EmptyResult {})),
            Err(error) => ResponseEnvelope::error(id, error),
        };
        Ok(ControllerBatch {
            response,
            events,
            session_started: self.client.session_started(),
        })
    }

    /// Cancels an active greetd session after a page or host lifecycle boundary.
    pub async fn cancel_for_lifecycle(&mut self) -> Result<(), ControllerError> {
        if !self.client.needs_cancel() {
            self.authentication = map_state(self.client.state(), self.client.session_started());
            return Ok(());
        }

        self.client
            .cancel()
            .await
            .map_err(|_| ControllerError::new("the controller could not cancel authentication"))?;
        self.discard_core_events().await?;
        self.authentication = map_state(self.client.state(), self.client.session_started());
        self.prompt = None;
        self.core_prompt = None;
        Ok(())
    }

    async fn execute(&mut self, request: FrontendRequest) -> Result<Vec<Event>, ProtocolErrorBody> {
        match request {
            FrontendRequest::AuthBegin(params) => {
                if matches!(
                    self.client.state(),
                    CoreAuthState::Idle | CoreAuthState::Failed
                ) {
                    self.prompt = None;
                    self.core_prompt = None;
                    self.messages.clear();
                }
                self.client
                    .begin_login(params.username().to_owned())
                    .await
                    .map_err(protocol_error)?;
                Ok(Vec::new())
            }
            FrontendRequest::AuthRespond(params) => {
                let (prompt_id, response) = params.into_parts();
                let Some(core_prompt) = self.core_prompt else {
                    return Err(stale_prompt_error());
                };
                if core_prompt.get() != prompt_id.get() {
                    return Err(stale_prompt_error());
                }
                self.client
                    .respond(core_prompt, response.into_core_secret())
                    .await
                    .map_err(protocol_error)?;
                Ok(Vec::new())
            }
            FrontendRequest::AuthCancel(_) => {
                self.client.cancel().await.map_err(protocol_error)?;
                Ok(Vec::new())
            }
            FrontendRequest::SessionSelect(params) => self.select_session(params.session_id()),
            FrontendRequest::PowerRequest(params) => self.request_power(params.action()).await,
            FrontendRequest::StateGet(_) => Ok(Vec::new()),
        }
    }

    async fn request_power(
        &mut self,
        action: PowerAction,
    ) -> Result<Vec<Event>, ProtocolErrorBody> {
        if !self.power.capabilities().power().contains(&action) {
            return Err(ProtocolErrorBody::new(
                ProtocolErrorCode::MethodDisabled,
                "the requested power operation is disabled",
                false,
            ));
        }
        self.cancel_for_lifecycle().await.map_err(|_| {
            ProtocolErrorBody::new(
                ProtocolErrorCode::Internal,
                "authentication could not be cancelled before the power operation",
                true,
            )
        })?;
        self.power.request(action).map_err(|_| {
            ProtocolErrorBody::new(
                ProtocolErrorCode::Internal,
                "the power service could not complete the operation",
                true,
            )
        })?;
        Ok(Vec::new())
    }

    fn select_session(&mut self, session_id: &str) -> Result<Vec<Event>, ProtocolErrorBody> {
        if self.client.session_started()
            || !matches!(
                self.client.state(),
                CoreAuthState::Idle
                    | CoreAuthState::Authenticating
                    | CoreAuthState::WaitingForSecret
                    | CoreAuthState::WaitingForVisible
                    | CoreAuthState::Authenticated
                    | CoreAuthState::Failed
            )
        {
            return Err(ProtocolErrorBody::new(
                ProtocolErrorCode::InvalidState,
                "session selection is invalid in the current authentication state",
                false,
            ));
        }
        let Some(index) = self
            .sessions
            .iter()
            .position(|session| session.summary.id() == session_id)
        else {
            return Err(session_not_found_error());
        };
        self.selected_session = Some(index);
        let event = SessionSelectedData::new(session_id.to_owned()).map_err(|_| {
            ProtocolErrorBody::new(
                ProtocolErrorCode::Internal,
                "the selected session could not be represented",
                false,
            )
        })?;
        Ok(vec![Event::SessionSelected(event)])
    }

    async fn drain_core_events(&mut self) -> Result<Vec<Event>, ControllerError> {
        let mut events = Vec::new();
        loop {
            match self.client.next_event().await {
                Ok(event) => events.push(self.apply_core_event(event)?),
                Err(BackendError::Core(CoreError::NoPendingEvent)) => return Ok(events),
                Err(_) => {
                    return Err(ControllerError::new(
                        "the controller could not consume a core event",
                    ));
                }
            }
        }
    }

    fn apply_core_event(&mut self, event: CoreAuthEvent) -> Result<Event, ControllerError> {
        match event {
            CoreAuthEvent::Prompt { id, kind, message } => {
                let prompt_id = PromptId::new(id.get())
                    .map_err(|_| ControllerError::new("the core prompt ID is not frontend-safe"))?;
                let prompt_kind = match kind {
                    CorePromptKind::Secret => PromptKind::Secret,
                    CorePromptKind::Visible => PromptKind::Visible,
                };
                let prompt = Prompt::new(prompt_id, prompt_kind, message).map_err(|_| {
                    ControllerError::new("the core prompt exceeds frontend protocol limits")
                })?;
                self.core_prompt = Some(id);
                self.prompt = Some(prompt.clone());
                Ok(Event::AuthPrompt(prompt))
            }
            CoreAuthEvent::Message { level, text } => {
                let level = match level {
                    CoreMessageLevel::Info => MessageLevel::Info,
                    CoreMessageLevel::Error => MessageLevel::Error,
                };
                let message = AuthMessage::new(level, text).map_err(|_| {
                    ControllerError::new("the core message exceeds frontend protocol limits")
                })?;
                if self.messages.len() == MAX_AUTH_MESSAGES {
                    self.messages.pop_front();
                }
                self.messages.push_back(message.clone());
                Ok(Event::AuthMessage(message))
            }
            CoreAuthEvent::Authenticated(_) => {
                self.prompt = None;
                self.core_prompt = None;
                Ok(Event::AuthSucceeded(EmptyResult {}))
            }
            CoreAuthEvent::AuthenticationFailed => {
                self.prompt = None;
                self.core_prompt = None;
                Ok(Event::AuthFailed(EmptyResult {}))
            }
            CoreAuthEvent::Cancelled => {
                self.prompt = None;
                self.core_prompt = None;
                Ok(Event::AuthCancelled(EmptyResult {}))
            }
        }
    }

    fn envelope_events(
        &mut self,
        events: Vec<Event>,
    ) -> Result<Vec<EventEnvelope>, ControllerError> {
        events
            .into_iter()
            .map(|event| {
                self.sequences
                    .allocate()
                    .map(|sequence| EventEnvelope::new(sequence, event))
                    .map_err(|_| ControllerError::new("the frontend event sequence is exhausted"))
            })
            .collect()
    }

    async fn discard_core_events(&mut self) -> Result<(), ControllerError> {
        loop {
            match self.client.next_event().await {
                Ok(mut event) => event.zeroize_for_controller(),
                Err(BackendError::Core(CoreError::NoPendingEvent)) => return Ok(()),
                Err(_) => {
                    return Err(ControllerError::new(
                        "the controller could not discard a core event",
                    ));
                }
            }
        }
    }

    async fn cancel_after_internal_failure(&mut self) {
        if self.client.needs_cancel() {
            let _ = self.client.cancel().await;
        }
        self.prompt = None;
        self.core_prompt = None;
        self.authentication = map_state(self.client.state(), self.client.session_started());
    }
}

fn map_state(state: CoreAuthState, session_started: bool) -> AuthState {
    if session_started {
        return AuthState::Started;
    }

    match state {
        CoreAuthState::Disconnected => AuthState::Disconnected,
        CoreAuthState::Idle => AuthState::Idle,
        CoreAuthState::Authenticating => AuthState::Authenticating,
        CoreAuthState::WaitingForSecret | CoreAuthState::WaitingForVisible => {
            AuthState::WaitingForPrompt
        }
        CoreAuthState::Authenticated => AuthState::Authenticated,
        CoreAuthState::Cancelling => AuthState::Cancelling,
        CoreAuthState::Failed => AuthState::Failed,
    }
}

fn protocol_error(error: BackendError) -> ProtocolErrorBody {
    match error {
        BackendError::Core(CoreError::InvalidState { .. }) => ProtocolErrorBody::new(
            ProtocolErrorCode::InvalidState,
            "operation is invalid in the current authentication state",
            false,
        ),
        BackendError::Core(CoreError::StalePrompt { .. }) => stale_prompt_error(),
        BackendError::Core(
            CoreError::NoPendingEvent
            | CoreError::PromptIdExhausted
            | CoreError::EmptySessionCommand
            | CoreError::EmptyIdentity,
        )
        | BackendError::Unavailable
        | BackendError::Protocol
        | BackendError::Service => ProtocolErrorBody::new(
            ProtocolErrorCode::Internal,
            "the authentication service could not complete the operation",
            false,
        ),
    }
}

fn stale_prompt_error() -> ProtocolErrorBody {
    ProtocolErrorBody::new(
        ProtocolErrorCode::StalePrompt,
        "authentication prompt is no longer active",
        false,
    )
}

fn session_not_found_error() -> ProtocolErrorBody {
    ProtocolErrorBody::new(
        ProtocolErrorCode::SessionNotFound,
        "selected session is absent from the trusted catalog",
        false,
    )
}

trait ZeroizeControllerEvent {
    fn zeroize_for_controller(&mut self);
}

impl ZeroizeControllerEvent for CoreAuthEvent {
    fn zeroize_for_controller(&mut self) {
        self.zeroize();
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::VecDeque,
        sync::{Arc, Mutex},
    };

    use serde_json::Value;
    use zeroize::Zeroize;

    use super::{HostController, PowerControl, PowerControlError, TrustedSession};
    use crate::protocol::{
        Capabilities, PowerAction, ProtocolErrorCode, SessionKind, SessionSummary, decode_request,
    };
    use fomalhaut_core::{
        AuthConversation, AuthEvent, AuthState, AuthenticatedIdentity, BackendError,
        ConversationBackend, CoreError, LoginBackend, PromptId, PromptKind, Secret, SessionCommand,
    };

    enum ScriptStep {
        Prompt { kind: PromptKind, message: String },
        Success,
        AuthenticationFailed,
        ServiceFailure(String),
        Unavailable(String),
    }

    struct ScriptedLoginBackend {
        conversation: AuthConversation,
        steps: VecDeque<ScriptStep>,
        pending_identity: Option<AuthenticatedIdentity>,
        session_started: bool,
    }

    struct RecordingPower {
        capabilities: Capabilities,
        calls: Arc<Mutex<Vec<PowerAction>>>,
        succeeds: bool,
    }

    impl PowerControl for RecordingPower {
        fn capabilities(&self) -> Capabilities {
            self.capabilities.clone()
        }

        fn request(&mut self, action: PowerAction) -> Result<(), PowerControlError> {
            self.calls
                .lock()
                .expect("recording power mutex is not poisoned")
                .push(action);
            self.succeeds.then_some(()).ok_or(PowerControlError)
        }
    }

    impl ScriptedLoginBackend {
        fn new(steps: impl IntoIterator<Item = ScriptStep>) -> Self {
            Self {
                conversation: AuthConversation::new(),
                steps: steps.into_iter().collect(),
                pending_identity: None,
                session_started: false,
            }
        }

        fn advance_authentication(&mut self) -> Result<(), BackendError> {
            match self.next_step()? {
                ScriptStep::Prompt { kind, message } => {
                    self.conversation.emit_prompt(kind, message)?;
                    Ok(())
                }
                ScriptStep::Success => {
                    let identity = self.pending_identity.take().ok_or(BackendError::Protocol)?;
                    self.conversation.authenticated(identity)?;
                    Ok(())
                }
                ScriptStep::AuthenticationFailed => {
                    self.pending_identity = None;
                    self.conversation.authentication_failed()?;
                    Ok(())
                }
                ScriptStep::ServiceFailure(mut detail) => {
                    detail.zeroize();
                    self.pending_identity = None;
                    self.conversation.fail();
                    Err(BackendError::Service)
                }
                ScriptStep::Unavailable(mut detail) => {
                    detail.zeroize();
                    self.pending_identity = None;
                    self.conversation.disconnect();
                    Err(BackendError::Unavailable)
                }
            }
        }

        fn next_step(&mut self) -> Result<ScriptStep, BackendError> {
            self.steps.pop_front().ok_or(BackendError::Unavailable)
        }
    }

    impl ConversationBackend for ScriptedLoginBackend {
        fn state(&self) -> AuthState {
            self.conversation.state()
        }

        fn needs_cancel(&self) -> bool {
            !self.session_started && self.conversation.needs_cancel()
        }

        async fn respond(
            &mut self,
            prompt: PromptId,
            _response: Secret,
        ) -> Result<(), BackendError> {
            self.conversation.begin_response(prompt)?;
            self.advance_authentication()
        }

        async fn cancel(&mut self) -> Result<(), BackendError> {
            self.conversation.begin_cancel()?;
            self.pending_identity = None;
            match self.next_step()? {
                ScriptStep::Success => self.conversation.cancelled().map_err(BackendError::from),
                ScriptStep::ServiceFailure(mut detail) => {
                    detail.zeroize();
                    self.conversation.fail();
                    Err(BackendError::Service)
                }
                ScriptStep::Unavailable(mut detail) => {
                    detail.zeroize();
                    self.conversation.disconnect();
                    Err(BackendError::Unavailable)
                }
                _ => {
                    self.conversation.fail();
                    Err(BackendError::Protocol)
                }
            }
        }

        async fn next_event(&mut self) -> Result<AuthEvent, BackendError> {
            self.conversation.next_event().map_err(BackendError::from)
        }
    }

    impl LoginBackend for ScriptedLoginBackend {
        async fn begin_login(&mut self, username: String) -> Result<(), BackendError> {
            let identity = AuthenticatedIdentity::new(username)?;
            self.conversation.begin()?;
            self.pending_identity = Some(identity);
            self.session_started = false;
            self.advance_authentication()
        }

        async fn start_session(&mut self, _command: SessionCommand) -> Result<(), BackendError> {
            if self.conversation.state() != AuthState::Authenticated || self.session_started {
                return Err(BackendError::Core(CoreError::InvalidState {
                    operation: "start session",
                    state: self.conversation.state(),
                }));
            }

            match self.next_step()? {
                ScriptStep::Success => {
                    self.session_started = true;
                    Ok(())
                }
                ScriptStep::ServiceFailure(mut detail) => {
                    detail.zeroize();
                    self.conversation.fail();
                    Err(BackendError::Service)
                }
                ScriptStep::Unavailable(mut detail) => {
                    detail.zeroize();
                    self.conversation.disconnect();
                    Err(BackendError::Unavailable)
                }
                _ => {
                    self.conversation.fail();
                    Err(BackendError::Protocol)
                }
            }
        }

        fn session_started(&self) -> bool {
            self.session_started
        }
    }

    fn request(json: &str) -> crate::protocol::RequestEnvelope {
        decode_request(json.as_bytes()).expect("the controller request fixture is valid")
    }

    fn json<T: serde::Serialize>(value: T) -> Value {
        serde_json::to_value(value).expect("controller output is serializable")
    }

    fn trusted_session(id: &str, name: &str, kind: SessionKind) -> TrustedSession {
        let summary = SessionSummary::new(id.to_owned(), name.to_owned(), kind)
            .expect("trusted session fixture is frontend-safe");
        let command = SessionCommand::new(
            vec![format!("/usr/bin/{name}")],
            vec!["XDG_SESSION_TYPE=wayland".to_owned()],
        )
        .expect("trusted session fixture has a command");
        TrustedSession::new(summary, command)
    }

    #[tokio::test]
    async fn state_get_returns_connected_idle_snapshot() {
        let client = ScriptedLoginBackend::new([]);
        let mut controller = HostController::new(client);
        let batch = controller
            .handle(request(
                r#"{"protocol":1,"id":1,"method":"state.get","params":{}}"#,
            ))
            .await
            .expect("state snapshot is valid");
        let (response, events) = batch.into_parts();

        assert_eq!(json(response)["result"]["authentication"], "idle");
        assert!(events.is_empty());
    }

    #[tokio::test]
    async fn enabled_power_request_cancels_authentication_before_dispatch() {
        let client = ScriptedLoginBackend::new([
            ScriptStep::Prompt {
                kind: PromptKind::Secret,
                message: "Password:".to_owned(),
            },
            ScriptStep::Success,
        ]);
        let calls = Arc::new(Mutex::new(Vec::new()));
        let power = RecordingPower {
            capabilities: Capabilities::with_power(&[PowerAction::Poweroff]),
            calls: Arc::clone(&calls),
            succeeds: true,
        };
        let mut controller =
            HostController::with_power_control(client, Vec::new(), Vec::new(), power);
        controller
            .handle(request(
                r#"{"protocol":1,"id":1,"method":"auth.begin","params":{"username":"alice"}}"#,
            ))
            .await
            .expect("authentication reaches a prompt");

        let batch = controller
            .handle(request(
                r#"{"protocol":1,"id":2,"method":"power.request","params":{"action":"poweroff"}}"#,
            ))
            .await
            .expect("power request maintains controller state");
        let (response, events) = batch.into_parts();

        assert_eq!(json(response)["ok"], true);
        assert_eq!(json(events)[0]["event"], "state.changed");
        assert_eq!(
            calls
                .lock()
                .expect("recording power mutex is readable")
                .as_slice(),
            &[PowerAction::Poweroff]
        );
    }

    #[tokio::test]
    async fn disabled_power_request_is_rejected_without_dispatch() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let power = RecordingPower {
            capabilities: Capabilities::with_power(&[PowerAction::Suspend]),
            calls: Arc::clone(&calls),
            succeeds: true,
        };
        let client = ScriptedLoginBackend::new([]);
        let mut controller =
            HostController::with_power_control(client, Vec::new(), Vec::new(), power);
        let batch = controller
            .handle(request(
                r#"{"protocol":1,"id":1,"method":"power.request","params":{"action":"reboot"}}"#,
            ))
            .await
            .expect("disabled request returns a protocol response");
        let (response, _) = batch.into_parts();

        assert_eq!(json(response)["error"]["code"], "method_disabled");
        assert!(
            calls
                .lock()
                .expect("recording power mutex is readable")
                .is_empty()
        );
    }

    #[tokio::test]
    async fn password_flow_emits_prompt_and_success_with_monotonic_events() {
        let client = ScriptedLoginBackend::new([
            ScriptStep::Prompt {
                kind: PromptKind::Secret,
                message: "Password:".to_owned(),
            },
            ScriptStep::Success,
        ]);
        let mut controller = HostController::new(client);

        let begin = controller
            .handle(request(
                r#"{"protocol":1,"id":2,"method":"auth.begin","params":{"username":"alice"}}"#,
            ))
            .await
            .expect("password prompt is frontend-safe");
        let (response, events) = begin.into_parts();
        assert_eq!(json(response)["ok"], true);
        let events = json(events);
        assert_eq!(events[0]["sequence"], 1);
        assert_eq!(events[0]["event"], "state.changed");
        assert_eq!(events[1]["sequence"], 2);
        assert_eq!(events[1]["event"], "auth.prompt");
        assert_eq!(events[1]["data"]["kind"], "secret");

        let respond = controller
            .handle(request(
                r#"{"protocol":1,"id":3,"method":"auth.respond","params":{"promptId":1,"response":"correct"}}"#,
            ))
            .await
            .expect("authentication success is frontend-safe");
        let (_, events) = respond.into_parts();
        let events = json(events);
        assert_eq!(events[0]["sequence"], 3);
        assert_eq!(events[0]["data"]["state"], "authenticated");
        assert_eq!(events[1]["sequence"], 4);
        assert_eq!(events[1]["event"], "auth.succeeded");
    }

    #[tokio::test]
    async fn stale_prompt_is_rejected_without_consuming_transport() {
        let client = ScriptedLoginBackend::new([ScriptStep::Prompt {
            kind: PromptKind::Visible,
            message: "Code:".to_owned(),
        }]);
        let mut controller = HostController::new(client);
        controller
            .handle(request(
                r#"{"protocol":1,"id":4,"method":"auth.begin","params":{"username":"alice"}}"#,
            ))
            .await
            .expect("visible prompt is frontend-safe");

        let stale = controller
            .handle(request(
                r#"{"protocol":1,"id":5,"method":"auth.respond","params":{"promptId":9,"response":"123456"}}"#,
            ))
            .await
            .expect("stale prompt returns a protocol response");
        let (response, events) = stale.into_parts();
        assert_eq!(
            json(response)["error"]["code"],
            json(ProtocolErrorCode::StalePrompt)
        );
        assert!(events.is_empty());
    }

    #[tokio::test]
    async fn frontend_cancel_returns_idle_and_cancelled_event() {
        let client = ScriptedLoginBackend::new([
            ScriptStep::Prompt {
                kind: PromptKind::Visible,
                message: "One-time code:".to_owned(),
            },
            ScriptStep::Success,
        ]);
        let mut controller = HostController::new(client);
        controller
            .handle(request(
                r#"{"protocol":1,"id":9,"method":"auth.begin","params":{"username":"alice"}}"#,
            ))
            .await
            .expect("visible prompt is frontend-safe");

        let cancelled = controller
            .handle(request(
                r#"{"protocol":1,"id":10,"method":"auth.cancel","params":{}}"#,
            ))
            .await
            .expect("frontend cancellation is frontend-safe");
        let (response, events) = cancelled.into_parts();
        assert_eq!(json(response)["ok"], true);
        let events = json(events);
        assert_eq!(events[0]["data"]["state"], "idle");
        assert_eq!(events[1]["event"], "auth.cancelled");
    }

    #[tokio::test]
    async fn unknown_session_is_rejected_and_power_remains_disabled() {
        let client = ScriptedLoginBackend::new([]);
        let mut controller = HostController::new(client);

        let select = controller
            .handle(request(
                r#"{"protocol":1,"id":11,"method":"session.select","params":{"sessionId":"wayland:sway"}}"#,
            ))
            .await
            .expect("unknown selection returns a protocol response");
        let (response, events) = select.into_parts();
        assert_eq!(json(response)["error"]["code"], "session_not_found");
        assert!(events.is_empty());

        let power = controller
            .handle(request(
                r#"{"protocol":1,"id":12,"method":"power.request","params":{"action":"reboot"}}"#,
            ))
            .await
            .expect("disabled power operation returns a protocol response");
        let (response, events) = power.into_parts();
        assert_eq!(json(response)["error"]["code"], "method_disabled");
        assert!(events.is_empty());
    }

    #[tokio::test]
    async fn trusted_selection_is_public_and_authentication_starts_it() {
        let client = ScriptedLoginBackend::new([
            ScriptStep::Prompt {
                kind: PromptKind::Secret,
                message: "Password:".to_owned(),
            },
            ScriptStep::Success,
            ScriptStep::Success,
        ]);
        let sessions = vec![
            trusted_session("wayland:first", "first", SessionKind::Wayland),
            trusted_session("wayland:second", "second", SessionKind::Wayland),
        ];
        let mut controller = HostController::with_sessions(client, sessions);

        let state = controller
            .handle(request(
                r#"{"protocol":1,"id":20,"method":"state.get","params":{}}"#,
            ))
            .await
            .expect("trusted catalog snapshot is valid");
        let (response, _) = state.into_parts();
        let response = json(response);
        assert_eq!(
            response["result"]["sessions"].as_array().map(Vec::len),
            Some(2)
        );
        assert_eq!(response["result"]["selectedSessionId"], "wayland:first");

        let selected = controller
            .handle(request(
                r#"{"protocol":1,"id":21,"method":"session.select","params":{"sessionId":"wayland:second"}}"#,
            ))
            .await
            .expect("known trusted session can be selected");
        let (response, events) = selected.into_parts();
        assert_eq!(json(response)["ok"], true);
        assert_eq!(json(events)[0]["event"], "session.selected");

        controller
            .handle(request(
                r#"{"protocol":1,"id":22,"method":"auth.begin","params":{"username":"alice"}}"#,
            ))
            .await
            .expect("authentication prompt is available");
        let started = controller
            .handle(request(
                r#"{"protocol":1,"id":23,"method":"auth.respond","params":{"promptId":1,"response":"correct"}}"#,
            ))
            .await
            .expect("trusted session starts after authentication");
        assert!(started.session_started());
        let (response, events) = started.into_parts();
        assert_eq!(json(response)["ok"], true);
        let events = json(events);
        assert_eq!(events[0]["data"]["state"], "started");
        assert!(events.as_array().is_some_and(|events| {
            events
                .iter()
                .any(|event| event["event"] == "auth.succeeded")
                && events
                    .iter()
                    .any(|event| event["event"] == "session.started")
        }));

        let after_start = controller
            .handle(request(
                r#"{"protocol":1,"id":24,"method":"session.select","params":{"sessionId":"wayland:first"}}"#,
            ))
            .await
            .expect("selection after start returns a protocol response");
        let (response, events) = after_start.into_parts();
        assert_eq!(json(response)["error"]["code"], "invalid_state");
        assert!(events.is_empty());
    }

    #[tokio::test]
    async fn trusted_session_start_failure_is_sanitized() {
        let client = ScriptedLoginBackend::new([
            ScriptStep::Success,
            ScriptStep::ServiceFailure("private session failure".to_owned()),
        ]);
        let sessions = vec![trusted_session(
            "wayland:first",
            "first",
            SessionKind::Wayland,
        )];
        let mut controller = HostController::with_sessions(client, sessions);

        let failed = controller
            .handle(request(
                r#"{"protocol":1,"id":25,"method":"auth.begin","params":{"username":"alice"}}"#,
            ))
            .await
            .expect("session start failure remains protocol-safe");
        assert!(!failed.session_started());
        let (response, events) = failed.into_parts();
        let response = json(response);
        assert_eq!(response["error"]["code"], "internal");
        assert!(!response.to_string().contains("private session failure"));
        assert_eq!(json(events)[0]["data"]["state"], "failed");
    }

    #[tokio::test]
    async fn transport_failure_is_sanitized_and_disconnects_public_state() {
        let client =
            ScriptedLoginBackend::new([ScriptStep::Unavailable("private stub detail".to_owned())]);
        let mut controller = HostController::new(client);

        let batch = controller
            .handle(request(
                r#"{"protocol":1,"id":13,"method":"auth.begin","params":{"username":"alice"}}"#,
            ))
            .await
            .expect("transport failure returns sanitized protocol output");
        let (response, events) = batch.into_parts();
        let response = json(response);
        assert_eq!(response["error"]["code"], "internal");
        assert!(!response.to_string().contains("private stub detail"));
        let events = json(events);
        assert_eq!(events[0]["data"]["state"], "disconnected");
    }

    #[tokio::test]
    async fn auth_failure_and_lifecycle_cancel_are_observable() {
        let client = ScriptedLoginBackend::new([
            ScriptStep::Prompt {
                kind: PromptKind::Secret,
                message: "Password:".to_owned(),
            },
            ScriptStep::AuthenticationFailed,
            ScriptStep::Prompt {
                kind: PromptKind::Secret,
                message: "Password:".to_owned(),
            },
            ScriptStep::Success,
        ]);
        let mut controller = HostController::new(client);

        controller
            .handle(request(
                r#"{"protocol":1,"id":6,"method":"auth.begin","params":{"username":"alice"}}"#,
            ))
            .await
            .expect("first prompt is frontend-safe");
        let failure = controller
            .handle(request(
                r#"{"protocol":1,"id":7,"method":"auth.respond","params":{"promptId":1,"response":"wrong"}}"#,
            ))
            .await
            .expect("authentication failure is frontend-safe");
        let (_, events) = failure.into_parts();
        assert!(
            json(events).as_array().is_some_and(|events| {
                events.iter().any(|event| event["event"] == "auth.failed")
            })
        );

        controller
            .handle(request(
                r#"{"protocol":1,"id":8,"method":"auth.begin","params":{"username":"alice"}}"#,
            ))
            .await
            .expect("retry prompt is frontend-safe");
        controller
            .cancel_for_lifecycle()
            .await
            .expect("page lifecycle cancellation succeeds");
        assert_eq!(
            json(controller.snapshot().expect("snapshot is valid"))["authentication"],
            "idle"
        );
    }
}
