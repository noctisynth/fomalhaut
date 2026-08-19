//! Capability-confined external theme manifests and resources.

use std::{
    error::Error,
    fmt,
    io::{self, Read},
    path::{Path, PathBuf},
};

use cap_std::{ambient_authority, fs::Dir};
use serde::Deserialize;

use crate::{assets::resolve_builtin_asset, protocol::PROTOCOL_VERSION};

const THEME_URI_PREFIX: &str = "fomalhaut://theme/";
const THEME_MANIFEST: &str = "theme.toml";
const MAX_MANIFEST_BYTES: u64 = 16 * 1024;
const MAX_ASSET_BYTES: u64 = 8 * 1024 * 1024;
const MAX_RESOURCE_PATH_BYTES: usize = 4096;
const MAX_THEME_ID_BYTES: usize = 64;
const THEME_SEARCH_ROOTS: [&str; 2] = [
    "/usr/local/share/fomalhaut/themes",
    "/usr/share/fomalhaut/themes",
];

/// One deterministically selected theme directory and any lower-priority matches.
pub struct DiscoveredTheme {
    directory: PathBuf,
    conflicts: Vec<PathBuf>,
}

impl DiscoveredTheme {
    /// Returns the selected absolute theme directory.
    #[must_use]
    pub fn directory(&self) -> &Path {
        &self.directory
    }

    /// Returns lower-priority directories that declared the same stable ID.
    #[must_use]
    pub fn conflicts(&self) -> &[PathBuf] {
        &self.conflicts
    }

    /// Consumes the discovery result and returns the selected directory.
    #[must_use]
    pub fn into_directory(self) -> PathBuf {
        self.directory
    }
}

/// Discovers a stable theme ID in fixed local and system package roots.
pub fn discover_theme(id: &str) -> Result<DiscoveredTheme, ThemeError> {
    let roots = THEME_SEARCH_ROOTS.map(PathBuf::from);
    discover_theme_in(id, &roots)
}

fn discover_theme_in(id: &str, roots: &[PathBuf]) -> Result<DiscoveredTheme, ThemeError> {
    validate_theme_id(id)?;
    let mut matches = Vec::new();
    for root_path in roots {
        let root = match Dir::open_ambient_dir(root_path, ambient_authority()) {
            Ok(root) => root,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(_) => return Err(ThemeError::DiscoveryUnavailable),
        };
        let entries = root
            .entries()
            .map_err(|_| ThemeError::DiscoveryUnavailable)?;
        let mut candidates = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|_| ThemeError::DiscoveryUnavailable)?;
            candidates.push(entry.file_name());
        }
        candidates.sort();
        for candidate in candidates {
            let directory = match root.open_dir(&candidate) {
                Ok(directory) => directory,
                Err(_) => continue,
            };
            let candidate_id = match read_manifest_id(&directory) {
                Ok(candidate_id) => candidate_id,
                Err(_) => continue,
            };
            if candidate_id == id {
                matches.push(root_path.join(candidate));
            }
        }
    }
    let directory = matches.first().cloned().ok_or(ThemeError::ThemeNotFound)?;
    Ok(DiscoveredTheme {
        directory,
        conflicts: matches.into_iter().skip(1).collect(),
    })
}

/// One resource body with a host-selected MIME type.
pub struct ThemeAsset {
    body: Vec<u8>,
    content_type: &'static str,
}

impl ThemeAsset {
    /// Consumes the resource into its owned body and fixed MIME type.
    #[must_use]
    pub fn into_parts(self) -> (Vec<u8>, &'static str) {
        (self.body, self.content_type)
    }
}

/// Runtime theme selected by trusted host configuration.
pub enum ThemeSource {
    Embedded,
    External(ExternalTheme),
}

impl ThemeSource {
    /// Opens and validates a configured external theme directory.
    pub fn external(root: PathBuf) -> Result<Self, ThemeError> {
        ExternalTheme::open(root).map(Self::External)
    }

