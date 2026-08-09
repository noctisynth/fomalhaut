//! Backend-verified account identity.

use std::fmt;

use zeroize::Zeroize;

use crate::CoreError;

/// Identity authenticated by a trusted login or reauthentication backend.
#[derive(Eq, PartialEq)]
pub struct AuthenticatedIdentity {
    account_name: String,
}

impl AuthenticatedIdentity {
    /// Constructs an identity from a backend-verified non-empty account name.
    pub fn new(account_name: impl Into<String>) -> Result<Self, CoreError> {
        let account_name = account_name.into();
        if account_name.is_empty() {
            return Err(CoreError::EmptyIdentity);
        }
        Ok(Self { account_name })
    }

    /// Returns the verified account name.
    #[must_use]
    pub fn account_name(&self) -> &str {
        &self.account_name
    }

    /// Clears the Rust-owned account name allocation.
    pub(crate) fn zeroize(&mut self) {
        self.account_name.zeroize();
    }
}

impl fmt::Debug for AuthenticatedIdentity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("AuthenticatedIdentity([REDACTED])")
    }
}

impl Drop for AuthenticatedIdentity {
    fn drop(&mut self) {
        self.zeroize();
    }
}
