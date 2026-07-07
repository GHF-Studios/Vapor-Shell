//! Discovery of the replaceable Steam installation and external source root.
//!
//! # Two-root model
//!
//! Vapor deliberately separates replaceable machinery from authored source:
//!
//! - [`InstallationPaths`] is anchored to the running executable. It contains
//!   bundled tools, toolchains, binaries, libraries, and installed content.
//! - [`SourceWorkspace`] is anchored to the invocation directory. It contains
//!   critical authored source and must be outside the installation.
//!
//! [`EnvironmentPaths`] discovers both roots and rejects overlapping layouts.
//! User navigation is confined to the source root; installation paths are
//! resources that commands may inspect or execute explicitly.

use crate::manifest::{self, VaporEntity};
use std::{
    env, fs,
    path::{Path, PathBuf},
};

/// Conventional installation directory containing runtime libraries.
pub const LIBRARY_DIR: &str = "lib";

/// Both independent roots required by one shell session.
#[derive(Debug, Clone)]
pub struct EnvironmentPaths {
    installation: InstallationPaths,
    source: SourceWorkspace,
}

impl EnvironmentPaths {
    /// Discover the installation from the executable and source from the caller.
    ///
    /// # Errors
    ///
    /// Fails when either root lacks a valid workspace marker or when the source
    /// workspace overlaps the replaceable installation.
    pub fn discover() -> Result<Self, String> {
        let executable = env::current_exe()
            .map_err(|error| format!("failed to locate the running Vapor shell: {error}"))?;
        let installation = InstallationPaths::from_executable(&executable)?;
        let invocation =
            configured_source(&installation)?
                .unwrap_or(env::current_dir().map_err(|error| {
                    format!("failed to read the invocation directory: {error}")
                })?);
        let source = SourceWorkspace::from_invocation(&invocation, &installation)?;
        Ok(Self {
            installation,
            source,
        })
    }

    /// Discover both roots from explicit paths.
    ///
    /// This is useful to launch Vapor from another host process and to test an
    /// installation layout without changing process-global state.
    ///
    /// # Errors
    ///
    /// Returns the same validation errors as [`Self::discover`].
    pub fn from_paths(executable: &Path, invocation: &Path) -> Result<Self, String> {
        let installation = InstallationPaths::from_executable(executable)?;
        let source = SourceWorkspace::from_invocation(invocation, &installation)?;
        Ok(Self {
            installation,
            source,
        })
    }

    /// Replaceable Steam application paths.
    pub fn installation(&self) -> &InstallationPaths {
        &self.installation
    }

    /// External authored-source workspace.
    pub fn source(&self) -> &SourceWorkspace {
        &self.source
    }
}

fn configured_source(installation: &InstallationPaths) -> Result<Option<PathBuf>, String> {
    if let Some(path) = env::var_os("VAPOR_WORKSPACE").filter(|value| !value.is_empty()) {
        return Ok(Some(PathBuf::from(path)));
    }
    let state = installation.root().join("state/source-workspace");
    if !state.is_file() {
        return Ok(None);
    }
    let value = fs::read_to_string(&state)
        .map_err(|error| format!("failed to read '{}': {error}", state.display()))?;
    let value = value.trim();
    if value.is_empty() {
        Ok(None)
    } else {
        Ok(Some(PathBuf::from(value)))
    }
}

/// Paths owned by the replaceable Steam installation.
#[derive(Debug, Clone)]
pub struct InstallationPaths {
    executable: PathBuf,
    root: PathBuf,
    binaries: PathBuf,
    libraries: Option<PathBuf>,
    cargo: Option<PathBuf>,
    identity_id: String,
}

