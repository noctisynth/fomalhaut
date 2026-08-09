//! Bounded system user discovery and trusted avatar ingestion.

use std::{
    collections::HashSet,
    fs::{File, OpenOptions},
    io::{Read, Take},
    os::{
        fd::{AsRawFd, RawFd},
        unix::{fs::MetadataExt, fs::OpenOptionsExt},
    },
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::mpsc::{self, RecvTimeoutError},
    thread,
    time::{Duration, Instant},
};

use fomalhaut_config::{UserDiscoveryConfig, UserProvider};
use fomalhaut_web::protocol::{MAX_USERS, UserSummary};
use zbus::{blocking::Proxy, zvariant::OwnedObjectPath};

const ACCOUNTS_DESTINATION: &str = "org.freedesktop.Accounts";
const ACCOUNTS_PATH: &str = "/org/freedesktop/Accounts";
const ACCOUNTS_INTERFACE: &str = "org.freedesktop.Accounts";
const USER_INTERFACE: &str = "org.freedesktop.Accounts.User";
const AVATAR_URI_PREFIX: &str = "fomalhaut://avatar/";
const TRUSTED_AVATAR_ROOT: &str = "/var/lib/AccountsService/icons";
const LOGIN_DEFS_PATH: &str = "/etc/login.defs";
const GETENT_PATH: &str = "/usr/bin/getent";
const MAX_AVATAR_BYTES: u64 = 2 * 1024 * 1024;
const MAX_LOGIN_DEFS_BYTES: u64 = 64 * 1024;
const MAX_GETENT_OUTPUT_BYTES: u64 = 2 * 1024 * 1024;
const MAX_NSS_ENTRIES: usize = 4096;
const DEFAULT_UID_MIN: u32 = 1000;
const DEFAULT_UID_MAX: u32 = 60_000;
const DISCOVERY_TIMEOUT: Duration = Duration::from_secs(3);
const ACCOUNTS_METHOD_TIMEOUT: Duration = Duration::from_millis(750);
const GETENT_TIMEOUT: Duration = Duration::from_millis(1500);
const GETENT_POLL_INTERVAL: Duration = Duration::from_millis(10);
const GETENT_READER_GRACE: Duration = Duration::from_millis(250);

/// One validated in-memory avatar served by the trusted URI scheme.
pub struct AvatarAsset {
    uri: String,
    body: Vec<u8>,
    content_type: &'static str,
}

impl AvatarAsset {
    /// Returns a copy suitable for one WebKit scheme response.
    pub fn response_body(&self) -> Vec<u8> {
        self.body.clone()
    }

    /// Returns the allowlisted media type detected from magic bytes.
    pub const fn content_type(&self) -> &'static str {
        self.content_type
    }

    /// Returns whether this asset owns the exact opaque URI.
    pub fn matches_uri(&self, uri: &str) -> bool {
        self.uri == uri
    }
}

/// Public user summaries paired with any validated avatar resources.
#[derive(Default)]
pub struct DiscoveredUsers {
    summaries: Vec<UserSummary>,
    avatars: Vec<AvatarAsset>,
}

