//! Optional Cargo workspace indexing for authored source.
//!
//! # Authority
//!
//! [`cargo metadata`](https://doc.rust-lang.org/cargo/commands/cargo-metadata.html)
//! reports Cargo packages, targets, the Cargo workspace root, and target
//! directory. That data is useful but rebuildable. `Vapor.toml` remains the
//! authority for Vapor workspace and content identity.
//!
//! # Runtime behavior
//!
//! Metadata is requested only when the external source root contains
//! `Cargo.toml`. Vapor invokes the Cargo executable bundled in the Steam
//! installation with `--format-version 1 --no-deps`. Failure degrades to
//! [`CargoIndex::Unavailable`] instead of preventing access to authored source.

use crate::{discovery::EnvironmentPaths, workflow};
use serde::Deserialize;
use std::{
    path::{Path, PathBuf},
    process::Command,
};

/// Result of attempting to index the source workspace with Cargo.
#[derive(Debug, Clone)]
pub enum CargoIndex {
    /// The Vapor source workspace has no root `Cargo.toml`.
    NotPresent,
    /// Cargo metadata was loaded successfully.
    Loaded(CargoWorkspace),
    /// Cargo metadata was applicable but could not be regenerated.
    Unavailable(String),
}

impl CargoIndex {
    /// Generate a rebuildable Cargo index for the source workspace.
    pub fn inspect(paths: &EnvironmentPaths) -> Self {
        let source_root = paths.source().root();
        let manifest = source_root.join("Cargo.toml");
        if !manifest.is_file() {
            return Self::NotPresent;
        }

        let Some(cargo) = paths.installation().bundled_cargo() else {
            return Self::Unavailable(format!(
                "Steam installation '{}' has no bundled Cargo executable",
                paths.installation().root().display()
            ));
        };

        match load(&cargo, &manifest, source_root, paths) {
            Ok(metadata) => Self::Loaded(metadata),
            Err(error) => Self::Unavailable(error),
        }
    }
}

/// Rebuildable description of one Cargo workspace.
#[derive(Debug, Clone)]
pub struct CargoWorkspace {
    root: PathBuf,
    target_directory: PathBuf,
    packages: Vec<CargoPackage>,
}

impl CargoWorkspace {
    /// Cargo's workspace root.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Directory where Cargo writes build artifacts.
    pub fn target_directory(&self) -> &Path {
        &self.target_directory
    }

    /// Packages reported as workspace members.
    pub fn packages(&self) -> &[CargoPackage] {
        &self.packages
    }
}

/// Cargo package and its declared build targets.
#[derive(Debug, Clone)]
pub struct CargoPackage {
    name: String,
    manifest_path: PathBuf,
    targets: Vec<CargoTarget>,
}

impl CargoPackage {
    /// Package name from `[package]`.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Absolute package manifest path.
    pub fn manifest_path(&self) -> &Path {
        &self.manifest_path
    }

    /// Library, binary, example, test, and other Cargo targets.
    pub fn targets(&self) -> &[CargoTarget] {
        &self.targets
    }
}

/// One target declared by a Cargo package.
#[derive(Debug, Clone)]
pub struct CargoTarget {
    name: String,
    kinds: Vec<String>,
}

impl CargoTarget {
    /// Target name passed to Cargo commands.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Cargo target kinds such as `lib`, `bin`, or `test`.
    pub fn kinds(&self) -> &[String] {
        &self.kinds
    }
}

fn load(
    cargo: &Path,
    manifest: &Path,
    source_root: &Path,
    paths: &EnvironmentPaths,
) -> Result<CargoWorkspace, String> {
    let output = Command::new(cargo)
        .args([
            "metadata",
            "--format-version",
            "1",
            "--no-deps",
            "--manifest-path",
        ])
        .arg(manifest)
        .env("VAPOR_HOME", paths.installation().root())
        .env("CARGO_HOME", paths.installation().root().join("cargo-home"))
        .env(
            "RUSTUP_HOME",
            paths.installation().root().join("rustup-home"),
        )
        .env("PATH", workflow::managed_path(paths)?)
        .env_remove("RUSTC_WRAPPER")
        .current_dir(source_root)
        .output()
        .map_err(|error| format!("failed to run bundled Cargo '{}': {error}", cargo.display()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        return Err(format!(
            "bundled Cargo metadata failed with {}{}",
            output.status,
            if stderr.is_empty() {
                String::new()
            } else {
                format!(": {stderr}")
            }
        ));
    }

    let raw: RawMetadata = serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("bundled Cargo returned invalid metadata JSON: {error}"))?;
    let workspace = CargoWorkspace::from(raw);
    if workspace.root != source_root {
        return Err(format!(
            "Cargo workspace root '{}' does not match Vapor source root '{}'",
            workspace.root.display(),
            source_root.display()
        ));
    }
    Ok(workspace)
}

#[derive(Debug, Deserialize)]
struct RawMetadata {
    workspace_root: PathBuf,
    target_directory: PathBuf,
    packages: Vec<RawPackage>,
}

#[derive(Debug, Deserialize)]
struct RawPackage {
    name: String,
    manifest_path: PathBuf,
    targets: Vec<RawTarget>,
}

#[derive(Debug, Deserialize)]
struct RawTarget {
    name: String,
    kind: Vec<String>,
}

impl From<RawMetadata> for CargoWorkspace {
    fn from(raw: RawMetadata) -> Self {
        Self {
            root: raw.workspace_root,
            target_directory: raw.target_directory,
            packages: raw.packages.into_iter().map(CargoPackage::from).collect(),
        }
    }
}

impl From<RawPackage> for CargoPackage {
    fn from(raw: RawPackage) -> Self {
        Self {
            name: raw.name,
            manifest_path: raw.manifest_path,
            targets: raw.targets.into_iter().map(CargoTarget::from).collect(),
        }
    }
}

impl From<RawTarget> for CargoTarget {
    fn from(raw: RawTarget) -> Self {
        Self {
            name: raw.name,
            kinds: raw.kind,
        }
    }
}