impl InstallationPaths {
    /// Discover an installation from the canonical executable location.
    ///
    /// The highest ancestor `Vapor.toml` must declare `[root]` and becomes
    /// the installation root.
    ///
    /// # Errors
    ///
    /// Fails for a missing executable, missing marker, invalid marker, or a
    /// highest marker that describes anything except the application root.
    pub fn from_executable(executable: &Path) -> Result<Self, String> {
        let executable = canonical_file(executable, "Vapor shell executable")?;
        let binaries = executable
            .parent()
            .ok_or_else(|| format!("executable has no parent: {}", executable.display()))?
            .to_path_buf();
        let marker = highest_marker(&binaries).ok_or_else(|| {
            format!(
                "the Vapor shell at '{}' is not installed inside a Vapor installation: no {} exists in any executable ancestor",
                executable.display(),
                manifest::FILE_NAME
            )
        })?;
        let root = marker
            .parent()
            .expect("an ancestor marker always has a parent")
            .to_path_buf();
        let expected_binaries = root.join("bin");
        let expected_name = format!("vapor{}", env::consts::EXE_SUFFIX);
        if binaries != expected_binaries
            || executable
                .file_name()
                .is_none_or(|name| name != expected_name.as_str())
        {
            return Err(format!(
                "the running executable is not laid out as an installed Vapor application\n  executable: {}\n  candidate app root: {}\n  expected command: {}\nnote: this usually means a source-built target/debug/vapor was run directly\nhelp: place the bootstrap application outside every source root and run its bin/vapor command",
                executable.display(),
                root.display(),
                expected_binaries.join(expected_name).display()
            ));
        }
        let identity_id = require_installation_marker(&marker, &root)?;

        let libraries = optional_directory(&root, LIBRARY_DIR)?;
        let cargo = bundled_cargo_candidates(&root)
            .into_iter()
            .find(|candidate| candidate.is_file());

        Ok(Self {
            executable,
            root,
            binaries,
            libraries,
            cargo,
            identity_id,
        })
    }

    /// Canonical running executable.
    pub fn executable(&self) -> &Path {
        &self.executable
    }

    /// Steam application root and machinery boundary.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Directory containing the running shell executable.
    pub fn binaries(&self) -> &Path {
        &self.binaries
    }

    /// Conventional `lib` directory, when installed.
    pub fn libraries(&self) -> Option<&Path> {
        self.libraries.as_deref()
    }

    /// Bundled Cargo executable, when present in a supported installation path.
    pub fn cargo(&self) -> Option<&Path> {
        self.cargo.as_deref()
    }

    /// Rescan the installation for Cargo after an explicit toolchain install.
    pub fn bundled_cargo(&self) -> Option<PathBuf> {
        bundled_cargo_candidates(&self.root)
            .into_iter()
            .find(|candidate| candidate.is_file())
    }

    /// Stable identity declared by the installation marker.
    pub fn identity_id(&self) -> &str {
        &self.identity_id
    }
}

/// Critical authored source that is managed separately from the installation.
#[derive(Debug, Clone)]
pub struct SourceWorkspace {
    invocation: PathBuf,
    root: PathBuf,
    identity_id: String,
}

impl SourceWorkspace {
    /// Discover the highest source marker above `invocation`.
    ///
    /// # Errors
    ///
    /// Fails when the invocation directory is invalid, no marker exists, the
    /// highest marker is not a source root, or the source root overlaps the Steam
    /// installation.
    pub fn from_invocation(
        invocation: &Path,
        installation: &InstallationPaths,
    ) -> Result<Self, String> {
        let invocation = canonical_directory(invocation, "invocation directory")?;
        let marker = highest_marker(&invocation).ok_or_else(|| {
            format!(
                    "'{}' is not inside an external Vapor source root: no {} exists in any ancestor\nhelp: invoke Vapor from a source repository, not from the Steam app directory",
                invocation.display(),
                manifest::FILE_NAME
            )
        })?;
        let root = marker
            .parent()
            .expect("an ancestor marker always has a parent")
            .to_path_buf();
        let identity_id = require_source_marker(&marker, &root)?;

        if roots_overlap(&root, installation.root()) {
            if root == installation.root() {
                return Err(format!(
                    "no external source root is selected: invocation resolved the Steam app root itself\n  app root: {}\nhelp: invoke Vapor from a separate source repository, or open the shell and select a remembered source root",
                    root.display()
                ));
            }
            return Err(format!(
                "the selected source root and Steam app root are not disjoint\n  source root: {}\n  app root:    {}\nhelp: keep authored repositories outside the replaceable Steam application tree",
                root.display(),
                installation.root().display()
            ));
        }

        Ok(Self {
            invocation,
            root,
            identity_id,
        })
    }

    /// Directory from which the shell was invoked.
    pub fn invocation(&self) -> &Path {
        &self.invocation
    }

    /// Highest external Vapor source root containing authored source.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Stable identity declared by the source marker.
    pub fn identity_id(&self) -> &str {
        &self.identity_id
    }

    /// Whether a canonical path belongs to this source workspace.
    pub fn contains(&self, path: &Path) -> bool {
        path.starts_with(&self.root)
    }
}

/// Reject a canonical path outside the given boundary.
///
/// # Errors
///
/// Returns an error containing both paths when `candidate` is not below `root`.
pub fn ensure_contained(root: &Path, candidate: &Path) -> Result<(), String> {
    if candidate.starts_with(root) {
        Ok(())
    } else {
        Err(format!(
            "Vapor source boundary violation: '{}' is outside '{}'",
            candidate.display(),
            root.display()
        ))
    }
}

