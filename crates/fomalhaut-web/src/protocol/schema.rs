//! Deterministic JSON Schema generation.

use schemars::{Schema, generate::SchemaSettings};

use super::WireMessage;

/// Generates the v1 wire schema using an explicitly pinned JSON Schema dialect.
#[must_use]
pub fn wire_schema() -> Schema {
    SchemaSettings::draft2020_12()
        .for_serialize()
        .into_generator()
        .into_root_schema_for::<WireMessage>()
}

/// Serializes the deterministic schema as pretty JSON with a trailing newline.
pub fn schema_json_pretty() -> Result<String, serde_json::Error> {
    let mut output = serde_json::to_string_pretty(&wire_schema())?;
    output.push('\n');
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::schema_json_pretty;

    #[test]
    fn checked_in_schema_matches_wire_types() {
        let generated = schema_json_pretty().expect("the wire schema contains serializable values");
        let checked_in = include_str!("../../../../protocol/v1.schema.json");

        assert_eq!(generated, checked_in);
    }
}
