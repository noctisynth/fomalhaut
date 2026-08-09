//! Sensitive authentication response storage.

use std::fmt;

use zeroize::Zeroize;

/// Authentication response whose Rust-side allocation is cleared on drop.
pub struct Secret(String);

impl Secret {
    /// Wraps a PAM response in zeroizing storage.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Consumes the wrapper and transfers its allocation to a trusted backend.
    ///
    /// The receiver becomes responsible for clearing every controllable copy.
    #[must_use]
    pub fn into_inner(mut self) -> String {
        std::mem::take(&mut self.0)
    }
}

impl From<String> for Secret {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl From<&str> for Secret {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Secret([REDACTED])")
    }
}

impl fmt::Display for Secret {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("[REDACTED]")
    }
}

impl Zeroize for Secret {
    fn zeroize(&mut self) {
        self.0.zeroize();
    }
}

impl Drop for Secret {
    fn drop(&mut self) {
        self.zeroize();
    }
}

#[cfg(test)]
mod tests {
    use super::Secret;
    use zeroize::Zeroize;

    #[test]
    fn formatting_never_exposes_the_value() {
        let secret = Secret::new("correct horse battery staple");

        assert_eq!(format!("{secret}"), "[REDACTED]");
        assert_eq!(format!("{secret:?}"), "Secret([REDACTED])");
    }

    #[test]
    fn zeroize_clears_the_owned_string() {
        let mut secret = Secret::new("sensitive");

        secret.zeroize();

        assert!(secret.0.is_empty());
    }
}
