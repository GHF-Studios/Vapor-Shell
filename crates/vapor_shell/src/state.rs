//! Mutable source selection and derived source-root context.
//!
//! # Lifecycle
//!
//! [`ShellState::new`] validates the external source-root identity, generates an
//! optional Cargo index, and anchors source-backed workflows at the source root.
//!
//! # Installation access
//!
//! Installation paths remain available through [`ShellState::active_paths`] for
//! explicit tool execution and diagnostics. They are not valid navigation
//! targets because authored source must remain outside the app root.

use crate::{
    cargo_metadata::{CargoIndex, CargoWorkspace},
    discovery::{EnvironmentPaths, InstallationPaths},
    manifest::{self, ContentKind, VaporEntity},
};
use std::path::{Path, PathBuf};

const EMPTY_CONTEXT: &str = "none";

/// Kind of source root selected for this session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceRootKind {
    /// The Vapor application source/depot root.
    Root,
    /// A normal source workspace.
    Workspace,
}

/// Validated identity of the external source root.
#[derive(Debug, Clone)]
pub struct SourceContext {
    kind: SourceRootKind,
    id: String,
    root: PathBuf,
}

impl SourceContext {
    /// Source root kind.
    pub fn kind(&self) -> SourceRootKind {
        self.kind
    }

    /// Stable source-root identifier.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Canonical source root.
    pub fn root(&self) -> &Path {
        &self.root
    }
}

/// Nearest content marker above the current directory.
#[derive(Debug, Clone)]
pub struct ContentContext {
    id: String,
    kind: ContentKind,
    root: PathBuf,
}

impl ContentContext {
    /// Stable content identifier.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Canonical content category.
    pub fn kind(&self) -> ContentKind {
        self.kind
    }

    /// Directory containing this content's Vapor manifest.
    pub fn root(&self) -> &Path {
        &self.root
    }
}

/// State owned by one interactive shell session.
#[derive(Debug, Clone)]
pub struct ShellState {
    installation: InstallationPaths,
    paths: Option<EnvironmentPaths>,
    current_dir: Option<PathBuf>,
    source: Option<SourceContext>,
    content: Option<ContentContext>,
    cargo: CargoIndex,
}

impl ShellState {
    /// Start a shell session with no active source root.
    pub fn closed(installation: InstallationPaths) -> Self {
        Self {
            installation,
            paths: None,
            current_dir: None,
            source: None,
            content: None,
            cargo: CargoIndex::NotPresent,
        }
    }

    /// Validate the source root and build derived runtime indexes.
    ///
    /// # Errors
    ///
    /// Fails only when the authoritative source Vapor manifest is invalid. Cargo
    /// metadata failure is retained as [`CargoIndex::Unavailable`].
    pub fn new(paths: EnvironmentPaths) -> Result<Self, String> {
        let mut state = Self::closed(paths.installation().clone());
        state.open_paths(paths)?;
        Ok(state)
    }

    /// Activate a source root and rebuild derived runtime indexes.
    ///
    /// # Errors
    ///
    /// Fails when the authoritative source Vapor manifest is invalid.
    pub fn open_paths(&mut self, paths: EnvironmentPaths) -> Result<Vec<String>, String> {
        let source_root = paths.source().root();
        let marker = manifest::source_root_marker(source_root)?;
        let source = match manifest::read(&marker, source_root)? {
            VaporEntity::Root { id, .. } => SourceContext {
                kind: SourceRootKind::Root,
                id,
                root: source_root.to_path_buf(),
            },
            VaporEntity::Workspace { id, .. } => SourceContext {
                kind: SourceRootKind::Workspace,
                id,
                root: source_root.to_path_buf(),
            },
            VaporEntity::Registry { id, .. } => {
                return Err(format!(
                    "source root unexpectedly describes registry '{id}'"
                ));
            }
            VaporEntity::Content { kind, id, .. } => {
                return Err(format!(
                    "source root '{}' unexpectedly describes {kind} '{id}'",
                    source_root.display()
                ));
            }
        };
        let cargo = CargoIndex::inspect(&paths);

        self.installation = paths.installation().clone();
        self.current_dir = Some(source_root.to_path_buf());
        self.paths = Some(paths);
        self.source = Some(source);
        self.content = None;
        self.cargo = cargo;
        Ok(self.refresh_context())
    }

    /// Close the active source root while keeping the app session alive.
    pub fn close_source(&mut self) {
        self.paths = None;
        self.current_dir = None;
        self.source = None;
        self.content = None;
        self.cargo = CargoIndex::NotPresent;
    }

