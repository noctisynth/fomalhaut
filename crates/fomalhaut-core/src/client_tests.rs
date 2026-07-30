use std::{
    collections::VecDeque,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};

use greetd_ipc::{AuthMessageType, ErrorType, Request, Response, codec::TokioCodec};
use tokio::net::UnixListener;

use super::{GreeterClient, SessionCommand};
use crate::{
    CoreError, GreeterEvent, GreeterState, MessageLevel, PromptId, PromptKind, Secret,
    ServerErrorKind, Transport, TransportError,
};

enum ExpectedRequest {
    Create(&'static str),
    Respond(Option<&'static str>),
    Start {
        command: &'static [&'static str],
        environment: &'static [&'static str],
    },
    Cancel,
}

impl ExpectedRequest {
    fn matches(&self, request: &Request) -> bool {
        match (self, request) {
            (Self::Create(expected), Request::CreateSession { username }) => username == expected,
            (Self::Respond(expected), Request::PostAuthMessageResponse { response }) => {
                response.as_deref() == *expected
            }
            (
                Self::Start {
                    command,
                    environment,
                },
                Request::StartSession { cmd, env },
            ) => {
                cmd.iter().map(String::as_str).eq(command.iter().copied())
                    && env
                        .iter()
                        .map(String::as_str)
                        .eq(environment.iter().copied())
            }
            (Self::Cancel, Request::CancelSession) => true,
            _ => false,
        }
    }
}

struct Step {
    expected: ExpectedRequest,
    response: Result<Response, TransportError>,
}

struct ScriptedTransport {
    steps: VecDeque<Step>,
}

impl ScriptedTransport {
    fn new(steps: impl IntoIterator<Item = Step>) -> Self {
        Self {
            steps: steps.into_iter().collect(),
        }
    }

    fn remaining(&self) -> usize {
        self.steps.len()
    }
}

impl Transport for ScriptedTransport {
    async fn exchange(&mut self, request: &Request) -> Result<Response, TransportError> {
        let step = self.steps.pop_front().ok_or(TransportError::Unavailable(
            "script has no response for request",
        ))?;

        if !step.expected.matches(request) {
            return Err(TransportError::Unavailable(
                "request did not match scripted expectation",
            ));
        }

        step.response
    }
}

struct DropAwareTransport {
    dropped: Arc<AtomicBool>,
}

impl Drop for DropAwareTransport {
    fn drop(&mut self) {
        self.dropped.store(true, Ordering::Relaxed);
    }
}

impl Transport for DropAwareTransport {
    async fn exchange(&mut self, _request: &Request) -> Result<Response, TransportError> {
        Err(TransportError::Unavailable(
            "drop-aware transport does not exchange messages",
        ))
    }
}

fn step(expected: ExpectedRequest, response: Response) -> Step {
    Step {
        expected,
        response: Ok(response),
    }
}

fn prompt(message_type: AuthMessageType, message: &str) -> Response {
    Response::AuthMessage {
        auth_message_type: message_type,
        auth_message: message.to_owned(),
    }
}

fn server_error(error_type: ErrorType, description: &str) -> Response {
    Response::Error {
        error_type,
        description: description.to_owned(),
    }
}

async fn take_prompt<T>(client: &mut GreeterClient<T>, expected_kind: PromptKind) -> PromptId {
    match client
        .next_event()
        .await
        .expect("the scripted flow emits a prompt")
    {
        GreeterEvent::Prompt { id, kind, .. } => {
            assert_eq!(kind, expected_kind);
            id
        }
        event => panic!("expected prompt, got {event:?}"),
    }
}

#[tokio::test]
async fn password_authentication_starts_a_session() {
    let transport = ScriptedTransport::new([
        step(
            ExpectedRequest::Create("alice"),
            prompt(AuthMessageType::Secret, "Password:"),
        ),
        step(ExpectedRequest::Respond(Some("hunter2")), Response::Success),
        step(
            ExpectedRequest::Start {
                command: &["sway", "--unsupported-gpu"],
                environment: &["XDG_SESSION_TYPE=wayland"],
            },
            Response::Success,
        ),
    ]);
    let mut client = GreeterClient::with_transport(transport);

    client
        .create_session("alice".to_owned())
        .await
        .expect("the scripted create request succeeds");
    let prompt_id = take_prompt(&mut client, PromptKind::Secret).await;
    assert_eq!(client.state(), GreeterState::WaitingForPrompt);
    assert!(client.needs_cancel());

    client
        .respond(prompt_id, Secret::new("hunter2"))
        .await
        .expect("the scripted password is accepted");
    assert_eq!(
        client
            .next_event()
            .await
            .expect("authentication emits an event"),
        GreeterEvent::Authenticated
    );
    assert_eq!(client.state(), GreeterState::Authenticated);

    let session = SessionCommand::new(
        vec!["sway".to_owned(), "--unsupported-gpu".to_owned()],
        vec!["XDG_SESSION_TYPE=wayland".to_owned()],
    )
    .expect("the command contains an executable");
    client
        .start_session(session)
        .await
        .expect("the scripted session starts");

    assert_eq!(
        client
            .next_event()
            .await
            .expect("session start emits an event"),
        GreeterEvent::SessionStarted
    );
    assert_eq!(client.state(), GreeterState::Started);
    assert!(!client.needs_cancel());
    assert_eq!(client.transport.remaining(), 0);
}

