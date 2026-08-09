//! Backend-neutral authentication domain types and capability boundaries.

mod backend;
mod conversation;
mod error;
mod event;
mod identity;
mod secret;
mod session;
mod state;

pub use backend::{ConversationBackend, LoginBackend, ReauthBackend};
pub use conversation::AuthConversation;
pub use error::{BackendError, CoreError};
pub use event::{AuthEvent, MessageLevel, PromptId, PromptKind};
pub use identity::AuthenticatedIdentity;
pub use secret::Secret;
pub use session::SessionCommand;
pub use state::AuthState;
