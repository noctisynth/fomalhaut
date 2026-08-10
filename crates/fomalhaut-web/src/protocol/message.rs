//! Responses, state snapshots, and sequenced events.

use schemars::JsonSchema;
use serde::Serialize;
use ts_rs::TS;

use super::{
    MAX_AUTH_MESSAGES, MAX_AVATAR_URL_BYTES, MAX_DISPLAY_TEXT_BYTES, MAX_SAFE_INTEGER,
    MAX_SESSION_ID_BYTES, MAX_SESSION_NAME_BYTES, MAX_SESSIONS, MAX_USER_DISPLAY_NAME_BYTES,
    MAX_USERNAME_BYTES, MAX_USERS, PROTOCOL_VERSION, PromptId, ProtocolErrorBody,
    ProtocolValueError, RequestEnvelope, RequestId, value::validate_text,
};

const AVATAR_URL_PREFIX: &str = "fomalhaut://avatar/";

/// Product role selected by the trusted native host.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "v1/protocol-message.ts")]
pub enum RuntimeMode {
    Greeter,
    Locker,
}

/// UI language resolved by the trusted native host.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[ts(export, export_to = "v1/protocol-message.ts")]
pub enum UiLocale {
    /// English UI strings.
    #[serde(rename = "en")]
    En,
    /// Simplified Chinese UI strings.
    #[serde(rename = "zh-CN")]
    ZhCn,
}

/// Authentication lifecycle visible to the frontend.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "v1/protocol-message.ts")]
pub enum AuthState {
    Idle,
    Authenticating,
    WaitingForSecret,
    WaitingForVisible,
    Authenticated,
    Cancelling,
    Failed,
}

/// Greeter-only trusted session lifecycle.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "v1/protocol-message.ts")]
pub enum LoginState {
    Idle,
    StartingSession,
    Started,
    Failed,
}

/// Locker-only native session-lock lifecycle.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "v1/protocol-message.ts")]
pub enum LockState {
    Acquiring,
    Locked,
    Unlocking,
    Released,
    Failed,
}

/// How a prompt answer should be rendered while typed.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "v1/protocol-message.ts")]
pub enum PromptKind {
    Secret,
    Visible,
}

/// Frontend-safe active prompt.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[ts(export, export_to = "v1/protocol-message.ts")]
pub struct Prompt {
    prompt_id: PromptId,
    kind: PromptKind,
    #[schemars(extend("x-fomalhaut-maxUtf8Bytes" = 4096))]
    message: String,
}

impl Prompt {
    /// Constructs a bounded prompt.
    pub fn new(
        prompt_id: PromptId,
        kind: PromptKind,
        message: String,
    ) -> Result<Self, ProtocolValueError> {
        validate_text(&message, MAX_DISPLAY_TEXT_BYTES, true, false)?;
        Ok(Self {
            prompt_id,
            kind,
            message,
        })
    }
}

/// Severity of a non-interactive authentication message.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "v1/protocol-message.ts")]
pub enum MessageLevel {
    Info,
    Error,
}

/// Bounded PAM message retained for state recovery.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[ts(export, export_to = "v1/protocol-message.ts")]
pub struct AuthMessage {
    level: MessageLevel,
    #[schemars(extend("x-fomalhaut-maxUtf8Bytes" = 4096))]
    text: String,
}

impl AuthMessage {
    /// Constructs a bounded authentication message.
    pub fn new(level: MessageLevel, text: String) -> Result<Self, ProtocolValueError> {
        validate_text(&text, MAX_DISPLAY_TEXT_BYTES, true, false)?;
        Ok(Self { level, text })
    }
}

/// Frontend-safe user metadata discovered by the trusted host.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[ts(export, export_to = "v1/protocol-message.ts")]
pub struct UserSummary {
    #[schemars(length(min = 1), extend("x-fomalhaut-maxUtf8Bytes" = 256))]
    username: String,
    #[schemars(length(min = 1), extend("x-fomalhaut-maxUtf8Bytes" = 256))]
    display_name: String,
    #[schemars(
        inner(regex(pattern = r"^fomalhaut://avatar/[0-9]+$")),
        extend("x-fomalhaut-maxUtf8Bytes" = 64)
    )]
    avatar_url: Option<String>,
}

