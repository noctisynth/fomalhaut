//! Observable states of a greeter client.

/// Current state of the greetd authentication and session lifecycle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GreeterState {
    /// The transport is no longer usable.
    Disconnected,
    /// Connected to greetd with no active session.
    Idle,
    /// Waiting for greetd to advance authentication.
    Authenticating,
    /// Waiting for the caller to answer the current PAM prompt.
    WaitingForPrompt,
    /// Authentication succeeded and a trusted session may be started.
    Authenticated,
    /// Waiting for greetd to start the user session.
    StartingSession,
    /// The user session started successfully.
    Started,
    /// Waiting for greetd to cancel the active session.
    Cancelling,
    /// Greetd rejected the operation and automatically cancelled its session.
    Failed,
}
