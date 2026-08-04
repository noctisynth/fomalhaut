//! Strict system configuration parsing and semantic validation.

use std::{
    error::Error,
    fmt,
    fs::File,
    io::{self, Read},
    path::{Path, PathBuf},
};

use fomalhaut_session::{DiscoveryConfig, SessionDirectory, SessionKind};
use fomalhaut_web::protocol::PowerAction;
use serde::Deserialize;

const CONFIG_PATH: &str = "/etc/fomalhaut/config.toml";
const MAX_CONFIG_BYTES: u64 = 64 * 1024;
const MIN_DISPLAY_SCALE: f64 = 0.5;
const MAX_DISPLAY_SCALE: f64 = 4.0;
const DEFAULT_EXECUTABLE_DIRS: [&str; 2] = ["/usr/local/bin", "/usr/bin"];
const DEFAULT_WAYLAND_DIRS: [&str; 2] = [
    "/usr/local/share/wayland-sessions",
    "/usr/share/wayland-sessions",
];
const DEFAULT_X11_DIRS: [&str; 2] = ["/usr/local/share/xsessions", "/usr/share/xsessions"];

/// Validated runtime configuration used by the trusted host.
pub struct AppConfig {
    theme_directory: Option<PathBuf>,
    discovery: DiscoveryConfig,
    users: UserDiscoveryConfig,
    power: PowerConfig,
    display: DisplayConfig,
}

impl AppConfig {
    /// Loads the fixed system configuration or safe defaults when it is absent.
    pub fn load() -> Result<Self, ConfigError> {
        load_from_path(Path::new(CONFIG_PATH))
    }

    /// Consumes the configuration into its theme, session, and user discovery inputs.
    #[must_use]
    pub fn into_parts(
        self,
    ) -> (
        Option<PathBuf>,
        DiscoveryConfig,
        UserDiscoveryConfig,
        PowerConfig,
        DisplayConfig,
    ) {
        (
            self.theme_directory,
            self.discovery,
            self.users,
            self.power,
            self.display,
        )
    }
}

/// Validated WebKit presentation settings.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DisplayConfig {
    scale: f64,
}

impl Default for DisplayConfig {
    fn default() -> Self {
        Self { scale: 1.0 }
    }
}

impl DisplayConfig {
    /// Returns the page-content zoom multiplier.
    #[must_use]
    pub const fn scale(self) -> f64 {
        self.scale
    }
}

/// Administrator allowlist for system power operations.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PowerConfig {
    actions: Vec<PowerAction>,
}

impl PowerConfig {
    /// Returns the configured actions in stable protocol order.
    #[must_use]
    pub fn actions(&self) -> &[PowerAction] {
        &self.actions
    }
}

/// Trusted user discovery policy selected by system configuration.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct UserDiscoveryConfig {
    provider: UserProvider,
}

impl UserDiscoveryConfig {
    /// Returns the selected provider policy.
    #[must_use]
    pub const fn provider(self) -> UserProvider {
        self.provider
    }

    /// Returns a policy that performs no system user enumeration.
    #[must_use]
    #[cfg(test)]
    pub const fn disabled() -> Self {
        Self {
            provider: UserProvider::None,
        }
    }

    #[cfg(test)]
    pub(crate) const fn for_test(provider: UserProvider) -> Self {
        Self { provider }
    }
}

/// Available system user discovery policies.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum UserProvider {
    #[default]
    Auto,
    AccountsService,
    Nss,
    None,
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawConfig {
    frontend: Option<RawFrontend>,
    sessions: Option<RawSessions>,
    users: Option<RawUsers>,
    power: Option<RawPower>,
    display: Option<RawDisplay>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawFrontend {
    path: PathBuf,
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSessions {
    wayland_dirs: Option<Vec<PathBuf>>,
    x11_dirs: Option<Vec<PathBuf>>,
    executable_search_paths: Option<Vec<PathBuf>>,
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawUsers {
    provider: Option<UserProvider>,
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPower {
    actions: Option<Vec<PowerAction>>,
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawDisplay {
    scale: Option<f64>,
}

/// Sanitized system configuration failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConfigError {
    Read,
    TooLarge,
    Parse,
    InvalidPath,
    InvalidPowerPolicy,
    InvalidDisplayScale,
}

impl fmt::Display for ConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Read => "the system configuration could not be read",
            Self::TooLarge => "the system configuration exceeds 64 KiB",
            Self::Parse => "the system configuration is invalid TOML",
            Self::InvalidPath => "the system configuration contains an invalid path",
            Self::InvalidPowerPolicy => "the system configuration contains an invalid power policy",
            Self::InvalidDisplayScale => {
                "the system configuration contains an invalid display scale"
            }
        })
    }
}