impl UserSummary {
    /// Constructs bounded public user metadata and validates a host-owned avatar URL.
    pub fn new(
        username: String,
        display_name: String,
        avatar_url: Option<String>,
    ) -> Result<Self, ProtocolValueError> {
        validate_text(&username, MAX_USERNAME_BYTES, false, true)?;
        validate_text(&display_name, MAX_USER_DISPLAY_NAME_BYTES, false, true)?;
        validate_avatar_url(avatar_url.as_deref())?;
        Ok(Self {
            username,
            display_name,
            avatar_url,
        })
    }

    /// Returns the login name passed to `auth.begin` when this user is selected.
    #[must_use]
    pub fn username(&self) -> &str {
        &self.username
    }

    /// Returns the human-readable label supplied by the trusted host.
    #[must_use]
    pub fn display_name(&self) -> &str {
        &self.display_name
    }

    /// Returns the optional opaque host-owned avatar URL.
    #[must_use]
    pub fn avatar_url(&self) -> Option<&str> {
        self.avatar_url.as_deref()
    }
}

/// Trusted identity fixed by the locker host.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[ts(export, export_to = "v1/protocol-message.ts")]
pub struct IdentitySummary {
    #[schemars(length(min = 1), extend("x-fomalhaut-maxUtf8Bytes" = 256))]
    username: String,
    #[schemars(length(min = 1), extend("x-fomalhaut-maxUtf8Bytes" = 256))]
    display_name: String,
    #[schemars(
        inner(regex(pattern = r"^fomalhaut://avatar/[0-9]+$")),
        extend("x-fomalhaut-maxUtf8Bytes" = 64)
    )]
    avatar_url: Option<String>,
}

impl IdentitySummary {
    /// Constructs a bounded identity from fields established by the native locker host.
    pub fn new(
        username: String,
        display_name: String,
        avatar_url: Option<String>,
    ) -> Result<Self, ProtocolValueError> {
        validate_text(&username, MAX_USERNAME_BYTES, false, true)?;
        validate_text(&display_name, MAX_USER_DISPLAY_NAME_BYTES, false, true)?;
        validate_avatar_url(avatar_url.as_deref())?;
        Ok(Self {
            username,
            display_name,
            avatar_url,
        })
    }
}

fn validate_avatar_url(avatar_url: Option<&str>) -> Result<(), ProtocolValueError> {
    let Some(url) = avatar_url else {
        return Ok(());
    };
    validate_text(url, MAX_AVATAR_URL_BYTES, false, true)?;
    let Some(identifier) = url.strip_prefix(AVATAR_URL_PREFIX) else {
        return Err(ProtocolValueError::InvalidCharacter);
    };
    if identifier.is_empty() || !identifier.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(ProtocolValueError::InvalidCharacter);
    }
    Ok(())
}

/// Public session family.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "v1/protocol-message.ts")]
pub enum SessionKind {
    Wayland,
    X11,
}

/// Frontend-safe session metadata.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[ts(export, export_to = "v1/protocol-message.ts")]
pub struct SessionSummary {
    #[schemars(length(min = 1), extend("x-fomalhaut-maxUtf8Bytes" = 256))]
    id: String,
    #[schemars(length(min = 1), extend("x-fomalhaut-maxUtf8Bytes" = 256))]
    name: String,
    kind: SessionKind,
}

impl SessionSummary {
    /// Constructs bounded public session metadata.
    pub fn new(id: String, name: String, kind: SessionKind) -> Result<Self, ProtocolValueError> {
        validate_text(&id, MAX_SESSION_ID_BYTES, false, true)?;
        validate_text(&name, MAX_SESSION_NAME_BYTES, false, false)?;
        Ok(Self { id, name, kind })
    }

    /// Returns the opaque session identifier.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }
}

/// Power operations currently exposed by the protocol vocabulary.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, TS, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "v1/protocol-message.ts")]
pub enum PowerAction {
    Poweroff,
    Reboot,
    Suspend,
}

/// Capabilities enabled by trusted host policy.
#[derive(Clone, Debug, Default, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[ts(export, export_to = "v1/protocol-message.ts")]
pub struct Capabilities {
    power: Vec<PowerAction>,
}

impl Capabilities {
    /// Returns capabilities with every power operation disabled.
    #[must_use]
    pub const fn disabled() -> Self {
        Self { power: Vec::new() }
    }

