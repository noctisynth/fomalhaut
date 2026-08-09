//! Sanitized backend-neutral authentication errors.

use std::{error::Error, fmt};

use crate::{AuthState, PromptId};

/// Invalid authentication-domain operation.
#[derive(Debug)]
pub enum CoreError {
    /// The requested operation is invalid in the current state.
    InvalidState {
        /// Attempted operation.
        operation: &'static str,
        /// State in which it was attempted.
        state: AuthState,
    },
    /// The response targets an old or inactive prompt.
    StalePrompt {
        /// Prompt that is currently active.
        expected: Option<PromptId>,
        /// Prompt supplied by the caller.
        received: PromptId,
    },
    /// No emitted event is waiting to be consumed.
    NoPendingEvent,
    /// Prompt identifiers can no longer be allocated.
    PromptIdExhausted,
    /// A session command must contain at least one argument.
    EmptySessionCommand,
    /// An authenticated identity must contain an account name.
    EmptyIdentity,
}

impl fmt::Display for CoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidState { operation, state } => {
                write!(
                    formatter,
                    "cannot {operation} while authentication is {state:?}"
                )
            }
            Self::StalePrompt { expected, received } => write!(
                formatter,
                "prompt {} is stale; active prompt is {}",
                received.get(),
                expected.map_or_else(|| "none".to_owned(), |id| id.get().to_string())
            ),
            Self::NoPendingEvent => formatter.write_str("no authentication event is pending"),
            Self::PromptIdExhausted => formatter.write_str("prompt identifier space exhausted"),
            Self::EmptySessionCommand => {
                formatter.write_str("session command must contain at least one argument")
            }
            Self::EmptyIdentity => formatter.write_str("authenticated identity cannot be empty"),
        }
    }
}

impl Error for CoreError {}

/// Sanitized failure shared by login and reauthentication backends.
#[derive(Debug)]
pub enum BackendError {
    /// The request violated the common authentication state machine.
    Core(CoreError),
    /// The backend transport or worker is unavailable.
    Unavailable,
    /// The backend returned an invalid protocol response.
    Protocol,
    /// The authentication service rejected a non-authentication operation.
    Service,
}

impl fmt::Display for BackendError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Core(error) => error.fmt(formatter),
            Self::Unavailable => formatter.write_str("authentication backend is unavailable"),
            Self::Protocol => formatter.write_str("authentication backend protocol failed"),
            Self::Service => formatter.write_str("authentication service failed"),
        }
    }
}

impl Error for BackendError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Core(error) => Some(error),
            Self::Unavailable | Self::Protocol | Self::Service => None,
        }
    }
}

impl From<CoreError> for BackendError {
    fn from(error: CoreError) -> Self {
        Self::Core(error)
    }
}
