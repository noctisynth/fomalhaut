//! Minimal systemd notification without expanding locker privileges or dependencies.

use std::{
    env,
    ffi::OsStr,
    os::{
        linux::net::SocketAddrExt,
        unix::{
            ffi::OsStrExt,
            net::{SocketAddr, UnixDatagram},
        },
    },
    path::Path,
};

const NOTIFY_SOCKET: &str = "NOTIFY_SOCKET";
const READY_MESSAGE: &[u8] = b"READY=1\nSTATUS=Session locked";

/// Notifies a supervising systemd user service after the compositor lock is confirmed.
pub(crate) fn notify_ready() -> Result<(), ReadinessError> {
    let Some(socket) = env::var_os(NOTIFY_SOCKET) else {
        return Ok(());
    };
    send_ready_to(&socket)
}

fn send_ready_to(socket: &OsStr) -> Result<(), ReadinessError> {
    let bytes = socket.as_bytes();
    if bytes.is_empty() || bytes.contains(&0) {
        return Err(ReadinessError);
    }
    let address = if let Some(abstract_name) = bytes.strip_prefix(b"@") {
        if abstract_name.is_empty() {
            return Err(ReadinessError);
        }
        SocketAddr::from_abstract_name(abstract_name).map_err(|_| ReadinessError)?
    } else {
        SocketAddr::from_pathname(Path::new(socket)).map_err(|_| ReadinessError)?
    };
    let datagram = UnixDatagram::unbound().map_err(|_| ReadinessError)?;
    let sent = datagram
        .send_to_addr(READY_MESSAGE, &address)
        .map_err(|_| ReadinessError)?;
    if sent != READY_MESSAGE.len() {
        return Err(ReadinessError);
    }
    Ok(())
}

/// Sanitized readiness notification failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ReadinessError;

impl std::fmt::Display for ReadinessError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("the systemd readiness notification could not be sent")
    }
}

impl std::error::Error for ReadinessError {}

#[cfg(test)]
mod tests {
    use std::{
        ffi::OsStr,
        os::{
            linux::net::SocketAddrExt,
            unix::net::{SocketAddr, UnixDatagram},
        },
        sync::atomic::{AtomicU64, Ordering},
        time::Duration,
    };

    use super::{READY_MESSAGE, send_ready_to};

    static SOCKET_SEQUENCE: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn sends_ready_to_filesystem_notify_socket() {
        let sequence = SOCKET_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "fomalhaut-notify-{}-{sequence}.sock",
            std::process::id()
        ));
        let receiver = UnixDatagram::bind(&path).expect("test notify socket can be bound");
        receiver
            .set_read_timeout(Some(Duration::from_secs(1)))
            .expect("test timeout can be configured");
        send_ready_to(path.as_os_str()).expect("filesystem notification succeeds");
        let mut message = [0_u8; 64];
        let length = receiver
            .recv(&mut message)
            .expect("notification is received");
        assert_eq!(&message[..length], READY_MESSAGE);
        std::fs::remove_file(path).expect("test notify socket can be removed");
    }

    #[test]
    fn sends_ready_to_abstract_notify_socket() {
        let sequence = SOCKET_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let name = format!("fomalhaut-notify-{}-{sequence}", std::process::id());
        let address = SocketAddr::from_abstract_name(name.as_bytes())
            .expect("test abstract address is valid");
        let receiver =
            UnixDatagram::bind_addr(&address).expect("test abstract notify socket can be bound");
        receiver
            .set_read_timeout(Some(Duration::from_secs(1)))
            .expect("test timeout can be configured");
        let notify_value = format!("@{name}");
        send_ready_to(OsStr::new(&notify_value)).expect("abstract notification succeeds");
        let mut message = [0_u8; 64];
        let length = receiver
            .recv(&mut message)
            .expect("notification is received");
        assert_eq!(&message[..length], READY_MESSAGE);
    }

    #[test]
    fn rejects_empty_or_malformed_notify_addresses() {
        assert!(send_ready_to(OsStr::new("")).is_err());
        assert!(send_ready_to(OsStr::new("@")).is_err());
    }
}