impl DiscoveredUsers {
    /// Consumes discovery output for the controller and GTK resource host.
    pub fn into_parts(self) -> (Vec<UserSummary>, Vec<AvatarAsset>) {
        (self.summaries, self.avatars)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UserDiscoveryError {
    Spawn,
    Timeout,
    Provider,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AccountsDiscoveryError {
    Unavailable,
    AccessDenied,
}

struct RawUser {
    uid: u32,
    username: String,
    display_name: String,
    icon_file: Option<PathBuf>,
    login_frequency: u64,
}

trait DiscoverySource {
    fn accounts_service(&self) -> Result<Vec<RawUser>, AccountsDiscoveryError>;
    fn nss(&self) -> Result<Vec<RawUser>, UserDiscoveryError>;
}

struct SystemDiscoverySource;

impl DiscoverySource for SystemDiscoverySource {
    fn accounts_service(&self) -> Result<Vec<RawUser>, AccountsDiscoveryError> {
        discover_accounts_service()
    }

    fn nss(&self) -> Result<Vec<RawUser>, UserDiscoveryError> {
        discover_nss()
    }
}

/// Runs system discovery outside the authentication worker with a hard wait limit.
pub fn discover_users(config: UserDiscoveryConfig) -> Result<DiscoveredUsers, UserDiscoveryError> {
    if config.provider() == UserProvider::None {
        return Ok(DiscoveredUsers::default());
    }

    let (sender, receiver) = mpsc::sync_channel(1);
    thread::Builder::new()
        .name("fomalhaut-user-discovery".to_owned())
        .spawn(move || {
            let _ = sender.send(discover_users_now(config));
        })
        .map_err(|_| UserDiscoveryError::Spawn)?;

    receiver
        .recv_timeout(DISCOVERY_TIMEOUT)
        .map_err(|_| UserDiscoveryError::Timeout)?
}

fn discover_users_now(config: UserDiscoveryConfig) -> Result<DiscoveredUsers, UserDiscoveryError> {
    discover_users_with(config, &SystemDiscoverySource)
}

fn discover_users_with(
    config: UserDiscoveryConfig,
    source: &impl DiscoverySource,
) -> Result<DiscoveredUsers, UserDiscoveryError> {
    let raw = match config.provider() {
        UserProvider::Auto => match source.accounts_service() {
            Ok(users) => users,
            Err(AccountsDiscoveryError::Unavailable) => source.nss()?,
            Err(AccountsDiscoveryError::AccessDenied) => Vec::new(),
        },
        UserProvider::AccountsService => source
            .accounts_service()
            .map_err(|_| UserDiscoveryError::Provider)?,
        UserProvider::Nss => source.nss()?,
        UserProvider::None => Vec::new(),
    };
    Ok(build_public_users(raw))
}

fn discover_accounts_service() -> Result<Vec<RawUser>, AccountsDiscoveryError> {
    let connection = zbus::blocking::connection::Builder::system()
        .map_err(|_| AccountsDiscoveryError::Unavailable)?
        .method_timeout(ACCOUNTS_METHOD_TIMEOUT)
        .build()
        .map_err(|_| AccountsDiscoveryError::Unavailable)?;
    let accounts = Proxy::new(
        &connection,
        ACCOUNTS_DESTINATION,
        ACCOUNTS_PATH,
        ACCOUNTS_INTERFACE,
    )
    .map_err(|_| AccountsDiscoveryError::Unavailable)?;
    let paths: Vec<OwnedObjectPath> = accounts
        .call("ListCachedUsers", &())
        .map_err(classify_accounts_error)?;

    let users = paths
        .into_iter()
        .filter_map(|path| accounts_service_user(&connection, path))
        .collect();
    Ok(users)
}

fn classify_accounts_error(error: zbus::Error) -> AccountsDiscoveryError {
    const ACCESS_DENIED: &str = "org.freedesktop.DBus.Error.AccessDenied";
    match error {
        zbus::Error::MethodError(name, _, _) if name.as_str() == ACCESS_DENIED => {
            AccountsDiscoveryError::AccessDenied
        }
        zbus::Error::FDO(error) if matches!(*error, zbus::fdo::Error::AccessDenied(_)) => {
            AccountsDiscoveryError::AccessDenied
        }
        _ => AccountsDiscoveryError::Unavailable,
    }
}

fn accounts_service_user(
    connection: &zbus::blocking::Connection,
    path: OwnedObjectPath,
) -> Option<RawUser> {
    let user = Proxy::new(
        connection,
        ACCOUNTS_DESTINATION,
        path.as_str(),
        USER_INTERFACE,
    )
    .ok()?;
    if user.get_property::<bool>("SystemAccount").ok()?
        || user.get_property::<bool>("Locked").ok()?
    {
        return None;
    }

    let uid = u32::try_from(user.get_property::<u64>("Uid").ok()?).ok()?;
    let username = user.get_property::<String>("UserName").ok()?;
    let real_name = user.get_property::<String>("RealName").ok()?;
    let icon_file = user
        .get_property::<String>("IconFile")
        .ok()
        .filter(|path| !path.is_empty())
        .map(PathBuf::from);
    let login_frequency = user.get_property::<u64>("LoginFrequency").unwrap_or(0);
    let display_name = if real_name.is_empty() {
        username.clone()
    } else {
        real_name
    };

    Some(RawUser {
        uid,
        username,
        display_name,
        icon_file,
        login_frequency,
    })
}

fn discover_nss() -> Result<Vec<RawUser>, UserDiscoveryError> {
    let (uid_min, uid_max) = read_uid_range();
    let output = run_getent()?;
    parse_passwd(&output, uid_min, uid_max)
}

fn run_getent() -> Result<String, UserDiscoveryError> {
    let output = run_bounded_command(Path::new(GETENT_PATH), &["passwd"], GETENT_TIMEOUT)?;
    String::from_utf8(output).map_err(|_| UserDiscoveryError::Provider)
}

fn run_bounded_command(
    executable: &Path,
    arguments: &[&str],
    timeout: Duration,
) -> Result<Vec<u8>, UserDiscoveryError> {
    let mut child = Command::new(executable)
        .args(arguments)
        .env_clear()
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| UserDiscoveryError::Provider)?;
    let Some(stdout) = child.stdout.take() else {
        terminate_child(&mut child);
        return Err(UserDiscoveryError::Provider);
    };
    let (sender, receiver) = mpsc::sync_channel(1);
    if thread::Builder::new()
        .name("fomalhaut-getent-output".to_owned())
        .spawn(move || {
            let mut output = Vec::new();
            let result = stdout
                .take(MAX_GETENT_OUTPUT_BYTES.saturating_add(1))
                .read_to_end(&mut output)
                .map(|_| output);
            let _ = sender.send(result);
        })
        .is_err()
    {
        terminate_child(&mut child);
        return Err(UserDiscoveryError::Spawn);
    }

    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < deadline => thread::sleep(GETENT_POLL_INTERVAL),
            Ok(None) => {
                terminate_child(&mut child);
                return Err(UserDiscoveryError::Timeout);
            }
            Err(_) => {
                terminate_child(&mut child);
                return Err(UserDiscoveryError::Provider);
            }
        }
    };
    if !status.success() {
        return Err(UserDiscoveryError::Provider);
    }
    let output = match receiver.recv_timeout(GETENT_READER_GRACE) {
        Ok(Ok(output)) => output,
        Ok(Err(_)) | Err(RecvTimeoutError::Timeout | RecvTimeoutError::Disconnected) => {
            return Err(UserDiscoveryError::Provider);
        }
    };
    if output.len() as u64 > MAX_GETENT_OUTPUT_BYTES {
        return Err(UserDiscoveryError::Provider);
    }
    Ok(output)
}

