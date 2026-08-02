//! Strict request envelope decoding.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::Value;

use super::{
    MAX_MESSAGE_BYTES, MAX_SAFE_INTEGER, MAX_SESSION_ID_BYTES, MAX_USERNAME_BYTES,
    PROTOCOL_VERSION, PowerAction, ProtocolDecodeError, ProtocolErrorBody, ProtocolSecret,
    ProtocolValueError, value::validate_text,
};

/// Correlation identifier supplied by the frontend.
#[derive(Clone, Copy, Debug, Eq, Hash, JsonSchema, PartialEq, Serialize)]
#[serde(transparent)]
pub struct RequestId(#[schemars(range(max = 9_007_199_254_740_991_u64))] u64);

impl RequestId {
    /// Constructs an ID exactly representable by JavaScript.
    pub fn new(value: u64) -> Result<Self, ProtocolValueError> {
        if value > MAX_SAFE_INTEGER {
            return Err(ProtocolValueError::UnsafeInteger);
        }
        Ok(Self(value))
    }

    /// Returns the numeric ID.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Identifier of the currently active authentication prompt.
#[derive(Clone, Copy, Debug, Eq, Hash, JsonSchema, PartialEq, Serialize)]
#[serde(transparent)]
pub struct PromptId(#[schemars(range(max = 9_007_199_254_740_991_u64))] u64);

impl PromptId {
    /// Constructs a prompt ID exactly representable by JavaScript.
    pub fn new(value: u64) -> Result<Self, ProtocolValueError> {
        if value > MAX_SAFE_INTEGER {
            return Err(ProtocolValueError::UnsafeInteger);
        }
        Ok(Self(value))
    }

    /// Returns the numeric ID.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Parameters for `auth.begin`.
#[derive(Debug, JsonSchema, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct AuthBeginParams {
    #[schemars(length(min = 1), extend("x-fomalhaut-maxUtf8Bytes" = 256))]
    username: String,
}

impl AuthBeginParams {
    fn new(username: String) -> Result<Self, ProtocolValueError> {
        validate_text(&username, MAX_USERNAME_BYTES, false, true)?;
        Ok(Self { username })
    }

    /// Returns the validated username.
    #[must_use]
    pub fn username(&self) -> &str {
        &self.username
    }
}

/// Parameters for `auth.respond`.
#[derive(Debug, JsonSchema, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct AuthRespondParams {
    prompt_id: PromptId,
    response: ProtocolSecret,
}

impl AuthRespondParams {
    /// Returns the validated prompt identifier.
    #[must_use]
    pub const fn prompt_id(&self) -> PromptId {
        self.prompt_id
    }

    /// Consumes the parameters and returns the zeroizing answer.
    #[must_use]
    pub fn into_response(self) -> ProtocolSecret {
        self.response
    }

    /// Consumes the parameters and returns the prompt identifier and zeroizing answer together.
    #[must_use]
    pub fn into_parts(self) -> (PromptId, ProtocolSecret) {
        (self.prompt_id, self.response)
    }
}

/// Parameters for `session.select`.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SessionSelectParams {
    #[schemars(length(min = 1), extend("x-fomalhaut-maxUtf8Bytes" = 256))]
    session_id: String,
}

impl SessionSelectParams {
    fn new(session_id: String) -> Result<Self, ProtocolValueError> {
        validate_text(&session_id, MAX_SESSION_ID_BYTES, false, true)?;
        Ok(Self { session_id })
    }

    /// Returns the opaque session identifier.
    #[must_use]
    pub fn session_id(&self) -> &str {
        &self.session_id
    }
}

/// Typed frontend operation.
#[derive(Debug, JsonSchema, Serialize)]
#[serde(tag = "method", content = "params")]
pub enum FrontendRequest {
    /// Retrieves a complete public state snapshot.
    #[serde(rename = "state.get")]
    StateGet(EmptyParams),
    /// Starts authentication for a username.
    #[serde(rename = "auth.begin")]
    AuthBegin(AuthBeginParams),
    /// Answers the active prompt.
    #[serde(rename = "auth.respond")]
    AuthRespond(AuthRespondParams),
    /// Cancels active authentication.
    #[serde(rename = "auth.cancel")]
    AuthCancel(EmptyParams),
    /// Selects an opaque trusted session.
    #[serde(rename = "session.select")]
    SessionSelect(SessionSelectParams),
    /// Requests an enumerated power action. The host keeps this disabled until policy exists.
    #[serde(rename = "power.request")]
    PowerRequest(PowerRequestParams),
}

/// Strict top-level request.
#[derive(Debug, JsonSchema, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RequestEnvelope {
    #[schemars(extend("const" = 1))]
    protocol: u16,
    id: RequestId,
    #[serde(flatten)]
    request: FrontendRequest,
}

