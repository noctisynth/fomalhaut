//! Strict request envelope decoding.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::Value;
use ts_rs::TS;

use super::{
    MAX_MESSAGE_BYTES, MAX_SAFE_INTEGER, MAX_SESSION_ID_BYTES, MAX_USERNAME_BYTES,
    PROTOCOL_VERSION, PowerAction, ProtocolDecodeError, ProtocolErrorBody, ProtocolErrorCode,
    ProtocolSecret, ProtocolValueError, RuntimeMode, value::validate_text,
};

/// Correlation identifier supplied by the frontend.
#[derive(Clone, Copy, Debug, Eq, Hash, JsonSchema, PartialEq, Serialize, TS)]
#[serde(transparent)]
#[ts(export, export_to = "v1/protocol-request.ts")]
pub struct RequestId(
    #[schemars(range(max = 9_007_199_254_740_991_u64))]
    #[ts(type = "number")]
    u64,
);

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
#[derive(Clone, Copy, Debug, Eq, Hash, JsonSchema, PartialEq, Serialize, TS)]
#[serde(transparent)]
#[ts(export, export_to = "v1/protocol-request.ts")]
pub struct PromptId(
    #[schemars(range(max = 9_007_199_254_740_991_u64))]
    #[ts(type = "number")]
    u64,
);

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

/// Greeter parameters for `auth.begin`.
#[derive(Debug, JsonSchema, Serialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[ts(export, export_to = "v1/protocol-request.ts")]
pub struct GreeterAuthBeginParams {
    #[schemars(length(min = 1), extend("x-fomalhaut-maxUtf8Bytes" = 256))]
    username: String,
}

impl GreeterAuthBeginParams {
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

/// Locker parameters for `auth.begin`.
#[derive(Debug, JsonSchema, Serialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(
    export,
    export_to = "v1/protocol-request.ts",
    type = "Record<string, never>"
)]
pub struct LockerAuthBeginParams {}

/// Role-specific parameters accepted by `auth.begin` before host-mode validation.
#[derive(Debug, JsonSchema, Serialize, TS)]
#[serde(untagged)]
#[ts(export, export_to = "v1/protocol-request.ts")]
pub enum AuthBeginParams {
    Greeter(GreeterAuthBeginParams),
    Locker(LockerAuthBeginParams),
}

impl AuthBeginParams {
    /// Returns the greeter username or `None` for the locker parameter form.
    #[must_use]
    pub fn username(&self) -> Option<&str> {
        match self {
            Self::Greeter(params) => Some(params.username()),
            Self::Locker(_) => None,
        }
    }

    /// Returns whether the empty locker parameter form was supplied.
    #[must_use]
    pub const fn is_locker(&self) -> bool {
        matches!(self, Self::Locker(_))
    }
}

/// Parameters for `auth.respond`.
#[derive(Debug, JsonSchema, Serialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[ts(export, export_to = "v1/protocol-request.ts")]
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
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[ts(export, export_to = "v1/protocol-request.ts")]
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
#[derive(Debug, JsonSchema, Serialize, TS)]
#[serde(tag = "method", content = "params")]
#[ts(export, export_to = "v1/protocol-request.ts")]
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
    /// Requests an enumerated power action allowed by trusted host policy.
    #[serde(rename = "power.request")]
    PowerRequest(PowerRequestParams),
}