fn terminate_child(child: &mut std::process::Child) {
    let _ = child.kill();
    let _ = child.wait();
}

fn parse_passwd(
    output: &str,
    uid_min: u32,
    uid_max: u32,
) -> Result<Vec<RawUser>, UserDiscoveryError> {
    let mut users = Vec::new();
    for (index, line) in output.lines().enumerate() {
        if index >= MAX_NSS_ENTRIES {
            return Err(UserDiscoveryError::Provider);
        }
        let fields = line.split(':').collect::<Vec<_>>();
        let [username, _, uid, _, _, _, shell] = fields.as_slice() else {
            return Err(UserDiscoveryError::Provider);
        };
        let uid = uid
            .parse::<u32>()
            .map_err(|_| UserDiscoveryError::Provider)?;
        if !(uid_min..=uid_max).contains(&uid) || !is_login_shell(Path::new(shell)) {
            continue;
        }
        users.push(RawUser {
            uid,
            username: (*username).to_owned(),
            display_name: (*username).to_owned(),
            icon_file: None,
            login_frequency: 0,
        });
    }
    Ok(users)
}

fn read_uid_range() -> (u32, u32) {
    let Ok(file) = File::open(LOGIN_DEFS_PATH) else {
        return (DEFAULT_UID_MIN, DEFAULT_UID_MAX);
    };
    let mut input = String::new();
    if file
        .take(MAX_LOGIN_DEFS_BYTES.saturating_add(1))
        .read_to_string(&mut input)
        .is_err()
        || input.len() as u64 > MAX_LOGIN_DEFS_BYTES
    {
        return (DEFAULT_UID_MIN, DEFAULT_UID_MAX);
    }

    let mut minimum = None;
    let mut maximum = None;
    for line in input.lines() {
        let mut fields = line.split_whitespace();
        match (fields.next(), fields.next(), fields.next()) {
            (Some("UID_MIN"), Some(value), None) => minimum = value.parse().ok(),
            (Some("UID_MAX"), Some(value), None) => maximum = value.parse().ok(),
            _ => {}
        }
    }
    match (minimum, maximum) {
        (Some(minimum), Some(maximum)) if minimum <= maximum => (minimum, maximum),
        _ => (DEFAULT_UID_MIN, DEFAULT_UID_MAX),
    }
}

