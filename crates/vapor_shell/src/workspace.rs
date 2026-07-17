//! Source-root policy loaded from `Vapor.toml` and adjacent source structure.
//!
//! `[root]` repositories are pure Vapor super-repositories; their direct Git
//! submodules provide Cargo workspaces. `[workspace]` repositories are normal
//! Vapor/Cargo workspaces rooted in the same directory. Vapor-managed projects
//! inside a workspace are registered by the workspace manifest instead of being
//! inferred from arbitrary nested `Vapor.toml` files.

use crate::{
    discovery::EnvironmentPaths,
    manifest::{self, ContentKind, VaporEntity},
};
use serde::{Deserialize, Serialize};
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
    runtime_targets: Vec<String>,
    cargo: Vec<CargoProject>,
    projects: Vec<WorkspaceProject>,
}

/// One Cargo workspace that Vapor can route workflows through.
#[derive(Debug, Clone)]
pub struct CargoProject {
    name: String,
    manifest: PathBuf,
    documentation: bool,
    binaries: Vec<String>,
}

/// One Vapor-managed child project registered by a workspace.
#[derive(Debug, Clone)]
pub struct WorkspaceProject {
    path: PathBuf,
    manifest: PathBuf,
    id: String,
    name: String,
    kind: WorkspaceProjectKind,
}

/// Canonical role of a registered workspace project.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceProjectKind {
    /// Non-content local project.
    Project,
    /// Content artifact project.
    Content(ContentKind),
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

        let runtime_targets = source_runtime_targets(&marker, kind)?;
        let cargo = match kind {
            SourceRootKind::Root => discover_root_cargo_workspaces(source_root)?,
            SourceRootKind::Workspace => discover_workspace_cargo(source_root, &name)?,
        };
        validate_projects(&cargo)?;
        let projects = match kind {
            SourceRootKind::Root => Vec::new(),
            SourceRootKind::Workspace => discover_workspace_projects(source_root)?,
        };
        validate_workspace_projects(&projects)?;

        Ok(Self {
            kind,
            id,
            name,
            organization,
            runtime_targets,
            cargo,
            projects,
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

    /// Declared release/runtime target matrix for this source root.
    pub fn runtime_targets(&self) -> &[String] {
        &self.runtime_targets
    }

    /// Cargo workspaces governed by this source root.
    pub fn cargo_projects(&self) -> &[CargoProject] {
        &self.cargo
    }

    /// Vapor-managed child projects registered by this source root.
    pub fn projects(&self) -> &[WorkspaceProject] {
        &self.projects
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

impl WorkspaceProject {
    /// Project path relative to the source root.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Project manifest path relative to the source root.
    pub fn manifest(&self) -> &Path {
        &self.manifest
    }

    /// Inferred fully-qualified project or content ID.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Local project name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Canonical project role.
    pub fn kind(&self) -> WorkspaceProjectKind {
        self.kind
    }
}

impl WorkspaceProjectKind {
    /// Return the content kind when this project is a content artifact.
    pub fn content_kind(self) -> Option<ContentKind> {
        match self {
            Self::Project => None,
            Self::Content(kind) => Some(kind),
        }
    }
}

impl std::fmt::Display for WorkspaceProjectKind {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Project => formatter.write_str("project"),
            Self::Content(kind) => write!(formatter, "{kind}"),
        }
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
        binaries: workspace_binaries(&root.join(manifest::FILE_NAME))?,
    }])
}

fn source_runtime_targets(path: &Path, kind: SourceRootKind) -> Result<Vec<String>, String> {
    #[derive(Deserialize)]
    struct SourceManifest {
        root: Option<RuntimeOwner>,
        workspace: Option<RuntimeOwner>,
    }
    #[derive(Deserialize)]
    struct RuntimeOwner {
        runtime: Option<RuntimeSection>,
    }
    #[derive(Deserialize)]
    struct RuntimeSection {
        #[serde(default)]
        targets: Vec<String>,
    }

    let source = fs::read_to_string(path)
        .map_err(|error| format!("failed to read '{}': {error}", path.display()))?;
    let manifest: SourceManifest = toml::from_str(&source)
        .map_err(|error| format!("failed to parse '{}': {error}", path.display()))?;
    let targets = match kind {
        SourceRootKind::Root => manifest.root.and_then(|owner| owner.runtime),
        SourceRootKind::Workspace => manifest.workspace.and_then(|owner| owner.runtime),
    }
    .map(|runtime| runtime.targets)
    .unwrap_or_default();
    validate_runtime_targets(path, targets)
}

