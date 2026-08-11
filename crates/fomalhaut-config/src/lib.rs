//! Strict shared configuration for Fomalhaut greeter and locker hosts.

use std::{
    error::Error,
    fmt,
    fs::File,
    io::{self, Read},
    path::{Path, PathBuf},
};

use fomalhaut_session::{DiscoveryConfig, SessionDirectory, SessionKind};
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

/// Fully validated global configuration before role capabilities are narrowed.
pub struct AppConfig {
    themes: ThemeConfig,
    discovery: DiscoveryConfig,
    users: UserDiscoveryConfig,
    power: PowerConfig,
    display: RoleDisplayConfig,
    locale: UiLocale,
}

impl AppConfig {
    /// Loads the fixed system configuration or safe defaults when it is absent.
    pub fn load() -> Result<Self, ConfigError> {
        load_from_path(Path::new(CONFIG_PATH))
    }

    /// Produces the greeter-only configuration capability view.
    #[must_use]
    pub fn for_greeter(&self) -> GreeterConfig {
        GreeterConfig {
            theme_directory: self.themes.for_greeter(),
            discovery: self.discovery.clone(),
            users: self.users,
            power: self.power.clone(),
            display: self.display.greeter,
            locale: self.locale,
        }
    }

    /// Produces the locker-only configuration capability view.
    #[must_use]
    pub fn for_locker(&self) -> LockerConfig {
        LockerConfig {
            theme_directory: self.themes.for_locker(),
            power: self.power.clone(),
            display: self.display.locker,
            locale: self.locale,
        }
    }
}

/// Validated configuration exposed to the greeter host.
pub struct GreeterConfig {
    theme_directory: Option<PathBuf>,
    discovery: DiscoveryConfig,
    users: UserDiscoveryConfig,
    power: PowerConfig,
    display: DisplayConfig,
    locale: UiLocale,
}

impl GreeterConfig {
    /// Consumes the greeter view into trusted host inputs.
    #[must_use]
    pub fn into_parts(
        self,
    ) -> (
        Option<PathBuf>,
        DiscoveryConfig,
        UserDiscoveryConfig,
        PowerConfig,
        DisplayConfig,
        UiLocale,
    ) {
        (
            self.theme_directory,
            self.discovery,
            self.users,
            self.power,
            self.display,
            self.locale,
        )
    }
}

/// Validated configuration exposed to the locker host.
pub struct LockerConfig {
    theme_directory: Option<PathBuf>,
    power: PowerConfig,
    display: DisplayConfig,
    locale: UiLocale,
}

impl LockerConfig {
    /// Returns the selected locker theme directory, or `None` for the embedded theme.
    #[must_use]
    pub fn theme_directory(&self) -> Option<&Path> {
        self.theme_directory.as_deref()
    }

    /// Returns the configured power allowlist.
    #[must_use]
    pub const fn power(&self) -> &PowerConfig {
        &self.power
    }

    /// Returns WebKit presentation settings.
    #[must_use]
    pub const fn display(&self) -> DisplayConfig {
        self.display
    }

    /// Returns the resolved UI locale shared with the locker frontend.
    #[must_use]
    pub const fn locale(&self) -> UiLocale {
        self.locale
    }
}

/// UI language resolved from strict configuration or the process locale.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
pub enum UiLocale {
    /// English UI strings.
    #[serde(rename = "en")]
    En,
    /// Simplified Chinese UI strings.
    #[serde(rename = "zh-CN")]
    ZhCn,
}

impl UiLocale {
    /// Returns the stable BCP 47 identifier exposed to themes.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::En => "en",
            Self::ZhCn => "zh-CN",
        }
    }

    /// Returns Desktop Entry locale suffixes in lookup priority order.
    #[must_use]
    pub const fn desktop_entry_locales(self) -> &'static [&'static str] {
        match self {
            Self::En => &["en"],
            Self::ZhCn => &["zh_CN", "zh"],
        }
    }
}

#[derive(Default)]
struct ThemeConfig {
    default: Option<PathBuf>,
    greeter: Option<PathBuf>,
    locker: Option<PathBuf>,
}

impl ThemeConfig {
    fn for_greeter(&self) -> Option<PathBuf> {
        self.greeter.clone().or_else(|| self.default.clone())
    }

    fn for_locker(&self) -> Option<PathBuf> {
        self.locker.clone().or_else(|| self.default.clone())
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

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct RoleDisplayConfig {
    greeter: DisplayConfig,
    locker: DisplayConfig,
}

/// Power operation accepted by the strict system configuration.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum PowerAction {
    /// Power off the system.
    Poweroff,
    /// Reboot the system.
    Reboot,
    /// Suspend the system.
    Suspend,
}

/// Administrator allowlist for system power operations.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PowerConfig {
    actions: Vec<PowerAction>,
}