fn is_login_shell(shell: &Path) -> bool {
    !shell
        .file_name()
        .is_some_and(|name| name == "nologin" || name == "false")
}

fn build_public_users(mut raw: Vec<RawUser>) -> DiscoveredUsers {
    raw.sort_by(|left, right| {
        right
            .login_frequency
            .cmp(&left.login_frequency)
            .then_with(|| left.username.cmp(&right.username))
    });

    let mut seen = HashSet::new();
    let mut summaries = Vec::new();
    let mut avatars = Vec::new();
    for user in raw {
        if summaries.len() >= MAX_USERS || !seen.insert(user.username.clone()) {
            continue;
        }
        let avatar = user
            .icon_file
            .as_deref()
            .and_then(|path| read_avatar(path, user.uid, avatars.len() + 1));
        let avatar_url = avatar.as_ref().map(|asset| asset.uri.clone());
        let Ok(summary) = UserSummary::new(user.username, user.display_name, avatar_url) else {
            continue;
        };
        if let Some(avatar) = avatar {
            avatars.push(avatar);
        }
        summaries.push(summary);
    }
    DiscoveredUsers { summaries, avatars }
}

fn read_avatar(path: &Path, uid: u32, identifier: usize) -> Option<AvatarAsset> {
    if !path.is_absolute() {
        return None;
    }
    let mut options = OpenOptions::new();
    options
        .read(true)
        .custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW);
    let file = options.open(path).ok()?;
    let metadata = file.metadata().ok()?;
    if !metadata.file_type().is_file() || metadata.len() > MAX_AVATAR_BYTES {
        return None;
    }
    if metadata.uid() != uid && !fd_is_in_trusted_avatar_root(file.as_raw_fd()) {
        return None;
    }

    let mut body = Vec::with_capacity(usize::try_from(metadata.len()).ok()?);
    let mut reader: Take<File> = file.take(MAX_AVATAR_BYTES.saturating_add(1));
    reader.read_to_end(&mut body).ok()?;
    if body.len() as u64 > MAX_AVATAR_BYTES {
        return None;
    }
    let content_type = avatar_content_type(&body)?;
    Some(AvatarAsset {
        uri: format!("{AVATAR_URI_PREFIX}{identifier}"),
        body,
        content_type,
    })
}

fn fd_is_in_trusted_avatar_root(fd: RawFd) -> bool {
    let fd_path = PathBuf::from(format!("/proc/self/fd/{fd}"));
    std::fs::read_link(fd_path)
        .ok()
        .is_some_and(|path| path.starts_with(TRUSTED_AVATAR_ROOT))
}

