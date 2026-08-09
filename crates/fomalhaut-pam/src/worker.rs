use std::{
    error::Error,
    ffi::{CStr, CString},
    fmt,
    io::{BufReader, BufWriter, Read, Write},
};

use pam_client::{Context, ConversationHandler, ErrorCode, Flag};

use crate::ipc::{
    IpcError, MAX_TRANSACTION_FRAMES, ParentMessage, WorkerMessage, WorkerMessageLevel,
    WorkerPromptKind, read_parent_message, write_worker_message,
};

const PAM_SERVICE: &str = "fomalhaut-lock";

/// Private command-line marker used to turn the locker executable into a one-shot PAM worker.
pub const PAM_WORKER_ARGUMENT: &str = "--fomalhaut-pam-worker";

/// Sanitized failure returned by the isolated PAM worker entry point.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PamWorkerError {
    /// The parent did not provide the expected bounded startup message.
    StartupProtocol,
    /// PAM could not create or complete the restricted transaction.
    Pam,
    /// The bounded parent/worker channel failed.
    Ipc,
}

impl fmt::Display for PamWorkerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::StartupProtocol => "the PAM worker startup protocol failed",
            Self::Pam => "the PAM transaction failed",
            Self::Ipc => "the PAM worker channel failed",
        })
    }
}

impl Error for PamWorkerError {}

/// Runs one PAM transaction over bounded stdin/stdout IPC.
///
/// The caller must invoke this only for an exact [`PAM_WORKER_ARGUMENT`] process invocation,
/// before GTK or any long-lived application state is initialized.
pub fn run_pam_worker() -> Result<(), PamWorkerError> {
    let reader = BufReader::new(std::io::stdin());
    let writer = BufWriter::new(std::io::stdout());
    run_pam_worker_with(reader, writer)
}

fn run_pam_worker_with<R: Read, W: Write>(mut reader: R, writer: W) -> Result<(), PamWorkerError> {
    let ParentMessage::Prepare(username) =
        read_parent_message(&mut reader).map_err(|_| PamWorkerError::StartupProtocol)?
    else {
        return Err(PamWorkerError::StartupProtocol);
    };
    let conversation = WorkerConversation::new(reader, writer);
    let mut context = Context::new(PAM_SERVICE, Some(&username), conversation)
        .map_err(|_| PamWorkerError::Pam)?;
    context
        .conversation_mut()
        .send(&WorkerMessage::Ready)
        .map_err(|_| PamWorkerError::Ipc)?;
    context
        .conversation_mut()
        .wait_for_begin()
        .map_err(|_| PamWorkerError::StartupProtocol)?;

    if let Err(error) = context.authenticate(Flag::NONE) {
        return finish_pam_error(&mut context, error.code());
    }
    if context.conversation().failed() {
        context
            .conversation_mut()
            .send(&WorkerMessage::Fatal)
            .map_err(|_| PamWorkerError::Ipc)?;
        return Err(PamWorkerError::Ipc);
    }
    if let Err(error) = context.acct_mgmt(Flag::NONE) {
        return finish_pam_error(&mut context, error.code());
    }
    if context.conversation().failed() {
        context
            .conversation_mut()
            .send(&WorkerMessage::Fatal)
            .map_err(|_| PamWorkerError::Ipc)?;
        return Err(PamWorkerError::Ipc);
    }
    context
        .conversation_mut()
        .send(&WorkerMessage::Authenticated)
        .map_err(|_| PamWorkerError::Ipc)
}

fn finish_pam_error<R: Read, W: Write>(
    context: &mut Context<WorkerConversation<R, W>>,
    code: ErrorCode,
) -> Result<(), PamWorkerError> {
    let rejected = matches!(
        code,
        ErrorCode::PERM_DENIED
            | ErrorCode::AUTH_ERR
            | ErrorCode::CRED_INSUFFICIENT
            | ErrorCode::USER_UNKNOWN
            | ErrorCode::MAXTRIES
            | ErrorCode::NEW_AUTHTOK_REQD
            | ErrorCode::ACCT_EXPIRED
            | ErrorCode::AUTHTOK_EXPIRED
    );
    let message = if rejected {
        WorkerMessage::Rejected
    } else {
        WorkerMessage::Fatal
    };
    context
        .conversation_mut()
        .send(&message)
        .map_err(|_| PamWorkerError::Ipc)?;
    if rejected {
        Ok(())
    } else {
        Err(PamWorkerError::Pam)
    }
}

struct WorkerConversation<R, W> {
    reader: R,
    writer: W,
    next_prompt: u64,
    frames: usize,
    failed: bool,
}

impl<R: Read, W: Write> WorkerConversation<R, W> {
    const fn new(reader: R, writer: W) -> Self {
        Self {
            reader,
            writer,
            next_prompt: 1,
            frames: 0,
            failed: false,
        }
    }

