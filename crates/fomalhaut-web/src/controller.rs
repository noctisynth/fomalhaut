//! Toolkit-neutral controller joining frontend protocol requests to the greetd core.

use std::{collections::VecDeque, error::Error, fmt};

use fomalhaut_core::{
    AuthEvent as CoreAuthEvent, AuthState as CoreAuthState, BackendError, CoreError, LoginBackend,
    MessageLevel as CoreMessageLevel, PromptId as CorePromptId, PromptKind as CorePromptKind,
    ReauthBackend, SessionCommand,
};

use crate::{
    bridge::{event_dispatch_script, response_json},
    protocol::{
        AuthMessage, AuthState, Capabilities, EmptyResult, Event, EventEnvelope, EventSequence,
        FrontendRequest, GreeterSnapshotFields, IdentitySummary, LockState, LoginState,
        MAX_AUTH_MESSAGES, MessageLevel, PowerAction, Prompt, PromptId, PromptKind,
        ProtocolErrorBody, ProtocolErrorCode, RequestEnvelope, ResponseEnvelope, ResponseResult,
        SessionSelectedData, SessionSummary, StateChangedData, StateSnapshot, UserSummary,
    },
};

/// One response and its ordered frontend events produced by a controller request.
#[derive(Debug)]
pub struct ControllerBatch {
    response: ResponseEnvelope,
    events: Vec<EventEnvelope>,
    session_started: bool,
    unlock_authorized: bool,
    trusted_fallback: bool,
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

    /// Takes the one-shot native unlock authorization produced by successful reauthentication.
    ///
    /// The token is internal to the trusted Rust host and is never serialized into the frontend
    /// protocol. Greeter batches and non-terminal locker batches return `None`.
    pub fn take_unlock_authorization(&mut self) -> Option<UnlockAuthorization> {
        if !self.unlock_authorized {
            return None;
        }
        self.unlock_authorized = false;
        Some(UnlockAuthorization { private: () })
    }

