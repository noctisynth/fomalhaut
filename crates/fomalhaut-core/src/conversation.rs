//! Reusable backend-neutral authentication conversation state machine.

use std::collections::VecDeque;

use crate::{
    AuthEvent, AuthState, AuthenticatedIdentity, CoreError, MessageLevel, PromptId, PromptKind,
};

/// Tracks one serial authentication conversation and its pending events.
pub struct AuthConversation {
    state: AuthState,
    events: VecDeque<AuthEvent>,
    active_prompt: Option<PromptId>,
    next_prompt_id: u64,
}

impl Default for AuthConversation {
    fn default() -> Self {
        Self::new()
    }
}

impl AuthConversation {
    /// Creates an idle conversation.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            state: AuthState::Idle,
            events: VecDeque::new(),
            active_prompt: None,
            next_prompt_id: 1,
        }
    }

    /// Returns the current authentication state.
    #[must_use]
    pub const fn state(&self) -> AuthState {
        self.state
    }

    /// Returns whether graceful shutdown must cancel the current transaction.
    #[must_use]
    pub const fn needs_cancel(&self) -> bool {
        matches!(
            self.state,
            AuthState::Authenticating
                | AuthState::WaitingForSecret
                | AuthState::WaitingForVisible
                | AuthState::Authenticated
        )
    }

    /// Starts a fresh authentication transaction.
    pub fn begin(&mut self) -> Result<(), CoreError> {
        if !matches!(self.state, AuthState::Idle | AuthState::Failed) {
            return Err(self.invalid_state("begin authentication"));
        }

        self.clear_events();
        self.active_prompt = None;
        self.state = AuthState::Authenticating;
        Ok(())
    }

    /// Validates and consumes the currently active prompt identifier.
    pub fn begin_response(&mut self, prompt: PromptId) -> Result<(), CoreError> {
        if !matches!(
            self.state,
            AuthState::WaitingForSecret | AuthState::WaitingForVisible
        ) {
            return Err(self.invalid_state("respond to prompt"));
        }

        if self.active_prompt != Some(prompt) {
            return Err(CoreError::StalePrompt {
                expected: self.active_prompt,
                received: prompt,
            });
        }

        self.active_prompt = None;
        self.state = AuthState::Authenticating;
        Ok(())
    }

    /// Emits a prompt and waits for its matching response.
    pub fn emit_prompt(&mut self, kind: PromptKind, message: String) -> Result<(), CoreError> {
        if self.state != AuthState::Authenticating {
            return Err(self.invalid_state("emit an authentication prompt"));
        }

        let id = PromptId::new(self.next_prompt_id);
        self.next_prompt_id = self
            .next_prompt_id
            .checked_add(1)
            .ok_or(CoreError::PromptIdExhausted)?;
        self.active_prompt = Some(id);
        self.state = match kind {
            PromptKind::Secret => AuthState::WaitingForSecret,
            PromptKind::Visible => AuthState::WaitingForVisible,
        };
        self.events
            .push_back(AuthEvent::Prompt { id, kind, message });
        Ok(())
    }

    /// Emits a non-interactive authentication message.
    pub fn emit_message(&mut self, level: MessageLevel, text: String) -> Result<(), CoreError> {
        if self.state != AuthState::Authenticating {
            return Err(self.invalid_state("emit an authentication message"));
        }

        self.events.push_back(AuthEvent::Message { level, text });
        Ok(())
    }

    /// Completes authentication for a backend-verified identity.
    pub fn authenticated(&mut self, identity: AuthenticatedIdentity) -> Result<(), CoreError> {
        if self.state != AuthState::Authenticating {
            return Err(self.invalid_state("complete authentication"));
        }

        self.active_prompt = None;
        self.state = AuthState::Authenticated;
        self.events.push_back(AuthEvent::Authenticated(identity));
        Ok(())
    }

    /// Enters cancellation for an active transaction.
    pub fn begin_cancel(&mut self) -> Result<(), CoreError> {
        if !self.needs_cancel() {
            return Err(self.invalid_state("cancel authentication"));
        }

        self.active_prompt = None;
        self.state = AuthState::Cancelling;
        Ok(())
    }

    /// Completes explicit cancellation and returns to idle.
    pub fn cancelled(&mut self) -> Result<(), CoreError> {
        if self.state != AuthState::Cancelling {
            return Err(self.invalid_state("complete cancellation"));
        }

        self.state = AuthState::Idle;
        self.events.push_back(AuthEvent::Cancelled);
        Ok(())
    }

    /// Records an authentication rejection.
    pub fn authentication_failed(&mut self) -> Result<(), CoreError> {
        if !matches!(
            self.state,
            AuthState::Authenticating
                | AuthState::WaitingForSecret
                | AuthState::WaitingForVisible
                | AuthState::Cancelling
        ) {
            return Err(self.invalid_state("fail authentication"));
        }

        self.active_prompt = None;
        self.state = AuthState::Failed;
        self.events.push_back(AuthEvent::AuthenticationFailed);
        Ok(())
    }

    /// Marks the conversation failed after a backend service error.
    pub fn fail(&mut self) {
        self.active_prompt = None;
        self.state = AuthState::Failed;
    }

    /// Marks the backing transport or worker permanently disconnected.
    pub fn disconnect(&mut self) {
        self.active_prompt = None;
        self.state = AuthState::Disconnected;
    }

    /// Consumes the oldest pending event.
    pub fn next_event(&mut self) -> Result<AuthEvent, CoreError> {
        self.events.pop_front().ok_or(CoreError::NoPendingEvent)
    }

    fn invalid_state(&self, operation: &'static str) -> CoreError {
        CoreError::InvalidState {
            operation,
            state: self.state,
        }
    }

    fn clear_events(&mut self) {
        for mut event in self.events.drain(..) {
            event.zeroize();
        }
    }
}