fn highest_marker(start: &Path) -> Option<PathBuf> {
    start
        .ancestors()
        .filter_map(|directory| {
            let marker = directory.join(manifest::FILE_NAME);
            marker.is_file().then_some(marker)
        })
        .last()
}

fn require_installation_marker(marker: &Path, root: &Path) -> Result<String, String> {
    match manifest::read(marker, root)? {
        VaporEntity::Root { id, .. } => Ok(id),
        VaporEntity::Registry { id, .. } => Err(format!(
            "highest Vapor manifest '{}' describes registry '{id}', not the Steam application root; the installation must contain [root]",
            marker.display()
        )),
        VaporEntity::Workspace { id, .. } => Err(format!(
            "highest Vapor manifest '{}' describes workspace '{id}', not the Steam application root; the installation must contain [root]",
            marker.display()
        )),
        VaporEntity::Project { id, .. } => Err(format!(
            "highest Vapor manifest '{}' describes project '{id}', not the Steam application root; the installation must contain [root]",
            marker.display()
        )),
        VaporEntity::Content { kind, id, .. } => Err(format!(
            "highest Vapor manifest '{}' describes {kind} '{id}', not the Steam application root; the installation must contain [root]",
            marker.display()
        )),
    }
}

fn require_source_marker(marker: &Path, root: &Path) -> Result<String, String> {
    match manifest::read(marker, root)? {
        VaporEntity::Root { id, .. } | VaporEntity::Workspace { id, .. } => Ok(id),
        VaporEntity::Registry { id, .. } => Err(format!(
            "highest Vapor manifest '{}' describes registry '{id}', not a source root; open a [root] or [workspace] repository",
            marker.display()
        )),
        VaporEntity::Project { id, .. } => Err(format!(
            "highest Vapor manifest '{}' describes project '{id}', not a source root; projects must live inside a [workspace] repository",
            marker.display()
        )),
        VaporEntity::Content { kind, id, .. } => Err(format!(
            "highest Vapor manifest '{}' describes {kind} '{id}', not a source root; content must live inside a [workspace] repository",
            marker.display()
        )),
    }
}

fn roots_overlap(left: &Path, right: &Path) -> bool {
    left.starts_with(right) || right.starts_with(left)
}

fn canonical_file(path: &Path, label: &str) -> Result<PathBuf, String> {
    let canonical = fs::canonicalize(path)
        .map_err(|error| format!("failed to resolve {label} '{}': {error}", path.display()))?;
    if canonical.is_file() {
        Ok(canonical)
    } else {
        Err(format!("{label} is not a file: {}", canonical.display()))
    }
}

fn canonical_directory(path: &Path, label: &str) -> Result<PathBuf, String> {
    let canonical = fs::canonicalize(path)
        .map_err(|error| format!("failed to resolve {label} '{}': {error}", path.display()))?;
    if canonical.is_dir() {
        Ok(canonical)
    } else {
        Err(format!(
            "{label} is not a directory: {}",
            canonical.display()
        ))
    }
}

fn optional_directory(root: &Path, name: &str) -> Result<Option<PathBuf>, String> {
    let candidate = root.join(name);
    if !candidate.exists() {
        return Ok(None);
    }
    let canonical = canonical_directory(&candidate, name)?;
    ensure_contained(root, &canonical)?;
    Ok(Some(canonical))
}

fn bundled_cargo_candidates(root: &Path) -> Vec<PathBuf> {
    let executable = format!("cargo{}", env::consts::EXE_SUFFIX);
    let toolchains_root = root.join("rustup-home").join("toolchains");
    let selected_toolchain = env::var_os("RUSTUP_TOOLCHAIN");
    let mut toolchains = fs::read_dir(&toolchains_root)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
        .collect::<Vec<_>>();

    toolchains.sort_by_key(|entry| {
        let name = entry.file_name();
        let selected = selected_toolchain.as_deref().is_some_and(|value| {
            name.to_string_lossy()
                .starts_with(value.to_string_lossy().as_ref())
        });
        (!selected, name)
    });

    let mut candidates = toolchains
        .into_iter()
        .map(|entry| entry.path().join("bin").join(&executable))
        .collect::<Vec<_>>();
    candidates.push(root.join("cargo-home").join("bin").join(&executable));
    candidates.push(root.join("bin").join(executable));
    candidates
}