/// Strict top-level request.
#[derive(Debug, JsonSchema, Serialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export, export_to = "v1/protocol-request.ts")]
pub struct RequestEnvelope {
    #[schemars(extend("const" = 1))]
    #[ts(type = "1")]
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

#[derive(Debug, Deserialize, JsonSchema, Serialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(
    export,
    export_to = "v1/protocol-request.ts",
    type = "Record<string, never>"
)]
pub struct EmptyParams {}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct RawAuthBeginParams {
    username: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawLockerAuthBeginParams {}

#[derive(Deserialize)]
#[serde(untagged)]
enum RawRoleAuthBeginParams {
    Greeter(RawAuthBeginParams),
    Locker(RawLockerAuthBeginParams),
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

#[derive(Debug, Deserialize, JsonSchema, Serialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[ts(export, export_to = "v1/protocol-request.ts")]
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
            let raw: RawRoleAuthBeginParams = parse_params(params, id)?;
            let params = match raw {
                RawRoleAuthBeginParams::Greeter(raw) => AuthBeginParams::Greeter(
                    GreeterAuthBeginParams::new(raw.username).map_err(invalid)?,
                ),
                RawRoleAuthBeginParams::Locker(_) => {
                    AuthBeginParams::Locker(LockerAuthBeginParams {})
                }
            };
            Ok(FrontendRequest::AuthBegin(params))
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

/// Decodes a request and rejects parameter or method capabilities belonging to another role.
pub fn decode_request_for_mode(
    input: &[u8],
    mode: RuntimeMode,
) -> Result<RequestEnvelope, ProtocolDecodeError> {
    let request = decode_request(input)?;
    let id = request.id();
    let allowed = match (mode, request.request()) {
        (RuntimeMode::Greeter, FrontendRequest::AuthBegin(params)) => params.username().is_some(),
        (RuntimeMode::Locker, FrontendRequest::AuthBegin(params)) => params.is_locker(),
        (RuntimeMode::Locker, FrontendRequest::SessionSelect(_)) => false,
        _ => true,
    };
    if allowed {
        return Ok(request);
    }

    let body = if matches!(request.request(), FrontendRequest::SessionSelect(_)) {
        ProtocolErrorBody::new(
            ProtocolErrorCode::MethodDisabled,
            "the method is disabled for this host mode",
            false,
        )
    } else {
        ProtocolErrorBody::invalid_params()
    };
    Err(ProtocolDecodeError::new(Some(id), body))
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
    use super::{FrontendRequest, decode_request, decode_request_for_mode};
    use crate::protocol::{
        MAX_AUTH_RESPONSE_BYTES, MAX_MESSAGE_BYTES, MAX_SAFE_INTEGER, PowerAction,
        ProtocolErrorCode, RuntimeMode,
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
            FrontendRequest::AuthBegin(params) if params.username() == Some("alice")
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
    fn host_mode_rejects_cross_role_auth_and_locker_sessions() {
        let greeter_begin =
            br#"{"protocol":1,"id":20,"method":"auth.begin","params":{"username":"alice"}}"#;
        let locker_begin = br#"{"protocol":1,"id":21,"method":"auth.begin","params":{}}"#;
        let select = br#"{"protocol":1,"id":22,"method":"session.select","params":{"sessionId":"wayland:sway"}}"#;

        decode_request_for_mode(greeter_begin, RuntimeMode::Greeter)
            .expect("greeter accepts username authentication");
        decode_request_for_mode(locker_begin, RuntimeMode::Locker)
            .expect("locker accepts parameterless reauthentication");

        let error = decode_request_for_mode(locker_begin, RuntimeMode::Greeter)
            .expect_err("greeter rejects locker authentication parameters");
        assert_eq!(error.request_id().map(|id| id.get()), Some(21));
        assert_eq!(error.body().code(), ProtocolErrorCode::InvalidParams);

        let error = decode_request_for_mode(greeter_begin, RuntimeMode::Locker)
            .expect_err("locker rejects a frontend-selected username");
        assert_eq!(error.request_id().map(|id| id.get()), Some(20));
        assert_eq!(error.body().code(), ProtocolErrorCode::InvalidParams);

        decode_request_for_mode(select, RuntimeMode::Greeter)
            .expect("greeter accepts session selection");
        let error = decode_request_for_mode(select, RuntimeMode::Locker)
            .expect_err("locker rejects session selection");
        assert_eq!(error.request_id().map(|id| id.get()), Some(22));
        assert_eq!(error.body().code(), ProtocolErrorCode::MethodDisabled);
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