    /// Returns whether a URI is the sole top-level entry allowed for this theme.
    #[must_use]
    pub fn allows_navigation(&self, uri: &str) -> bool {
        match self {
            Self::Embedded => matches!(uri, "fomalhaut://theme/" | "fomalhaut://theme/index.html"),
            Self::External(theme) => theme.allows_navigation(uri),
        }
    }

    /// Returns whether a URI has valid theme syntax and an allowlisted media type.
    #[must_use]
    pub fn allows_resource_uri(&self, uri: &str) -> bool {
        match self {
            Self::Embedded => resolve_builtin_asset(uri).is_some(),
            Self::External(theme) => theme.allows_resource_uri(uri),
        }
    }

    /// Resolves one exact theme URI to an owned, bounded resource.
    pub fn resolve(&self, uri: &str) -> Result<Option<ThemeAsset>, ThemeError> {
        match self {
            Self::Embedded => Ok(resolve_builtin_asset(uri).map(|asset| ThemeAsset {
                body: asset.body().to_vec(),
                content_type: asset.content_type(),
            })),
            Self::External(theme) => theme.resolve(uri),
        }
    }
}

/// Validated external theme rooted at an open directory capability.
pub struct ExternalTheme {
    root: Dir,
    entrypoint: ThemePath,
}

impl ExternalTheme {
    fn open(root: PathBuf) -> Result<Self, ThemeError> {
        if !root.is_absolute() {
            return Err(ThemeError::InvalidRoot);
        }
        let directory = Dir::open_ambient_dir(root, ambient_authority())
            .map_err(|_| ThemeError::InvalidRoot)?;
        let manifest = read_manifest(&directory)?;
        validate_theme_id(&manifest.theme.id)?;
        validate_theme_name(&manifest.theme.name)?;
        if manifest.theme.protocol != PROTOCOL_VERSION {
            return Err(ThemeError::UnsupportedProtocol);
        }
        let entrypoint = ThemePath::parse(&manifest.theme.entrypoint)?;
        let content_type = mime_type(&entrypoint).ok_or(ThemeError::UnsupportedMediaType)?;
        if content_type != "text/html; charset=utf-8" {
            return Err(ThemeError::InvalidEntrypoint);
        }
        let entrypoint_body = read_file(&directory, &entrypoint, MAX_ASSET_BYTES)
            .map_err(|_| ThemeError::InvalidEntrypoint)?;
        std::str::from_utf8(&entrypoint_body).map_err(|_| ThemeError::InvalidEntrypoint)?;
        Ok(Self {
            root: directory,
            entrypoint,
        })
    }

    fn allows_navigation(&self, uri: &str) -> bool {
        if uri == THEME_URI_PREFIX {
            return true;
        }
        uri.strip_prefix(THEME_URI_PREFIX)
            .and_then(|path| ThemePath::parse(path).ok())
            .is_some_and(|path| path == self.entrypoint)
    }

    fn resolve(&self, uri: &str) -> Result<Option<ThemeAsset>, ThemeError> {
        let Some(path) = uri.strip_prefix(THEME_URI_PREFIX) else {
            return Ok(None);
        };
        let path = if path.is_empty() {
            self.entrypoint.clone()
        } else {
            match ThemePath::parse(path) {
                Ok(path) => path,
                Err(_) => return Ok(None),
            }
        };
        let Some(content_type) = mime_type(&path) else {
            return Ok(None);
        };
        let body = match read_file(&self.root, &path, MAX_ASSET_BYTES) {
            Ok(body) => body,
            Err(ThemeError::ResourceUnavailable) => return Ok(None),
            Err(error) => return Err(error),
        };
        Ok(Some(ThemeAsset { body, content_type }))
    }

