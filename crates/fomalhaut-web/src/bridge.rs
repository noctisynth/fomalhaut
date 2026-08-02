//! Serialization helpers for the JavaScript bridge boundary.

use std::{error::Error, fmt};

use crate::protocol::{EventEnvelope, ResponseEnvelope};

/// Sanitized bridge serialization failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BridgeSerializationError;

impl fmt::Display for BridgeSerializationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("the frontend bridge could not serialize a protocol message")
    }
}

impl Error for BridgeSerializationError {}

/// Serializes a correlated protocol response for `ScriptMessageReply`.
pub fn response_json(response: &ResponseEnvelope) -> Result<String, BridgeSerializationError> {
    serde_json::to_string(response).map_err(|_| BridgeSerializationError)
}

/// Builds a JavaScript call that dispatches one already typed protocol event.
pub fn event_dispatch_script(event: &EventEnvelope) -> Result<String, BridgeSerializationError> {
    let json = serde_json::to_string(event).map_err(|_| BridgeSerializationError)?;
    let quoted_json = serde_json::to_string(&json).map_err(|_| BridgeSerializationError)?;
    Ok(format!(
        "window.dispatchEvent(new CustomEvent('fomalhaut:event', {{ detail: JSON.parse({quoted_json}) }}));"
    ))
}

#[cfg(test)]
mod tests {
    use super::event_dispatch_script;
    use crate::protocol::{AuthState, Event, EventEnvelope, EventSequence, StateChangedData};

    #[test]
    fn event_script_embeds_json_without_inline_source_injection() {
        let sequence = EventSequence::default()
            .allocate()
            .expect("the first event sequence is available");
        let event = EventEnvelope::new(
            sequence,
            Event::StateChanged(StateChangedData::new(AuthState::Idle)),
        );
        let script = event_dispatch_script(&event).expect("the typed event is serializable");

        assert!(script.contains("JSON.parse"));
        assert!(script.contains("state.changed"));
        assert!(!script.contains("</script>"));
    }
}
