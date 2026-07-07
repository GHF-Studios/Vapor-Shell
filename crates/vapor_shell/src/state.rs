//! Mutable source navigation and derived source-root context.
//!
//! # Lifecycle
//!
//! [`ShellState::new`] validates the external source-root identity, generates an
//! optional Cargo index, and starts at the source root. Navigation methods
//! canonicalize user paths and enforce that root as a hard ceiling.
//!
//! # Installation access
//!
//! Installation paths remain available through [`ShellState::paths`] for
//! explicit tool execution and diagnostics. They are not valid navigation
//! targets because authored source must remain outside the replaceable app.

use crate::{
    cargo_metadata::{CargoIndex, CargoWorkspace},
    discovery::{EnvironmentPaths, ensure_contained},
    manifest::{self, ContentKind, VaporEntity},
};
use std::{
    fs,
    path::{Path, PathBuf},
};

const EMPTY_CONTEXT: &str = "none";

/// Kind of source root selected for this session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceRootKind {
    /// The Vapor application/depot source root.
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

    /// Directory containing this content's `Vapor.toml`.
    pub fn root(&self) -> &Path {
        &self.root
    }
}

/// State owned by one interactive shell session.
#[derive(Debug, Clone)]
pub struct ShellState {
    paths: EnvironmentPaths,
    current_dir: PathBuf,
    source: SourceContext,
    content: Option<ContentContext>,
    cargo: CargoIndex,
}

impl ShellState {
    /// Validate the source root and build derived, replaceable indexes.
    ///
    /// # Errors
    ///
    /// Fails only when the authoritative source `Vapor.toml` is invalid. Cargo
    /// metadata failure is retained as [`CargoIndex::Unavailable`].
    pub fn new(paths: EnvironmentPaths) -> Result<Self, String> {
        let source_root = paths.source().root();
        let marker = source_root.join(manifest::FILE_NAME);
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
            VaporEntity::Project { id, .. } => {
                return Err(format!("source root unexpectedly describes project '{id}'"));
            }
            VaporEntity::Content { kind, id, .. } => {
                return Err(format!(
                    "source root '{}' unexpectedly describes {kind} '{id}'",
                    source_root.display()
                ));
            }
        };
        let cargo = CargoIndex::inspect(&paths);

        Ok(Self {
            current_dir: source_root.to_path_buf(),
            paths,
            source,
            content: None,
            cargo,
        })
    }

    /// Installation and external-source roots.
    pub fn paths(&self) -> &EnvironmentPaths {
        &self.paths
    }

    /// Current internal directory, always inside the source root.
    pub fn current_dir(&self) -> &Path {
        &self.current_dir
    }

    /// Validated source-root identity.
    pub fn source(&self) -> &SourceContext {
        &self.source
    }

    /// Nearest active content identity, if any.
    pub fn content(&self) -> Option<&ContentContext> {
        self.content.as_ref()
    }

    /// Replaceable Cargo index status.
    pub fn cargo_index(&self) -> &CargoIndex {
        &self.cargo
    }

    /// Rebuild Cargo-derived state after an explicit toolchain installation.
    pub fn refresh_cargo_index(&mut self) {
        self.cargo = CargoIndex::inspect(&self.paths);
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
        format!("[{}::{content_id}] ", self.source.id())
    }

    /// Recompute nearest-content context from the current source directory.
    ///
    /// Invalid nested markers are returned as warnings so a malformed child
    /// cannot make the entire external source root inaccessible.
    pub fn refresh_context(&mut self) -> Vec<String> {
        self.content = None;
        let mut warnings = Vec::new();
        let source = self.paths.source();
        let mut cursor = Some(self.current_dir.as_path());

        while let Some(directory) = cursor {
            if !source.contains(directory) {
                break;
            }

            let marker = directory.join(manifest::FILE_NAME);
            if marker.is_file() {
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
                            && self.source.kind() == SourceRootKind::Workspace =>
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
                    Ok(VaporEntity::Project { .. }) => {}
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

    /// Resolve a directory and enforce the external source boundary.
    ///
    /// # Errors
    ///
    /// Fails for nonexistent paths, non-directories, and canonical paths outside
    /// the source root (including symlink escapes).
    pub fn resolve_directory(&self, target: &Path) -> Result<PathBuf, String> {
        let candidate = if target.is_absolute() {
            target.to_path_buf()
        } else {
            self.current_dir.join(target)
        };
        let canonical = fs::canonicalize(&candidate).map_err(|error| {
            format!(
                "cannot resolve directory '{}': {error}",
                candidate.display()
            )
        })?;
        if !canonical.is_dir() {
            return Err(format!("not a directory: {}", canonical.display()));
        }

        ensure_contained(self.paths.source().root(), &canonical)?;
        Ok(canonical)
    }

    /// Change to a user-supplied source path and refresh content context.
    ///
    /// # Errors
    ///
    /// Returns errors from [`Self::resolve_directory`].
    pub fn change_directory(&mut self, target: &Path) -> Result<Vec<String>, String> {
        let resolved = self.resolve_directory(target)?;
        self.change_directory_to(resolved)
    }

    /// Change to a canonical, trusted source path and refresh content context.
    ///
    /// # Errors
    ///
    /// Fails when `target` is outside the source root.
    pub fn change_directory_to(&mut self, target: PathBuf) -> Result<Vec<String>, String> {
        ensure_contained(self.paths.source().root(), &target)?;
        self.current_dir = target;
        Ok(self.refresh_context())
    }

    /// Move toward the source root by exactly `levels` parents.
    ///
    /// # Errors
    ///
    /// Fails rather than moving above the source root.
    pub fn move_up(&mut self, levels: usize) -> Result<Vec<String>, String> {
        let source_root = self.paths.source().root();
        let mut target = self.current_dir.clone();

        for _ in 0..levels {
            if target == source_root {
                return Err(format!(
                    "source root boundary reached at {}",
                    source_root.display()
                ));
            }
            target = target.parent().map(Path::to_path_buf).ok_or_else(|| {
                format!("source root boundary reached at {}", source_root.display())
            })?;
        }

        self.change_directory_to(target)
    }
}