impl PowerConfig {
    /// Returns configured actions in stable poweroff/reboot/suspend order.
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
    /// Constructs an explicit user discovery policy.
    #[must_use]
    pub const fn new(provider: UserProvider) -> Self {
        Self { provider }
    }

    /// Returns the selected provider policy.
    #[must_use]
    pub const fn provider(self) -> UserProvider {
        self.provider
    }

    /// Returns a policy that performs no system user enumeration.
    #[must_use]
    pub const fn disabled() -> Self {
        Self::new(UserProvider::None)
    }
}

/// Available system user discovery policies.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum UserProvider {
    /// Prefer AccountsService and use the constrained NSS fallback when unavailable.
    #[default]
    Auto,
    /// Only query AccountsService.
    AccountsService,
    /// Only query the constrained NSS provider.
    Nss,
    /// Disable user enumeration.
    None,
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawConfig {
    themes: Option<RawThemes>,
    sessions: Option<RawSessions>,
    users: Option<RawUsers>,
    power: Option<RawPower>,
    display: Option<RawDisplay>,
    locale: Option<RawLocale>,
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawLocale {
    language: Option<UiLocale>,
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawThemes {
    default: Option<PathBuf>,
    greeter: Option<PathBuf>,
    locker: Option<PathBuf>,
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
    scale: Option<RawDisplayScale>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum RawDisplayScale {
    Shared(f64),
    Roles(RawRoleDisplayScale),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRoleDisplayScale {
    greeter: f64,
    locker: f64,
}

/// Sanitized system configuration failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConfigError {
    /// The fixed configuration path could not be read.
    Read,
    /// The configuration exceeded the fixed input bound.
    TooLarge,
    /// TOML syntax, UTF-8, or a strict field failed to parse.
    Parse,
    /// A configured path was relative or contained NUL.
    InvalidPath,
    /// A power action was duplicated or exceeded the fixed action set.
    InvalidPowerPolicy,
    /// Display scale was not finite or outside the supported range.
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
    validate_with_detected_locale(raw, detect_ui_locale_from_environment())
}

fn validate_with_detected_locale(
    raw: RawConfig,
    detected_locale: UiLocale,
) -> Result<AppConfig, ConfigError> {
    let themes = match raw.themes {
        Some(themes) => ThemeConfig {
            default: themes.default,
            greeter: themes.greeter,
            locker: themes.locker,
        },
        None => ThemeConfig::default(),
    };
    validate_optional_path(themes.default.as_deref())?;
    validate_optional_path(themes.greeter.as_deref())?;
    validate_optional_path(themes.locker.as_deref())?;

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
    let locale = raw
        .locale
        .unwrap_or_default()
        .language
        .unwrap_or(detected_locale);
    let discovery = DiscoveryConfig::new(directories)
        .with_executable_search_paths(executable)
        .with_locales(
            locale
                .desktop_entry_locales()
                .iter()
                .map(ToString::to_string)
                .collect(),
        );
    let users =
        UserDiscoveryConfig::new(raw.users.unwrap_or_default().provider.unwrap_or_default());
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
    let (greeter_scale, locker_scale) = match raw.display.unwrap_or_default().scale {
        None => (1.0, 1.0),
        Some(RawDisplayScale::Shared(scale)) => (scale, scale),
        Some(RawDisplayScale::Roles(roles)) => (roles.greeter, roles.locker),
    };
    let valid_scale =
        |scale: f64| scale.is_finite() && (MIN_DISPLAY_SCALE..=MAX_DISPLAY_SCALE).contains(&scale);
    if !valid_scale(greeter_scale) || !valid_scale(locker_scale) {
        return Err(ConfigError::InvalidDisplayScale);
    }
    let display = RoleDisplayConfig {
        greeter: DisplayConfig {
            scale: greeter_scale,
        },
        locker: DisplayConfig {
            scale: locker_scale,
        },
    };
    Ok(AppConfig {
        themes,
        discovery,
        users,
        power,
        display,
        locale,
    })
}

fn detect_ui_locale_from_environment() -> UiLocale {
    for name in ["LC_ALL", "LC_MESSAGES", "LANG"] {
        let Some(value) = std::env::var_os(name) else {
            continue;
        };
        if value.is_empty() {
            continue;
        }
        return value.to_str().map_or(UiLocale::En, ui_locale_from_posix);
    }
    UiLocale::En
}

#[cfg(test)]
fn ui_locale_from_candidates<T: AsRef<str>>(
    candidates: impl IntoIterator<Item = Option<T>>,
) -> UiLocale {
    candidates
        .into_iter()
        .flatten()
        .find(|value| !value.as_ref().trim().is_empty())
        .map_or(UiLocale::En, |value| ui_locale_from_posix(value.as_ref()))
}

fn ui_locale_from_posix(value: &str) -> UiLocale {
    let normalized = value
        .trim()
        .split(['.', '@'])
        .next()
        .unwrap_or_default()
        .replace('_', "-")
        .to_ascii_lowercase();
    if normalized == "zh" || normalized.starts_with("zh-") {
        UiLocale::ZhCn
    } else {
        UiLocale::En
    }
}

fn paths<const N: usize>(values: &[&str; N]) -> Vec<PathBuf> {
    values.iter().map(PathBuf::from).collect()
}

fn validate_optional_path(path: Option<&Path>) -> Result<(), ConfigError> {
    path.map_or(Ok(()), validate_path)
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

    use super::{
        ConfigError, PowerAction, RawConfig, UiLocale, UserProvider, load_from_path,
        ui_locale_from_candidates, ui_locale_from_posix, validate_with_detected_locale,
    };

    fn parse(input: &str) -> Result<super::AppConfig, ConfigError> {
        let raw = toml::from_str::<RawConfig>(input).map_err(|_| ConfigError::Parse)?;
        validate_with_detected_locale(raw, UiLocale::En)
    }

    #[test]
    fn missing_file_uses_fixed_safe_defaults_for_both_roles() {
        let path =
            std::env::temp_dir().join(format!("fomalhaut-missing-config-{}", std::process::id()));
        let config = load_from_path(&path).expect("an absent configuration uses defaults");
        let (theme, discovery, users, power, display, locale) = config.for_greeter().into_parts();
        let locker = config.for_locker();

        assert_eq!(theme, None);
        assert_eq!(discovery.directories().len(), 4);
        assert_eq!(users.provider(), UserProvider::Auto);
        assert!(power.actions().is_empty());
        assert_eq!(display.scale(), 1.0);
        assert!(matches!(locale, UiLocale::En | UiLocale::ZhCn));
        assert_eq!(locker.theme_directory(), None);
        assert!(locker.power().actions().is_empty());
        assert_eq!(locker.display().scale(), 1.0);
    }

    #[test]
    fn role_theme_overrides_take_priority_over_default() {
        let config = parse(
            r#"
                [themes]
                default = "/srv/fomalhaut/default"
                greeter = "/srv/fomalhaut/greeter"
                locker = "/srv/fomalhaut/locker"
            "#,
        )
        .expect("theme configuration is valid");
        let (greeter_theme, _, _, _, _, _) = config.for_greeter().into_parts();

        assert_eq!(
            greeter_theme.as_deref(),
            Some(Path::new("/srv/fomalhaut/greeter"))
        );
        assert_eq!(
            config.for_locker().theme_directory(),
            Some(Path::new("/srv/fomalhaut/locker"))
        );

        let default_only = parse(
            r#"
                [themes]
                default = "/srv/fomalhaut/default"
            "#,
        )
        .expect("default theme configuration is valid");
        let (greeter_theme, _, _, _, _, _) = default_only.for_greeter().into_parts();
        assert_eq!(
            greeter_theme.as_deref(),
            Some(Path::new("/srv/fomalhaut/default"))
        );
        assert_eq!(
            default_only.for_locker().theme_directory(),
            Some(Path::new("/srv/fomalhaut/default"))
        );
    }

    #[test]
    fn legacy_frontend_is_rejected_as_an_unknown_field() {
        assert_eq!(
            parse(
                r#"
                [frontend]
                path = "/srv/fomalhaut/legacy"
            "#,
            )
            .err(),
            Some(ConfigError::Parse)
        );
    }

    #[test]
    fn explicit_session_priority_is_preserved_for_greeter() {
        let config = parse(
            r#"
                [themes]
                default = "/srv/fomalhaut/theme"

                [sessions]
                wayland_dirs = ["/opt/first", "/opt/second"]
                x11_dirs = []
                executable_search_paths = ["/opt/bin"]
            "#,
        )
        .expect("configuration fixture is semantically valid");
        let (theme, discovery, _, _, _, _) = config.for_greeter().into_parts();
        assert_eq!(theme.as_deref(), Some(Path::new("/srv/fomalhaut/theme")));
        assert_eq!(discovery.directories().len(), 2);
        assert_eq!(discovery.directories()[0].path(), Path::new("/opt/first"));
        assert_eq!(discovery.directories()[1].path(), Path::new("/opt/second"));
    }

    #[test]
    fn user_provider_is_strict_and_can_disable_enumeration() {
        let config = parse(
            r#"
                [users]
                provider = "none"
            "#,
        )
        .expect("user provider fixture is valid");
        let (_, _, users, _, _, _) = config.for_greeter().into_parts();
        assert_eq!(users.provider(), UserProvider::None);

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
        let config = parse(
            r#"
                [power]
                actions = ["suspend", "poweroff"]
            "#,
        )
        .expect("power policy fixture is valid");
        let (_, _, _, power, _, _) = config.for_greeter().into_parts();
        assert_eq!(
            power.actions(),
            &[PowerAction::Poweroff, PowerAction::Suspend]
        );

        assert_eq!(
            parse(
                r#"
                    [power]
                    actions = ["reboot", "reboot"]
                "#,
            )
            .err(),
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
        let config = parse(
            r#"
                [display]
                scale = 1.5
            "#,
        )
        .expect("fractional display scale is within bounds");
        let (_, _, _, _, display, _) = config.for_greeter().into_parts();
        assert_eq!(display.scale(), 1.5);
        assert_eq!(config.for_locker().display().scale(), 1.5);

        let roles = parse(
            r#"
                [display]
                scale.greeter = 1.5
                scale.locker = 1.0
            "#,
        )
        .expect("role-specific display scales are valid dotted keys");
        let (_, _, _, _, greeter_display, _) = roles.for_greeter().into_parts();
        assert_eq!(greeter_display.scale(), 1.5);
        assert_eq!(roles.for_locker().display().scale(), 1.0);

        let role_table = parse(
            r#"
                [display.scale]
                greeter = 2.0
                locker = 1.25
            "#,
        )
        .expect("an explicit role scale table is equivalent to dotted keys");
        let (_, _, _, _, greeter_display, _) = role_table.for_greeter().into_parts();
        assert_eq!(greeter_display.scale(), 2.0);
        assert_eq!(role_table.for_locker().display().scale(), 1.25);

        for scale in ["0.49", "4.01", "nan", "+inf", "-inf"] {
            assert_eq!(
                parse(&format!("[display]\nscale = {scale}\n")).err(),
                Some(ConfigError::InvalidDisplayScale)
            );
            assert_eq!(
                parse(&format!(
                    "[display]\nscale.greeter = 1.0\nscale.locker = {scale}\n"
                ))
                .err(),
                Some(ConfigError::InvalidDisplayScale)
            );
        }

        for invalid in [
            "[display]\nscale.greeter = 1.5\n",
            "[display]\nscale.greeter = 1.5\nscale.locker = 1.0\nscale.other = 1.0\n",
            "[display]\nscale = 1.5\nscale.greeter = 1.5\n",
        ] {
            assert_eq!(parse(invalid).err(), Some(ConfigError::Parse));
        }
    }

    #[test]
    fn locale_override_is_strict_and_shared_by_both_roles() {
        let config = parse("[locale]\nlanguage = \"zh-CN\"\n")
            .expect("a supported locale override is valid");
        let (_, discovery, _, _, _, greeter_locale) = config.for_greeter().into_parts();

        assert_eq!(greeter_locale, UiLocale::ZhCn);
        assert_eq!(config.for_locker().locale(), UiLocale::ZhCn);
        assert_eq!(
            discovery,
            discovery
                .clone()
                .with_locales(vec!["zh_CN".to_owned(), "zh".to_owned()])
        );
        assert_eq!(
            parse("[locale]\nlanguage = \"fr\"\n").err(),
            Some(ConfigError::Parse)
        );
    }

    #[test]
    fn posix_locale_detection_normalizes_chinese_and_falls_back_to_english() {
        for value in ["zh_CN.UTF-8", "zh-TW@variant", " zh_Hans ", "zh"] {
            assert_eq!(ui_locale_from_posix(value), UiLocale::ZhCn);
        }
        for value in ["en_US.UTF-8", "C.UTF-8", "POSIX", "fr_FR", ""] {
            assert_eq!(ui_locale_from_posix(value), UiLocale::En);
        }

        let raw = toml::from_str::<RawConfig>("")
            .expect("an empty configuration is a valid detection fixture");
        let config = validate_with_detected_locale(raw, UiLocale::ZhCn)
            .expect("a detected locale is accepted without an override");
        let (_, _, _, _, _, locale) = config.for_greeter().into_parts();
        assert_eq!(locale, UiLocale::ZhCn);

        assert_eq!(
            ui_locale_from_candidates([Some("en_US.UTF-8"), Some("zh_CN"), None]),
            UiLocale::En
        );
        assert_eq!(
            ui_locale_from_candidates([Some(""), Some("zh_TW.UTF-8"), Some("en_US")]),
            UiLocale::ZhCn
        );
    }

    #[test]
    fn unknown_fields_and_relative_paths_are_rejected() {
        assert!(toml::from_str::<RawConfig>("network = true").is_err());
        assert_eq!(
            parse(
                r#"
                    [themes]
                    locker = "relative/theme"
                "#,
            )
            .err(),
            Some(ConfigError::InvalidPath)
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
