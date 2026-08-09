use std::io::{self, Read, Write};

pub(crate) const MAX_USERNAME_BYTES: usize = 256;
pub(crate) const MAX_PROMPT_BYTES: usize = 4 * 1024;
pub(crate) const MAX_ANSWER_BYTES: usize = 16 * 1024;
pub(crate) const MAX_TRANSACTION_FRAMES: usize = 128;

const HEADER_BYTES: usize = 5;
const MAX_FRAME_BYTES: usize = MAX_ANSWER_BYTES + 8;

const PREPARE: u8 = 1;
const BEGIN: u8 = 2;
const ANSWER: u8 = 3;
const READY: u8 = 16;
const PROMPT_SECRET: u8 = 17;
const PROMPT_VISIBLE: u8 = 18;
const MESSAGE_INFO: u8 = 19;
const MESSAGE_ERROR: u8 = 20;
const AUTHENTICATED: u8 = 21;
const REJECTED: u8 = 22;
const FATAL: u8 = 23;

pub(crate) enum ParentMessage {
    Prepare(String),
    Begin,
    Answer { prompt: u64, response: Vec<u8> },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum WorkerPromptKind {
    Secret,
    Visible,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum WorkerMessageLevel {
    Info,
    Error,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum WorkerMessage {
    Ready,
    Prompt {
        prompt: u64,
        kind: WorkerPromptKind,
        message: String,
    },
    Message {
        level: WorkerMessageLevel,
        text: String,
    },
    Authenticated,
    Rejected,
    Fatal,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum IpcError {
    Disconnected,
    Io,
    InvalidFrame,
    LimitExceeded,
}

pub(crate) fn write_parent_message(
    writer: &mut impl Write,
    message: &ParentMessage,
) -> Result<(), IpcError> {
    match message {
        ParentMessage::Prepare(username) => {
            validate_text(username.as_bytes(), MAX_USERNAME_BYTES, false)?;
            write_frame(writer, PREPARE, username.as_bytes())
        }
        ParentMessage::Begin => write_frame(writer, BEGIN, &[]),
        ParentMessage::Answer { prompt, response } => {
            if response.len() > MAX_ANSWER_BYTES || response.contains(&0) {
                return Err(IpcError::LimitExceeded);
            }
            let mut payload = Vec::with_capacity(8 + response.len());
            payload.extend_from_slice(&prompt.to_be_bytes());
            payload.extend_from_slice(response);
            let result = write_frame(writer, ANSWER, &payload);
            payload.fill(0);
            result
        }
    }
}

pub(crate) fn read_parent_message(reader: &mut impl Read) -> Result<ParentMessage, IpcError> {
    let (tag, payload) = read_frame(reader)?;
    match tag {
        PREPARE => Ok(ParentMessage::Prepare(decode_text(
            payload,
            MAX_USERNAME_BYTES,
            false,
        )?)),
        BEGIN if payload.is_empty() => Ok(ParentMessage::Begin),
        ANSWER if payload.len() >= 8 && payload.len() <= MAX_ANSWER_BYTES + 8 => {
            let mut payload = payload;
            let prompt = u64::from_be_bytes(
                payload[..8]
                    .try_into()
                    .map_err(|_| IpcError::InvalidFrame)?,
            );
            payload.rotate_left(8);
            payload.truncate(payload.len() - 8);
            if payload.contains(&0) {
                payload.fill(0);
                return Err(IpcError::InvalidFrame);
            }
            Ok(ParentMessage::Answer {
                prompt,
                response: payload,
            })
        }
        _ => Err(IpcError::InvalidFrame),
    }
}

pub(crate) fn write_worker_message(
    writer: &mut impl Write,
    message: &WorkerMessage,
) -> Result<(), IpcError> {
    match message {
        WorkerMessage::Ready => write_frame(writer, READY, &[]),
        WorkerMessage::Prompt {
            prompt,
            kind,
            message,
        } => {
            validate_text(message.as_bytes(), MAX_PROMPT_BYTES, true)?;
            let mut payload = Vec::with_capacity(8 + message.len());
            payload.extend_from_slice(&prompt.to_be_bytes());
            payload.extend_from_slice(message.as_bytes());
            let tag = match kind {
                WorkerPromptKind::Secret => PROMPT_SECRET,
                WorkerPromptKind::Visible => PROMPT_VISIBLE,
            };
            write_frame(writer, tag, &payload)
        }
        WorkerMessage::Message { level, text } => {
            validate_text(text.as_bytes(), MAX_PROMPT_BYTES, true)?;
            let tag = match level {
                WorkerMessageLevel::Info => MESSAGE_INFO,
                WorkerMessageLevel::Error => MESSAGE_ERROR,
            };
            write_frame(writer, tag, text.as_bytes())
        }
        WorkerMessage::Authenticated => write_frame(writer, AUTHENTICATED, &[]),
        WorkerMessage::Rejected => write_frame(writer, REJECTED, &[]),
        WorkerMessage::Fatal => write_frame(writer, FATAL, &[]),
    }
}

pub(crate) fn read_worker_message(reader: &mut impl Read) -> Result<WorkerMessage, IpcError> {
    let (tag, payload) = read_frame(reader)?;
    match tag {
        READY if payload.is_empty() => Ok(WorkerMessage::Ready),
        PROMPT_SECRET | PROMPT_VISIBLE if payload.len() >= 8 => {
            let prompt = u64::from_be_bytes(
                payload[..8]
                    .try_into()
                    .map_err(|_| IpcError::InvalidFrame)?,
            );
            let message = decode_text(payload[8..].to_vec(), MAX_PROMPT_BYTES, true)?;
            let kind = if tag == PROMPT_SECRET {
                WorkerPromptKind::Secret
            } else {
                WorkerPromptKind::Visible
            };
            Ok(WorkerMessage::Prompt {
                prompt,
                kind,
                message,
            })
        }
        MESSAGE_INFO | MESSAGE_ERROR => {
            let text = decode_text(payload, MAX_PROMPT_BYTES, true)?;
            let level = if tag == MESSAGE_INFO {
                WorkerMessageLevel::Info
            } else {
                WorkerMessageLevel::Error
            };
            Ok(WorkerMessage::Message { level, text })
        }
        AUTHENTICATED if payload.is_empty() => Ok(WorkerMessage::Authenticated),
        REJECTED if payload.is_empty() => Ok(WorkerMessage::Rejected),
        FATAL if payload.is_empty() => Ok(WorkerMessage::Fatal),
        _ => Err(IpcError::InvalidFrame),
    }
}

fn write_frame(writer: &mut impl Write, tag: u8, payload: &[u8]) -> Result<(), IpcError> {
    if payload.len() > MAX_FRAME_BYTES {
        return Err(IpcError::LimitExceeded);
    }
    let length = u32::try_from(payload.len()).map_err(|_| IpcError::LimitExceeded)?;
    let mut header = [0_u8; HEADER_BYTES];
    header[0] = tag;
    header[1..].copy_from_slice(&length.to_be_bytes());
    writer.write_all(&header).map_err(classify_io)?;
    writer.write_all(payload).map_err(classify_io)?;
    writer.flush().map_err(classify_io)
}

fn read_frame(reader: &mut impl Read) -> Result<(u8, Vec<u8>), IpcError> {
    let mut header = [0_u8; HEADER_BYTES];
    reader.read_exact(&mut header).map_err(classify_io)?;
    let length = u32::from_be_bytes(header[1..].try_into().map_err(|_| IpcError::InvalidFrame)?);
    let length = usize::try_from(length).map_err(|_| IpcError::LimitExceeded)?;
    if length > MAX_FRAME_BYTES {
        return Err(IpcError::LimitExceeded);
    }
    let mut payload = vec![0_u8; length];
    reader.read_exact(&mut payload).map_err(classify_io)?;
    Ok((header[0], payload))
}

fn decode_text(payload: Vec<u8>, maximum: usize, allow_empty: bool) -> Result<String, IpcError> {
    validate_text(&payload, maximum, allow_empty)?;
    String::from_utf8(payload).map_err(|_| IpcError::InvalidFrame)
}

fn validate_text(value: &[u8], maximum: usize, allow_empty: bool) -> Result<(), IpcError> {
    if (!allow_empty && value.is_empty()) || value.len() > maximum || value.contains(&0) {
        return Err(IpcError::LimitExceeded);
    }
    std::str::from_utf8(value).map_err(|_| IpcError::InvalidFrame)?;
    Ok(())
}

fn classify_io(error: io::Error) -> IpcError {
    if error.kind() == io::ErrorKind::UnexpectedEof
        || matches!(
            error.kind(),
            io::ErrorKind::BrokenPipe | io::ErrorKind::ConnectionReset
        )
    {
        IpcError::Disconnected
    } else {
        IpcError::Io
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::{
        IpcError, MAX_ANSWER_BYTES, ParentMessage, WorkerMessage, WorkerMessageLevel,
        WorkerPromptKind, read_parent_message, read_worker_message, write_parent_message,
        write_worker_message,
    };

    #[test]
    fn round_trips_bounded_parent_messages() {
        let messages = [
            ParentMessage::Prepare("alice".to_owned()),
            ParentMessage::Begin,
            ParentMessage::Answer {
                prompt: 42,
                response: b"correct horse".to_vec(),
            },
        ];
        let mut bytes = Vec::new();
        for message in &messages {
            write_parent_message(&mut bytes, message).expect("bounded message can be encoded");
        }
        let mut reader = Cursor::new(bytes);
        assert!(matches!(
            read_parent_message(&mut reader),
            Ok(ParentMessage::Prepare(username)) if username == "alice"
        ));
        assert!(matches!(
            read_parent_message(&mut reader),
            Ok(ParentMessage::Begin)
        ));
        assert!(matches!(
            read_parent_message(&mut reader),
            Ok(ParentMessage::Answer { prompt: 42, response }) if response == b"correct horse"
        ));
    }

    #[test]
    fn round_trips_worker_prompt() {
        let mut bytes = Vec::new();
        write_worker_message(
            &mut bytes,
            &WorkerMessage::Prompt {
                prompt: 7,
                kind: WorkerPromptKind::Secret,
                message: "Password:".to_owned(),
            },
        )
        .expect("bounded prompt can be encoded");
        assert_eq!(
            read_worker_message(&mut Cursor::new(bytes)),
            Ok(WorkerMessage::Prompt {
                prompt: 7,
                kind: WorkerPromptKind::Secret,
                message: "Password:".to_owned(),
            })
        );
    }

    #[test]
    fn permits_empty_pam_prompt_and_message_text() {
        let mut bytes = Vec::new();
        write_worker_message(
            &mut bytes,
            &WorkerMessage::Prompt {
                prompt: 3,
                kind: WorkerPromptKind::Visible,
                message: String::new(),
            },
        )
        .expect("PAM may provide an empty prompt label");
        write_worker_message(
            &mut bytes,
            &WorkerMessage::Message {
                level: WorkerMessageLevel::Info,
                text: String::new(),
            },
        )
        .expect("PAM may provide an empty informational message");
        let mut reader = Cursor::new(bytes);
        assert_eq!(
            read_worker_message(&mut reader),
            Ok(WorkerMessage::Prompt {
                prompt: 3,
                kind: WorkerPromptKind::Visible,
                message: String::new(),
            })
        );
        assert_eq!(
            read_worker_message(&mut reader),
            Ok(WorkerMessage::Message {
                level: WorkerMessageLevel::Info,
                text: String::new(),
            })
        );
    }

    #[test]
    fn rejects_oversized_or_nul_answers() {
        let oversized = ParentMessage::Answer {
            prompt: 1,
            response: vec![b'x'; MAX_ANSWER_BYTES + 1],
        };
        let nul = ParentMessage::Answer {
            prompt: 1,
            response: b"bad\0answer".to_vec(),
        };
        assert_eq!(
            write_parent_message(&mut Vec::new(), &oversized),
            Err(IpcError::LimitExceeded)
        );
        assert_eq!(
            write_parent_message(&mut Vec::new(), &nul),
            Err(IpcError::LimitExceeded)
        );
    }
}
