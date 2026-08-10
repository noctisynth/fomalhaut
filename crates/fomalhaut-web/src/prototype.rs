//! Static bridge behavior for the non-authenticating WebKitGTK prototype.

use crate::protocol::{
    AuthState, Capabilities, Event, EventEnvelope, EventSequence, FrontendRequest,
    GreeterSnapshotFields, LoginState, ProtocolErrorBody, ProtocolErrorCode, ResponseEnvelope,
    ResponseResult, StateChangedData, StateSnapshot, UiLocale, decode_request,
};

const INVALID_MESSAGE: &str = "the native protocol bridge rejected the message";
const SERIALIZATION_FAILURE: &str = "the native protocol bridge could not encode a response";

/// Result of handling one prototype bridge message.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PrototypeReply {
    /// A serialized protocol v1 response that may be returned to JavaScript.
    Json(String),
    /// A stable rejection for messages without a safely recoverable request ID.
    Rejected(&'static str),
}

/// Handles a frontend request without connecting to greetd or starting a session.
#[must_use]
pub fn handle_prototype_request(input: &[u8]) -> PrototypeReply {
    let request = match decode_request(input) {
        Ok(request) => request,
        Err(error) => {
            let Some(id) = error.request_id() else {
                return PrototypeReply::Rejected(INVALID_MESSAGE);
            };
            return serialize(ResponseEnvelope::error(id, error.body().clone()));
        }
    };

    let id = request.id();
    let response = match request.request() {
        FrontendRequest::StateGet(_) => match prototype_state() {
            Ok(state) => ResponseEnvelope::success(id, ResponseResult::State(state)),
            Err(error) => ResponseEnvelope::error(id, error),
        },
        FrontendRequest::PowerRequest(_) => ResponseEnvelope::error(
            id,
            ProtocolErrorBody::new(
                ProtocolErrorCode::MethodDisabled,
                "power operations are disabled",
                false,
            ),
        ),
        _ => ResponseEnvelope::error(
            id,
            ProtocolErrorBody::new(
                ProtocolErrorCode::InvalidState,
                "authentication is unavailable in the host prototype",
                false,
            ),
        ),
    };
    serialize(response)
}

/// Builds a JavaScript call that delivers one serialized protocol event to the prototype page.
pub fn prototype_event_dispatch_script() -> Result<String, PrototypeScriptError> {
    let mut sequences = EventSequence::default();
    let sequence = sequences.allocate().map_err(|_| PrototypeScriptError)?;
    let event = EventEnvelope::new(
        sequence,
        Event::StateChanged(StateChangedData::new(AuthState::Failed)),
    );
    let json = serde_json::to_string(&event).map_err(|_| PrototypeScriptError)?;
    let quoted_json = serde_json::to_string(&json).map_err(|_| PrototypeScriptError)?;

    Ok(format!(
        "window.dispatchEvent(new CustomEvent('fomalhaut:event', {{ detail: JSON.parse({quoted_json}) }}));"
    ))
}

/// Sanitized failure to construct the prototype event dispatch script.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PrototypeScriptError;

impl std::fmt::Display for PrototypeScriptError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("the prototype event could not be serialized")
    }
}

impl std::error::Error for PrototypeScriptError {}

fn prototype_state() -> Result<StateSnapshot, ProtocolErrorBody> {
    StateSnapshot::greeter(GreeterSnapshotFields {
        locale: UiLocale::En,
        authentication: AuthState::Failed,
        login: LoginState::Idle,
        prompt: None,
        messages: Vec::new(),
        sequence: EventSequence::default().watermark(),
        users: Vec::new(),
        sessions: Vec::new(),
        selected_session_id: None,
        capabilities: Capabilities::disabled(),
    })
    .map_err(|_| {
        ProtocolErrorBody::new(
            ProtocolErrorCode::Internal,
            "the host could not construct its public state",
            false,
        )
    })
}

fn serialize(response: ResponseEnvelope) -> PrototypeReply {
    match serde_json::to_string(&response) {
        Ok(json) => PrototypeReply::Json(json),
        Err(_) => PrototypeReply::Rejected(SERIALIZATION_FAILURE),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use super::{PrototypeReply, handle_prototype_request, prototype_event_dispatch_script};

    fn response_value(request: &str) -> Value {
        let PrototypeReply::Json(response) = handle_prototype_request(request.as_bytes()) else {
            panic!("the correlated request must produce a JSON response");
        };
        serde_json::from_str(&response).expect("the bridge emits valid JSON")
    }

    #[test]
    fn state_probe_returns_failed_empty_greeter_snapshot() {
        let value = response_value(r#"{"protocol":1,"id":7,"method":"state.get","params":{}}"#);

        assert_eq!(value["protocol"], 1);
        assert_eq!(value["id"], 7);
        assert_eq!(value["ok"], true);
        assert_eq!(value["result"]["mode"], "greeter");
        assert_eq!(value["result"]["authentication"], "failed");
        assert_eq!(value["result"]["sessions"], serde_json::json!([]));
        assert_eq!(
            value["result"]["capabilities"]["power"],
            serde_json::json!([])
        );
    }

    #[test]
    fn known_mutating_operations_are_disabled_in_the_prototype() {
        let auth = response_value(
            r#"{"protocol":1,"id":8,"method":"auth.begin","params":{"username":"alice"}}"#,
        );
        assert_eq!(auth["error"]["code"], "invalid_state");

        let power = response_value(
            r#"{"protocol":1,"id":9,"method":"power.request","params":{"action":"reboot"}}"#,
        );
        assert_eq!(power["error"]["code"], "method_disabled");
    }

    #[test]
    fn malformed_uncorrelated_messages_are_rejected_without_a_response_id() {
        assert!(matches!(
            handle_prototype_request(b"{"),
            PrototypeReply::Rejected(_)
        ));
    }

    #[test]
    fn correlated_decode_failures_return_protocol_errors() {
        let value = response_value(r#"{"protocol":1,"id":10,"method":"unknown","params":{}}"#);
        assert_eq!(value["id"], 10);
        assert_eq!(value["error"]["code"], "unknown_method");
    }

    #[test]
    fn outbound_probe_is_a_serialized_protocol_event() {
        let script =
            prototype_event_dispatch_script().expect("the fixed prototype event is serializable");
        assert!(script.contains("state.changed"));
        assert!(script.contains("failed"));
        assert!(script.contains("JSON.parse"));
    }
}