    fn allows_resource_uri(&self, uri: &str) -> bool {
        let Some(path) = uri.strip_prefix(THEME_URI_PREFIX) else {
            return false;
        };
        let path = if path.is_empty() {
            self.entrypoint.clone()
        } else {
            match ThemePath::parse(path) {
                Ok(path) => path,
                Err(_) => return false,
            }
        };
        mime_type(&path).is_some()
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ManifestDocument {
    theme: RawTheme,
}

#[derive(Deserialize)]
struct ManifestIdDocument {
    theme: ManifestId,
}

#[derive(Deserialize)]
struct ManifestId {
    id: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawTheme {
    id: String,
    name: String,
    protocol: u16,
    entrypoint: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ThemePath {
    path: PathBuf,
}

impl ThemePath {
    fn manifest() -> Self {
        Self {
            path: PathBuf::from(THEME_MANIFEST),
        }
    }

    fn parse(value: &str) -> Result<Self, ThemeError> {
        if value.is_empty()
            || value.len() > MAX_RESOURCE_PATH_BYTES
            || value.starts_with('/')
            || value.ends_with('/')
            || value.contains(['\\', '%', '?', '#', '\0'])
            || !value.bytes().all(|byte| {
                byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'/')
            })
            || value
                .split('/')
                .any(|segment| segment.is_empty() || matches!(segment, "." | ".."))
        {
            return Err(ThemeError::InvalidResourcePath);
        }
        Ok(Self {
            path: PathBuf::from(value),
        })
    }
}

/// Sanitized external-theme validation or resource failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ThemeError {
    InvalidRoot,
    InvalidThemeId,
    ThemeNotFound,
    DiscoveryUnavailable,
    InvalidManifest,
    ManifestTooLarge,
    UnsupportedProtocol,
    InvalidEntrypoint,
    InvalidResourcePath,
    UnsupportedMediaType,
    ResourceUnavailable,
    ResourceTooLarge,
}

impl fmt::Display for ThemeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidRoot => "the external theme root is invalid",
            Self::InvalidThemeId => "the external theme identifier is invalid",
            Self::ThemeNotFound => "the configured theme identifier was not found",
            Self::DiscoveryUnavailable => "the system theme roots could not be enumerated",
            Self::InvalidManifest => "the external theme manifest is invalid",
            Self::ManifestTooLarge => "the external theme manifest exceeds 16 KiB",
            Self::UnsupportedProtocol => "the external theme protocol is unsupported",
            Self::InvalidEntrypoint => "the external theme entrypoint is invalid",
            Self::InvalidResourcePath => "the external theme resource path is invalid",
            Self::UnsupportedMediaType => "the external theme resource type is unsupported",
            Self::ResourceUnavailable => "the external theme resource is unavailable",
            Self::ResourceTooLarge => "the external theme resource exceeds 8 MiB",
        })
    }
}

impl Error for ThemeError {}

fn validate_theme_name(name: &str) -> Result<(), ThemeError> {
    if name.is_empty()
        || name.len() > 256
        || name.chars().any(char::is_control)
        || name.contains('\0')
    {
        return Err(ThemeError::InvalidManifest);
    }
    Ok(())
}

fn validate_theme_id(id: &str) -> Result<(), ThemeError> {
    if id.is_empty()
        || id.len() > MAX_THEME_ID_BYTES
        || !id.split('-').all(|segment| {
            !segment.is_empty()
                && segment
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        })
    {
        return Err(ThemeError::InvalidThemeId);
    }
    Ok(())
}

fn read_manifest(root: &Dir) -> Result<ManifestDocument, ThemeError> {
    let manifest = read_manifest_text(root)?;
    toml::from_str(&manifest).map_err(|_| ThemeError::InvalidManifest)
}

fn read_manifest_id(root: &Dir) -> Result<String, ThemeError> {
    let manifest = read_manifest_text(root)?;
    let manifest: ManifestIdDocument =
        toml::from_str(&manifest).map_err(|_| ThemeError::InvalidManifest)?;
    Ok(manifest.theme.id)
}