fn validate_runtime_targets(path: &Path, targets: Vec<String>) -> Result<Vec<String>, String> {
    let mut seen = HashSet::new();
    for target in &targets {
        let valid = !target.is_empty()
            && target.chars().all(|character| {
                character.is_ascii_alphanumeric() || matches!(character, '-' | '_')
            });
        if !valid {
            return Err(format!(
                "runtime target must be a Rust target triple such as x86_64-pc-windows-gnullvm in '{}': {target}",
                path.display()
            ));
        }
        if !seen.insert(target.clone()) {
            return Err(format!(
                "duplicate runtime target in '{}': {target}",
                path.display()
            ));
        }
    }
    Ok(targets)
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

            let binaries = if marker.is_file() {
                workspace_binaries(&marker)?
            } else {
                Vec::new()
            };

            Ok(CargoProject {
                name,
                manifest: path.join("Cargo.toml"),
                documentation: true,
                binaries,
            })
        })
        .collect()
}

fn workspace_binaries(marker: &Path) -> Result<Vec<String>, String> {
    let policy = read_workspace_policy(marker)?;
    validate_binary_names(&policy.binaries, marker)?;
    Ok(policy.binaries)
}

fn discover_workspace_projects(root: &Path) -> Result<Vec<WorkspaceProject>, String> {
    let marker = root.join(manifest::FILE_NAME);
    read_workspace_policy(&marker)?
        .projects
        .into_iter()
        .map(|project| load_workspace_project(root, &project))
        .collect()
}

fn load_workspace_project(
    root: &Path,
    project: &WorkspaceProjectConfig,
) -> Result<WorkspaceProject, String> {
    validate_relative(&project.path)?;
    let absolute = root.join(&project.path);
    if !absolute.is_dir() {
        return Err(format!(
            "registered workspace project '{}' is not a directory",
            project.path.display()
        ));
    }
    let manifest = project.path.join(manifest::FILE_NAME);
    let marker = root.join(&manifest);
    if !marker.is_file() {
        return Err(format!(
            "registered workspace project '{}' is missing {}",
            project.path.display(),
            manifest::FILE_NAME
        ));
    }
    let entity = manifest::read(&marker, root)?;
    let (id, name, kind) = match entity {
        VaporEntity::Project { id, name } => (id, name, WorkspaceProjectKind::Project),
        VaporEntity::Content { kind, id, name } => (id, name, WorkspaceProjectKind::Content(kind)),
        VaporEntity::Root { id, .. } => {
            return Err(format!(
                "registered workspace project '{}' declares root '{id}'; workspace projects must use [project] or a content section",
                project.path.display()
            ));
        }
        VaporEntity::Workspace { id, .. } => {
            return Err(format!(
                "registered workspace project '{}' declares workspace '{id}'; nested source roots are not workspace projects",
                project.path.display()
            ));
        }
        VaporEntity::Registry { id, .. } => {
            return Err(format!(
                "registered workspace project '{}' declares registry '{id}'; registries are not workspace projects",
                project.path.display()
            ));
        }
    };
    Ok(WorkspaceProject {
        path: project.path.clone(),
        manifest,
        id,
        name,
        kind,
    })
}

fn read_workspace_policy(marker: &Path) -> Result<WorkspacePolicy, String> {
    let source = fs::read_to_string(marker)
        .map_err(|error| format!("failed to read '{}': {error}", marker.display()))?;
    let document: WorkspaceToml = toml::from_str(&source)
        .map_err(|error| format!("failed to parse '{}': {error}", marker.display()))?;
    Ok(document.workspace.unwrap_or_default())
}

#[derive(Debug, Deserialize)]
struct WorkspaceToml {
    workspace: Option<WorkspacePolicy>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct WorkspacePolicy {
    #[serde(default)]
    binaries: Vec<String>,
    #[serde(default)]
    projects: Vec<WorkspaceProjectConfig>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct WorkspaceProjectConfig {
    path: PathBuf,
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

fn validate_workspace_projects(projects: &[WorkspaceProject]) -> Result<(), String> {
    let mut paths = HashSet::new();
    let mut ids = HashSet::new();
    for project in projects {
        validate_relative(&project.path)?;
        validate_relative(&project.manifest)?;
        if !paths.insert(&project.path) {
            return Err(format!(
                "duplicate workspace project path: {}",
                project.path.display()
            ));
        }
        if !ids.insert(&project.id) {
            return Err(format!("duplicate workspace project id: {}", project.id));
        }
    }
    Ok(())
}

fn validate_binary_names(names: &[String], marker: &Path) -> Result<(), String> {
    let mut seen = HashSet::new();
    for name in names {
        if name.is_empty()
            || name.chars().any(|character| {
                !character.is_ascii_lowercase()
                    && !character.is_ascii_digit()
                    && character != '-'
                    && character != '_'
            })
        {
            return Err(format!(
                "workspace binary names in '{}' must be non-empty lowercase ASCII names",
                marker.display()
            ));
        }
        if !seen.insert(name) {
            return Err(format!(
                "duplicate workspace binary '{}' in '{}'",
                name,
                marker.display()
            ));
        }
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
