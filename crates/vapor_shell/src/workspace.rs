//! Umbrella-workspace policy loaded from the root `Vapor.toml`.

use crate::{discovery::EnvironmentPaths, manifest};
use serde::Deserialize;
use std::{
    collections::HashSet,
    fs,
    path::{Component, Path, PathBuf},
};

/// Root workspace configuration and its governed Cargo projects.
#[derive(Debug, Clone, Deserialize)]
pub struct WorkspaceManifest {
    id: String,
    #[serde(default)]
    cargo: Vec<CargoProject>,
}

/// One project repository containing an independently valid Cargo workspace.
#[derive(Debug, Clone, Deserialize)]
pub struct CargoProject {
    name: String,
    manifest: PathBuf,
    #[serde(default)]
    documentation: bool,
    #[serde(default)]
    binaries: Vec<String>,
}

impl WorkspaceManifest {
    /// Load and validate `[workspace]` policy from the external source root.
    ///
    /// # Errors
    ///
    /// Fails for malformed TOML, unsafe or missing Cargo manifest paths,
    /// duplicate names, or an identity mismatch with discovery.
    pub fn load(paths: &EnvironmentPaths) -> Result<Self, String> {
        let path = paths.source().root().join(manifest::FILE_NAME);
        let text = fs::read_to_string(&path)
            .map_err(|error| format!("failed to read '{}': {error}", path.display()))?;
        #[derive(Deserialize)]
        struct Root {
            workspace: WorkspaceManifest,
        }
        let manifest = toml::from_str::<Root>(&text)
            .map_err(|error| format!("failed to parse '{}': {error}", path.display()))?
            .workspace;

        if manifest.id.trim().is_empty() {
            return Err("workspace id cannot be empty".to_owned());
        }
        if manifest.id != paths.source().workspace_id() {
            return Err(format!(
                "workspace identity changed during startup: discovered '{}' but loaded '{}'",
                paths.source().workspace_id(),
                manifest.id
            ));
        }

        let mut names = HashSet::new();
        for project in &manifest.cargo {
            if project.name.is_empty()
                || project
                    .name
                    .chars()
                    .any(|character| !character.is_ascii_alphanumeric() && character != '-')
            {
                return Err(format!("invalid Cargo project name: {}", project.name));
            }
            if !names.insert(&project.name) {
                return Err(format!("duplicate Cargo project name: {}", project.name));
            }
            validate_relative(&project.manifest)?;
            let absolute = paths.source().root().join(&project.manifest);
            if !absolute.is_file() {
                return Err(format!(
                    "Cargo project '{}' has no manifest at '{}'",
                    project.name,
                    absolute.display()
                ));
            }
            for binary in &project.binaries {
                if binary.is_empty()
                    || binary
                        .chars()
                        .any(|character| !character.is_ascii_alphanumeric() && character != '_')
                {
                    return Err(format!(
                        "invalid deployed binary '{}' for project '{}'",
                        binary, project.name
                    ));
                }
            }
        }

        Ok(manifest)
    }

    /// Stable workspace identifier.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Cargo projects governed by this workspace.
    pub fn cargo_projects(&self) -> &[CargoProject] {
        &self.cargo
    }
}

impl CargoProject {
    /// Stable command-line project name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Cargo manifest path relative to the source workspace.
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
            "Cargo manifest path must be safe and relative: {}",
            path.display()
        ))
    } else {
        Ok(())
    }
}
