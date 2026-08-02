//! Trusted desktop-session discovery and parsing.

mod discovery;
mod error;
mod exec;
mod model;

pub use discovery::{DiscoveryConfig, discover};
pub use error::{DiscoveryError, Rejection, RejectionReason, SessionLookupError};
pub use model::{
    DiscoveryReport, SessionCatalog, SessionDirectory, SessionId, SessionInfo, SessionKind,
};