fn read_manifest_text(root: &Dir) -> Result<String, ThemeError> {
    let manifest = read_file(root, &ThemePath::manifest(), MAX_MANIFEST_BYTES).map_err(
        |error| match error {
            ThemeError::ResourceTooLarge => ThemeError::ManifestTooLarge,
            _ => ThemeError::InvalidManifest,
        },
    )?;
    String::from_utf8(manifest).map_err(|_| ThemeError::InvalidManifest)
}

fn read_file(root: &Dir, path: &ThemePath, limit: u64) -> Result<Vec<u8>, ThemeError> {
    let file = root
        .open(&path.path)
        .map_err(|_| ThemeError::ResourceUnavailable)?;
    let metadata = file
        .metadata()
        .map_err(|_| ThemeError::ResourceUnavailable)?;
    if !metadata.is_file() {
        return Err(ThemeError::ResourceUnavailable);
    }
    if metadata.len() > limit {
        return Err(ThemeError::ResourceTooLarge);
    }
    let mut body = Vec::new();
    file.take(limit.saturating_add(1))
        .read_to_end(&mut body)
        .map_err(|_| ThemeError::ResourceUnavailable)?;
    if body.len() as u64 > limit {
        return Err(ThemeError::ResourceTooLarge);
    }
    Ok(body)
}