    /// Constructs capabilities from trusted host policy in stable protocol order.
    #[must_use]
    pub fn with_power(actions: &[PowerAction]) -> Self {
        let power = [
            PowerAction::Poweroff,
            PowerAction::Reboot,
            PowerAction::Suspend,
        ]
        .into_iter()
        .filter(|action| actions.contains(action))
        .collect();
        Self { power }
    }

    /// Returns allowed power actions.
    #[must_use]
    pub fn power(&self) -> &[PowerAction] {
        &self.power
    }
}

/// Complete role-discriminated state needed to rebuild a frontend after refresh.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "mode", rename_all = "snake_case")]
#[ts(export, export_to = "v1/protocol-message.ts")]
pub enum StateSnapshot {
    Greeter(GreeterStateSnapshot),
    Locker(LockerStateSnapshot),
}

/// Greeter-only public state.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[ts(export, export_to = "v1/protocol-message.ts")]
pub struct GreeterStateSnapshot {
    locale: UiLocale,
    authentication: AuthState,
    login: LoginState,
    prompt: Option<Prompt>,
    #[schemars(length(max = 16))]
    messages: Vec<AuthMessage>,
    sequence: Sequence,
    #[schemars(length(max = 128))]
    users: Vec<UserSummary>,
    #[schemars(length(max = 128))]
    sessions: Vec<SessionSummary>,
    #[schemars(extend("x-fomalhaut-maxUtf8Bytes" = 256))]
    selected_session_id: Option<String>,
    capabilities: Capabilities,
}

/// Crate-internal fields used to construct and validate a greeter snapshot atomically.
pub(crate) struct GreeterSnapshotFields {
    pub(crate) locale: UiLocale,
    pub(crate) authentication: AuthState,
    pub(crate) login: LoginState,
    pub(crate) prompt: Option<Prompt>,
    pub(crate) messages: Vec<AuthMessage>,
    pub(crate) sequence: Sequence,
    pub(crate) users: Vec<UserSummary>,
    pub(crate) sessions: Vec<SessionSummary>,
    pub(crate) selected_session_id: Option<String>,
    pub(crate) capabilities: Capabilities,
}

impl StateSnapshot {
    /// Constructs a bounded, internally consistent greeter snapshot.
    pub(crate) fn greeter(fields: GreeterSnapshotFields) -> Result<Self, ProtocolValueError> {
        let GreeterSnapshotFields {
            locale,
            authentication,
            login,
            prompt,
            messages,
            sequence,
            users,
            sessions,
            selected_session_id,
            capabilities,
        } = fields;
        if messages.len() > MAX_AUTH_MESSAGES
            || users.len() > MAX_USERS
            || sessions.len() > MAX_SESSIONS
        {
            return Err(ProtocolValueError::TooManyItems);
        }
        if let Some(selected) = &selected_session_id {
            validate_text(selected, MAX_SESSION_ID_BYTES, false, true)?;
            if !sessions.iter().any(|session| session.id == *selected) {
                return Err(ProtocolValueError::UnknownSelection);
            }
        }
        Ok(Self::Greeter(GreeterStateSnapshot {
            locale,
            authentication,
            login,
            prompt,
            messages,
            sequence,
            users,
            sessions,
            selected_session_id,
            capabilities,
        }))
    }

    /// Constructs a bounded locker snapshot without user or session enumeration capabilities.
    pub(crate) fn locker(fields: LockerSnapshotFields) -> Result<Self, ProtocolValueError> {
        let LockerSnapshotFields {
            locale,
            authentication,
            lock,
            prompt,
            messages,
            sequence,
            identity,
            capabilities,
        } = fields;
        if messages.len() > MAX_AUTH_MESSAGES {
            return Err(ProtocolValueError::TooManyItems);
        }
        Ok(Self::Locker(LockerStateSnapshot {
            locale,
            authentication,
            lock,
            prompt,
            messages,
            sequence,
            identity,
            capabilities,
        }))
    }
}

/// Crate-internal fields used to construct a locker snapshot atomically.
pub(crate) struct LockerSnapshotFields {
    pub(crate) locale: UiLocale,
    pub(crate) authentication: AuthState,
    pub(crate) lock: LockState,
    pub(crate) prompt: Option<Prompt>,
    pub(crate) messages: Vec<AuthMessage>,
    pub(crate) sequence: Sequence,
    pub(crate) identity: IdentitySummary,
    pub(crate) capabilities: Capabilities,
}

