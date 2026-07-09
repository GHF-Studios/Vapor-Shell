//! Source-root policy loaded from `Vapor.toml` and adjacent source structure.
//!
//! `[root]` repositories are pure Vapor super-repositories; their direct Git
//! submodules provide Cargo workspaces. `[workspace]` repositories are normal
//! Vapor/Cargo workspaces rooted in the same directory.

use crate::{
    discovery::EnvironmentPaths,
    manifest::{self, VaporEntity},
};
use serde::Serialize;
use std::{
    collections::HashSet,
    fs,
    path::{Component, Path, PathBuf},
};

/// Source root kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SourceRootKind {
    /// Vapor application source/depot root.
    Root,
    /// Normal source workspace.
    Workspace,
}

/// Source-root configuration and its governed Cargo workspaces.
#[derive(Debug, Clone)]
pub struct WorkspaceManifest {
    kind: SourceRootKind,
    id: String,
    name: String,
    organization: String,
    cargo: Vec<CargoProject>,
}

/// One Cargo workspace that Vapor can route workflows through.
#[derive(Debug, Clone)]
pub struct CargoProject {
    name: String,
    manifest: PathBuf,
    documentation: bool,
    binaries: Vec<String>,
}

impl WorkspaceManifest {
    /// Load and validate source-root policy.
    ///
    /// # Errors
    ///
    /// Fails for malformed Vapor identity, unsafe submodule paths, missing
    /// required Cargo manifests, duplicate project names, or an identity
    /// mismatch with startup discovery.
    pub fn load(paths: &EnvironmentPaths) -> Result<Self, String> {
        let source_root = paths.source().root();
        let marker = source_root.join(manifest::FILE_NAME);
        let entity = manifest::read(&marker, source_root)?;
        let (kind, id, name, organization) = match entity {
            VaporEntity::Root {
                id,
                name,
                organization,
            } => (SourceRootKind::Root, id, name, organization),
            VaporEntity::Workspace {
                id,
                name,
                organization,
            } => (SourceRootKind::Workspace, id, name, organization),
            VaporEntity::Registry { id, .. } => {
                return Err(format!("registry '{id}' is not a buildable source root"));
            }
            VaporEntity::Project { id, .. } => {
                return Err(format!("project '{id}' is not a source root"));
            }
            VaporEntity::Content { kind, id, .. } => {
                return Err(format!("{kind} '{id}' is not a source root"));
            }
        };

        if id != paths.source().identity_id() {
            return Err(format!(
                "source identity changed during startup: discovered '{}' but loaded '{}'",
                paths.source().identity_id(),
                id
            ));
        }

        let cargo = match kind {
            SourceRootKind::Root => discover_root_cargo_workspaces(source_root)?,
            SourceRootKind::Workspace => discover_workspace_cargo(source_root, &name)?,
        };
        validate_projects(&cargo)?;

        Ok(Self {
            kind,
            id,
            name,
            organization,
            cargo,
        })
    }

    /// Source root kind.
    pub fn kind(&self) -> SourceRootKind {
        self.kind
    }

    /// Inferred source-root identifier.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Local source-root name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Owning organization namespace.
    pub fn organization(&self) -> &str {
        &self.organization
    }

    /// Cargo workspaces governed by this source root.
    pub fn cargo_projects(&self) -> &[CargoProject] {
        &self.cargo
    }
}

impl CargoProject {
    /// Stable command-line project name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Cargo manifest path relative to the source root.
    pub fn manifest(&self) -> &Path {
        &self.manifest
    }

    /// Whether this project contributes installed Rustdoc.
    pub fn documentation(&self) -> bool {
        self.documentation
    }

    /// Binary package names promoted into the installation after a root build.
    pub fn binaries(&self) -> &[String] {
        &self.binaries
    }
}

fn discover_workspace_cargo(root: &Path, name: &str) -> Result<Vec<CargoProject>, String> {
    let manifest = PathBuf::from("Cargo.toml");
    let absolute = root.join(&manifest);
    if !absolute.is_file() {
        return Err(format!(
            "Vapor workspace '{}' must also be a Cargo workspace with '{}'",
            root.display(),
            absolute.display()
        ));
    }
    Ok(vec![CargoProject {
        name: name.to_owned(),
        manifest,
        documentation: true,
        binaries: Vec::new(),
    }])
}

fn discover_root_cargo_workspaces(root: &Path) -> Result<Vec<CargoProject>, String> {
    submodule_paths(root)?
        .into_iter()
        .filter_map(|path| {
            let cargo_manifest = root.join(&path).join("Cargo.toml");
            cargo_manifest.is_file().then_some(path)
        })
        .map(|path| {
            let marker = root.join(&path).join(manifest::FILE_NAME);
            let name = if marker.is_file() {
                match manifest::read(&marker, root)? {
                    VaporEntity::Workspace { name, .. } => name,
                    VaporEntity::Root { id, .. } => {
                        return Err(format!(
                            "root submodule '{}' declares nested root '{id}'; direct app children must be [workspace]",
                            path.display()
                        ));
                    }
                    VaporEntity::Registry { id, .. } => {
                        return Err(format!(
                            "root submodule '{}' declares registry '{id}'; direct app children must be [workspace]",
                            path.display()
                        ));
                    }
                    VaporEntity::Project { id, .. } => {
                        return Err(format!(
                            "root submodule '{}' declares project '{id}'; direct app children must be [workspace]",
                            path.display()
                        ));
                    }
                    VaporEntity::Content { kind, id, .. } => {
                        return Err(format!(
                            "root submodule '{}' declares {kind} '{id}'; direct app children must be [workspace]",
                            path.display()
                        ));
                    }
                }
            } else {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .ok_or_else(|| format!("invalid submodule path '{}'", path.display()))?
                    .to_ascii_lowercase()
            };

            Ok(CargoProject {
                name,
                manifest: path.join("Cargo.toml"),
                documentation: true,
                binaries: Vec::new(),
            })
        })
        .collect()
}

fn submodule_paths(root: &Path) -> Result<Vec<PathBuf>, String> {
    let path = root.join(".gitmodules");
    if !path.is_file() {
        return Ok(Vec::new());
    }
    let text = fs::read_to_string(&path)
        .map_err(|error| format!("failed to read '{}': {error}", path.display()))?;
    text.lines()
        .filter_map(|line| {
            let line = line.trim();
            line.strip_prefix("path")
                .and_then(|tail| tail.trim_start().strip_prefix('='))
                .map(str::trim)
        })
        .map(PathBuf::from)
        .map(|path| {
            validate_relative(&path)?;
            Ok(path)
        })
        .collect()
}

fn validate_projects(projects: &[CargoProject]) -> Result<(), String> {
    let mut names = HashSet::new();
    for project in projects {
        if project.name.is_empty()
            || project.name.chars().any(|character| {
                !character.is_ascii_lowercase() && !character.is_ascii_digit() && character != '-'
            })
        {
            return Err(format!("invalid Cargo workspace name: {}", project.name));
        }
        if !names.insert(&project.name) {
            return Err(format!("duplicate Cargo workspace name: {}", project.name));
        }
        validate_relative(&project.manifest)?;
    }
    Ok(())
}

fn validate_relative(path: &Path) -> Result<(), String> {
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || path.components().any(|part| {
            matches!(
                part,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        Err(format!(
            "source path must be safe and relative: {}",
            path.display()
        ))
    } else {
        Ok(())
    }
}