impl Drop for AuthConversation {
    fn drop(&mut self) {
        self.clear_events();
        self.active_prompt = None;
    }
}

#[cfg(test)]
mod tests {
    use super::AuthConversation;
    use crate::{AuthEvent, AuthState, AuthenticatedIdentity, CoreError, PromptKind};

    #[test]
    fn rejects_duplicate_prompt_answers() {
        let mut conversation = AuthConversation::new();
        conversation.begin().expect("idle conversation can begin");
        conversation
            .emit_prompt(PromptKind::Secret, "Password:".to_owned())
            .expect("authenticating conversation can emit a prompt");
        let prompt = match conversation
            .next_event()
            .expect("the emitted prompt is queued")
        {
            AuthEvent::Prompt { id, .. } => id,
            event => panic!("expected prompt, got {event:?}"),
        };

        conversation
            .begin_response(prompt)
            .expect("the active prompt can be answered once");
        let error = conversation
            .begin_response(prompt)
            .expect_err("the same prompt cannot be answered twice");

        assert!(matches!(error, CoreError::InvalidState { .. }));
        assert_eq!(conversation.state(), AuthState::Authenticating);
    }

    #[test]
    fn authentication_and_cancellation_are_distinct_outcomes() {
        let mut conversation = AuthConversation::new();
        conversation.begin().expect("idle conversation can begin");
        conversation
            .authenticated(
                AuthenticatedIdentity::new("alice").expect("the fixture identity is non-empty"),
            )
            .expect("authentication can complete");
        assert!(matches!(
            conversation
                .next_event()
                .expect("authentication emits an event"),
            AuthEvent::Authenticated(_)
        ));

        conversation
            .begin_cancel()
            .expect("authenticated conversation can be cancelled");
        conversation.cancelled().expect("cancellation can complete");

        assert_eq!(conversation.state(), AuthState::Idle);
        assert_eq!(
            conversation
                .next_event()
                .expect("cancellation emits an event"),
            AuthEvent::Cancelled
        );
    }

    #[test]
    fn prompt_identifier_exhaustion_fails_closed() {
        let mut conversation = AuthConversation::new();
        conversation.next_prompt_id = u64::MAX;
        conversation.begin().expect("idle conversation can begin");

        let error = conversation
            .emit_prompt(PromptKind::Secret, "Password:".to_owned())
            .expect_err("prompt identifier exhaustion is rejected");

        assert!(matches!(error, CoreError::PromptIdExhausted));
        assert_eq!(conversation.state(), AuthState::Authenticating);
        assert!(conversation.needs_cancel());
    }
}
