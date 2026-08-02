//! Sanitized protocol errors.

use std::{error::Error, fmt};

use schemars::JsonSchema;
use serde::Serialize;

use super::RequestId;

/// Stable error category exposed to the frontend.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProtocolErrorCode {
    /// Input is not valid JSON.
    InvalidJson,
    /// Input exceeded the message size limit.
    MessageTooLarge,
    /// The protocol major version is unsupported.
    UnsupportedVersion,
    /// The common request envelope is malformed.
    InvalidRequest,
    /// The requested method is unknown.
    UnknownMethod,
    /// Method parameters are malformed or outside configured bounds.
    InvalidParams,
    /// The operation is invalid in the current host state.
    InvalidState,
    /// The prompt identifier is no longer active.
    StalePrompt,
    /// The selected session is absent from the trusted catalog.
    SessionNotFound,
    /// The method exists but is disabled by policy.
    MethodDisabled,
    /// A sanitized internal failure occurred.
    Internal,
}

/// Frontend-safe error body.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ProtocolErrorBody {
    code: ProtocolErrorCode,
    message: String,
    retryable: bool,
}

impl ProtocolErrorBody {
    /// Constructs a sanitized error body.
    #[must_use]
    pub fn new(code: ProtocolErrorCode, message: &'static str, retryable: bool) -> Self {
        Self {
            code,
            message: message.to_owned(),
            retryable,
        }
    }

    /// Returns the stable machine-readable category.
    #[must_use]
    pub const fn code(&self) -> ProtocolErrorCode {
        self.code
    }

    /// Returns the sanitized display text.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Returns whether retrying may succeed without changing the request.
    #[must_use]
    pub const fn retryable(&self) -> bool {
        self.retryable
    }

    pub(crate) fn invalid_json() -> Self {
        Self::new(
            ProtocolErrorCode::InvalidJson,
            "invalid JSON message",
            false,
        )
    }

    pub(crate) fn message_too_large() -> Self {
        Self::new(
            ProtocolErrorCode::MessageTooLarge,
            "protocol message exceeds the size limit",
            false,
        )
    }

    pub(crate) fn unsupported_version() -> Self {
        Self::new(
            ProtocolErrorCode::UnsupportedVersion,
            "unsupported protocol version",
            false,
        )
    }

    pub(crate) fn invalid_request() -> Self {
        Self::new(
            ProtocolErrorCode::InvalidRequest,
            "invalid request envelope",
            false,
        )
    }

    pub(crate) fn unknown_method() -> Self {
        Self::new(
            ProtocolErrorCode::UnknownMethod,
            "unknown protocol method",
            false,
        )
    }

    pub(crate) fn invalid_params() -> Self {
        Self::new(
            ProtocolErrorCode::InvalidParams,
            "invalid method parameters",
            false,
        )
    }
}

/// Request decoding failure with an ID when one was safely recoverable.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProtocolDecodeError {
    request_id: Option<RequestId>,
    body: ProtocolErrorBody,
}

impl ProtocolDecodeError {
    pub(crate) const fn new(request_id: Option<RequestId>, body: ProtocolErrorBody) -> Self {
        Self { request_id, body }
    }

    /// Returns the request ID if it was parsed and within the safe integer range.
    #[must_use]
    pub const fn request_id(&self) -> Option<RequestId> {
        self.request_id
    }

    /// Returns the frontend-safe error body.
    #[must_use]
    pub const fn body(&self) -> &ProtocolErrorBody {
        &self.body
    }
}

impl fmt::Display for ProtocolDecodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.body.message.fmt(formatter)
    }
}

impl Error for ProtocolDecodeError {}

/// Failure to construct an outbound value within protocol limits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProtocolValueError {
    /// A required string is empty.
    Empty,
    /// A value exceeds its UTF-8 byte limit.
    TooLong,
    /// A value contains NUL or forbidden control characters.
    InvalidCharacter,
    /// A JavaScript-visible integer is outside the exact range.
    UnsafeInteger,
    /// A collection exceeds its protocol limit.
    TooManyItems,
    /// The selected session is not present in the supplied snapshot.
    UnknownSelection,
    /// An event sequence cannot advance without wrapping.
    SequenceExhausted,
}

impl fmt::Display for ProtocolValueError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Empty => "protocol value must not be empty",
            Self::TooLong => "protocol value exceeds its byte limit",
            Self::InvalidCharacter => "protocol value contains a forbidden character",
            Self::UnsafeInteger => "protocol integer is not exactly representable in JavaScript",
            Self::TooManyItems => "protocol collection exceeds its item limit",
            Self::UnknownSelection => "selected session is absent from the state snapshot",
            Self::SequenceExhausted => "protocol event sequence is exhausted",
        })
    }
}

impl Error for ProtocolValueError {}
