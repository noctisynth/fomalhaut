use std::{
    error::Error,
    fmt,
    fs::File,
    io::Read,
    path::Path,
    process::{Child, Command, Stdio},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

use fomalhaut_core::AuthenticatedIdentity;

use crate::ipc::MAX_USERNAME_BYTES;

const PROC_STATUS_PATH: &str = "/proc/self/status";
const GETENT_PATH: &str = "/usr/bin/getent";
const MAX_PROC_STATUS_BYTES: u64 = 64 * 1024;
const MAX_PASSWD_BYTES: u64 = 16 * 1024;
const LOOKUP_TIMEOUT: Duration = Duration::from_secs(2);
const LOOKUP_POLL_INTERVAL: Duration = Duration::from_millis(10);
const READER_GRACE: Duration = Duration::from_millis(250);

/// Account identity derived from the locker's real process credentials and NSS.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CurrentUserIdentity {
    uid: u32,
    username: String,
    display_name: String,
}

impl CurrentUserIdentity {
    /// Resolves the non-privileged current process UID through the system NSS database.
    pub fn discover() -> Result<Self, IdentityDiscoveryError> {
        let status = read_bounded_file(Path::new(PROC_STATUS_PATH), MAX_PROC_STATUS_BYTES)?;
        let uid = parse_process_uid(&status)?;
        let passwd = run_getent(uid)?;
        parse_passwd_identity(uid, &passwd)
    }

    /// Returns the real numeric UID used for account resolution.
    #[must_use]
    pub const fn uid(&self) -> u32 {
        self.uid
    }

    /// Returns the NSS account name fixed for all reauthentication attempts.
    #[must_use]
    pub fn username(&self) -> &str {
        &self.username
    }

    /// Returns the trusted display label derived from the account database.
    #[must_use]
    pub fn display_name(&self) -> &str {
        &self.display_name
    }

    pub(crate) fn from_trusted_parts(
        uid: u32,
        username: &str,
        display_name: &str,
    ) -> Result<Self, IdentityDiscoveryError> {
        if !valid_identity_text(username, MAX_USERNAME_BYTES)
            || !valid_identity_text(display_name, MAX_USERNAME_BYTES)
        {
            return Err(IdentityDiscoveryError::InvalidAccount);
        }
        Ok(Self {
            uid,
            username: username.to_owned(),
            display_name: display_name.to_owned(),
        })
    }

    pub(crate) fn authenticated_identity(
        &self,
    ) -> Result<AuthenticatedIdentity, fomalhaut_core::CoreError> {
        AuthenticatedIdentity::new(self.username.clone())
    }
}

/// Failure while deriving the current session account without trusting theme input.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IdentityDiscoveryError {
    /// `/proc/self/status` could not be read within its fixed bound.
    ProcessStatus,
    /// The process is root or its real, effective, saved and filesystem UIDs differ.
    PrivilegedProcess,
    /// The fixed NSS lookup command could not be started or completed in time.
    AccountLookup,
    /// NSS returned malformed, ambiguous or out-of-policy account data.
    InvalidAccount,
}

impl fmt::Display for IdentityDiscoveryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::ProcessStatus => "the current process credentials could not be inspected",
            Self::PrivilegedProcess => {
                "the locker refuses to authenticate from a privileged or credential-changing process"
            }
            Self::AccountLookup => "the current account could not be resolved through NSS",
            Self::InvalidAccount => "the current account record is invalid",
        })
    }
}

impl Error for IdentityDiscoveryError {}

fn read_bounded_file(path: &Path, maximum: u64) -> Result<Vec<u8>, IdentityDiscoveryError> {
    let file = File::open(path).map_err(|_| IdentityDiscoveryError::ProcessStatus)?;
    let mut bytes = Vec::new();
    file.take(maximum.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|_| IdentityDiscoveryError::ProcessStatus)?;
    if u64::try_from(bytes.len()).map_or(true, |length| length > maximum) {
        return Err(IdentityDiscoveryError::ProcessStatus);
    }
    Ok(bytes)
}

fn parse_process_uid(status: &[u8]) -> Result<u32, IdentityDiscoveryError> {
    let status = std::str::from_utf8(status).map_err(|_| IdentityDiscoveryError::ProcessStatus)?;
    let Some(uid_line) = status.lines().find_map(|line| line.strip_prefix("Uid:")) else {
        return Err(IdentityDiscoveryError::ProcessStatus);
    };
    let values = uid_line
        .split_ascii_whitespace()
        .map(str::parse::<u32>)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| IdentityDiscoveryError::ProcessStatus)?;
    let [real, effective, saved, filesystem] = values.as_slice() else {
        return Err(IdentityDiscoveryError::ProcessStatus);
    };
    if real != effective || real != saved || real != filesystem {
        return Err(IdentityDiscoveryError::PrivilegedProcess);
    }
    if *real == 0 {
        return Err(IdentityDiscoveryError::PrivilegedProcess);
    }
    Ok(*real)
}