/// Locker-only public state.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[ts(export, export_to = "v1/protocol-message.ts")]
pub struct LockerStateSnapshot {
    locale: UiLocale,
    authentication: AuthState,
    lock: LockState,
    prompt: Option<Prompt>,
    #[schemars(length(max = 16))]
    messages: Vec<AuthMessage>,
    sequence: Sequence,
    identity: IdentitySummary,
    capabilities: Capabilities,
}

/// Empty success payload.
#[derive(Clone, Copy, Debug, Default, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(
    export,
    export_to = "v1/protocol-message.ts",
    type = "Record<string, never>"
)]
pub struct EmptyResult {}

/// Successful operation payload.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(untagged)]
#[ts(export, export_to = "v1/protocol-message.ts")]
pub enum ResponseResult {
    Empty(EmptyResult),
    State(StateSnapshot),
}

/// Response associated with a request ID.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(untagged)]
#[ts(export, export_to = "v1/protocol-message.ts")]
pub enum ResponseEnvelope {
    Success(SuccessResponse),
    Error(ErrorResponse),
}

impl ResponseEnvelope {
    /// Constructs a successful response.
    #[must_use]
    pub fn success(id: RequestId, result: ResponseResult) -> Self {
        Self::Success(SuccessResponse {
            protocol: PROTOCOL_VERSION,
            id,
            ok: true,
            result,
        })
    }

    /// Constructs a sanitized error response.
    #[must_use]
    pub fn error(id: RequestId, error: ProtocolErrorBody) -> Self {
        Self::Error(ErrorResponse {
            protocol: PROTOCOL_VERSION,
            id,
            ok: false,
            error,
        })
    }
}

#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export, export_to = "v1/protocol-message.ts")]
pub struct SuccessResponse {
    #[schemars(extend("const" = 1))]
    #[ts(type = "1")]
    protocol: u16,
    id: RequestId,
    #[schemars(extend("const" = true))]
    #[ts(type = "true")]
    ok: bool,
    result: ResponseResult,
}

#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export, export_to = "v1/protocol-message.ts")]
pub struct ErrorResponse {
    #[schemars(extend("const" = 1))]
    #[ts(type = "1")]
    protocol: u16,
    id: RequestId,
    #[schemars(extend("const" = false))]
    #[ts(type = "false")]
    ok: bool,
    error: ProtocolErrorBody,
}

/// Monotonically increasing event sequence value.
#[derive(Clone, Copy, Debug, Eq, Hash, JsonSchema, PartialEq, Serialize, TS)]
#[serde(transparent)]
#[ts(export, export_to = "v1/protocol-message.ts")]
pub struct Sequence(
    #[schemars(range(max = 9_007_199_254_740_991_u64))]
    #[ts(type = "number")]
    u64,
);

impl Sequence {
    /// Returns the initial watermark before any event has been published.
    #[must_use]
    pub const fn initial() -> Self {
        Self(0)
    }

    /// Returns the numeric sequence.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Allocates event sequences without wraparound.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EventSequence {
    next: u64,
}

impl Default for EventSequence {
    fn default() -> Self {
        Self { next: 1 }
    }
}

impl EventSequence {
    /// Allocates the next sequence or reports exhaustion.
    pub fn allocate(&mut self) -> Result<Sequence, ProtocolValueError> {
        if self.next > MAX_SAFE_INTEGER {
            return Err(ProtocolValueError::SequenceExhausted);
        }
        let sequence = Sequence(self.next);
        self.next = self
            .next
            .checked_add(1)
            .ok_or(ProtocolValueError::SequenceExhausted)?;
        Ok(sequence)
    }

    /// Returns the last allocated sequence, or zero before the first event.
    #[must_use]
    pub const fn watermark(&self) -> Sequence {
        Sequence(self.next.saturating_sub(1))
    }
}

