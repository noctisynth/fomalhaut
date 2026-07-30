//! Structured errors that do not expose authentication responses.

use std::{error::Error, fmt, io};

use greetd_ipc::codec;

use crate::{GreeterState, PromptId};

/// Transport-level failure while communicating with greetd.
#[derive(Debug)]
pub enum TransportError {
    /// The greetd Unix socket could not be opened.
    Connect(io::Error),
    /// A request or response failed greetd IPC encoding or I/O.
    Codec(codec::Error),
    /// A test or alternate transport became unavailable.
    Unavailable(&'static str),
}

impl fmt::Display for TransportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Connect(error) => write!(formatter, "failed to connect to greetd: {error}"),
            Self::Codec(error) => write!(formatter, "greetd IPC transport failed: {error}"),
            Self::Unavailable(reason) => write!(formatter, "transport unavailable: {reason}"),
        }
    }
}

impl Error for TransportError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Connect(error) => Some(error),
            Self::Codec(error) => Some(error),
            Self::Unavailable(_) => None,
        }
    }
}

impl From<codec::Error> for TransportError {
    fn from(error: codec::Error) -> Self {
        Self::Codec(error)
    }
}

/// Sanitized category of an error returned by greetd.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ServerErrorKind {
    /// Authentication failed.
    Authentication,
    /// Greetd reported a general error.
    General,
}

/// Failure while driving the authentication state machine.
#[derive(Debug)]
pub enum CoreError {
    /// Communication with greetd failed.
    Transport(TransportError),
    /// The requested operation is invalid in the current state.
    InvalidState {
        /// Attempted operation.
        operation: &'static str,
        /// State in which it was attempted.
        state: GreeterState,
    },
    /// The response targets an old or otherwise inactive prompt.
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
    /// Greetd returned a response that is invalid for the active operation.
    UnexpectedResponse {
        /// Operation being processed.
        operation: &'static str,
        /// Sanitized response category.
        response: &'static str,
    },
    /// Greetd rejected an operation. Its raw description is intentionally omitted.
    Server(ServerErrorKind),
}

impl fmt::Display for CoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Transport(error) => error.fmt(formatter),
            Self::InvalidState { operation, state } => {
                write!(formatter, "cannot {operation} while greeter is {state:?}")
            }
            Self::StalePrompt { expected, received } => write!(
                formatter,
                "prompt {} is stale; active prompt is {}",
                received.get(),
                expected.map_or_else(|| "none".to_owned(), |id| id.get().to_string())
            ),
            Self::NoPendingEvent => formatter.write_str("no greeter event is pending"),
            Self::PromptIdExhausted => formatter.write_str("prompt identifier space exhausted"),
            Self::EmptySessionCommand => {
                formatter.write_str("session command must contain at least one argument")
            }
            Self::UnexpectedResponse {
                operation,
                response,
            } => write!(
                formatter,
                "greetd returned {response} while attempting to {operation}"
            ),
            Self::Server(ServerErrorKind::Authentication) => {
                formatter.write_str("greetd rejected authentication")
            }
            Self::Server(ServerErrorKind::General) => {
                formatter.write_str("greetd reported a general error")
            }
        }
    }
}

impl Error for CoreError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Transport(error) => Some(error),
            Self::InvalidState { .. }
            | Self::StalePrompt { .. }
            | Self::NoPendingEvent
            | Self::PromptIdExhausted
            | Self::EmptySessionCommand
            | Self::UnexpectedResponse { .. }
            | Self::Server(_) => None,
        }
    }
}

impl From<TransportError> for CoreError {
    fn from(error: TransportError) -> Self {
        Self::Transport(error)
    }
}
