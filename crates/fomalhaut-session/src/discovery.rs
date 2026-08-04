//! Ordered discovery of trusted X11 and Wayland sessions.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
};

use freedesktop_desktop_entry::DesktopEntry;

use crate::{
    DiscoveryError, DiscoveryReport, Rejection, RejectionReason, SessionCatalog, SessionDirectory,
    SessionId, SessionInfo, SessionKind, exec::parse_exec, model::CatalogEntry,
};

/// Explicit inputs controlling deterministic session discovery.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DiscoveryConfig {
    directories: Vec<SessionDirectory>,
    locales: Vec<String>,
    executable_search_paths: Vec<PathBuf>,
}

impl DiscoveryConfig {
    /// Constructs a configuration using the supplied directory priority order.
    #[must_use]
    pub fn new(directories: Vec<SessionDirectory>) -> Self {
        Self {
            directories,
            locales: Vec::new(),
            executable_search_paths: Vec::new(),
        }
    }

    /// Sets preferred locale identifiers in descending priority.
    #[must_use]
    pub fn with_locales(mut self, locales: Vec<String>) -> Self {
        self.locales = locales;
        self
    }

    /// Sets paths used to resolve a relative `TryExec` value.
    #[must_use]
    pub fn with_executable_search_paths(mut self, paths: Vec<PathBuf>) -> Self {
        self.executable_search_paths = paths;
        self
    }

    /// Returns the configured directory priority order.
    #[must_use]
    pub fn directories(&self) -> &[SessionDirectory] {
        &self.directories
    }
}

/// Discovers all valid sessions according to an explicit configuration.
pub fn discover(config: &DiscoveryConfig) -> Result<DiscoveryReport, DiscoveryError> {
    let mut entries = BTreeMap::new();
    let mut seen = BTreeSet::new();
    let mut rejections = Vec::new();

    for directory in &config.directories {
        let paths = read_directory(directory.path())?;
        for path in paths {
            let Some(stem) = desktop_stem(&path) else {
                rejections.push(Rejection::new(path, None, RejectionReason::InvalidFileName));
                continue;
            };
            let id = SessionId::from_file_stem(directory.kind(), &stem);
            if !seen.insert(id.clone()) {
                rejections.push(Rejection::new(path, Some(id), RejectionReason::Duplicate));
                continue;
            }

            match load_entry(&path, &id, &stem, directory.kind(), config) {
                Ok(entry) => {
                    entries.insert(id, entry);
                }
                Err(reason) => rejections.push(Rejection::new(path, Some(id), reason)),
            }
        }
    }

    Ok(DiscoveryReport::new(
        SessionCatalog::new(entries),
        rejections,
    ))
}

fn read_directory(path: &Path) -> Result<Vec<PathBuf>, DiscoveryError> {
    let directory = match fs::read_dir(path) {
        Ok(directory) => directory,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(source) => {
            return Err(DiscoveryError::ReadDirectory {
                path: path.to_owned(),
                source,
            });
        }
    };

    let mut paths = Vec::new();
    for entry in directory {
        let entry = entry.map_err(|source| DiscoveryError::ReadDirectoryEntry {
            path: path.to_owned(),
            source,
        })?;
        let file_type = entry
            .file_type()
            .map_err(|source| DiscoveryError::ReadDirectoryEntry {
                path: path.to_owned(),
                source,
            })?;
        if file_type.is_file()
            && entry
                .path()
                .extension()
                .is_some_and(|value| value == "desktop")
        {
            paths.push(entry.path());
        }
    }
    paths.sort();
    Ok(paths)
}

fn desktop_stem(path: &Path) -> Option<String> {
    path.file_stem()?.to_str().map(str::to_owned)
}

fn load_entry(
    path: &Path,
    id: &SessionId,
    stem: &str,
    kind: SessionKind,
    config: &DiscoveryConfig,
) -> Result<CatalogEntry, RejectionReason> {
    let entry = DesktopEntry::from_path(path.to_owned(), Some(&config.locales))
        .map_err(|_| RejectionReason::InvalidDesktopEntry)?;
    if entry.groups.desktop_entry().is_none() {
        return Err(RejectionReason::MissingDesktopEntryGroup);
    }

    if strict_bool(&entry, "Hidden")?.unwrap_or(false) {
        return Err(RejectionReason::Hidden);
    }
    if strict_bool(&entry, "NoDisplay")?.unwrap_or(false) {
        return Err(RejectionReason::NoDisplay);
    }
    if entry.type_().is_some_and(|kind| kind != "Application") {
        return Err(RejectionReason::UnsupportedType);
    }

    let name = entry
        .name(&config.locales)
        .filter(|name| !name.trim().is_empty())
        .ok_or(RejectionReason::MissingName)?
        .into_owned();
    let exec = entry.exec().ok_or(RejectionReason::MissingExec)?;
    let command = parse_exec(exec).map_err(|_| RejectionReason::InvalidExec)?;
    validate_try_exec(entry.try_exec(), &config.executable_search_paths)?;
    let environment = session_environment(&entry, stem, kind)?;

    Ok(CatalogEntry {
        info: SessionInfo::new(id.clone(), name, kind),
        command,
        environment,
    })
}

