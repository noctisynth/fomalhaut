//! Greetd login state machine.

use std::path::Path;

use fomalhaut_core::{
    AuthConversation, AuthEvent, AuthState, AuthenticatedIdentity, BackendError,
    ConversationBackend, CoreError, LoginBackend, MessageLevel, PromptId, PromptKind, Secret,
    SessionCommand,
};
use greetd_ipc::{AuthMessageType, ErrorType, Request, Response};
use zeroize::Zeroize;

use crate::{GreetdError, GreeterState, ServerErrorKind, Transport, UnixTransport};

#[derive(Clone, Copy)]
enum Operation {
    Authenticate,
    StartSession,
    Cancel,
    CleanupAfterAuthenticationFailure,
}

struct ScrubbedRequest(Request);

impl ScrubbedRequest {
    fn new(request: Request) -> Self {
        Self(request)
    }

    fn as_request(&self) -> &Request {
        &self.0
    }

    fn replace(&mut self, request: Request) {
        scrub_request(&mut self.0);
        self.0 = request;
    }
}

impl Drop for ScrubbedRequest {
    fn drop(&mut self) {
        scrub_request(&mut self.0);
    }
}

/// Drives one sequential greetd authentication and session lifecycle.
pub struct GreeterClient<T> {
    transport: T,
    conversation: AuthConversation,
    pending_identity: Option<AuthenticatedIdentity>,
    starting_session: bool,
    session_started: bool,
}

impl GreeterClient<UnixTransport> {
    /// Connects to greetd and creates an idle client.
    pub async fn connect(path: impl AsRef<Path>) -> Result<Self, GreetdError> {
        let transport = UnixTransport::connect(path).await?;
        Ok(Self::with_transport(transport))
    }
}

impl<T> GreeterClient<T> {
    /// Creates an idle client over a supplied transport.
    #[must_use]
    pub fn with_transport(transport: T) -> Self {
        Self {
            transport,
            conversation: AuthConversation::new(),
            pending_identity: None,
            starting_session: false,
            session_started: false,
        }
    }

    /// Returns the complete greetd login lifecycle state.
    #[must_use]
    pub fn state(&self) -> GreeterState {
        if self.session_started {
            return GreeterState::Started;
        }
        if self.starting_session {
            return GreeterState::StartingSession;
        }

        match self.conversation.state() {
            AuthState::Disconnected => GreeterState::Disconnected,
            AuthState::Idle => GreeterState::Idle,
            AuthState::Authenticating => GreeterState::Authenticating,
            AuthState::WaitingForSecret | AuthState::WaitingForVisible => {
                GreeterState::WaitingForPrompt
            }
            AuthState::Authenticated => GreeterState::Authenticated,
            AuthState::Cancelling => GreeterState::Cancelling,
            AuthState::Failed => GreeterState::Failed,
        }
    }

    /// Returns whether graceful shutdown must cancel an active greetd session.
    #[must_use]
    pub fn needs_cancel(&self) -> bool {
        !self.starting_session && !self.session_started && self.conversation.needs_cancel()
    }

    /// Consumes the next event emitted by the authentication state machine.
    pub async fn next_event(&mut self) -> Result<AuthEvent, GreetdError> {
        self.conversation.next_event().map_err(GreetdError::from)
    }
}

impl<T: Transport> GreeterClient<T> {
    /// Begins authentication for a host-validated username.
    pub async fn create_session(&mut self, username: String) -> Result<(), GreetdError> {
        let identity = AuthenticatedIdentity::new(username.clone())?;
        self.conversation.begin()?;
        self.pending_identity = Some(identity);
        self.starting_session = false;
        self.session_started = false;
        self.drive(Request::CreateSession { username }, Operation::Authenticate)
            .await
    }

    /// Answers the currently active authentication prompt.
    pub async fn respond(&mut self, prompt: PromptId, response: Secret) -> Result<(), GreetdError> {
        self.conversation.begin_response(prompt)?;
        self.drive(
            Request::PostAuthMessageResponse {
                response: Some(response.into_inner()),
            },
            Operation::Authenticate,
        )
        .await
    }

    /// Starts the trusted session selected by the host.
    pub async fn start_session(&mut self, session: SessionCommand) -> Result<(), GreetdError> {
        if self.conversation.state() != AuthState::Authenticated
            || self.starting_session
            || self.session_started
        {
            return Err(CoreError::InvalidState {
                operation: "start session",
                state: self.conversation.state(),
            }
            .into());
        }

        let (cmd, env) = session.into_parts();
        self.starting_session = true;
        self.drive(Request::StartSession { cmd, env }, Operation::StartSession)
            .await
    }

    /// Cancels the active greetd session and returns to idle.
    pub async fn cancel(&mut self) -> Result<(), GreetdError> {
        if self.starting_session || self.session_started {
            return Err(CoreError::InvalidState {
                operation: "cancel session",
                state: self.conversation.state(),
            }
            .into());
        }

        self.conversation.begin_cancel()?;
        self.pending_identity = None;
        self.drive(Request::CancelSession, Operation::Cancel).await
    }

