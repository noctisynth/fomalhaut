//! Events emitted by the greetd state machine.

use std::fmt;

use zeroize::Zeroize;

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

/// Severity of a PAM message that does not request input.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MessageLevel {
    /// Informational message.
    Info,
    /// Error message from the authentication conversation.
    Error,
}

/// UI-independent events produced by the greeter state machine.
#[derive(Eq, PartialEq)]
pub enum GreeterEvent {
    /// PAM is waiting for a response.
    Prompt {
        /// Identifier that must be echoed when responding.
        id: PromptId,
        /// Whether the response is secret or visible.
        kind: PromptKind,
        /// Prompt text supplied by PAM.
        message: String,
    },
    /// PAM supplied a message that does not request input.
    Message {
        /// Message severity.
        level: MessageLevel,
        /// Message text supplied by PAM.
        text: String,
    },
    /// Authentication completed successfully.
    Authenticated,
    /// The requested user session started successfully.
    SessionStarted,
    /// Authentication failed after the client cancelled the rejected greetd session.
    AuthenticationFailed,
    /// An active authentication session was cancelled.
    Cancelled,
}

impl fmt::Debug for GreeterEvent {
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
            Self::Authenticated => formatter.write_str("Authenticated"),
            Self::SessionStarted => formatter.write_str("SessionStarted"),
            Self::AuthenticationFailed => formatter.write_str("AuthenticationFailed"),
            Self::Cancelled => formatter.write_str("Cancelled"),
        }
    }
}

impl GreeterEvent {
    pub(crate) fn zeroize(&mut self) {
        match self {
            Self::Prompt { message, .. } => message.zeroize(),
            Self::Message { text, .. } => text.zeroize(),
            Self::Authenticated
            | Self::SessionStarted
            | Self::AuthenticationFailed
            | Self::Cancelled => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{GreeterEvent, MessageLevel, PromptId, PromptKind};

    #[test]
    fn debug_redacts_pam_text() {
        let prompt = GreeterEvent::Prompt {
            id: PromptId::new(1),
            kind: PromptKind::Secret,
            message: "Password containing secret".to_owned(),
        };
        let message = GreeterEvent::Message {
            level: MessageLevel::Error,
            text: "entered token was 123456".to_owned(),
        };

        assert!(!format!("{prompt:?}").contains("secret"));
        assert!(!format!("{message:?}").contains("123456"));
    }
}
