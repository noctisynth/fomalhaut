//! Backend-neutral authentication states.

/// Current state of a serial authentication conversation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuthState {
    /// The backend transport or worker is no longer usable.
    Disconnected,
    /// No authentication transaction is active.
    Idle,
    /// The authentication service is advancing the conversation.
    Authenticating,
    /// The backend is waiting for a hidden response.
    WaitingForSecret,
    /// The backend is waiting for a visible response.
    WaitingForVisible,
    /// Authentication completed successfully.
    Authenticated,
    /// The backend is cancelling the active transaction.
    Cancelling,
    /// Authentication or its backing service failed.
    Failed,
}
