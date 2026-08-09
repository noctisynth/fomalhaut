//! Backend-neutral authentication events.

use std::fmt;

use zeroize::Zeroize;

use crate::AuthenticatedIdentity;

/// Identifies the authentication prompt that may currently be answered.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct PromptId(u64);

impl PromptId {
    pub(crate) const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Returns the numeric prompt identifier.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Describes how an authentication response should be presented while typed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PromptKind {
    /// The response must not be displayed.
    Secret,
    /// The response may be displayed.
    Visible,
}

/// Severity of a conversation message that does not request input.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MessageLevel {
    /// Informational message.
    Info,
    /// Error message from the authentication conversation.
    Error,
}

/// UI-independent events produced by an authentication backend.
#[derive(Eq, PartialEq)]
pub enum AuthEvent {
    /// The backend is waiting for a response.
    Prompt {
        /// Identifier that must be echoed when responding.
        id: PromptId,
        /// Whether the response is secret or visible.
        kind: PromptKind,
        /// Prompt text supplied by the authentication service.
        message: String,
    },
    /// The authentication service supplied a message that needs no response.
    Message {
        /// Message severity.
        level: MessageLevel,
        /// Message text supplied by the authentication service.
        text: String,
    },
    /// Authentication completed successfully for a trusted identity.
    Authenticated(AuthenticatedIdentity),
    /// Authentication was rejected.
    AuthenticationFailed,
    /// An active authentication transaction was cancelled.
    Cancelled,
}

impl fmt::Debug for AuthEvent {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Prompt { id, kind, .. } => formatter
                .debug_struct("Prompt")
                .field("id", id)
                .field("kind", kind)
                .field("message", &"[REDACTED]")
                .finish(),
            Self::Message { level, .. } => formatter
                .debug_struct("Message")
                .field("level", level)
                .field("text", &"[REDACTED]")
                .finish(),
            Self::Authenticated(_) => formatter.write_str("Authenticated([REDACTED])"),
            Self::AuthenticationFailed => formatter.write_str("AuthenticationFailed"),
            Self::Cancelled => formatter.write_str("Cancelled"),
        }
    }
}

impl AuthEvent {
    /// Clears Rust-owned text retained by this event.
    pub fn zeroize(&mut self) {
        match self {
            Self::Prompt { message, .. } => message.zeroize(),
            Self::Message { text, .. } => text.zeroize(),
            Self::Authenticated(identity) => identity.zeroize(),
            Self::AuthenticationFailed | Self::Cancelled => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{AuthEvent, MessageLevel, PromptId, PromptKind};

    #[test]
    fn debug_redacts_conversation_text() {
        let prompt = AuthEvent::Prompt {
            id: PromptId::new(1),
            kind: PromptKind::Secret,
            message: "Password containing secret".to_owned(),
        };
        let message = AuthEvent::Message {
            level: MessageLevel::Error,
            text: "entered token was 123456".to_owned(),
        };

        assert!(!format!("{prompt:?}").contains("secret"));
        assert!(!format!("{message:?}").contains("123456"));
    }
}
