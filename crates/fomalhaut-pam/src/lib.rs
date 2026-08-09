//! Isolated current-user PAM reauthentication backend for Fomalhaut.

mod backend;
mod identity;
mod ipc;
mod worker;

pub use backend::{PamBackendError, PamReauthBackend};
pub use identity::{CurrentUserIdentity, IdentityDiscoveryError};
pub use worker::{PAM_WORKER_ARGUMENT, PamWorkerError, run_pam_worker};
