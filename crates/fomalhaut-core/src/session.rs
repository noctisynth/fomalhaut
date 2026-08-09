//! Trusted session command value.

use crate::CoreError;

/// Command and environment selected by a trusted host.
#[derive(Clone)]
pub struct SessionCommand {
    command: Vec<String>,
    environment: Vec<String>,
}

impl SessionCommand {
    /// Constructs a session command, rejecting an empty argument vector.
    pub fn new(command: Vec<String>, environment: Vec<String>) -> Result<Self, CoreError> {
        if command.is_empty() {
            return Err(CoreError::EmptySessionCommand);
        }

        Ok(Self {
            command,
            environment,
        })
    }

    /// Consumes the trusted value into its command and environment arrays.
    #[must_use]
    pub fn into_parts(self) -> (Vec<String>, Vec<String>) {
        (self.command, self.environment)
    }
}