    /// Steam installation/app root for this session.
    pub fn installation(&self) -> &InstallationPaths {
        &self.installation
    }

    /// Installation and external-source roots.
    ///
    /// # Errors
    ///
    /// Returns an actionable diagnostic when no source is open.
    pub fn active_paths(&self) -> Result<&EnvironmentPaths, String> {
        self.paths.as_ref().ok_or_else(no_source_error)
    }

    /// Current internal directory, always inside the source root.
    ///
    /// # Errors
    ///
    /// Returns an actionable diagnostic when no source is open.
    pub fn current_dir(&self) -> Result<&Path, String> {
        self.current_dir.as_deref().ok_or_else(no_source_error)
    }

    /// Validated source-root identity.
    pub fn source(&self) -> Option<&SourceContext> {
        self.source.as_ref()
    }

    /// Nearest active content identity, if any.
    pub fn content(&self) -> Option<&ContentContext> {
        self.content.as_ref()
    }

    /// Replaceable Cargo index status.
    pub fn cargo_index(&self) -> &CargoIndex {
        &self.cargo
    }

    /// Rebuild Cargo-derived state after installer-managed tools change.
    pub fn refresh_cargo_index(&mut self) {
        self.cargo = self
            .paths
            .as_ref()
            .map_or(CargoIndex::NotPresent, CargoIndex::inspect);
    }

    /// Successfully loaded Cargo metadata, if available.
    pub fn cargo_workspace(&self) -> Option<&CargoWorkspace> {
        match &self.cargo {
            CargoIndex::Loaded(metadata) => Some(metadata),
            CargoIndex::NotPresent | CargoIndex::Unavailable(_) => None,
        }
    }

    /// Render the semantic portion of the interactive prompt.
    pub fn prompt_context(&self) -> String {
        let content_id = self
            .content
            .as_ref()
            .map_or(EMPTY_CONTEXT, |item| item.id());
        self.source.as_ref().map_or_else(
            || "[closed::none] ".to_owned(),
            |source| format!("[{}::{content_id}] ", source.id()),
        )
    }

    /// Recompute nearest-content context from the current source directory.
    ///
    /// Invalid nested markers are returned as warnings so a malformed child
    /// cannot make the entire external source root inaccessible.
    pub fn refresh_context(&mut self) -> Vec<String> {
        self.content = None;
        let mut warnings = Vec::new();
        let Some(paths) = self.paths.as_ref() else {
            return warnings;
        };
        let Some(current_dir) = self.current_dir.as_deref() else {
            return warnings;
        };
        let source = paths.source();
        let mut cursor = Some(current_dir);

        while let Some(directory) = cursor {
            if !source.contains(directory) {
                break;
            }

            let mut markers = Vec::new();
            if let Some(marker) = manifest::content_marker_at(directory) {
                markers.push(marker);
            }
            markers.extend(
                manifest::source_root_marker_candidates(directory)
                    .into_iter()
                    .filter(|marker| marker.is_file()),
            );
            for marker in markers {
                match manifest::read(&marker, source.root()) {
                    Ok(VaporEntity::Content { kind, id, .. }) if self.content.is_none() => {
                        self.content = Some(ContentContext {
                            id,
                            kind,
                            root: directory.to_path_buf(),
                        });
                    }
                    Ok(VaporEntity::Workspace { .. })
                        if directory != source.root()
                            && self.source.as_ref().is_some_and(|source| {
                                source.kind() == SourceRootKind::Workspace
                            }) =>
                    {
                        warnings.push(format!(
                            "nested workspace manifest '{}' is not allowed inside source workspace '{}'",
                            marker.display(),
                            source.root().display()
                        ));
                    }
                    Ok(VaporEntity::Root { .. }) if directory != source.root() => {
                        warnings.push(format!(
                            "nested root manifest '{}' is not allowed inside source root '{}'",
                            marker.display(),
                            source.root().display()
                        ));
                    }
                    Ok(VaporEntity::Registry { .. }) => {}
                    Ok(_) => {}
                    Err(error) => warnings.push(error),
                }
            }

            if directory == source.root() {
                break;
            }
            cursor = directory.parent();
        }

        warnings
    }
}

fn no_source_error() -> String {
    "no Vapor source is open\nhelp: open an indexed source with `source open NAME`, open a path with `source open PATH`, or index one with `source add PATH`"
        .to_owned()
}
