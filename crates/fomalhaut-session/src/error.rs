//! Errors and per-file rejection diagnostics for session discovery.

use std::{error::Error, fmt, io, path::PathBuf};

use crate::SessionId;

/// A directory-level failure that prevents deterministic discovery.
#[derive(Debug)]
pub enum DiscoveryError {
    /// A configured directory exists but cannot be read.
    ReadDirectory {
        /// Configured directory.
        path: PathBuf,
        /// Underlying I/O failure.
        source: io::Error,
    },
    /// An entry in a readable directory could not be enumerated.
    ReadDirectoryEntry {
        /// Configured directory.
        path: PathBuf,
        /// Underlying I/O failure.
        source: io::Error,
    },
}

impl fmt::Display for DiscoveryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ReadDirectory { path, source } => {
                write!(
                    formatter,
                    "cannot read session directory {}: {source}",
                    path.display()
                )
            }
            Self::ReadDirectoryEntry { path, source } => write!(
                formatter,
                "cannot enumerate an entry in session directory {}: {source}",
                path.display()
            ),
        }
    }
}

impl Error for DiscoveryError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::ReadDirectory { source, .. } | Self::ReadDirectoryEntry { source, .. } => {
                Some(source)
            }
        }
    }
}

/// Sanitized reason why a desktop file was not offered as a session.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RejectionReason {
    /// The file name cannot be represented as a stable UTF-8 identifier.
    InvalidFileName,
    /// A higher-priority entry already claimed this identifier.
    Duplicate,
    /// The file cannot be read or decoded as a desktop entry.
    InvalidDesktopEntry,
    /// The required `[Desktop Entry]` group is missing.
    MissingDesktopEntryGroup,
    /// A boolean field is neither `true` nor `false`.
    InvalidBoolean,
    /// `Hidden=true` suppresses this identifier.
    Hidden,
    /// `NoDisplay=true` suppresses this identifier.
    NoDisplay,
    /// The entry is not `Type=Application`.
    UnsupportedType,
    /// The localized display name is absent or empty.
    MissingName,
    /// The command is absent or empty.
    MissingExec,
    /// The command cannot be converted to a safe argv.
    InvalidExec,
    /// `TryExec` is invalid, missing, or not executable.
    TryExecUnavailable,
    /// Session metadata cannot safely form environment variables.
    InvalidEnvironment,
}

/// Diagnostic for one rejected desktop file.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Rejection {
    path: PathBuf,
    id: Option<SessionId>,
    reason: RejectionReason,
}

impl Rejection {
    pub(crate) fn new(path: PathBuf, id: Option<SessionId>, reason: RejectionReason) -> Self {
        Self { path, id, reason }
    }

    /// Returns the rejected file path on the trusted host filesystem.
    #[must_use]
    pub fn path(&self) -> &std::path::Path {
        &self.path
    }

    /// Returns the claimed session identifier, if the filename allowed one.
    #[must_use]
    pub const fn id(&self) -> Option<&SessionId> {
        self.id.as_ref()
    }

    /// Returns the sanitized rejection category.
    #[must_use]
    pub const fn reason(&self) -> RejectionReason {
        self.reason
    }
}

/// A selected ID does not belong to the current trusted catalog.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionLookupError;

impl fmt::Display for SessionLookupError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("session identifier is not present in the current catalog")
    }
}

impl Error for SessionLookupError {}
