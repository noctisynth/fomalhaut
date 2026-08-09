//! greetd-backed login capability for Fomalhaut.

mod client;
mod error;
mod state;
mod transport;

pub use client::GreeterClient;
pub use error::{GreetdError, ServerErrorKind, TransportError};
pub use state::GreeterState;
pub use transport::{Transport, UnixTransport};
