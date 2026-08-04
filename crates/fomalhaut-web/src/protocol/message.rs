//! Responses, state snapshots, and sequenced events.

use schemars::JsonSchema;
use serde::Serialize;
use ts_rs::TS;

use super::{
    MAX_AUTH_MESSAGES, MAX_DISPLAY_TEXT_BYTES, MAX_SAFE_INTEGER, MAX_SESSION_ID_BYTES,
    MAX_SESSION_NAME_BYTES, MAX_SESSIONS, PROTOCOL_VERSION, PromptId, ProtocolErrorBody,
    ProtocolValueError, RequestEnvelope, RequestId, value::validate_text,
};

/// Authentication lifecycle visible to the frontend.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "v1/protocol-message.ts")]
pub enum AuthState {
    Disconnected,
    Idle,
    Authenticating,
    WaitingForPrompt,
    Authenticated,
    StartingSession,
    Started,
    Cancelling,
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

    /// Returns allowed power actions.
    #[must_use]
    pub fn power(&self) -> &[PowerAction] {
        &self.power
    }
}

/// Complete state needed to rebuild a frontend after refresh.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[ts(export, export_to = "v1/protocol-message.ts")]
pub struct StateSnapshot {
    authentication: AuthState,
    prompt: Option<Prompt>,
    #[schemars(length(max = 16))]
    messages: Vec<AuthMessage>,
    #[schemars(length(max = 128))]
    sessions: Vec<SessionSummary>,
    #[schemars(extend("x-fomalhaut-maxUtf8Bytes" = 256))]
    selected_session_id: Option<String>,
    capabilities: Capabilities,
}

impl StateSnapshot {
    /// Constructs a bounded, internally consistent state snapshot.
    pub fn new(
        authentication: AuthState,
        prompt: Option<Prompt>,
        messages: Vec<AuthMessage>,
        sessions: Vec<SessionSummary>,
        selected_session_id: Option<String>,
        capabilities: Capabilities,
    ) -> Result<Self, ProtocolValueError> {
        if messages.len() > MAX_AUTH_MESSAGES || sessions.len() > MAX_SESSIONS {
            return Err(ProtocolValueError::TooManyItems);
        }
        if let Some(selected) = &selected_session_id {
            validate_text(selected, MAX_SESSION_ID_BYTES, false, true)?;
            if !sessions.iter().any(|session| session.id == *selected) {
                return Err(ProtocolValueError::UnknownSelection);
            }
        }
        Ok(Self {
            authentication,
            prompt,
            messages,
            sessions,
            selected_session_id,
            capabilities,
        })
    }
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
        ResponseEnvelope, ResponseResult, Sequence, SessionKind, SessionSelectedData,
        SessionSummary, StateSnapshot,
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
        let snapshot = StateSnapshot::new(
            AuthState::Idle,
            None,
            Vec::new(),
            vec![session],
            Some("wayland:sway".to_owned()),
            Capabilities::disabled(),
        );
        assert!(snapshot.is_ok());

        let error = StateSnapshot::new(
            AuthState::Idle,
            None,
            Vec::new(),
            Vec::new(),
            Some("wayland:missing".to_owned()),
            Capabilities::disabled(),
        )
        .expect_err("a snapshot cannot select an absent session");
        assert_eq!(error, ProtocolValueError::UnknownSelection);
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