/// Typed event emitted by the trusted host.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "event", content = "data")]
#[ts(export, export_to = "v1/protocol-message.ts")]
pub enum Event {
    #[serde(rename = "state.changed")]
    StateChanged(StateChangedData),
    #[serde(rename = "auth.prompt")]
    AuthPrompt(Prompt),
    #[serde(rename = "auth.message")]
    AuthMessage(AuthMessage),
    #[serde(rename = "auth.succeeded")]
    AuthSucceeded(EmptyResult),
    #[serde(rename = "auth.failed")]
    AuthFailed(EmptyResult),
    #[serde(rename = "auth.cancelled")]
    AuthCancelled(EmptyResult),
    #[serde(rename = "session.selected")]
    SessionSelected(SessionSelectedData),
    #[serde(rename = "session.started")]
    SessionStarted(EmptyResult),
    #[serde(rename = "lock.acquired")]
    LockAcquired(EmptyResult),
    #[serde(rename = "lock.failed")]
    LockFailed(EmptyResult),
    #[serde(rename = "lock.released")]
    LockReleased(EmptyResult),
}

/// Authentication-state event data.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[ts(export, export_to = "v1/protocol-message.ts")]
pub struct StateChangedData {
    state: AuthState,
}

impl StateChangedData {
    /// Constructs state-change data.
    #[must_use]
    pub const fn new(state: AuthState) -> Self {
        Self { state }
    }
}

/// Session-selection event data.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[ts(export, export_to = "v1/protocol-message.ts")]
pub struct SessionSelectedData {
    #[schemars(length(min = 1), extend("x-fomalhaut-maxUtf8Bytes" = 256))]
    session_id: String,
}

impl SessionSelectedData {
    /// Constructs bounded session-selection event data.
    pub fn new(session_id: String) -> Result<Self, ProtocolValueError> {
        validate_text(&session_id, MAX_SESSION_ID_BYTES, false, true)?;
        Ok(Self { session_id })
    }
}

/// Sequenced event envelope.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export, export_to = "v1/protocol-message.ts")]
pub struct EventEnvelope {
    #[schemars(extend("const" = 1))]
    #[ts(type = "1")]
    protocol: u16,
    sequence: Sequence,
    #[serde(flatten)]
    event: Event,
}

impl EventEnvelope {
    /// Constructs a versioned event envelope.
    #[must_use]
    pub const fn new(sequence: Sequence, event: Event) -> Self {
        Self {
            protocol: PROTOCOL_VERSION,
            sequence,
            event,
        }
    }
}

/// Every top-level JSON message described by the v1 schema.
#[derive(JsonSchema, Serialize, TS)]
#[serde(untagged)]
#[ts(export, export_to = "v1/protocol-message.ts")]
pub enum WireMessage {
    Request(RequestEnvelope),
    Response(ResponseEnvelope),
    Event(EventEnvelope),
}

#[cfg(test)]
mod tests {
    use super::{
        AuthState, Capabilities, EmptyResult, Event, EventEnvelope, EventSequence,
        GreeterSnapshotFields, IdentitySummary, LockState, LockerSnapshotFields, LoginState,
        ResponseEnvelope, ResponseResult, Sequence, SessionKind, SessionSelectedData,
        SessionSummary, StateSnapshot, UiLocale, UserSummary,
    };
    use crate::protocol::{
        MAX_SAFE_INTEGER, ProtocolErrorBody, ProtocolErrorCode, ProtocolValueError, RequestId,
    };

    #[test]
    fn response_envelopes_preserve_success_error_invariants() {
        let id = RequestId::new(42).expect("42 is a JavaScript-safe integer");
        let success = ResponseEnvelope::success(id, ResponseResult::Empty(EmptyResult {}));
        let success = serde_json::to_value(success).expect("response serialization is infallible");
        assert_eq!(success["protocol"], 1);
        assert_eq!(success["id"], 42);
        assert_eq!(success["ok"], true);
        assert_eq!(success["result"], serde_json::json!({}));
        assert!(success.get("error").is_none());

        let error = ResponseEnvelope::error(
            id,
            ProtocolErrorBody::new(ProtocolErrorCode::InvalidState, "invalid state", false),
        );
        let error = serde_json::to_value(error).expect("error serialization is infallible");
        assert_eq!(error["ok"], false);
        assert_eq!(error["error"]["code"], "invalid_state");
        assert!(error.get("result").is_none());
    }

