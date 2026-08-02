//! Shared value validation helpers.

use super::ProtocolValueError;

pub(crate) fn validate_text(
    value: &str,
    maximum_bytes: usize,
    allow_empty: bool,
    reject_controls: bool,
) -> Result<(), ProtocolValueError> {
    if !allow_empty && value.is_empty() {
        return Err(ProtocolValueError::Empty);
    }
    if value.len() > maximum_bytes {
        return Err(ProtocolValueError::TooLong);
    }
    if value.contains('\0') || (reject_controls && value.chars().any(char::is_control)) {
        return Err(ProtocolValueError::InvalidCharacter);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::validate_text;
    use crate::protocol::ProtocolValueError;

    #[test]
    fn limits_text_by_utf8_bytes_instead_of_character_count() {
        assert_eq!(validate_text("北落师门", 12, false, false), Ok(()));
        assert_eq!(
            validate_text("北落师门", 11, false, false),
            Err(ProtocolValueError::TooLong)
        );
    }
}