#[tokio::test]
async fn info_and_error_messages_are_acknowledged_before_visible_prompt() {
    let transport = ScriptedTransport::new([
        step(
            ExpectedRequest::Create("alice"),
            prompt(AuthMessageType::Info, "Insert security key"),
        ),
        step(
            ExpectedRequest::Respond(None),
            prompt(AuthMessageType::Error, "Touch timed out"),
        ),
        step(
            ExpectedRequest::Respond(None),
            prompt(AuthMessageType::Visible, "Recovery code:"),
        ),
        step(
            ExpectedRequest::Respond(Some("recovery-code")),
            Response::Success,
        ),
    ]);
    let mut client = GreeterClient::with_transport(transport);

    client
        .create_session("alice".to_owned())
        .await
        .expect("automatic acknowledgements succeed");

    assert_eq!(
        client.next_event().await.expect("info message is queued"),
        GreeterEvent::Message {
            level: MessageLevel::Info,
            text: "Insert security key".to_owned(),
        }
    );
    assert_eq!(
        client.next_event().await.expect("error message is queued"),
        GreeterEvent::Message {
            level: MessageLevel::Error,
            text: "Touch timed out".to_owned(),
        }
    );
    let prompt_id = take_prompt(&mut client, PromptKind::Visible).await;

    client
        .respond(prompt_id, Secret::new("recovery-code"))
        .await
        .expect("visible answer is accepted");
    assert_eq!(
        client
            .next_event()
            .await
            .expect("authentication emits an event"),
        GreeterEvent::Authenticated
    );
    assert_eq!(client.transport.remaining(), 0);
}

#[tokio::test]
async fn multifactor_flow_rejects_stale_prompt_ids() {
    let transport = ScriptedTransport::new([
        step(
            ExpectedRequest::Create("alice"),
            prompt(AuthMessageType::Secret, "Password:"),
        ),
        step(
            ExpectedRequest::Respond(Some("password")),
            prompt(AuthMessageType::Secret, "TOTP:"),
        ),
        step(ExpectedRequest::Respond(Some("123456")), Response::Success),
    ]);
    let mut client = GreeterClient::with_transport(transport);

    client
        .create_session("alice".to_owned())
        .await
        .expect("first prompt is returned");
    let password_prompt = take_prompt(&mut client, PromptKind::Secret).await;
    client
        .respond(password_prompt, Secret::new("password"))
        .await
        .expect("password advances to MFA");
    let totp_prompt = take_prompt(&mut client, PromptKind::Secret).await;
    assert_ne!(password_prompt, totp_prompt);

    let error = client
        .respond(password_prompt, Secret::new("stale"))
        .await
        .expect_err("the old prompt id must be rejected");
    assert!(matches!(
        error,
        CoreError::StalePrompt {
            expected: Some(expected),
            received,
        } if expected == totp_prompt && received == password_prompt
    ));
    assert_eq!(client.transport.remaining(), 1);

    client
        .respond(totp_prompt, Secret::new("123456"))
        .await
        .expect("current MFA prompt is accepted");
    assert_eq!(
        client
            .next_event()
            .await
            .expect("authentication emits an event"),
        GreeterEvent::Authenticated
    );
}

#[tokio::test]
async fn authentication_failure_can_be_retried() {
    let transport = ScriptedTransport::new([
        step(
            ExpectedRequest::Create("alice"),
            server_error(ErrorType::AuthError, "entered password was secret"),
        ),
        step(ExpectedRequest::Create("alice"), Response::Success),
    ]);
    let mut client = GreeterClient::with_transport(transport);

    client
        .create_session("alice".to_owned())
        .await
        .expect("authentication failure is emitted as an event");
    assert_eq!(
        client.next_event().await.expect("failure emits an event"),
        GreeterEvent::AuthenticationFailed
    );
    assert_eq!(client.state(), GreeterState::Failed);

    client
        .create_session("alice".to_owned())
        .await
        .expect("greetd automatically cancelled the failed attempt");
    assert_eq!(
        client
            .next_event()
            .await
            .expect("retry emits authentication success"),
        GreeterEvent::Authenticated
    );
    assert_eq!(client.state(), GreeterState::Authenticated);
}