    #[test]
    fn state_snapshot_requires_bounded_known_selection() {
        let session = SessionSummary::new(
            "wayland:sway".to_owned(),
            "Sway".to_owned(),
            SessionKind::Wayland,
        )
        .expect("the session fixture is within bounds");
        let snapshot = StateSnapshot::greeter(GreeterSnapshotFields {
            locale: UiLocale::ZhCn,
            authentication: AuthState::Idle,
            login: LoginState::Idle,
            prompt: None,
            messages: Vec::new(),
            sequence: Sequence::initial(),
            users: vec![
                UserSummary::new(
                    "alice".to_owned(),
                    "Alice".to_owned(),
                    Some("fomalhaut://avatar/1".to_owned()),
                )
                .expect("the user fixture is within bounds"),
            ],
            sessions: vec![session],
            selected_session_id: Some("wayland:sway".to_owned()),
            capabilities: Capabilities::disabled(),
        });
        assert!(snapshot.is_ok());

        let error = StateSnapshot::greeter(GreeterSnapshotFields {
            locale: UiLocale::En,
            authentication: AuthState::Idle,
            login: LoginState::Idle,
            prompt: None,
            messages: Vec::new(),
            sequence: Sequence::initial(),
            users: Vec::new(),
            sessions: Vec::new(),
            selected_session_id: Some("wayland:missing".to_owned()),
            capabilities: Capabilities::disabled(),
        })
        .expect_err("a snapshot cannot select an absent session");
        assert_eq!(error, ProtocolValueError::UnknownSelection);
    }

    #[test]
    fn locker_snapshot_has_no_user_or_session_catalog() {
        let identity = IdentitySummary::new(
            "alice".to_owned(),
            "Alice".to_owned(),
            Some("fomalhaut://avatar/1".to_owned()),
        )
        .expect("the trusted identity fixture is valid");
        let snapshot = StateSnapshot::locker(LockerSnapshotFields {
            locale: UiLocale::ZhCn,
            authentication: AuthState::Idle,
            lock: LockState::Locked,
            prompt: None,
            messages: Vec::new(),
            sequence: Sequence::initial(),
            identity,
            capabilities: Capabilities::disabled(),
        })
        .expect("the locker snapshot is valid");
        let value = serde_json::to_value(snapshot).expect("snapshot serialization succeeds");

        assert_eq!(value["mode"], "locker");
        assert_eq!(value["locale"], "zh-CN");
        assert_eq!(value["sequence"], 0);
        assert!(value.get("users").is_none());
        assert!(value.get("sessions").is_none());
        assert!(value.get("selectedSessionId").is_none());
    }

    #[test]
    fn user_summary_accepts_only_opaque_host_avatar_urls() {
        assert!(
            UserSummary::new(
                "alice".to_owned(),
                "Alice".to_owned(),
                Some("fomalhaut://avatar/42".to_owned())
            )
            .is_ok()
        );
        for invalid in [
            "file:///var/lib/AccountsService/icons/alice",
            "fomalhaut://theme/avatar.png",
            "fomalhaut://avatar/1?path=secret",
        ] {
            assert!(
                UserSummary::new(
                    "alice".to_owned(),
                    "Alice".to_owned(),
                    Some(invalid.to_owned())
                )
                .is_err()
            );
        }
    }

    #[test]
    fn event_sequences_are_monotonic_and_do_not_wrap() {
        let mut sequence = EventSequence::default();
        assert_eq!(sequence.allocate().expect("first sequence exists").get(), 1);
        assert_eq!(
            sequence.allocate().expect("second sequence exists").get(),
            2
        );

        let mut exhausted = EventSequence {
            next: MAX_SAFE_INTEGER,
        };
        assert_eq!(
            exhausted.allocate().expect("maximum sequence is valid"),
            Sequence(MAX_SAFE_INTEGER)
        );
        assert_eq!(
            exhausted.allocate(),
            Err(ProtocolValueError::SequenceExhausted)
        );
    }

    #[test]
    fn events_use_the_versioned_flat_envelope() {
        let data = SessionSelectedData::new("x11:xfce".to_owned())
            .expect("the session ID is within bounds");
        let event = EventEnvelope::new(Sequence(7), Event::SessionSelected(data));
        let event = serde_json::to_value(event).expect("event serialization is infallible");
        assert_eq!(event["protocol"], 1);
        assert_eq!(event["sequence"], 7);
        assert_eq!(event["event"], "session.selected");
        assert_eq!(event["data"]["sessionId"], "x11:xfce");
    }
}