fn avatar_content_type(body: &[u8]) -> Option<&'static str> {
    if body.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if body.starts_with(&[0xff, 0xd8, 0xff]) {
        Some("image/jpeg")
    } else if body.len() >= 12 && body.starts_with(b"RIFF") && &body[8..12] == b"WEBP" {
        Some("image/webp")
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::{
        AccountsDiscoveryError, DiscoverySource, MAX_AVATAR_BYTES, MAX_NSS_ENTRIES, RawUser,
        UserDiscoveryError, avatar_content_type, build_public_users, discover_users_with,
        is_login_shell, parse_passwd, read_avatar, run_bounded_command,
    };
    use fomalhaut_config::{UserDiscoveryConfig, UserProvider};
    use std::{
        cell::Cell,
        fs::{self, File},
        os::unix::{fs::MetadataExt, fs::symlink},
        path::{Path, PathBuf},
        sync::atomic::{AtomicU64, Ordering},
        time::Duration,
    };

    static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

    #[derive(Clone, Copy)]
    enum AccountsOutcome {
        Empty,
        Unavailable,
        AccessDenied,
    }

    struct MockSource {
        accounts_outcome: AccountsOutcome,
        accounts_calls: Cell<usize>,
        nss_calls: Cell<usize>,
    }

    impl MockSource {
        fn new(accounts_outcome: AccountsOutcome) -> Self {
            Self {
                accounts_outcome,
                accounts_calls: Cell::new(0),
                nss_calls: Cell::new(0),
            }
        }
    }

    impl DiscoverySource for MockSource {
        fn accounts_service(&self) -> Result<Vec<RawUser>, AccountsDiscoveryError> {
            self.accounts_calls
                .set(self.accounts_calls.get().saturating_add(1));
            match self.accounts_outcome {
                AccountsOutcome::Empty => Ok(Vec::new()),
                AccountsOutcome::Unavailable => Err(AccountsDiscoveryError::Unavailable),
                AccountsOutcome::AccessDenied => Err(AccountsDiscoveryError::AccessDenied),
            }
        }

        fn nss(&self) -> Result<Vec<RawUser>, UserDiscoveryError> {
            self.nss_calls.set(self.nss_calls.get().saturating_add(1));
            Ok(vec![RawUser {
                uid: 1000,
                username: "fallback".to_owned(),
                display_name: "fallback".to_owned(),
                icon_file: None,
                login_frequency: 0,
            }])
        }
    }

    #[test]
    fn raster_magic_allowlist_rejects_active_content() {
        assert_eq!(
            avatar_content_type(b"\x89PNG\r\n\x1a\nrest"),
            Some("image/png")
        );
        assert_eq!(avatar_content_type(b"\xff\xd8\xffrest"), Some("image/jpeg"));
        assert_eq!(avatar_content_type(b"RIFF0000WEBPrest"), Some("image/webp"));
        assert_eq!(avatar_content_type(b"<svg><script/></svg>"), None);
    }

    #[test]
    fn nss_shell_filter_rejects_non_login_shells() {
        assert!(!is_login_shell(Path::new("/usr/bin/nologin")));
        assert!(!is_login_shell(Path::new("/bin/false")));
        assert!(is_login_shell(Path::new("/bin/bash")));
    }

    #[test]
    fn auto_falls_back_only_when_accounts_service_is_unavailable() {
        let unavailable = MockSource::new(AccountsOutcome::Unavailable);
        let discovered =
            discover_users_with(UserDiscoveryConfig::new(UserProvider::Auto), &unavailable)
                .expect("the mock NSS fallback succeeds");
        assert_eq!(unavailable.accounts_calls.get(), 1);
        assert_eq!(unavailable.nss_calls.get(), 1);
        assert_eq!(discovered.summaries[0].username(), "fallback");

        for outcome in [AccountsOutcome::Empty, AccountsOutcome::AccessDenied] {
            let source = MockSource::new(outcome);
            let discovered =
                discover_users_with(UserDiscoveryConfig::new(UserProvider::Auto), &source)
                    .expect("empty and denied results remain nonfatal");
            assert!(discovered.summaries.is_empty());
            assert_eq!(source.accounts_calls.get(), 1);
            assert_eq!(source.nss_calls.get(), 0);
        }
    }

    #[test]
    fn public_users_are_sorted_deduplicated_and_bounded() {
        let users = vec![
            RawUser {
                uid: 1001,
                username: "bob".to_owned(),
                display_name: "Bob".to_owned(),
                icon_file: None,
                login_frequency: 1,
            },
            RawUser {
                uid: 1000,
                username: "alice".to_owned(),
                display_name: "Alice".to_owned(),
                icon_file: None,
                login_frequency: 9,
            },
            RawUser {
                uid: 1002,
                username: "alice".to_owned(),
                display_name: "Duplicate".to_owned(),
                icon_file: Some(PathBuf::from("relative.png")),
                login_frequency: 0,
            },
        ];
        let (summaries, avatars) = build_public_users(users).into_parts();
        assert_eq!(summaries[0].username(), "alice");
        assert_eq!(summaries[1].username(), "bob");
        assert!(avatars.is_empty());
    }

    #[test]
    fn passwd_parser_filters_uid_shell_and_preserves_empty_success() {
        let users = parse_passwd(
            "root:x:0:0:root:/root:/bin/bash\nalice:x:1000:1000:Alice:/home/alice:/bin/bash\nservice:x:1001:1001::/:/usr/bin/nologin\n",
            1000,
            60_000,
        )
        .expect("passwd fixture has the required seven fields");
        assert_eq!(users.len(), 1);
        assert_eq!(users[0].username, "alice");
        assert!(
            parse_passwd("root:x:0:0:root:/root:/bin/bash\n", 1000, 60_000)
                .expect("a filtered passwd fixture remains a successful empty result")
                .is_empty()
        );
    }

    #[test]
    fn passwd_parser_rejects_malformed_provider_output() {
        assert!(matches!(
            parse_passwd("missing:fields\n", 1000, 60_000),
            Err(UserDiscoveryError::Provider)
        ));
        assert!(matches!(
            parse_passwd(
                "alice:x:not-a-uid:1000::/home/alice:/bin/bash\n",
                1000,
                60_000
            ),
            Err(UserDiscoveryError::Provider)
        ));
    }

    #[test]
    fn passwd_parser_rejects_an_unbounded_catalog() {
        let mut fixture = String::new();
        for uid in 0..=MAX_NSS_ENTRIES {
            fixture.push_str(&format!("user{uid}:x:{uid}:1000::/:/bin/bash\n"));
        }
        assert!(matches!(
            parse_passwd(&fixture, 0, u32::MAX),
            Err(UserDiscoveryError::Provider)
        ));
    }

    #[test]
    fn bounded_command_terminates_a_timed_out_child() {
        assert!(matches!(
            run_bounded_command(
                Path::new("/usr/bin/sleep"),
                &["1"],
                Duration::from_millis(10)
            ),
            Err(UserDiscoveryError::Timeout)
        ));
    }

    #[test]
    fn avatar_ingestion_accepts_raster_and_rejects_symlink_active_and_oversized_files() {
        let sequence = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let directory = std::env::temp_dir().join(format!(
            "fomalhaut-avatar-test-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&directory).expect("the unique avatar test directory can be created");

        let png = directory.join("avatar.png");
        fs::write(&png, b"\x89PNG\r\n\x1a\nfixture").expect("the avatar fixture can be written");
        let uid = fs::metadata(&png)
            .expect("the avatar fixture has metadata")
            .uid();
        let asset = read_avatar(&png, uid, 7).expect("a user-owned PNG is accepted");
        assert!(asset.matches_uri("fomalhaut://avatar/7"));
        assert_eq!(asset.content_type(), "image/png");

        let link = directory.join("avatar-link.png");
        symlink(&png, &link).expect("the final-component symlink fixture can be created");
        assert!(read_avatar(&link, uid, 8).is_none());

        let svg = directory.join("avatar.svg");
        fs::write(&svg, b"<svg><script/></svg>")
            .expect("the active-content fixture can be written");
        assert!(read_avatar(&svg, uid, 9).is_none());

        let oversized = directory.join("oversized.png");
        let oversized_file =
            File::create(&oversized).expect("the oversized avatar fixture can be created");
        oversized_file
            .set_len(MAX_AVATAR_BYTES.saturating_add(1))
            .expect("the sparse avatar fixture can be sized");
        assert!(read_avatar(&oversized, uid, 10).is_none());

        fs::remove_dir_all(directory).expect("avatar test fixtures can be removed");
    }
}