impl RequestEnvelope {
    /// Returns the request correlation ID.
    #[must_use]
    pub const fn id(&self) -> RequestId {
        self.id
    }

    /// Returns the typed operation.
    #[must_use]
    pub const fn request(&self) -> &FrontendRequest {
        &self.request
    }

    /// Consumes the envelope into its correlation ID and typed operation.
    #[must_use]
    pub fn into_parts(self) -> (RequestId, FrontendRequest) {
        (self.id, self.request)
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawEnvelope {
    protocol: u16,
    id: u64,
    method: String,
    params: Value,
}

#[derive(Debug, Deserialize, JsonSchema, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EmptyParams {}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct RawAuthBeginParams {
    username: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct RawAuthRespondParams {
    prompt_id: u64,
    response: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct RawSessionSelectParams {
    session_id: String,
}

#[derive(Debug, Deserialize, JsonSchema, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct PowerRequestParams {
    action: PowerAction,
}

impl PowerRequestParams {
    /// Returns the requested enumerated action.
    #[must_use]
    pub const fn action(&self) -> PowerAction {
        self.action
    }
}

/// Decodes and validates one untrusted JSON request.
pub fn decode_request(input: &[u8]) -> Result<RequestEnvelope, ProtocolDecodeError> {
    if input.len() > MAX_MESSAGE_BYTES {
        return Err(ProtocolDecodeError::new(
            None,
            ProtocolErrorBody::message_too_large(),
        ));
    }

    let value: Value = serde_json::from_slice(input)
        .map_err(|_| ProtocolDecodeError::new(None, ProtocolErrorBody::invalid_json()))?;
    let id_hint = request_id_hint(&value);
    let raw: RawEnvelope = serde_json::from_value(value)
        .map_err(|_| ProtocolDecodeError::new(id_hint, ProtocolErrorBody::invalid_request()))?;
    let id = RequestId::new(raw.id)
        .map_err(|_| ProtocolDecodeError::new(id_hint, ProtocolErrorBody::invalid_request()))?;
    if raw.protocol != PROTOCOL_VERSION {
        return Err(ProtocolDecodeError::new(
            Some(id),
            ProtocolErrorBody::unsupported_version(),
        ));
    }

    let request = decode_method(&raw.method, raw.params, id)?;
    Ok(RequestEnvelope {
        protocol: PROTOCOL_VERSION,
        id,
        request,
    })
}

fn decode_method(
    method: &str,
    params: Value,
    id: RequestId,
) -> Result<FrontendRequest, ProtocolDecodeError> {
    let invalid = |_: ProtocolValueError| {
        ProtocolDecodeError::new(Some(id), ProtocolErrorBody::invalid_params())
    };
    match method {
        "state.get" => Ok(FrontendRequest::StateGet(parse_params(params, id)?)),
        "auth.begin" => {
            let raw: RawAuthBeginParams = parse_params(params, id)?;
            Ok(FrontendRequest::AuthBegin(
                AuthBeginParams::new(raw.username).map_err(invalid)?,
            ))
        }
        "auth.respond" => {
            let raw: RawAuthRespondParams = parse_params(params, id)?;
            let prompt_id = PromptId::new(raw.prompt_id).map_err(invalid)?;
            let response = ProtocolSecret::new(raw.response).map_err(invalid)?;
            Ok(FrontendRequest::AuthRespond(AuthRespondParams {
                prompt_id,
                response,
            }))
        }
        "auth.cancel" => Ok(FrontendRequest::AuthCancel(parse_params(params, id)?)),
        "session.select" => {
            let raw: RawSessionSelectParams = parse_params(params, id)?;
            Ok(FrontendRequest::SessionSelect(
                SessionSelectParams::new(raw.session_id).map_err(invalid)?,
            ))
        }
        "power.request" => Ok(FrontendRequest::PowerRequest(parse_params(params, id)?)),
        _ => Err(ProtocolDecodeError::new(
            Some(id),
            ProtocolErrorBody::unknown_method(),
        )),
    }
}

fn parse_params<T: DeserializeOwned>(
    params: Value,
    id: RequestId,
) -> Result<T, ProtocolDecodeError> {
    serde_json::from_value(params)
        .map_err(|_| ProtocolDecodeError::new(Some(id), ProtocolErrorBody::invalid_params()))
}

fn request_id_hint(value: &Value) -> Option<RequestId> {
    value
        .as_object()?
        .get("id")?
        .as_u64()
        .and_then(|value| RequestId::new(value).ok())
}

#[cfg(test)]
mod tests {
    use super::{FrontendRequest, decode_request};
    use crate::protocol::{
        MAX_AUTH_RESPONSE_BYTES, MAX_MESSAGE_BYTES, MAX_SAFE_INTEGER, PowerAction,
        ProtocolErrorCode,
    };

    #[test]
    fn decodes_every_supported_method() {
        let cases = [
            r#"{"protocol":1,"id":1,"method":"state.get","params":{}}"#,
            r#"{"protocol":1,"id":2,"method":"auth.begin","params":{"username":"alice"}}"#,
            r#"{"protocol":1,"id":3,"method":"auth.respond","params":{"promptId":7,"response":"token"}}"#,
            r#"{"protocol":1,"id":4,"method":"auth.cancel","params":{}}"#,
            r#"{"protocol":1,"id":5,"method":"session.select","params":{"sessionId":"wayland:sway"}}"#,
            r#"{"protocol":1,"id":6,"method":"power.request","params":{"action":"reboot"}}"#,
        ];

        for message in cases {
            decode_request(message.as_bytes()).expect("the v1 request fixture is valid");
        }

        let begin = decode_request(cases[1].as_bytes()).expect("auth.begin is valid");
        assert!(matches!(
            begin.request(),
            FrontendRequest::AuthBegin(params) if params.username() == "alice"
        ));
        let select = decode_request(cases[4].as_bytes()).expect("session.select is valid");
        assert!(matches!(
            select.request(),
            FrontendRequest::SessionSelect(params) if params.session_id() == "wayland:sway"
        ));
        let power = decode_request(cases[5].as_bytes()).expect("power.request is valid");
        assert!(matches!(
            power.request(),
            FrontendRequest::PowerRequest(params) if params.action() == PowerAction::Reboot
        ));
    }

    #[test]
    fn rejects_unknown_fields_methods_and_versions_with_correlation() {
        let fixtures = [
            (
                r#"{"protocol":1,"id":12,"method":"state.get","params":{},"extra":true}"#,
                ProtocolErrorCode::InvalidRequest,
            ),
            (
                r#"{"protocol":1,"id":12,"method":"state.get","params":{"extra":true}}"#,
                ProtocolErrorCode::InvalidParams,
            ),
            (
                r#"{"protocol":1,"id":12,"method":"auth.unknown","params":{}}"#,
                ProtocolErrorCode::UnknownMethod,
            ),
            (
                r#"{"protocol":2,"id":12,"method":"state.get","params":{}}"#,
                ProtocolErrorCode::UnsupportedVersion,
            ),
        ];

        for (message, expected) in fixtures {
            let error =
                decode_request(message.as_bytes()).expect_err("the fixture must be rejected");
            assert_eq!(error.request_id().map(|id| id.get()), Some(12));
            assert_eq!(error.body().code(), expected);
        }
    }

    #[test]
    fn rejects_unrecoverable_json_and_unsafe_or_oversized_values() {
        let json_error = decode_request(b"{").expect_err("truncated JSON is invalid");
        assert_eq!(json_error.request_id(), None);
        assert_eq!(json_error.body().code(), ProtocolErrorCode::InvalidJson);

        let unsafe_id = format!(
            r#"{{"protocol":1,"id":{},"method":"state.get","params":{{}}}}"#,
            MAX_SAFE_INTEGER + 1
        );
        let error = decode_request(unsafe_id.as_bytes()).expect_err("unsafe integer is invalid");
        assert_eq!(error.request_id(), None);
        assert_eq!(error.body().code(), ProtocolErrorCode::InvalidRequest);

        let response = "x".repeat(MAX_AUTH_RESPONSE_BYTES + 1);
        let oversized = format!(
            r#"{{"protocol":1,"id":9,"method":"auth.respond","params":{{"promptId":1,"response":"{response}"}}}}"#
        );
        let error = decode_request(oversized.as_bytes()).expect_err("oversized answer is invalid");
        assert_eq!(error.request_id().map(|id| id.get()), Some(9));
        assert_eq!(error.body().code(), ProtocolErrorCode::InvalidParams);

        let oversized_message = vec![b' '; MAX_MESSAGE_BYTES + 1];
        let error = decode_request(&oversized_message).expect_err("oversized message is invalid");
        assert_eq!(error.request_id(), None);
        assert_eq!(error.body().code(), ProtocolErrorCode::MessageTooLarge);
    }
}