fn strict_bool(entry: &DesktopEntry, key: &str) -> Result<Option<bool>, RejectionReason> {
    match entry.desktop_entry(key) {
        None => Ok(None),
        Some("true") => Ok(Some(true)),
        Some("false") => Ok(Some(false)),
        Some(_) => Err(RejectionReason::InvalidBoolean),
    }
}

fn validate_try_exec(value: Option<&str>, search_paths: &[PathBuf]) -> Result<(), RejectionReason> {
    let Some(value) = value else {
        return Ok(());
    };
    if value.is_empty() || value.contains(['\0', '\n', '\r']) {
        return Err(RejectionReason::TryExecUnavailable);
    }

    let path = Path::new(value);
    if path.is_absolute() {
        return executable(path)
            .then_some(())
            .ok_or(RejectionReason::TryExecUnavailable);
    }
    if path.components().count() != 1 {
        return Err(RejectionReason::TryExecUnavailable);
    }
    search_paths
        .iter()
        .any(|directory| executable(&directory.join(path)))
        .then_some(())
        .ok_or(RejectionReason::TryExecUnavailable)
}

fn executable(path: &Path) -> bool {
    fs::metadata(path)
        .is_ok_and(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
}

fn session_environment(
    entry: &DesktopEntry,
    stem: &str,
    kind: SessionKind,
) -> Result<Vec<String>, RejectionReason> {
    if !valid_environment_value(stem) {
        return Err(RejectionReason::InvalidEnvironment);
    }
    let mut environment = vec![
        format!("XDG_SESSION_DESKTOP={stem}"),
        format!("DESKTOP_SESSION={stem}"),
        format!("XDG_SESSION_TYPE={}", kind.xdg_value()),
    ];

    if let Some(names) = entry.desktop_entry("DesktopNames") {
        let names = names
            .split(';')
            .filter(|name| !name.is_empty())
            .collect::<Vec<_>>();
        if names.is_empty() || names.iter().any(|name| !valid_desktop_name(name)) {
            return Err(RejectionReason::InvalidEnvironment);
        }
        environment.push(format!("XDG_CURRENT_DESKTOP={}", names.join(":")));
    }
    Ok(environment)
}

fn valid_environment_value(value: &str) -> bool {
    !value.is_empty() && !value.chars().any(char::is_control)
}

fn valid_desktop_name(value: &str) -> bool {
    valid_environment_value(value) && !value.contains([':', '='])
}

#[cfg(test)]
mod tests {
    use std::{fs, os::unix::fs::PermissionsExt, path::Path};

    use super::{DiscoveryConfig, discover};
    use crate::{RejectionReason, SessionDirectory, SessionKind};

    fn fixture_root(name: &str) -> std::path::PathBuf {
        let root =
            std::env::temp_dir().join(format!("fomalhaut-session-{}-{name}", std::process::id()));
        if root.exists() {
            fs::remove_dir_all(&root).expect("a previous test fixture can be removed");
        }
        fs::create_dir_all(&root).expect("the test fixture directory can be created");
        root
    }

    fn write(path: &Path, contents: &str) {
        fs::write(path, contents).expect("the desktop fixture can be written");
    }

    #[test]
    fn discovers_localized_wayland_and_x11_sessions() {
        let root = fixture_root("kinds");
        let wayland = root.join("wayland");
        let x11 = root.join("x11");
        fs::create_dir_all(&wayland).expect("wayland fixture directory can be created");
        fs::create_dir_all(&x11).expect("x11 fixture directory can be created");
        write(
            &wayland.join("sway.desktop"),
            "[Desktop Entry]\nType=Application\nName=Sway\nName[zh_CN]=摇曳\nExec=sway --unsupported-gpu\nDesktopNames=sway;wlroots;\n",
        );
        write(
            &x11.join("xfce.desktop"),
            "[Desktop Entry]\nType=Application\nName=XFCE\nExec=startxfce4\n",
        );

        let config = DiscoveryConfig::new(vec![
            SessionDirectory::new(&wayland, SessionKind::Wayland),
            SessionDirectory::new(&x11, SessionKind::X11),
        ])
        .with_locales(vec!["zh_CN".to_owned()]);
        let report = discover(&config).expect("valid fixture directories can be discovered");
        let sessions = report.catalog().sessions().collect::<Vec<_>>();

        assert_eq!(sessions.len(), 2);
        assert_eq!(sessions[0].name(), "摇曳");
        assert_eq!(sessions[0].kind(), SessionKind::Wayland);
        assert_eq!(sessions[1].name(), "XFCE");
        assert_eq!(sessions[1].kind(), SessionKind::X11);
        assert!(report.rejections().is_empty());

        let sway = report
            .catalog()
            .entries
            .get(sessions[0].id())
            .expect("the exposed ID resolves inside the catalog");
        assert_eq!(sway.command, ["sway", "--unsupported-gpu"]);
        assert_eq!(
            sway.environment,
            [
                "XDG_SESSION_DESKTOP=sway",
                "DESKTOP_SESSION=sway",
                "XDG_SESSION_TYPE=wayland",
                "XDG_CURRENT_DESKTOP=sway:wlroots"
            ]
        );
        assert!(report.catalog().command(sessions[0].id()).is_ok());

        fs::remove_dir_all(root).expect("the test fixture can be removed");
    }

    #[test]
    fn higher_priority_hidden_entry_suppresses_duplicate() {
        let root = fixture_root("priority");
        let high = root.join("high");
        let low = root.join("low");
        fs::create_dir_all(&high).expect("high-priority directory can be created");
        fs::create_dir_all(&low).expect("low-priority directory can be created");
        write(
            &high.join("same.desktop"),
            "[Desktop Entry]\nType=Application\nName=Hidden\nExec=hidden\nHidden=true\n",
        );
        write(
            &low.join("same.desktop"),
            "[Desktop Entry]\nType=Application\nName=Fallback\nExec=fallback\n",
        );

        let report = discover(&DiscoveryConfig::new(vec![
            SessionDirectory::new(high, SessionKind::Wayland),
            SessionDirectory::new(low, SessionKind::Wayland),
        ]))
        .expect("readable directories can be discovered");

        assert!(report.catalog().is_empty());
        assert_eq!(report.rejections().len(), 2);
        assert_eq!(report.rejections()[0].reason(), RejectionReason::Hidden);
        assert_eq!(report.rejections()[1].reason(), RejectionReason::Duplicate);
        fs::remove_dir_all(root).expect("the test fixture can be removed");
    }

    #[test]
    fn filters_invalid_entries_and_checks_try_exec() {
        let root = fixture_root("invalid");
        let sessions = root.join("sessions");
        let bin = root.join("bin");
        fs::create_dir_all(&sessions).expect("session directory can be created");
        fs::create_dir_all(&bin).expect("binary directory can be created");
        let executable = bin.join("available-session");
        fs::write(&executable, "fixture").expect("TryExec fixture can be written");
        let mut permissions = fs::metadata(&executable)
            .expect("TryExec metadata is available")
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&executable, permissions)
            .expect("TryExec fixture can be made executable");

        write(
            &sessions.join("available.desktop"),
            "[Desktop Entry]\nType=Application\nName=Available\nExec=available-session\nTryExec=available-session\n",
        );
        write(
            &sessions.join("missing.desktop"),
            "[Desktop Entry]\nType=Application\nName=Missing\nExec=missing\nTryExec=missing\n",
        );
        write(
            &sessions.join("field-code.desktop"),
            "[Desktop Entry]\nType=Application\nName=Field Code\nExec=session %U\n",
        );
        write(
            &sessions.join("bad-bool.desktop"),
            "[Desktop Entry]\nType=Application\nName=Bad Bool\nExec=session\nHidden=yes\n",
        );
        write(
            &sessions.join("no-display.desktop"),
            "[Desktop Entry]\nType=Application\nName=No Display\nExec=session\nNoDisplay=true\n",
        );
        write(
            &sessions.join("wrong-type.desktop"),
            "[Desktop Entry]\nType=Link\nName=Wrong Type\nExec=session\n",
        );
        write(
            &sessions.join("type-omitted.desktop"),
            "[Desktop Entry]\nName=Plasma (Wayland)\nExec=startplasma-wayland\nDesktopNames=KDE\n",
        );

        let report = discover(
            &DiscoveryConfig::new(vec![SessionDirectory::new(sessions, SessionKind::Wayland)])
                .with_executable_search_paths(vec![bin]),
        )
        .expect("readable fixtures can be discovered");

        assert_eq!(report.catalog().len(), 2);
        assert!(
            report
                .catalog()
                .sessions()
                .any(|session| session.name() == "Plasma (Wayland)")
        );
        let reasons = report
            .rejections()
            .iter()
            .map(crate::Rejection::reason)
            .collect::<Vec<_>>();
        assert!(reasons.contains(&RejectionReason::InvalidBoolean));
        assert!(reasons.contains(&RejectionReason::InvalidExec));
        assert!(reasons.contains(&RejectionReason::NoDisplay));
        assert!(reasons.contains(&RejectionReason::TryExecUnavailable));
        assert!(reasons.contains(&RejectionReason::UnsupportedType));
        fs::remove_dir_all(root).expect("the test fixture can be removed");
    }

    #[test]
    fn missing_directories_are_not_errors() {
        let root = fixture_root("missing-directory");
        let report = discover(&DiscoveryConfig::new(vec![SessionDirectory::new(
            root.join("absent"),
            SessionKind::Wayland,
        )]))
        .expect("an absent optional session directory is ignored");
        assert!(report.catalog().is_empty());
        fs::remove_dir_all(root).expect("the test fixture can be removed");
    }
}
