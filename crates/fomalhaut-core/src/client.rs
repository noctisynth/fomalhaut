//! Authentication state machine and trusted session command.

use std::{collections::VecDeque, path::Path};

use greetd_ipc::{AuthMessageType, ErrorType, Request, Response};
use zeroize::Zeroize;

use crate::{
    CoreError, GreeterEvent, GreeterState, MessageLevel, PromptId, PromptKind, Secret,
    ServerErrorKind, Transport, UnixTransport,
};

#[derive(Clone, Copy)]
enum Operation {
    Authenticate,
    StartSession,
    Cancel,
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

/// Command and environment selected by a trusted host.
pub struct SessionCommand {
    command: Vec<String>,
    environment: Vec<String>,
}

impl SessionCommand {
    /// Constructs a session command, rejecting an empty argument vector.
    pub fn new(command: Vec<String>, environment: Vec<String>) -> Result<Self, CoreError> {
        if command.is_empty() {
            return Err(CoreError::EmptySessionCommand);
        }

        Ok(Self {
            command,
            environment,
        })
    }

    fn into_parts(self) -> (Vec<String>, Vec<String>) {
        (self.command, self.environment)
    }
}

/// Drives one sequential greetd authentication and session lifecycle.
pub struct GreeterClient<T> {
    transport: T,
    state: GreeterState,
    events: VecDeque<GreeterEvent>,
    active_prompt: Option<PromptId>,
    next_prompt_id: u64,
}

impl GreeterClient<UnixTransport> {
    /// Connects to greetd and creates an idle client.
    pub async fn connect(path: impl AsRef<Path>) -> Result<Self, CoreError> {
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
            state: GreeterState::Idle,
            events: VecDeque::new(),
            active_prompt: None,
            next_prompt_id: 1,
        }
    }

    /// Returns the current authentication lifecycle state.
    #[must_use]
    pub const fn state(&self) -> GreeterState {
        self.state
    }

    /// Returns whether graceful shutdown must cancel an active greetd session.
    #[must_use]
    pub const fn needs_cancel(&self) -> bool {
        matches!(
            self.state,
            GreeterState::Authenticating
                | GreeterState::WaitingForPrompt
                | GreeterState::Authenticated
        )
    }

    /// Consumes the next event emitted by the state machine.
    pub async fn next_event(&mut self) -> Result<GreeterEvent, CoreError> {
        self.events.pop_front().ok_or(CoreError::NoPendingEvent)
    }
}

impl<T: Transport> GreeterClient<T> {
    /// Begins authentication for a username.
    pub async fn create_session(&mut self, username: String) -> Result<(), CoreError> {
        if !matches!(self.state, GreeterState::Idle | GreeterState::Failed) {
            return Err(self.invalid_state("create session"));
        }

        self.active_prompt = None;
        self.state = GreeterState::Authenticating;
        self.drive(Request::CreateSession { username }, Operation::Authenticate)
            .await
    }

    /// Answers the currently active PAM prompt.
    pub async fn respond(
        &mut self,
        prompt: PromptId,
        mut response: Secret,
    ) -> Result<(), CoreError> {
        if self.state != GreeterState::WaitingForPrompt {
            return Err(self.invalid_state("respond to prompt"));
        }

        if self.active_prompt != Some(prompt) {
            return Err(CoreError::StalePrompt {
                expected: self.active_prompt,
                received: prompt,
            });
        }

        self.active_prompt = None;
        self.state = GreeterState::Authenticating;
        self.drive(
            Request::PostAuthMessageResponse {
                response: Some(response.take()),
            },
            Operation::Authenticate,
        )
        .await
    }

    /// Starts the trusted session selected by the host.
    pub async fn start_session(&mut self, session: SessionCommand) -> Result<(), CoreError> {
        if self.state != GreeterState::Authenticated {
            return Err(self.invalid_state("start session"));
        }

        let (cmd, env) = session.into_parts();
        self.state = GreeterState::StartingSession;
        self.drive(Request::StartSession { cmd, env }, Operation::StartSession)
            .await
    }