fn run_getent(uid: u32) -> Result<Vec<u8>, IdentityDiscoveryError> {
    let mut child = Command::new(GETENT_PATH)
        .args(["passwd", &uid.to_string()])
        .env_clear()
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| IdentityDiscoveryError::AccountLookup)?;
    let Some(stdout) = child.stdout.take() else {
        terminate_child(&mut child);
        return Err(IdentityDiscoveryError::AccountLookup);
    };
    let (sender, receiver) = mpsc::sync_channel(1);
    let reader = thread::Builder::new()
        .name("fomalhaut-current-account".to_owned())
        .spawn(move || {
            let mut bytes = Vec::new();
            let result = stdout
                .take(MAX_PASSWD_BYTES.saturating_add(1))
                .read_to_end(&mut bytes)
                .map(|_| bytes);
            let _ = sender.send(result);
        })
        .map_err(|_| {
            terminate_child(&mut child);
            IdentityDiscoveryError::AccountLookup
        })?;

    let deadline = Instant::now() + LOOKUP_TIMEOUT;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < deadline => thread::sleep(LOOKUP_POLL_INTERVAL),
            Ok(None) | Err(_) => {
                terminate_child(&mut child);
                let _ = reader.join();
                return Err(IdentityDiscoveryError::AccountLookup);
            }
        }
    };
    if !status.success() {
        let _ = reader.join();
        return Err(IdentityDiscoveryError::AccountLookup);
    }
    let bytes = receiver
        .recv_timeout(READER_GRACE)
        .map_err(|_| IdentityDiscoveryError::AccountLookup)?
        .map_err(|_| IdentityDiscoveryError::AccountLookup)?;
    if reader.join().is_err()
        || u64::try_from(bytes.len()).map_or(true, |length| length > MAX_PASSWD_BYTES)
    {
        return Err(IdentityDiscoveryError::AccountLookup);
    }
    Ok(bytes)
}

fn terminate_child(child: &mut Child) {
    match child.try_wait() {
        Ok(Some(_)) => {}
        Ok(None) | Err(_) => {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

fn parse_passwd_identity(
    expected_uid: u32,
    passwd: &[u8],
) -> Result<CurrentUserIdentity, IdentityDiscoveryError> {
    let passwd = std::str::from_utf8(passwd).map_err(|_| IdentityDiscoveryError::InvalidAccount)?;
    let mut records = passwd.lines().filter(|line| !line.is_empty());
    let record = records
        .next()
        .ok_or(IdentityDiscoveryError::InvalidAccount)?;
    if records.next().is_some() {
        return Err(IdentityDiscoveryError::InvalidAccount);
    }
    let fields = record.split(':').collect::<Vec<_>>();
    let [username, _, uid, _, gecos, _, _] = fields.as_slice() else {
        return Err(IdentityDiscoveryError::InvalidAccount);
    };
    let uid = uid
        .parse::<u32>()
        .map_err(|_| IdentityDiscoveryError::InvalidAccount)?;
    if uid != expected_uid || !valid_identity_text(username, MAX_USERNAME_BYTES) {
        return Err(IdentityDiscoveryError::InvalidAccount);
    }
    let display_name = gecos.split(',').next().unwrap_or_default().trim();
    let display_name = if display_name.is_empty() {
        *username
    } else {
        display_name
    };
    if !valid_identity_text(display_name, MAX_USERNAME_BYTES) {
        return Err(IdentityDiscoveryError::InvalidAccount);
    }
    CurrentUserIdentity::from_trusted_parts(uid, username, display_name)
}

fn valid_identity_text(value: &str, maximum: usize) -> bool {
    !value.is_empty()
        && value.len() <= maximum
        && !value.chars().any(char::is_control)
        && !value.contains(':')
}

#[cfg(test)]
mod tests {
    use super::{
        IdentityDiscoveryError, parse_passwd_identity, parse_process_uid, valid_identity_text,
    };

    #[test]
    fn accepts_one_unprivileged_uid_and_matching_nss_record() {
        let status = b"Name:\tfomalhaut\nUid:\t1000\t1000\t1000\t1000\n";
        let uid = parse_process_uid(status).expect("equal process UIDs are trusted");
        let identity = parse_passwd_identity(
            uid,
            b"alice:x:1000:1000:Alice Example,,,:/home/alice:/bin/bash\n",
        )
        .expect("matching NSS record is valid");
        assert_eq!(identity.uid(), 1000);
        assert_eq!(identity.username(), "alice");
        assert_eq!(identity.display_name(), "Alice Example");
    }

    #[test]
    fn rejects_credential_changing_processes() {
        let status = b"Uid:\t1000\t0\t0\t0\n";
        assert_eq!(
            parse_process_uid(status),
            Err(IdentityDiscoveryError::PrivilegedProcess)
        );
        let root = b"Uid:\t0\t0\t0\t0\n";
        assert_eq!(
            parse_process_uid(root),
            Err(IdentityDiscoveryError::PrivilegedProcess)
        );
    }

    #[test]
    fn rejects_ambiguous_or_mismatched_account_records() {
        assert_eq!(
            parse_passwd_identity(1000, b"alice:x:1001:1001::/home/alice:/bin/bash\n"),
            Err(IdentityDiscoveryError::InvalidAccount)
        );
        assert_eq!(
            parse_passwd_identity(
                1000,
                b"alice:x:1000:1000::/home/alice:/bin/bash\nbob:x:1000:1000::/home/bob:/bin/bash\n",
            ),
            Err(IdentityDiscoveryError::InvalidAccount)
        );
    }

    #[test]
    fn bounds_and_sanitizes_public_identity_text() {
        assert!(valid_identity_text("Alice Example", 256));
        assert!(!valid_identity_text("Alice\nExample", 256));
        assert!(!valid_identity_text("", 256));
    }
}
