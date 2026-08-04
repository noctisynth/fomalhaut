//! Strict, versioned messages exchanged with an untrusted frontend.

mod error;
mod message;
mod request;
mod schema;
mod secret;
mod value;

pub use error::{ProtocolDecodeError, ProtocolErrorBody, ProtocolErrorCode, ProtocolValueError};
pub use message::{
    AuthMessage, AuthState, Capabilities, EmptyResult, Event, EventEnvelope, EventSequence,
    MessageLevel, PowerAction, Prompt, PromptKind, ResponseEnvelope, ResponseResult, Sequence,
    SessionKind, SessionSelectedData, SessionSummary, StateChangedData, StateSnapshot, UserSummary,
    WireMessage,
};
pub use request::{
    AuthBeginParams, AuthRespondParams, EmptyParams, FrontendRequest, PowerRequestParams, PromptId,
    RequestEnvelope, RequestId, SessionSelectParams, decode_request,
};
pub use schema::{schema_json_pretty, wire_schema};
pub use secret::ProtocolSecret;

/// Current frontend protocol major version.
pub const PROTOCOL_VERSION: u16 = 1;
/// Largest accepted JSON message in UTF-8 bytes.
pub const MAX_MESSAGE_BYTES: usize = 128 * 1024;
/// Largest accepted username in UTF-8 bytes.
pub const MAX_USERNAME_BYTES: usize = 256;
/// Largest frontend-visible user display name in UTF-8 bytes.
pub const MAX_USER_DISPLAY_NAME_BYTES: usize = 256;
/// Largest host-generated avatar URL in UTF-8 bytes.
pub const MAX_AVATAR_URL_BYTES: usize = 64;
/// Largest accepted authentication response in UTF-8 bytes.
pub const MAX_AUTH_RESPONSE_BYTES: usize = 16 * 1024;
/// Largest opaque session identifier in UTF-8 bytes.
pub const MAX_SESSION_ID_BYTES: usize = 256;
/// Largest frontend-visible session name in UTF-8 bytes.
pub const MAX_SESSION_NAME_BYTES: usize = 256;
/// Largest PAM prompt or message text in UTF-8 bytes.
pub const MAX_DISPLAY_TEXT_BYTES: usize = 4 * 1024;
/// Largest number of sessions in one state snapshot.
pub const MAX_SESSIONS: usize = 128;
/// Largest number of users in one state snapshot.
pub const MAX_USERS: usize = 128;
/// Largest number of retained authentication messages in one snapshot.
pub const MAX_AUTH_MESSAGES: usize = 16;
/// Largest integer JavaScript can represent exactly.
pub const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;