impl Error for ConfigError {}

fn load_from_path(path: &Path) -> Result<AppConfig, ConfigError> {
    let raw = match read_bounded(path)? {
        Some(raw) => toml::from_str::<RawConfig>(&raw).map_err(|_| ConfigError::Parse)?,
        None => RawConfig::default(),
    };
    validate(raw)
}

fn read_bounded(path: &Path) -> Result<Option<String>, ConfigError> {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(ConfigError::Read),
    };
    if file.metadata().map_err(|_| ConfigError::Read)?.len() > MAX_CONFIG_BYTES {
        return Err(ConfigError::TooLarge);
    }

    let mut bytes = Vec::new();
    file.take(MAX_CONFIG_BYTES.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|_| ConfigError::Read)?;
    if bytes.len() as u64 > MAX_CONFIG_BYTES {
        return Err(ConfigError::TooLarge);
    }
    String::from_utf8(bytes)
        .map(Some)
        .map_err(|_| ConfigError::Parse)
}

fn validate(raw: RawConfig) -> Result<AppConfig, ConfigError> {
    let theme_directory = raw.frontend.map(|frontend| frontend.path);
    if let Some(path) = &theme_directory {
        validate_path(path)?;
    }

    let sessions = raw.sessions.unwrap_or_default();
    let wayland = sessions
        .wayland_dirs
        .unwrap_or_else(|| paths(&DEFAULT_WAYLAND_DIRS));
    let x11 = sessions
        .x11_dirs
        .unwrap_or_else(|| paths(&DEFAULT_X11_DIRS));
    let executable = sessions
        .executable_search_paths
        .unwrap_or_else(|| paths(&DEFAULT_EXECUTABLE_DIRS));
    validate_paths(&wayland)?;
    validate_paths(&x11)?;
    validate_paths(&executable)?;

    let directories = wayland
        .into_iter()
        .map(|path| SessionDirectory::new(path, SessionKind::Wayland))
        .chain(
            x11.into_iter()
                .map(|path| SessionDirectory::new(path, SessionKind::X11)),
        )
        .collect();
    let discovery = DiscoveryConfig::new(directories).with_executable_search_paths(executable);
    let users = UserDiscoveryConfig {
        provider: raw.users.unwrap_or_default().provider.unwrap_or_default(),
    };
    let configured_power = raw.power.unwrap_or_default().actions.unwrap_or_default();
    if configured_power.len() > 3
        || configured_power
            .iter()
            .enumerate()
            .any(|(index, action)| configured_power[..index].contains(action))
    {
        return Err(ConfigError::InvalidPowerPolicy);
    }
    let power = PowerConfig {
        actions: [
            PowerAction::Poweroff,
            PowerAction::Reboot,
            PowerAction::Suspend,
        ]
        .into_iter()
        .filter(|action| configured_power.contains(action))
        .collect(),
    };
    let display_scale = raw.display.unwrap_or_default().scale.unwrap_or(1.0);
    if !display_scale.is_finite()
        || !(MIN_DISPLAY_SCALE..=MAX_DISPLAY_SCALE).contains(&display_scale)
    {
        return Err(ConfigError::InvalidDisplayScale);
    }
    let display = DisplayConfig {
        scale: display_scale,
    };
    Ok(AppConfig {
        theme_directory,
        discovery,
        users,
        power,
        display,
    })
}

fn paths<const N: usize>(values: &[&str; N]) -> Vec<PathBuf> {
    values.iter().map(PathBuf::from).collect()
}

fn validate_paths(paths: &[PathBuf]) -> Result<(), ConfigError> {
    paths.iter().try_for_each(|path| validate_path(path))
}

