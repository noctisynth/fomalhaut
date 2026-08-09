//! Capability-separated authentication backend traits.

use std::future::Future;

use crate::{AuthEvent, AuthState, BackendError, PromptId, Secret, SessionCommand};

/// Shared interactive authentication operations.
pub trait ConversationBackend: Send {
    /// Returns the current backend-neutral authentication state.
    fn state(&self) -> AuthState;

    /// Returns whether graceful shutdown must cancel an active transaction.
    fn needs_cancel(&self) -> bool;

    /// Answers the one currently active prompt.
    fn respond(
        &mut self,
        prompt: PromptId,
        response: Secret,
    ) -> impl Future<Output = Result<(), BackendError>> + Send;

    /// Cancels the active authentication transaction.
    fn cancel(&mut self) -> impl Future<Output = Result<(), BackendError>> + Send;

    /// Consumes the next event emitted by the backend.
    fn next_event(&mut self) -> impl Future<Output = Result<AuthEvent, BackendError>> + Send;
}

/// Backend capability that may choose an account and start a trusted session.
pub trait LoginBackend: ConversationBackend {
    /// Begins login authentication for a host-validated username.
    fn begin_login(
        &mut self,
        username: String,
    ) -> impl Future<Output = Result<(), BackendError>> + Send;

    /// Starts a session command resolved by the trusted host.
    fn start_session(
        &mut self,
        command: SessionCommand,
    ) -> impl Future<Output = Result<(), BackendError>> + Send;

    /// Returns whether the trusted session has started successfully.
    fn session_started(&self) -> bool;
}

/// Backend capability that can only reauthenticate the current session user.
pub trait ReauthBackend: ConversationBackend {
    /// Begins reauthentication for the identity fixed by the backend.
    fn begin_reauth(&mut self) -> impl Future<Output = Result<(), BackendError>> + Send;
}