    /// Returns whether a locker authentication service failure requires trusted native fallback.
    ///
    /// Ordinary authentication rejection remains a normal frontend state and returns `false`.
    /// Greeter batches also always return `false`.
    #[must_use]
    pub const fn requires_trusted_fallback(&self) -> bool {
        self.trusted_fallback
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

/// One-shot proof that the reauthentication controller authorized a native unlock attempt.
///
/// Only [`ControllerBatch::take_unlock_authorization`] can construct this value. Consuming it does
/// not unlock the session; the locker host remains responsible for the session-lock handle and
/// the compositor roundtrip.
#[derive(Debug)]
pub struct UnlockAuthorization {
    private: (),
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
struct AuthPublicState {
    authentication: AuthState,
    prompt: Option<Prompt>,
    core_prompt: Option<CorePromptId>,
    messages: VecDeque<AuthMessage>,
    sequences: EventSequence,
}

impl AuthPublicState {
    fn new(state: CoreAuthState) -> Self {
        Self {
            authentication: map_state(state),
            prompt: None,
            core_prompt: None,
            messages: VecDeque::new(),
            sequences: EventSequence::default(),
        }
    }

    fn reset_conversation(&mut self) {
        self.prompt = None;
        self.core_prompt = None;
        self.messages.clear();
    }

    fn update_state(&mut self, state: CoreAuthState) {
        self.authentication = map_state(state);
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

    async fn drain_core_events<B: fomalhaut_core::ConversationBackend>(
        &mut self,
        client: &mut B,
    ) -> Result<Vec<Event>, ControllerError> {
        let mut events = Vec::new();
        loop {
            match client.next_event().await {
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

    async fn discard_core_events<B: fomalhaut_core::ConversationBackend>(
        &mut self,
        client: &mut B,
    ) -> Result<(), ControllerError> {
        loop {
            match client.next_event().await {
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

    async fn cancel_for_lifecycle<B: fomalhaut_core::ConversationBackend>(
        &mut self,
        client: &mut B,
    ) -> Result<(), ControllerError> {
        if !client.needs_cancel() {
            self.update_state(client.state());
            return Ok(());
        }

        client
            .cancel()
            .await
            .map_err(|_| ControllerError::new("the controller could not cancel authentication"))?;
        self.discard_core_events(client).await?;
        self.update_state(client.state());
        self.prompt = None;
        self.core_prompt = None;
        Ok(())
    }

    async fn cancel_after_internal_failure<B: fomalhaut_core::ConversationBackend>(
        &mut self,
        client: &mut B,
    ) {
        if client.needs_cancel() {
            let _ = client.cancel().await;
        }
        self.prompt = None;
        self.core_prompt = None;
        self.update_state(client.state());
    }
}

/// Greeter controller combining public authentication state with login-only capabilities.
pub struct GreeterController<B> {
    client: B,
    auth: AuthPublicState,
    login: LoginState,
    sessions: Vec<TrustedSession>,
    users: Vec<UserSummary>,
    power: Box<dyn PowerControl>,
    selected_session: Option<usize>,
}

impl<B: LoginBackend> GreeterController<B> {
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
        let auth = AuthPublicState::new(client.state());
        let login = if client.session_started() {
            LoginState::Started
        } else {
            LoginState::Idle
        };
        let selected_session = (!sessions.is_empty()).then_some(0);
        Self {
            client,
            auth,
            login,
            sessions,
            users,
            power: Box::new(power),
            selected_session,
        }
    }

    /// Returns a bounded, frontend-safe snapshot of the current controller state.
    pub fn snapshot(&self) -> Result<StateSnapshot, ControllerError> {
        StateSnapshot::greeter(GreeterSnapshotFields {
            authentication: self.auth.authentication,
            login: self.login,
            prompt: self.auth.prompt.clone(),
            messages: self.auth.messages.iter().cloned().collect(),
            sequence: self.auth.sequences.watermark(),
            users: self.users.clone(),
            sessions: self
                .sessions
                .iter()
                .map(|session| session.summary.clone())
                .collect(),
            selected_session_id: self
                .selected_session
                .and_then(|index| self.sessions.get(index))
                .map(|session| session.summary.id().to_owned()),
            capabilities: self.power.capabilities(),
        })
        .map_err(|_| ControllerError::new("the controller public state is invalid"))
    }
}

impl<B: LoginBackend> GreeterController<B> {
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
                unlock_authorized: false,
                trusted_fallback: false,
            });
        }

        let previous_state = self.auth.authentication;
        let mut operation = self.execute(request).await;
        let mut detail_events = match operation.as_mut() {
            Ok(events) => std::mem::take(events),
            Err(_) => Vec::new(),
        };
        let core_events = match self.auth.drain_core_events(&mut self.client).await {
            Ok(events) => events,
            Err(error) => {
                self.auth
                    .cancel_after_internal_failure(&mut self.client)
                    .await;
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
            self.login = LoginState::StartingSession;
            match self.client.start_session(command).await {
                Ok(()) => {
                    self.login = LoginState::Started;
                    detail_events.push(Event::SessionStarted(EmptyResult {}));
                }
                Err(error) => {
                    self.login = LoginState::Failed;
                    operation = Err(protocol_error(error));
                }
            }
            let session_events = match self.auth.drain_core_events(&mut self.client).await {
                Ok(events) => events,
                Err(error) => {
                    self.auth
                        .cancel_after_internal_failure(&mut self.client)
                        .await;
                    return Err(error);
                }
            };
            detail_events.extend(session_events);
        }

        self.auth.update_state(self.client.state());

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
        if previous_state != self.auth.authentication {
            events.push(Event::StateChanged(StateChangedData::new(
                self.auth.authentication,
            )));
        }
        events.extend(detail_events);
        let events = self.auth.envelope_events(events)?;

        let response = match operation {
            Ok(_) => ResponseEnvelope::success(id, ResponseResult::Empty(EmptyResult {})),
            Err(error) => ResponseEnvelope::error(id, error),
        };
        Ok(ControllerBatch {
            response,
            events,
            session_started: self.client.session_started(),
            unlock_authorized: false,
            trusted_fallback: false,
        })
    }

    /// Cancels an active greetd session after a page or host lifecycle boundary.
    pub async fn cancel_for_lifecycle(&mut self) -> Result<(), ControllerError> {
        self.auth.cancel_for_lifecycle(&mut self.client).await
    }

    async fn execute(&mut self, request: FrontendRequest) -> Result<Vec<Event>, ProtocolErrorBody> {
        match request {
            FrontendRequest::AuthBegin(params) => {
                let Some(username) = params.username() else {
                    return Err(ProtocolErrorBody::new(
                        ProtocolErrorCode::InvalidParams,
                        "auth.begin parameters do not match the greeter mode",
                        false,
                    ));
                };
                if matches!(
                    self.client.state(),
                    CoreAuthState::Idle | CoreAuthState::Failed
                ) {
                    self.auth.reset_conversation();
                    self.login = LoginState::Idle;
                }
                self.client
                    .begin_login(username.to_owned())
                    .await
                    .map_err(protocol_error)?;
                Ok(Vec::new())
            }
            FrontendRequest::AuthRespond(params) => {
                let (prompt_id, response) = params.into_parts();
                let Some(core_prompt) = self.auth.core_prompt else {
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
}

/// Locker controller combining shared authentication state with native lock lifecycle state.
///
/// This controller only accepts a [`ReauthBackend`]. It can authorize the trusted native host to
/// attempt an unlock, but it neither owns nor exposes a Wayland session-lock handle.
pub struct LockerController<B> {
    client: B,
    auth: AuthPublicState,
    lock: LockState,
    identity: IdentitySummary,
}

impl<B: ReauthBackend> LockerController<B> {
    /// Wraps a connected current-user reauthentication backend and trusted identity summary.
    #[must_use]
    pub fn new(client: B, identity: IdentitySummary) -> Self {
        let auth = AuthPublicState::new(client.state());
        Self {
            client,
            auth,
            lock: LockState::Acquiring,
            identity,
        }
    }

    /// Returns a bounded locker snapshot without user or session selection capabilities.
    pub fn snapshot(&self) -> Result<StateSnapshot, ControllerError> {
        StateSnapshot::locker(
            self.auth.authentication,
            self.lock,
            self.auth.prompt.clone(),
            self.auth.messages.iter().cloned().collect(),
            self.auth.sequences.watermark(),
            self.identity.clone(),
            Capabilities::disabled(),
        )
        .map_err(|_| ControllerError::new("the controller public state is invalid"))
    }

    /// Records compositor confirmation that the session lock is active.
    pub fn mark_lock_acquired(&mut self) -> Result<Vec<EventEnvelope>, ControllerError> {
        if self.lock != LockState::Acquiring {
            return Err(ControllerError::new(
                "the lock cannot be acquired in its current lifecycle state",
            ));
        }
        self.lock = LockState::Locked;
        self.auth
            .envelope_events(vec![Event::LockAcquired(EmptyResult {})])
    }

    /// Records a native session-lock failure without interpreting it as authentication failure.
    pub fn mark_lock_failed(&mut self) -> Result<Vec<EventEnvelope>, ControllerError> {
        if matches!(self.lock, LockState::Failed | LockState::Released) {
            return Err(ControllerError::new(
                "the lock cannot fail in its current lifecycle state",
            ));
        }
        self.lock = LockState::Failed;
        self.auth
            .envelope_events(vec![Event::LockFailed(EmptyResult {})])
    }

    /// Consumes controller authorization after the native host starts the unlock roundtrip.
    pub fn begin_unlock(
        &mut self,
        authorization: UnlockAuthorization,
    ) -> Result<(), ControllerError> {
        let UnlockAuthorization { private: () } = authorization;
        if self.lock != LockState::Locked || self.auth.authentication != AuthState::Authenticated {
            return Err(ControllerError::new(
                "the lock cannot begin unlocking in its current state",
            ));
        }
        self.lock = LockState::Unlocking;
        Ok(())
    }

    /// Records compositor-visible completion after an authorized native unlock roundtrip.
    pub fn mark_lock_released(&mut self) -> Result<Vec<EventEnvelope>, ControllerError> {
        if self.lock != LockState::Unlocking {
            return Err(ControllerError::new(
                "the lock cannot be released in its current lifecycle state",
            ));
        }
        self.lock = LockState::Released;
        self.auth
            .envelope_events(vec![Event::LockReleased(EmptyResult {})])
    }

    /// Handles one strictly decoded locker request as a serial transaction.
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
                session_started: false,
                unlock_authorized: false,
                trusted_fallback: false,
            });
        }

        let previous_state = self.auth.authentication;
        let previous_backend_state = self.client.state();
        let mut operation = self.execute(request).await;
        let mut detail_events = match operation.as_mut() {
            Ok(events) => std::mem::take(events),
            Err(_) => Vec::new(),
        };
        let core_events = match self.auth.drain_core_events(&mut self.client).await {
            Ok(events) => events,
            Err(error) => {
                self.auth
                    .cancel_after_internal_failure(&mut self.client)
                    .await;
                return Err(error);
            }
        };
        detail_events.extend(core_events);
        let backend_state = self.client.state();
        self.auth.update_state(backend_state);

        let trusted_fallback = operation.is_err()
            && !matches!(
                previous_backend_state,
                CoreAuthState::Failed | CoreAuthState::Disconnected
            )
            && matches!(
                backend_state,
                CoreAuthState::Failed | CoreAuthState::Disconnected
            );

        let unlock_authorized = operation.is_ok()
            && previous_state != AuthState::Authenticated
            && self.auth.authentication == AuthState::Authenticated;
        let mut events = Vec::with_capacity(detail_events.len().saturating_add(1));
        if previous_state != self.auth.authentication {
            events.push(Event::StateChanged(StateChangedData::new(
                self.auth.authentication,
            )));
        }
        events.extend(detail_events);
        let events = self.auth.envelope_events(events)?;
        let response = match operation {
            Ok(_) => ResponseEnvelope::success(id, ResponseResult::Empty(EmptyResult {})),
            Err(error) => ResponseEnvelope::error(id, error),
        };
        Ok(ControllerBatch {
            response,
            events,
            session_started: false,
            unlock_authorized,
            trusted_fallback,
        })
    }

    /// Cancels an active reauthentication transaction on a page or host lifecycle boundary.
    pub async fn cancel_for_lifecycle(&mut self) -> Result<(), ControllerError> {
        self.auth.cancel_for_lifecycle(&mut self.client).await
    }

    async fn execute(&mut self, request: FrontendRequest) -> Result<Vec<Event>, ProtocolErrorBody> {
        if self.lock != LockState::Locked {
            return Err(ProtocolErrorBody::new(
                ProtocolErrorCode::InvalidState,
                "authentication is unavailable in the current lock lifecycle state",
                false,
            ));
        }

        match request {
            FrontendRequest::AuthBegin(params) => {
                if !params.is_locker() {
                    return Err(ProtocolErrorBody::new(
                        ProtocolErrorCode::InvalidParams,
                        "auth.begin parameters do not match the locker mode",
                        false,
                    ));
                }
                if matches!(
                    self.client.state(),
                    CoreAuthState::Idle | CoreAuthState::Failed
                ) {
                    self.auth.reset_conversation();
                }
                self.client.begin_reauth().await.map_err(protocol_error)?;
                Ok(Vec::new())
            }
            FrontendRequest::AuthRespond(params) => {
                let (prompt_id, response) = params.into_parts();
                let Some(core_prompt) = self.auth.core_prompt else {
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
            FrontendRequest::SessionSelect(_) | FrontendRequest::PowerRequest(_) => {
                Err(ProtocolErrorBody::new(
                    ProtocolErrorCode::MethodDisabled,
                    "the method is disabled for the locker mode",
                    false,
                ))
            }
            FrontendRequest::StateGet(_) => Ok(Vec::new()),
        }
    }
}

fn map_state(state: CoreAuthState) -> AuthState {
    match state {
        CoreAuthState::Disconnected => AuthState::Failed,
        CoreAuthState::Idle => AuthState::Idle,
        CoreAuthState::Authenticating => AuthState::Authenticating,
        CoreAuthState::WaitingForSecret => AuthState::WaitingForSecret,
        CoreAuthState::WaitingForVisible => AuthState::WaitingForVisible,
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

    use super::{
        GreeterController, LockerController, PowerControl, PowerControlError, TrustedSession,
    };
    use crate::protocol::{
        Capabilities, IdentitySummary, PowerAction, ProtocolErrorCode, SessionKind, SessionSummary,
        decode_request,
    };
    use fomalhaut_core::{
        AuthConversation, AuthEvent, AuthState, AuthenticatedIdentity, BackendError,
        ConversationBackend, CoreError, LoginBackend, PromptId, PromptKind, ReauthBackend, Secret,
        SessionCommand,
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

    struct ScriptedReauthBackend(ScriptedLoginBackend);

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

    impl ScriptedReauthBackend {
        fn new(steps: impl IntoIterator<Item = ScriptStep>) -> Self {
            Self(ScriptedLoginBackend::new(steps))
        }
    }

    impl ConversationBackend for ScriptedReauthBackend {
        fn state(&self) -> AuthState {
            self.0.state()
        }

        fn needs_cancel(&self) -> bool {
            self.0.conversation.needs_cancel()
        }

        async fn respond(
            &mut self,
            prompt: PromptId,
            response: Secret,
        ) -> Result<(), BackendError> {
            self.0.respond(prompt, response).await
        }

        async fn cancel(&mut self) -> Result<(), BackendError> {
            self.0.cancel().await
        }

        async fn next_event(&mut self) -> Result<AuthEvent, BackendError> {
            self.0.next_event().await
        }
    }

    impl ReauthBackend for ScriptedReauthBackend {
        async fn begin_reauth(&mut self) -> Result<(), BackendError> {
            let identity = AuthenticatedIdentity::new("current-user".to_owned())?;
            self.0.conversation.begin()?;
            self.0.pending_identity = Some(identity);
            self.0.advance_authentication()
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

    fn locker_identity() -> IdentitySummary {
        IdentitySummary::new(
            "alice".to_owned(),
            "Alice".to_owned(),
            Some("fomalhaut://avatar/1".to_owned()),
        )
        .expect("locker identity fixture is frontend-safe")
    }

    #[tokio::test]
    async fn state_get_returns_connected_idle_snapshot() {
        let client = ScriptedLoginBackend::new([]);
        let mut controller = GreeterController::new(client);
        let batch = controller
            .handle(request(
                r#"{"protocol":1,"id":1,"method":"state.get","params":{}}"#,
            ))
            .await
            .expect("state snapshot is valid");
        let (response, events) = batch.into_parts();

        assert_eq!(json(&response)["result"]["mode"], "greeter");
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
            GreeterController::with_power_control(client, Vec::new(), Vec::new(), power);
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
            GreeterController::with_power_control(client, Vec::new(), Vec::new(), power);
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
        let mut controller = GreeterController::new(client);

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
        let mut controller = GreeterController::new(client);
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
        let mut controller = GreeterController::new(client);
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
        let mut controller = GreeterController::new(client);

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
        let mut controller = GreeterController::with_sessions(client, sessions);

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
        assert_eq!(events[0]["data"]["state"], "authenticated");
        assert!(events.as_array().is_some_and(|events| {
            events
                .iter()
                .any(|event| event["event"] == "auth.succeeded")
                && events
                    .iter()
                    .any(|event| event["event"] == "session.started")
        }));
        assert_eq!(
            json(controller.snapshot().expect("snapshot remains valid"))["login"],
            "started"
        );

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
        let mut controller = GreeterController::with_sessions(client, sessions);

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
        let mut controller = GreeterController::new(client);

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
        assert_eq!(events[0]["data"]["state"], "failed");
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
        let mut controller = GreeterController::new(client);

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

    #[tokio::test]
    async fn locker_snapshot_and_native_lifecycle_are_sequenced() {
        let client = ScriptedReauthBackend::new([]);
        let mut controller = LockerController::new(client, locker_identity());

        let state = controller
            .handle(request(
                r#"{"protocol":1,"id":30,"method":"state.get","params":{}}"#,
            ))
            .await
            .expect("locker snapshot is valid");
        let (response, events) = state.into_parts();
        let response = json(response);
        assert_eq!(response["result"]["mode"], "locker");
        assert_eq!(response["result"]["lock"], "acquiring");
        assert_eq!(response["result"]["sequence"], 0);
        assert_eq!(response["result"]["identity"]["username"], "alice");
        assert!(response["result"].get("sessions").is_none());
        assert!(events.is_empty());

        let acquired = controller
            .mark_lock_acquired()
            .expect("the compositor can confirm initial lock acquisition");
        assert_eq!(json(&acquired)[0]["sequence"], 1);
        assert_eq!(json(acquired)[0]["event"], "lock.acquired");
        let snapshot = json(
            controller
                .snapshot()
                .expect("locker snapshot remains valid"),
        );
        assert_eq!(snapshot["lock"], "locked");
        assert_eq!(snapshot["sequence"], 1);

        let failed = controller
            .mark_lock_failed()
            .expect("native lock failure is observable");
        assert_eq!(json(failed)[0]["event"], "lock.failed");
        assert_eq!(
            json(controller.snapshot().expect("failed snapshot is valid"))["lock"],
            "failed"
        );
        assert!(controller.mark_lock_acquired().is_err());
    }

    #[tokio::test]
    async fn locker_rejects_prelock_and_cross_role_requests() {
        let client = ScriptedReauthBackend::new([]);
        let mut controller = LockerController::new(client, locker_identity());

        let prelock = controller
            .handle(request(
                r#"{"protocol":1,"id":31,"method":"auth.begin","params":{}}"#,
            ))
            .await
            .expect("pre-lock authentication returns a protocol response");
        let (response, events) = prelock.into_parts();
        assert_eq!(json(response)["error"]["code"], "invalid_state");
        assert!(events.is_empty());

        controller
            .mark_lock_acquired()
            .expect("the compositor confirms the lock");
        let greeter_begin = controller
            .handle(request(
                r#"{"protocol":1,"id":32,"method":"auth.begin","params":{"username":"mallory"}}"#,
            ))
            .await
            .expect("cross-role auth parameters return a protocol response");
        let (response, events) = greeter_begin.into_parts();
        assert_eq!(json(response)["error"]["code"], "invalid_params");
        assert!(events.is_empty());

        let session = controller
            .handle(request(
                r#"{"protocol":1,"id":33,"method":"session.select","params":{"sessionId":"wayland:sway"}}"#,
            ))
            .await
            .expect("locker session selection returns a protocol response");
        let (response, events) = session.into_parts();
        assert_eq!(json(response)["error"]["code"], "method_disabled");
        assert!(events.is_empty());
    }

    #[tokio::test]
    async fn locker_multistep_reauth_only_authorizes_native_unlock_once() {
        let client = ScriptedReauthBackend::new([
            ScriptStep::Prompt {
                kind: PromptKind::Secret,
                message: "Password:".to_owned(),
            },
            ScriptStep::Prompt {
                kind: PromptKind::Visible,
                message: "One-time code:".to_owned(),
            },
            ScriptStep::Success,
        ]);
        let mut controller = LockerController::new(client, locker_identity());
        controller
            .mark_lock_acquired()
            .expect("the compositor confirms the lock");

        let mut begin = controller
            .handle(request(
                r#"{"protocol":1,"id":34,"method":"auth.begin","params":{}}"#,
            ))
            .await
            .expect("locker authentication starts");
        assert!(begin.take_unlock_authorization().is_none());
        let (_, events) = begin.into_parts();
        assert_eq!(json(events)[1]["data"]["kind"], "secret");

        let mut password = controller
            .handle(request(
                r#"{"protocol":1,"id":35,"method":"auth.respond","params":{"promptId":1,"response":"correct"}}"#,
            ))
            .await
            .expect("password advances to the visible prompt");
        assert!(password.take_unlock_authorization().is_none());
        let (_, events) = password.into_parts();
        assert_eq!(json(events)[1]["data"]["kind"], "visible");

        let mut otp = controller
            .handle(request(
                r#"{"protocol":1,"id":36,"method":"auth.respond","params":{"promptId":2,"response":"123456"}}"#,
            ))
            .await
            .expect("the second factor authenticates");
        let authorization = otp
            .take_unlock_authorization()
            .expect("successful reauthentication produces native authorization");
        assert!(otp.take_unlock_authorization().is_none());
        let (_, events) = otp.into_parts();
        let events = json(events);
        assert_eq!(events[0]["data"]["state"], "authenticated");
        assert!(events.as_array().is_some_and(|events| {
            events
                .iter()
                .any(|event| event["event"] == "auth.succeeded")
        }));
        assert_eq!(
            json(controller.snapshot().expect("authorized snapshot is valid"))["lock"],
            "locked"
        );

        controller
            .begin_unlock(authorization)
            .expect("only the native host consumes unlock authorization");
        assert_eq!(
            json(controller.snapshot().expect("unlocking snapshot is valid"))["lock"],
            "unlocking"
        );
        let released = controller
            .mark_lock_released()
            .expect("the compositor roundtrip completes release");
        assert_eq!(json(released)[0]["event"], "lock.released");
        assert_eq!(
            json(controller.snapshot().expect("released snapshot is valid"))["lock"],
            "released"
        );
    }

    #[tokio::test]
    async fn locker_auth_failure_cancel_and_disconnect_remain_locked() {
        let client = ScriptedReauthBackend::new([
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
            ScriptStep::Unavailable("private worker detail".to_owned()),
        ]);
        let mut controller = LockerController::new(client, locker_identity());
        controller
            .mark_lock_acquired()
            .expect("the compositor confirms the lock");

        controller
            .handle(request(
                r#"{"protocol":1,"id":37,"method":"auth.begin","params":{}}"#,
            ))
            .await
            .expect("first prompt is available");
        let mut failed = controller
            .handle(request(
                r#"{"protocol":1,"id":38,"method":"auth.respond","params":{"promptId":1,"response":"wrong"}}"#,
            ))
            .await
            .expect("authentication failure remains protocol-safe");
        assert!(failed.take_unlock_authorization().is_none());
        assert!(!failed.requires_trusted_fallback());
        assert_eq!(
            json(controller.snapshot().expect("failure snapshot is valid"))["lock"],
            "locked"
        );

        controller
            .handle(request(
                r#"{"protocol":1,"id":39,"method":"auth.begin","params":{}}"#,
            ))
            .await
            .expect("retry prompt is available");
        controller
            .cancel_for_lifecycle()
            .await
            .expect("lifecycle cancellation succeeds");
        let snapshot = json(controller.snapshot().expect("cancelled snapshot is valid"));
        assert_eq!(snapshot["authentication"], "idle");
        assert_eq!(snapshot["lock"], "locked");

        let disconnected = controller
            .handle(request(
                r#"{"protocol":1,"id":40,"method":"auth.begin","params":{}}"#,
            ))
            .await
            .expect("worker disconnect returns a sanitized response");
        assert!(disconnected.requires_trusted_fallback());
        let (response, events) = disconnected.into_parts();
        assert_eq!(json(&response)["error"]["code"], "internal");
        assert!(!json(response).to_string().contains("private worker detail"));
        assert_eq!(json(events)[0]["data"]["state"], "failed");
        assert_eq!(
            json(controller.snapshot().expect("disconnect snapshot is valid"))["lock"],
            "locked"
        );
    }
}