fn validate_path(path: &Path) -> Result<(), ConfigError> {
    if !path.is_absolute() || path.as_os_str().as_encoded_bytes().contains(&0) {
        return Err(ConfigError::InvalidPath);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{fs, path::Path};

    use fomalhaut_web::protocol::PowerAction;

    use super::{ConfigError, RawConfig, load_from_path, validate};

    #[test]
    fn missing_file_uses_fixed_safe_defaults() {
        let path =
            std::env::temp_dir().join(format!("fomalhaut-missing-config-{}", std::process::id()));
        let config = load_from_path(&path).expect("an absent configuration uses defaults");
        let (theme, discovery, users, power, display) = config.into_parts();
        assert_eq!(theme, None);
        assert_eq!(discovery.directories().len(), 4);
        assert_eq!(users.provider(), super::UserProvider::Auto);
        assert!(power.actions().is_empty());
        assert_eq!(display.scale(), 1.0);
    }

    #[test]
    fn valid_configuration_preserves_explicit_session_priority() {
        let raw = toml::from_str::<RawConfig>(
            r#"
                [frontend]
                path = "/srv/fomalhaut/theme"

                [sessions]
                wayland_dirs = ["/opt/first", "/opt/second"]
                x11_dirs = []
                executable_search_paths = ["/opt/bin"]
            "#,
        )
        .expect("configuration fixture is valid TOML");
        let config = validate(raw).expect("configuration fixture is semantically valid");
        let (theme, discovery, _, _, _) = config.into_parts();
        assert_eq!(theme.as_deref(), Some(Path::new("/srv/fomalhaut/theme")));
        assert_eq!(discovery.directories().len(), 2);
        assert_eq!(discovery.directories()[0].path(), Path::new("/opt/first"));
        assert_eq!(discovery.directories()[1].path(), Path::new("/opt/second"));
    }

    #[test]
    fn user_provider_is_strict_and_can_disable_enumeration() {
        let raw = toml::from_str::<RawConfig>(
            r#"
                [users]
                provider = "none"
            "#,
        )
        .expect("user provider fixture is valid TOML");
        let config = validate(raw).expect("user provider fixture is valid");
        let (_, _, users, _, _) = config.into_parts();
        assert_eq!(users.provider(), super::UserProvider::None);

        assert!(
            toml::from_str::<RawConfig>(
                r#"
                    [users]
                    provider = "passwd"
                "#,
            )
            .is_err()
        );
    }

    #[test]
    fn power_policy_is_strict_unique_and_stably_ordered() {
        let raw = toml::from_str::<RawConfig>(
            r#"
                [power]
                actions = ["suspend", "poweroff"]
            "#,
        )
        .expect("power policy fixture is valid TOML");
        let config = validate(raw).expect("power policy fixture is valid");
        let (_, _, _, power, _) = config.into_parts();
        assert_eq!(
            power.actions(),
            &[PowerAction::Poweroff, PowerAction::Suspend]
        );

        let duplicate = toml::from_str::<RawConfig>(
            r#"
                [power]
                actions = ["reboot", "reboot"]
            "#,
        )
        .expect("duplicate policy is syntactically valid TOML");
        assert_eq!(
            validate(duplicate).err(),
            Some(ConfigError::InvalidPowerPolicy)
        );
        assert!(
            toml::from_str::<RawConfig>(
                r#"
                    [power]
                    actions = ["hibernate"]
                "#,
            )
            .is_err()
        );
    }

    #[test]
    fn display_scale_accepts_fractional_zoom_and_rejects_unsafe_values() {
        let raw = toml::from_str::<RawConfig>(
            r#"
                [display]
                scale = 1.5
            "#,
        )
        .expect("fractional display scale is valid TOML");
        let config = validate(raw).expect("fractional display scale is within bounds");
        let (_, _, _, _, display) = config.into_parts();
        assert_eq!(display.scale(), 1.5);

        for scale in ["0.49", "4.01", "nan", "+inf", "-inf"] {
            let raw = toml::from_str::<RawConfig>(&format!("[display]\nscale = {scale}\n"))
                .expect("invalid display scale fixture is valid TOML");
            assert_eq!(validate(raw).err(), Some(ConfigError::InvalidDisplayScale));
        }
    }

    #[test]
    fn unknown_fields_and_relative_paths_are_rejected() {
        assert!(toml::from_str::<RawConfig>("network = true").is_err());
        let raw = toml::from_str::<RawConfig>(
            r#"
                [frontend]
                path = "relative/theme"
            "#,
        )
        .expect("relative path is syntactically valid");
        assert_eq!(
            validate(raw).err(),
            Some(ConfigError::InvalidPath),
            "relative paths must fail semantic validation"
        );
    }

    #[test]
    fn oversized_configuration_is_rejected_before_toml_parsing() {
        let path =
            std::env::temp_dir().join(format!("fomalhaut-large-config-{}", std::process::id()));
        fs::write(&path, vec![b' '; 64 * 1024 + 1])
            .expect("oversized configuration fixture can be written");
        assert_eq!(load_from_path(&path).err(), Some(ConfigError::TooLarge));
        fs::remove_file(path).expect("configuration fixture can be removed");
    }
}