    /// Cancels the active greetd session and returns to idle.
    pub async fn cancel(&mut self) -> Result<(), CoreError> {
        if !self.needs_cancel() {
            return Err(self.invalid_state("cancel session"));
        }

        self.active_prompt = None;
        self.state = GreeterState::Cancelling;
        self.drive(Request::CancelSession, Operation::Cancel).await
    }

    async fn drive(&mut self, request: Request, operation: Operation) -> Result<(), CoreError> {
        let mut request = ScrubbedRequest::new(request);

        loop {
            let response = self.transport.exchange(request.as_request()).await;
            let response = match response {
                Ok(response) => response,
                Err(error) => {
                    self.state = GreeterState::Disconnected;
                    self.active_prompt = None;
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
                    return self.handle_server_error(error_type, operation);
                }
                Response::AuthMessage {
                    auth_message_type,
                    mut auth_message,
                } => {
                    if !matches!(operation, Operation::Authenticate) {
                        auth_message.zeroize();
                        self.state = GreeterState::Authenticating;
                        return Err(CoreError::UnexpectedResponse {
                            operation: operation_name(operation),
                            response: "an authentication message",
                        });
                    }

                    match auth_message_type {
                        AuthMessageType::Secret => {
                            return self.emit_prompt(PromptKind::Secret, auth_message);
                        }
                        AuthMessageType::Visible => {
                            return self.emit_prompt(PromptKind::Visible, auth_message);
                        }
                        AuthMessageType::Info => {
                            self.events.push_back(GreeterEvent::Message {
                                level: MessageLevel::Info,
                                text: auth_message,
                            });
                            request.replace(Request::PostAuthMessageResponse { response: None });
                        }
                        AuthMessageType::Error => {
                            self.events.push_back(GreeterEvent::Message {
                                level: MessageLevel::Error,
                                text: auth_message,
                            });
                            request.replace(Request::PostAuthMessageResponse { response: None });
                        }
                    }
                }
            }
        }
    }

    fn emit_prompt(&mut self, kind: PromptKind, message: String) -> Result<(), CoreError> {
        let id = PromptId::new(self.next_prompt_id);
        self.next_prompt_id = match self.next_prompt_id.checked_add(1) {
            Some(next) => next,
            None => {
                self.state = GreeterState::Authenticating;
                self.active_prompt = None;
                return Err(CoreError::PromptIdExhausted);
            }
        };
        self.active_prompt = Some(id);
        self.state = GreeterState::WaitingForPrompt;
        self.events
            .push_back(GreeterEvent::Prompt { id, kind, message });
        Ok(())
    }

    fn handle_success(&mut self, operation: Operation) -> Result<(), CoreError> {
        match operation {
            Operation::Authenticate => {
                self.state = GreeterState::Authenticated;
                self.events.push_back(GreeterEvent::Authenticated);
                Ok(())
            }
            Operation::StartSession => {
                self.state = GreeterState::Started;
                self.events.push_back(GreeterEvent::SessionStarted);
                Ok(())
            }
            Operation::Cancel => {
                self.state = GreeterState::Idle;
                self.events.push_back(GreeterEvent::Cancelled);
                Ok(())
            }
        }
    }

    fn handle_server_error(
        &mut self,
        error_type: ErrorType,
        operation: Operation,
    ) -> Result<(), CoreError> {
        self.state = GreeterState::Failed;
        self.active_prompt = None;

        match (operation, error_type) {
            (Operation::Authenticate, ErrorType::AuthError) => {
                self.events.push_back(GreeterEvent::AuthenticationFailed);
                Ok(())
            }
            (_, ErrorType::AuthError) => Err(CoreError::Server(ServerErrorKind::Authentication)),
            (_, ErrorType::Error) => Err(CoreError::Server(ServerErrorKind::General)),
        }
    }

    fn invalid_state(&self, operation: &'static str) -> CoreError {
        CoreError::InvalidState {
            operation,
            state: self.state,
        }
    }
}

impl<T> Drop for GreeterClient<T> {
    fn drop(&mut self) {
        for event in &mut self.events {
            event.zeroize();
        }
        self.active_prompt = None;
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
    }
}

#[cfg(test)]
#[path = "client_tests.rs"]
mod tests;
