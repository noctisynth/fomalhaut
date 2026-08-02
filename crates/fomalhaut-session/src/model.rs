//! Trusted session identifiers, metadata, and catalog storage.

use std::{collections::BTreeMap, fmt, path::PathBuf};

use fomalhaut_core::SessionCommand;

use crate::{Rejection, SessionLookupError};

/// Display-server family associated with a session directory.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SessionKind {
    /// Native Wayland session.
    Wayland,
    /// X11 session.
    X11,
}

impl SessionKind {
    pub(crate) const fn id_prefix(self) -> &'static str {
        match self {
            Self::Wayland => "wayland",
            Self::X11 => "x11",
        }
    }

    pub(crate) const fn xdg_value(self) -> &'static str {
        match self {
            Self::Wayland => "wayland",
            Self::X11 => "x11",
        }
    }
}

/// One configured session search directory and its display-server family.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionDirectory {
    path: PathBuf,
    kind: SessionKind,
}

impl SessionDirectory {
    /// Constructs a typed session search directory.
    #[must_use]
    pub fn new(path: impl Into<PathBuf>, kind: SessionKind) -> Self {
        Self {
            path: path.into(),
            kind,
        }
    }

    /// Returns the configured directory path.
    #[must_use]
    pub fn path(&self) -> &std::path::Path {
        &self.path
    }

    /// Returns the session family assigned to the directory.
    #[must_use]
    pub const fn kind(&self) -> SessionKind {
        self.kind
    }
}

/// Opaque, stable identifier for a discovered session.
#[derive(Clone, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SessionId(String);

impl SessionId {
    pub(crate) fn from_file_stem(kind: SessionKind, stem: &str) -> Self {
        Self(format!("{}:{stem}", kind.id_prefix()))
    }

    /// Returns the opaque value that may be passed through the frontend protocol.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for SessionId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_tuple("SessionId").field(&self.0).finish()
    }
}

impl fmt::Display for SessionId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

/// Metadata safe to expose to an untrusted frontend.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionInfo {
    id: SessionId,
    name: String,
    kind: SessionKind,
}

impl SessionInfo {
    pub(crate) fn new(id: SessionId, name: String, kind: SessionKind) -> Self {
        Self { id, name, kind }
    }

    /// Returns the opaque session identifier.
    #[must_use]
    pub const fn id(&self) -> &SessionId {
        &self.id
    }

    /// Returns the localized display name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the display-server family.
    #[must_use]
    pub const fn kind(&self) -> SessionKind {
        self.kind
    }
}

pub(crate) struct CatalogEntry {
    pub(crate) info: SessionInfo,
    pub(crate) command: Vec<String>,
    pub(crate) environment: Vec<String>,
}

/// Immutable mapping from frontend-safe metadata to trusted launch commands.
pub struct SessionCatalog {
    pub(crate) entries: BTreeMap<SessionId, CatalogEntry>,
}

impl SessionCatalog {
    pub(crate) fn new(entries: BTreeMap<SessionId, CatalogEntry>) -> Self {
        Self { entries }
    }

    /// Iterates sessions in stable identifier order.
    pub fn sessions(&self) -> impl ExactSizeIterator<Item = &SessionInfo> {
        self.entries.values().map(|entry| &entry.info)
    }

    /// Returns the number of selectable sessions.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns whether no selectable sessions were discovered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Resolves an ID from this catalog to a trusted core command.
    pub fn command(&self, id: &SessionId) -> Result<SessionCommand, SessionLookupError> {
        let entry = self.entries.get(id).ok_or(SessionLookupError)?;
        SessionCommand::new(entry.command.clone(), entry.environment.clone())
            .map_err(|_| SessionLookupError)
    }
}

/// Successful discovery output plus sanitized per-file diagnostics.
pub struct DiscoveryReport {
    catalog: SessionCatalog,
    rejections: Vec<Rejection>,
}

impl DiscoveryReport {
    pub(crate) fn new(catalog: SessionCatalog, rejections: Vec<Rejection>) -> Self {
        Self {
            catalog,
            rejections,
        }
    }

    /// Returns the trusted session catalog.
    #[must_use]
    pub const fn catalog(&self) -> &SessionCatalog {
        &self.catalog
    }

    /// Consumes the report and returns the trusted session catalog.
    #[must_use]
    pub fn into_catalog(self) -> SessionCatalog {
        self.catalog
    }

    /// Returns sanitized diagnostics for rejected entries.
    #[must_use]
    pub fn rejections(&self) -> &[Rejection] {
        &self.rejections
    }
}
