//! UI-independent greetd IPC and authentication state machine.

mod client;
mod error;
mod event;
mod secret;
mod state;
mod transport;

pub use client::{GreeterClient, SessionCommand};
pub use error::{CoreError, ServerErrorKind, TransportError};
pub use event::{GreeterEvent, MessageLevel, PromptId, PromptKind};
pub use secret::Secret;
pub use state::GreeterState;
pub use transport::{Transport, UnixTransport};