#[tokio::test]
async fn passwordless_authentication_succeeds_immediately() {
    let transport =
        ScriptedTransport::new([step(ExpectedRequest::Create("alice"), Response::Success)]);
    let mut client = GreeterClient::with_transport(transport);

    client
        .create_session("alice".to_owned())
        .await
        .expect("passwordless authentication succeeds");

    assert_eq!(
        client.next_event().await.expect("success emits an event"),
        GreeterEvent::Authenticated
    );
}

#[tokio::test]
async fn active_authentication_can_be_cancelled() {
    let transport = ScriptedTransport::new([
        step(
            ExpectedRequest::Create("alice"),
            prompt(AuthMessageType::Secret, "Password:"),
        ),
        step(ExpectedRequest::Cancel, Response::Success),
    ]);
    let mut client = GreeterClient::with_transport(transport);

    client
        .create_session("alice".to_owned())
        .await
        .expect("prompt is returned");
    let _ = take_prompt(&mut client, PromptKind::Secret).await;
    client
        .cancel()
        .await
        .expect("the active session is cancelled");

    assert_eq!(
        client.next_event().await.expect("cancel emits an event"),
        GreeterEvent::Cancelled
    );
    assert_eq!(client.state(), GreeterState::Idle);
    assert!(!client.needs_cancel());
}

#[tokio::test]
async fn authenticated_session_can_be_cancelled_before_start() {
    let transport = ScriptedTransport::new([
        step(ExpectedRequest::Create("alice"), Response::Success),
        step(ExpectedRequest::Cancel, Response::Success),
    ]);
    let mut client = GreeterClient::with_transport(transport);

    client
        .create_session("alice".to_owned())
        .await
        .expect("authentication succeeds");
    let _ = client
        .next_event()
        .await
        .expect("authentication event is queued");
    client
        .cancel()
        .await
        .expect("authenticated session is still cancellable");

    assert_eq!(
        client.next_event().await.expect("cancel emits an event"),
        GreeterEvent::Cancelled
    );
    assert_eq!(client.state(), GreeterState::Idle);
}

#[tokio::test]
async fn transport_failure_disconnects_without_replaying() {
    let transport = ScriptedTransport::new([Step {
        expected: ExpectedRequest::Create("alice"),
        response: Err(TransportError::Unavailable("greetd closed the socket")),
    }]);
    let mut client = GreeterClient::with_transport(transport);

    let error = client
        .create_session("alice".to_owned())
        .await
        .expect_err("transport failure must be returned");

    assert!(matches!(error, CoreError::Transport(_)));
    assert_eq!(client.state(), GreeterState::Disconnected);
    assert!(!client.needs_cancel());
}

#[tokio::test]
async fn generic_server_error_is_sanitized() {
    let transport = ScriptedTransport::new([step(
        ExpectedRequest::Create("alice"),
        server_error(ErrorType::Error, "password=must-not-leak"),
    )]);
    let mut client = GreeterClient::with_transport(transport);

    let error = client
        .create_session("alice".to_owned())
        .await
        .expect_err("generic greetd error must be returned");
    let display = error.to_string();
    let debug = format!("{error:?}");

    assert!(matches!(error, CoreError::Server(ServerErrorKind::General)));
    assert!(!display.contains("must-not-leak"));
    assert!(!debug.contains("must-not-leak"));
    assert_eq!(client.state(), GreeterState::Failed);
}

#[tokio::test]
async fn session_start_failure_is_sanitized() {
    let transport = ScriptedTransport::new([
        step(ExpectedRequest::Create("alice"), Response::Success),
        step(
            ExpectedRequest::Start {
                command: &["sway"],
                environment: &[],
            },
            server_error(ErrorType::Error, "private environment value"),
        ),
    ]);
    let mut client = GreeterClient::with_transport(transport);
    client
        .create_session("alice".to_owned())
        .await
        .expect("authentication succeeds");
    let _ = client
        .next_event()
        .await
        .expect("authentication event is queued");
    let session =
        SessionCommand::new(vec!["sway".to_owned()], Vec::new()).expect("command is non-empty");

    let error = client
        .start_session(session)
        .await
        .expect_err("greetd rejects session start");

    assert!(matches!(
        &error,
        CoreError::Server(ServerErrorKind::General)
    ));
    assert!(!format!("{error:?}").contains("private environment value"));
    assert_eq!(client.state(), GreeterState::Failed);
}