    async fn drive(
        &mut self,
        request: Request,
        mut operation: Operation,
    ) -> Result<(), GreetdError> {
        let mut request = ScrubbedRequest::new(request);

        loop {
            let response = self.transport.exchange(request.as_request()).await;
            let response = match response {
                Ok(response) => response,
                Err(error) => {
                    self.conversation.disconnect();
                    self.pending_identity = None;
                    self.starting_session = false;
                    return Err(error.into());
                }
            };

            match response {
                Response::Success => return self.handle_success(operation),
                Response::Error {
                    error_type,
                    mut description,
                } => {
                    description.zeroize();
                    if matches!(operation, Operation::CleanupAfterAuthenticationFailure) {
                        return self.finish_authentication_failure_cleanup();
                    }
                    if matches!(operation, Operation::Authenticate)
                        && matches!(&error_type, ErrorType::AuthError)
                    {
                        self.conversation.begin_cancel()?;
                        self.pending_identity = None;
                        self.starting_session = false;
                        request.replace(Request::CancelSession);
                        operation = Operation::CleanupAfterAuthenticationFailure;
                        continue;
                    }
                    return self.handle_server_error(error_type);
                }
                Response::AuthMessage {
                    auth_message_type,
                    mut auth_message,
                } => {
                    if !matches!(operation, Operation::Authenticate) {
                        auth_message.zeroize();
                        self.conversation.fail();
                        self.starting_session = false;
                        return Err(GreetdError::UnexpectedResponse {
                            operation: operation_name(operation),
                            response: "an authentication message",
                        });
                    }

                    match auth_message_type {
                        AuthMessageType::Secret => {
                            return self
                                .conversation
                                .emit_prompt(PromptKind::Secret, auth_message)
                                .map_err(GreetdError::from);
                        }
                        AuthMessageType::Visible => {
                            return self
                                .conversation
                                .emit_prompt(PromptKind::Visible, auth_message)
                                .map_err(GreetdError::from);
                        }
                        AuthMessageType::Info => {
                            self.conversation
                                .emit_message(MessageLevel::Info, auth_message)?;
                            request.replace(Request::PostAuthMessageResponse { response: None });
                        }
                        AuthMessageType::Error => {
                            self.conversation
                                .emit_message(MessageLevel::Error, auth_message)?;
                            request.replace(Request::PostAuthMessageResponse { response: None });
                        }
                    }
                }
            }
        }
    }

    fn handle_success(&mut self, operation: Operation) -> Result<(), GreetdError> {
        match operation {
            Operation::Authenticate => {
                let Some(identity) = self.pending_identity.take() else {
                    self.conversation.fail();
                    return Err(GreetdError::MissingIdentity);
                };
                self.conversation.authenticated(identity)?;
                Ok(())
            }
            Operation::StartSession => {
                self.starting_session = false;
                self.session_started = true;
                Ok(())
            }
            Operation::Cancel => {
                self.conversation.cancelled()?;
                Ok(())
            }
            Operation::CleanupAfterAuthenticationFailure => {
                self.finish_authentication_failure_cleanup()
            }
        }
    }

    fn finish_authentication_failure_cleanup(&mut self) -> Result<(), GreetdError> {
        self.conversation.authentication_failed()?;
        Ok(())
    }

    fn handle_server_error(&mut self, error_type: ErrorType) -> Result<(), GreetdError> {
        self.conversation.fail();
        self.pending_identity = None;
        self.starting_session = false;

        match error_type {
            ErrorType::AuthError => Err(GreetdError::Server(ServerErrorKind::Authentication)),
            ErrorType::Error => Err(GreetdError::Server(ServerErrorKind::General)),
        }
    }
}

impl<T: Transport> ConversationBackend for GreeterClient<T> {
    fn state(&self) -> AuthState {
        self.conversation.state()
    }

    fn needs_cancel(&self) -> bool {
        GreeterClient::needs_cancel(self)
    }

    async fn respond(&mut self, prompt: PromptId, response: Secret) -> Result<(), BackendError> {
        GreeterClient::respond(self, prompt, response)
            .await
            .map_err(GreetdError::into_backend_error)
    }

    async fn cancel(&mut self) -> Result<(), BackendError> {
        GreeterClient::cancel(self)
            .await
            .map_err(GreetdError::into_backend_error)
    }

    async fn next_event(&mut self) -> Result<AuthEvent, BackendError> {
        GreeterClient::next_event(self)
            .await
            .map_err(GreetdError::into_backend_error)
    }
}

impl<T: Transport> LoginBackend for GreeterClient<T> {
    async fn begin_login(&mut self, username: String) -> Result<(), BackendError> {
        self.create_session(username)
            .await
            .map_err(GreetdError::into_backend_error)
    }

    async fn start_session(&mut self, command: SessionCommand) -> Result<(), BackendError> {
        GreeterClient::start_session(self, command)
            .await
            .map_err(GreetdError::into_backend_error)
    }

    fn session_started(&self) -> bool {
        self.session_started
    }
}

fn scrub_request(request: &mut Request) {
    match request {
        Request::CreateSession { username } => username.zeroize(),
        Request::PostAuthMessageResponse { response } => response.zeroize(),
        Request::StartSession { cmd, env } => {
            cmd.zeroize();
            env.zeroize();
        }
        Request::CancelSession => {}
    }
}

const fn operation_name(operation: Operation) -> &'static str {
    match operation {
        Operation::Authenticate => "authenticate",
        Operation::StartSession => "start session",
        Operation::Cancel => "cancel session",
        Operation::CleanupAfterAuthenticationFailure => "clean up after authentication failure",
    }
}

#[cfg(test)]
#[path = "client_tests.rs"]
mod tests;
