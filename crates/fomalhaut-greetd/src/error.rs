//! Structured greetd errors that do not expose authentication responses.

use std::{error::Error, fmt, io};

use fomalhaut_core::{BackendError, CoreError};
use greetd_ipc::codec;

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
    /// Authentication failed outside the normal cancellable flow.
    Authentication,
    /// Greetd reported a general error.
    General,
}

/// Failure while driving the greetd login backend.
#[derive(Debug)]
pub enum GreetdError {
    /// The common authentication state machine rejected the operation.
    Core(CoreError),
    /// Communication with greetd failed.
    Transport(TransportError),
    /// Greetd returned a response invalid for the active operation.
    UnexpectedResponse {
        /// Operation being processed.
        operation: &'static str,
        /// Sanitized response category.
        response: &'static str,
    },
    /// Greetd rejected an operation. Its raw description is intentionally omitted.
    Server(ServerErrorKind),
    /// An internal login identity invariant was violated.
    MissingIdentity,
}

impl GreetdError {
    pub(crate) fn into_backend_error(self) -> BackendError {
        match self {
            Self::Core(error) => BackendError::Core(error),
            Self::Transport(_) => BackendError::Unavailable,
            Self::UnexpectedResponse { .. } | Self::MissingIdentity => BackendError::Protocol,
            Self::Server(_) => BackendError::Service,
        }
    }
}

impl fmt::Display for GreetdError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Core(error) => error.fmt(formatter),
            Self::Transport(error) => error.fmt(formatter),
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
            Self::MissingIdentity => {
                formatter.write_str("greetd completed authentication without an active identity")
            }
        }
    }
}

impl Error for GreetdError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Core(error) => Some(error),
            Self::Transport(error) => Some(error),
            Self::UnexpectedResponse { .. } | Self::Server(_) | Self::MissingIdentity => None,
        }
    }
}

impl From<CoreError> for GreetdError {
    fn from(error: CoreError) -> Self {
        Self::Core(error)
    }
}

impl From<TransportError> for GreetdError {
    fn from(error: TransportError) -> Self {
        Self::Transport(error)
    }
}