fn mime_type(path: &ThemePath) -> Option<&'static str> {
    let extension = path.path.extension()?.to_str()?;
    if extension.eq_ignore_ascii_case("html") || extension.eq_ignore_ascii_case("htm") {
        Some("text/html; charset=utf-8")
    } else if extension.eq_ignore_ascii_case("css") {
        Some("text/css; charset=utf-8")
    } else if extension.eq_ignore_ascii_case("js") || extension.eq_ignore_ascii_case("mjs") {
        Some("application/javascript")
    } else if extension.eq_ignore_ascii_case("json") {
        Some("application/json")
    } else if extension.eq_ignore_ascii_case("svg") {
        Some("image/svg+xml")
    } else if extension.eq_ignore_ascii_case("png") {
        Some("image/png")
    } else if extension.eq_ignore_ascii_case("jpg") || extension.eq_ignore_ascii_case("jpeg") {
        Some("image/jpeg")
    } else if extension.eq_ignore_ascii_case("gif") {
        Some("image/gif")
    } else if extension.eq_ignore_ascii_case("webp") {
        Some("image/webp")
    } else if extension.eq_ignore_ascii_case("ico") {
        Some("image/x-icon")
    } else if extension.eq_ignore_ascii_case("woff") {
        Some("font/woff")
    } else if extension.eq_ignore_ascii_case("woff2") {
        Some("font/woff2")
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        os::unix::fs::symlink,
        path::{Path, PathBuf},
        sync::atomic::{AtomicU64, Ordering},
    };

    use super::{MAX_ASSET_BYTES, MAX_MANIFEST_BYTES, ThemeError, ThemeSource, discover_theme_in};

    static FIXTURE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

    fn fixture(name: &str) -> PathBuf {
        let sequence = FIXTURE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "fomalhaut-theme-{}-{sequence}-{name}",
            std::process::id()
        ));
        fs::create_dir_all(root.join("assets"))
            .expect("external theme fixture directory can be created");
        fs::write(
            root.join("theme.toml"),
            "[theme]\nid = \"fixture\"\nname = \"Fixture\"\nprotocol = 1\nentrypoint = \"index.html\"\n",
        )
        .expect("external theme manifest can be written");
        fs::write(root.join("index.html"), "<!doctype html>")
            .expect("external theme entrypoint can be written");
        fs::write(root.join("assets/app.js"), "'use strict';")
            .expect("external theme script can be written");
        root
    }

    fn cleanup(path: &Path) {
        fs::remove_dir_all(path).expect("external theme fixture can be removed");
    }

    fn discovery_root(name: &str) -> PathBuf {
        let sequence = FIXTURE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "fomalhaut-theme-discovery-{}-{sequence}-{name}",
            std::process::id()
        ));
        fs::create_dir_all(&root).expect("theme discovery root can be created");
        root
    }

    fn discovery_theme(root: &Path, directory: &str, id: &str, with_entrypoint: bool) -> PathBuf {
        let theme = root.join(directory);
        fs::create_dir_all(&theme).expect("discovered theme directory can be created");
        fs::write(
            theme.join("theme.toml"),
            format!(
                "[theme]\nid = {id:?}\nname = \"Discovery fixture\"\nprotocol = 1\nentrypoint = \"index.html\"\n"
            ),
        )
        .expect("discovered theme manifest can be written");
        if with_entrypoint {
            fs::write(theme.join("index.html"), "<!doctype html>")
                .expect("discovered theme entrypoint can be written");
        }
        theme
    }

    #[test]
    fn discovery_prefers_local_root_then_lexical_directory_order() {
        let local = discovery_root("local-priority");
        let system = discovery_root("system-priority");
        let local_second = discovery_theme(&local, "z-second", "nocturne", true);
        let system_match = discovery_theme(&system, "a-system", "nocturne", true);
        let local_first = discovery_theme(&local, "a-first", "nocturne", true);

        let discovered = discover_theme_in("nocturne", &[local.clone(), system.clone()])
            .expect("a matching theme is discovered");

        assert_eq!(discovered.directory(), local_first);
        assert_eq!(discovered.conflicts(), &[local_second, system_match]);
        cleanup(&local);
        cleanup(&system);
    }

    #[test]
    fn discovery_ignores_missing_roots_and_unrelated_invalid_manifests() {
        let missing = discovery_root("missing");
        cleanup(&missing);
        let system = discovery_root("valid-system");
        fs::create_dir_all(system.join("broken"))
            .expect("invalid discovery candidate can be created");
        fs::write(system.join("broken/theme.toml"), "not toml")
            .expect("invalid discovery manifest can be written");
        let expected = discovery_theme(&system, "expected", "nocturne", true);

        let discovered = discover_theme_in("nocturne", &[missing, system.clone()])
            .expect("the existing system root supplies the theme");

        assert_eq!(discovered.directory(), expected);
        assert!(discovered.conflicts().is_empty());
        cleanup(&system);
    }

    #[test]
    fn discovery_accepts_an_internal_release_symlink() {
        let root = discovery_root("release-symlink");
        let releases = root.join(".nocturne-releases");
        discovery_theme(&releases, "release-1", "nocturne", true);
        symlink(".nocturne-releases/release-1", root.join("nocturne"))
            .expect("source installation theme symlink can be created");

        let discovered = discover_theme_in("nocturne", std::slice::from_ref(&root))
            .expect("the active source release is discovered through its relative symlink");

        assert_eq!(discovered.directory(), root.join("nocturne"));
        assert!(discovered.conflicts().is_empty());
        cleanup(&root);
    }

    #[test]
    fn discovery_rejects_invalid_or_missing_ids() {
        let root = discovery_root("invalid-id");
        assert_eq!(
            discover_theme_in("Nocturne", std::slice::from_ref(&root)).err(),
            Some(ThemeError::InvalidThemeId)
        );
        assert_eq!(
            discover_theme_in("missing", std::slice::from_ref(&root)).err(),
            Some(ThemeError::ThemeNotFound)
        );
        cleanup(&root);
    }

    #[test]
    fn discovery_rejects_an_existing_non_directory_search_root() {
        let root = discovery_root("non-directory-root");
        cleanup(&root);
        fs::write(&root, "not a directory").expect("non-directory search root can be written");

        assert_eq!(
            discover_theme_in("nocturne", std::slice::from_ref(&root)).err(),
            Some(ThemeError::DiscoveryUnavailable)
        );
        fs::remove_file(root).expect("non-directory search root can be removed");
    }

    #[test]
    fn selected_broken_theme_does_not_fall_back_to_lower_priority_match() {
        let local = discovery_root("broken-local");
        let system = discovery_root("valid-system-fallback");
        let broken = discovery_theme(&local, "broken", "nocturne", false);
        fs::write(
            broken.join("theme.toml"),
            "[theme]\nid = \"nocturne\"\nunknown = true\n",
        )
        .expect("high-priority malformed manifest can be written");
        discovery_theme(&system, "valid", "nocturne", true);

        let discovered = discover_theme_in("nocturne", &[local.clone(), system.clone()])
            .expect("the higher-priority manifest selects the local theme");
        assert_eq!(discovered.directory(), broken);
        assert!(matches!(
            ThemeSource::external(discovered.into_directory()),
            Err(ThemeError::InvalidManifest)
        ));
        cleanup(&local);
        cleanup(&system);
    }

    #[test]
    fn external_theme_resolves_entrypoint_and_allowlisted_assets() {
        let root = fixture("valid");
        let theme = ThemeSource::external(root.clone()).expect("external theme is valid");
        assert!(theme.allows_navigation("fomalhaut://theme/"));
        assert!(theme.allows_navigation("fomalhaut://theme/index.html"));
        assert!(!theme.allows_navigation("fomalhaut://theme/assets/app.js"));

        let entrypoint = theme
            .resolve("fomalhaut://theme/")
            .expect("entrypoint lookup succeeds")
            .expect("entrypoint exists");
        let (body, content_type) = entrypoint.into_parts();
        assert_eq!(body, b"<!doctype html>");
        assert_eq!(content_type, "text/html; charset=utf-8");
        assert!(
            theme
                .resolve("fomalhaut://theme/assets/app.js")
                .expect("script lookup succeeds")
                .is_some()
        );
        cleanup(&root);
    }

    #[test]
    fn unsafe_or_unknown_resource_paths_are_not_resolved() {
        let root = fixture("paths");
        let theme = ThemeSource::external(root.clone()).expect("external theme is valid");
        for uri in [
            "fomalhaut://theme/../outside",
            "fomalhaut://theme/assets//app.js",
            "fomalhaut://theme/assets/%2e%2e/app.js",
            "fomalhaut://theme/assets/app.js?query",
            "fomalhaut://theme/assets/app.exe",
            "https://example.com/index.html",
        ] {
            assert!(
                theme
                    .resolve(uri)
                    .expect("unsafe paths are rejected without filesystem access")
                    .is_none()
            );
        }
        cleanup(&root);
    }

    #[test]
    fn capability_allows_internal_symlink_and_rejects_external_escape() {
        let root = fixture("symlink");
        fs::write(root.join("assets/real.js"), "internal asset")
            .expect("internal symlink target can be written");
        symlink("real.js", root.join("assets/internal.js"))
            .expect("internal symlink fixture can be created");
        let outside =
            std::env::temp_dir().join(format!("fomalhaut-theme-outside-{}", std::process::id()));
        fs::write(&outside, "outside secret").expect("outside fixture can be written");
        symlink(&outside, root.join("assets/escape.js"))
            .expect("escape symlink fixture can be created");
        let theme = ThemeSource::external(root.clone()).expect("external theme is valid");
        let asset = theme
            .resolve("fomalhaut://theme/assets/internal.js")
            .expect("internal symlink lookup succeeds")
            .expect("internal symlink target is available");
        let (body, content_type) = asset.into_parts();
        assert_eq!(body, b"internal asset");
        assert_eq!(content_type, "application/javascript");
        assert!(
            theme
                .resolve("fomalhaut://theme/assets/escape.js")
                .expect("external escape is rejected as an unavailable resource")
                .is_none()
        );
        cleanup(&root);
        fs::remove_file(outside).expect("outside fixture can be removed");
    }

    #[test]
    fn configured_theme_root_may_itself_be_a_symlink() {
        let root = fixture("root-target");
        let link =
            std::env::temp_dir().join(format!("fomalhaut-theme-root-link-{}", std::process::id()));
        symlink(&root, &link).expect("configured theme root symlink can be created");
        let theme = ThemeSource::external(link.clone())
            .expect("a configured root symlink establishes its target as the capability root");
        assert!(
            theme
                .resolve("fomalhaut://theme/")
                .expect("symlinked root lookup succeeds")
                .is_some()
        );
        fs::remove_file(link).expect("configured root symlink can be removed");
        cleanup(&root);
    }

    #[test]
    fn invalid_manifest_protocol_and_entrypoint_are_rejected() {
        let root = fixture("manifest");
        fs::write(
            root.join("theme.toml"),
            "[theme]\nname = \"Fixture\"\nprotocol = 1\nentrypoint = \"index.html\"\n",
        )
        .expect("manifest without an ID can be written");
        assert!(matches!(
            ThemeSource::external(root.clone()),
            Err(ThemeError::InvalidManifest)
        ));

        fs::write(
            root.join("theme.toml"),
            "[theme]\nid = \"Invalid ID\"\nname = \"Fixture\"\nprotocol = 1\nentrypoint = \"index.html\"\n",
        )
        .expect("manifest with an invalid ID can be written");
        assert!(matches!(
            ThemeSource::external(root.clone()),
            Err(ThemeError::InvalidThemeId)
        ));

        fs::write(
            root.join("theme.toml"),
            "[theme]\nid = \"fixture\"\nname = \"Fixture\"\nprotocol = 2\nentrypoint = \"index.html\"\n",
        )
        .expect("unsupported manifest can be written");
        assert!(matches!(
            ThemeSource::external(root.clone()),
            Err(ThemeError::UnsupportedProtocol)
        ));

        fs::write(
            root.join("theme.toml"),
            "[theme]\nid = \"fixture\"\nname = \"Fixture\"\nprotocol = 1\nentrypoint = \"../index.html\"\n",
        )
        .expect("unsafe manifest can be written");
        assert!(matches!(
            ThemeSource::external(root.clone()),
            Err(ThemeError::InvalidResourcePath)
        ));

        fs::write(
            root.join("theme.toml"),
            "[theme]\nid = \"fixture\"\nname = \"Fixture\"\nprotocol = 1\nentrypoint = \"index.html\"\n",
        )
        .expect("valid manifest can be restored");
        fs::write(root.join("index.html"), [0xff])
            .expect("invalid UTF-8 entrypoint can be written");
        assert!(matches!(
            ThemeSource::external(root.clone()),
            Err(ThemeError::InvalidEntrypoint)
        ));
        cleanup(&root);
    }

    #[test]
    fn manifest_and_resource_limits_are_enforced() {
        let root = fixture("limits");
        fs::write(
            root.join("theme.toml"),
            vec![
                b'x';
                usize::try_from(MAX_MANIFEST_BYTES + 1)
                    .expect("the fixed manifest limit fits usize on supported targets")
            ],
        )
        .expect("oversized manifest can be written");
        assert!(matches!(
            ThemeSource::external(root.clone()),
            Err(ThemeError::ManifestTooLarge)
        ));

        fs::write(
            root.join("theme.toml"),
            "[theme]\nid = \"fixture\"\nname = \"Fixture\"\nprotocol = 1\nentrypoint = \"index.html\"\n",
        )
        .expect("valid manifest can be restored");
        let theme = ThemeSource::external(root.clone()).expect("external theme is valid");
        fs::write(
            root.join("assets/large.js"),
            vec![
                b'x';
                usize::try_from(MAX_ASSET_BYTES + 1)
                    .expect("the fixed asset limit fits usize on supported targets")
            ],
        )
        .expect("oversized resource can be written");
        assert!(matches!(
            theme.resolve("fomalhaut://theme/assets/large.js"),
            Err(ThemeError::ResourceTooLarge)
        ));
        cleanup(&root);
    }
}