    const fn failed(&self) -> bool {
        self.failed
    }

    fn wait_for_begin(&mut self) -> Result<(), IpcError> {
        self.count_frame()?;
        match read_parent_message(&mut self.reader)? {
            ParentMessage::Begin => Ok(()),
            ParentMessage::Prepare(_) | ParentMessage::Answer { .. } => Err(IpcError::InvalidFrame),
        }
    }

    fn send(&mut self, message: &WorkerMessage) -> Result<(), IpcError> {
        self.count_frame()?;
        write_worker_message(&mut self.writer, message)
    }

    fn prompt(&mut self, prompt: &CStr, kind: WorkerPromptKind) -> Result<CString, ErrorCode> {
        if self.failed {
            return Err(ErrorCode::CONV_ERR);
        }
        let prompt = prompt.to_str().map_err(|_| ErrorCode::CONV_ERR)?;
        let id = self.next_prompt;
        self.next_prompt = self.next_prompt.checked_add(1).ok_or(ErrorCode::CONV_ERR)?;
        self.send(&WorkerMessage::Prompt {
            prompt: id,
            kind,
            message: prompt.to_owned(),
        })
        .map_err(|_| {
            self.failed = true;
            ErrorCode::CONV_ERR
        })?;
        self.count_frame().map_err(|_| {
            self.failed = true;
            ErrorCode::CONV_ERR
        })?;
        let ParentMessage::Answer {
            prompt: response_prompt,
            mut response,
        } = read_parent_message(&mut self.reader).map_err(|_| {
            self.failed = true;
            ErrorCode::CONV_ERR
        })?
        else {
            self.failed = true;
            return Err(ErrorCode::CONV_ERR);
        };
        if response_prompt != id {
            response.fill(0);
            self.failed = true;
            return Err(ErrorCode::CONV_ERR);
        }
        CString::new(response).map_err(|error| {
            let mut response = error.into_vec();
            response.fill(0);
            self.failed = true;
            ErrorCode::CONV_ERR
        })
    }

    fn message(&mut self, message: &CStr, level: WorkerMessageLevel) {
        if self.failed {
            return;
        }
        let Ok(text) = message.to_str() else {
            self.failed = true;
            return;
        };
        if self
            .send(&WorkerMessage::Message {
                level,
                text: text.to_owned(),
            })
            .is_err()
        {
            self.failed = true;
        }
    }

    fn count_frame(&mut self) -> Result<(), IpcError> {
        self.frames = self.frames.checked_add(1).ok_or(IpcError::LimitExceeded)?;
        if self.frames > MAX_TRANSACTION_FRAMES {
            return Err(IpcError::LimitExceeded);
        }
        Ok(())
    }
}

impl<R: Read, W: Write> ConversationHandler for WorkerConversation<R, W> {
    fn prompt_echo_on(&mut self, prompt: &CStr) -> Result<CString, ErrorCode> {
        self.prompt(prompt, WorkerPromptKind::Visible)
    }

    fn prompt_echo_off(&mut self, prompt: &CStr) -> Result<CString, ErrorCode> {
        self.prompt(prompt, WorkerPromptKind::Secret)
    }

    fn text_info(&mut self, message: &CStr) {
        self.message(message, WorkerMessageLevel::Info);
    }

    fn error_msg(&mut self, message: &CStr) {
        self.message(message, WorkerMessageLevel::Error);
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use pam_client::ConversationHandler;

    use crate::ipc::{
        ParentMessage, WorkerMessage, WorkerPromptKind, read_worker_message, write_parent_message,
    };

    use super::WorkerConversation;

    #[test]
    fn conversation_round_trips_matching_secret_prompt() {
        let mut input = Vec::new();
        write_parent_message(
            &mut input,
            &ParentMessage::Answer {
                prompt: 1,
                response: b"secret".to_vec(),
            },
        )
        .expect("test answer can be encoded");
        let mut conversation = WorkerConversation::new(Cursor::new(input), Vec::new());
        let response = conversation
            .prompt_echo_off(c"Password:")
            .expect("matching answer is accepted");
        assert_eq!(response.as_bytes(), b"secret");
        assert_eq!(
            read_worker_message(&mut Cursor::new(conversation.writer)),
            Ok(WorkerMessage::Prompt {
                prompt: 1,
                kind: WorkerPromptKind::Secret,
                message: "Password:".to_owned(),
            })
        );
    }

    #[test]
    fn conversation_rejects_wrong_prompt_identifier() {
        let mut input = Vec::new();
        write_parent_message(
            &mut input,
            &ParentMessage::Answer {
                prompt: 9,
                response: b"secret".to_vec(),
            },
        )
        .expect("test answer can be encoded");
        let mut conversation = WorkerConversation::new(Cursor::new(input), Vec::new());
        assert!(conversation.prompt_echo_off(c"Password:").is_err());
        assert!(conversation.failed());
    }
}
