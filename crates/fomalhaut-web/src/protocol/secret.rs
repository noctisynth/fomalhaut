//! Zeroizing authentication answer received from the frontend.

use std::fmt;

use fomalhaut_core::Secret;
use schemars::JsonSchema;
use serde::Serialize;
use zeroize::Zeroize;

use super::{MAX_AUTH_RESPONSE_BYTES, ProtocolValueError, value::validate_text};

/// Frontend authentication answer with redacted formatting and zeroizing drop.
#[derive(JsonSchema, Serialize)]
#[serde(transparent)]
#[schemars(transparent)]
pub struct ProtocolSecret(#[schemars(extend("x-fomalhaut-maxUtf8Bytes" = 16_384))] String);

impl ProtocolSecret {
    /// Validates and wraps an authentication answer.
    pub fn new(value: String) -> Result<Self, ProtocolValueError> {
        validate_text(&value, MAX_AUTH_RESPONSE_BYTES, true, false)?;
        Ok(Self(value))
    }

    /// Converts directly to the Core secret type without exposing a generic string API.
    #[must_use]
    pub fn into_core_secret(mut self) -> Secret {
        Secret::new(std::mem::take(&mut self.0))
    }
}

impl fmt::Debug for ProtocolSecret {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ProtocolSecret([REDACTED])")
    }
}

impl fmt::Display for ProtocolSecret {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("[REDACTED]")
    }
}

impl Drop for ProtocolSecret {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

#[cfg(test)]
mod tests {
    use super::ProtocolSecret;

    #[test]
    fn formatting_is_redacted() {
        let secret = ProtocolSecret::new("correct horse battery staple".to_owned())
            .expect("the fixture is within protocol limits");
        assert_eq!(format!("{secret}"), "[REDACTED]");
        assert_eq!(format!("{secret:?}"), "ProtocolSecret([REDACTED])");
    }
}