#[tokio::test]
async fn authentication_message_during_start_is_rejected() {
    let transport = ScriptedTransport::new([
        step(ExpectedRequest::Create("alice"), Response::Success),
        step(
            ExpectedRequest::Start {
                command: &["sway"],
                environment: &[],
            },
            prompt(AuthMessageType::Secret, "unexpected"),
        ),
        step(ExpectedRequest::Cancel, Response::Success),
    ]);
    let mut client = GreeterClient::with_transport(transport);
    client
        .create_session("alice".to_owned())
        .await
        .expect("authentication succeeds");
    let _ = client
        .next_event()
        .await
        .expect("authentication event is queued");
    let session =
        SessionCommand::new(vec!["sway".to_owned()], Vec::new()).expect("command is non-empty");

    let error = client
        .start_session(session)
        .await
        .expect_err("auth prompt during start violates the protocol");

    assert!(matches!(error, CoreError::UnexpectedResponse { .. }));
    assert_eq!(client.state(), GreeterState::Authenticating);
    assert!(client.needs_cancel());
    client
        .cancel()
        .await
        .expect("the active greetd session can be cancelled");
    assert_eq!(
        client.next_event().await.expect("cancel emits an event"),
        GreeterEvent::Cancelled
    );
}

#[tokio::test]
async fn invalid_operations_do_not_touch_the_transport() {
    let transport = ScriptedTransport::new([]);
    let mut client = GreeterClient::with_transport(transport);

    let respond_error = client
        .respond(PromptId::new(1), Secret::new("ignored"))
        .await
        .expect_err("idle client has no prompt");
    let cancel_error = client
        .cancel()
        .await
        .expect_err("idle client has no session");
    let session =
        SessionCommand::new(vec!["sway".to_owned()], Vec::new()).expect("command is non-empty");
    let start_error = client
        .start_session(session)
        .await
        .expect_err("idle client is not authenticated");

    assert!(matches!(respond_error, CoreError::InvalidState { .. }));
    assert!(matches!(cancel_error, CoreError::InvalidState { .. }));
    assert!(matches!(start_error, CoreError::InvalidState { .. }));
    assert!(matches!(
        SessionCommand::new(Vec::new(), Vec::new()),
        Err(CoreError::EmptySessionCommand)
    ));
    assert!(matches!(
        client.next_event().await,
        Err(CoreError::NoPendingEvent)
    ));
    assert_eq!(client.transport.remaining(), 0);
}

#[test]
fn dropping_client_drops_its_transport_without_async_work() {
    let dropped = Arc::new(AtomicBool::new(false));
    {
        let transport = DropAwareTransport {
            dropped: Arc::clone(&dropped),
        };
        let _client = GreeterClient::with_transport(transport);
        assert!(!dropped.load(Ordering::Relaxed));
    }
    assert!(dropped.load(Ordering::Relaxed));
}

#[tokio::test]
async fn prompt_identifier_exhaustion_fails_closed() {
    let transport = ScriptedTransport::new([
        step(
            ExpectedRequest::Create("alice"),
            prompt(AuthMessageType::Secret, "Password:"),
        ),
        step(ExpectedRequest::Cancel, Response::Success),
    ]);
    let mut client = GreeterClient::with_transport(transport);
    client.next_prompt_id = u64::MAX;

    let error = client
        .create_session("alice".to_owned())
        .await
        .expect_err("prompt id exhaustion must stop authentication");

    assert!(matches!(error, CoreError::PromptIdExhausted));
    assert_eq!(client.state(), GreeterState::Authenticating);
    assert!(client.needs_cancel());
    client
        .cancel()
        .await
        .expect("internal failure still permits explicit cancellation");
}

static SOCKET_COUNTER: AtomicU64 = AtomicU64::new(0);

fn socket_path() -> PathBuf {
    let sequence = SOCKET_COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "fomalhaut-core-{}-{sequence}.sock",
        std::process::id()
    ))
}

#[tokio::test]
async fn unix_transport_uses_the_greetd_codec() {
    let path = socket_path();
    let listener = UnixListener::bind(&path).expect("unique test socket path can be bound");
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.expect("test client connects");
        let request = Request::read_from(&mut stream)
            .await
            .expect("test server reads greetd request");
        assert!(matches!(
            request,
            Request::CreateSession { username } if username == "alice"
        ));
        Response::Success
            .write_to(&mut stream)
            .await
            .expect("test server writes greetd response");
    });

    let mut client = GreeterClient::connect(&path)
        .await
        .expect("client connects to test socket");
    client
        .create_session("alice".to_owned())
        .await
        .expect("codec round trip succeeds");
    assert_eq!(
        client.next_event().await.expect("success emits an event"),
        GreeterEvent::Authenticated
    );

    server.await.expect("test server task completes");
    std::fs::remove_file(path).expect("test socket is removable");
}
