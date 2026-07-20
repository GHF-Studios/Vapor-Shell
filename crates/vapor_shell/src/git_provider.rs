//! Developer Git provider discovery and linking.
//!
//! Git is not a player-mode dependency. Commands that explicitly need Git use
//! a developer provider resolved from `VAPOR_GIT`, app-local state, or the host
//! PATH/common install locations.

use crate::discovery::InstallationPaths;
use serde::{Deserialize, Serialize};
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

const PROVIDER_FILE: &str = "providers/git.toml";
const GIT_ENV: &str = "VAPOR_GIT";

/// Resolved developer Git provider.
#[derive(Debug, Clone)]
pub(crate) struct GitProvider {
    path: PathBuf,
    source: GitProviderSource,
}

impl GitProvider {
    /// Git executable path.
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    /// Where the provider was resolved from.
    pub(crate) fn source(&self) -> GitProviderSource {
        self.source
    }
}

/// Source of a resolved Git provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GitProviderSource {
    /// Explicit `VAPOR_GIT`.
    Environment,
    /// Persisted app-local provider state.
    Linked,
    /// Executable found through PATH.
    Path,
    /// Executable found in an OS-specific common location.
    CommonLocation,
}

impl GitProviderSource {
    /// Human-readable source label.
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Environment => "VAPOR_GIT",
            Self::Linked => "linked provider",
            Self::Path => "PATH",
            Self::CommonLocation => "common location",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GitProviderConfig {
    path: PathBuf,
}

/// Build a command for the resolved Git provider.
///
/// # Errors
///
/// Returns an actionable error if Git cannot be discovered or linked.
pub(crate) fn command(installation: &InstallationPaths) -> Result<Command, String> {
    Ok(Command::new(resolve(installation)?.path))
}

/// Resolve the developer Git provider.
///
/// # Errors
///
/// Returns an actionable error if no usable Git provider exists.
pub(crate) fn resolve(installation: &InstallationPaths) -> Result<GitProvider, String> {
    if let Some(value) = env::var_os(GIT_ENV).filter(|value| !value.is_empty()) {
        let path = PathBuf::from(value);
        return verify(path, GitProviderSource::Environment);
    }

    if let Some(path) = linked_path(installation)? {
        return verify(path, GitProviderSource::Linked);
    }

    for path in path_candidates() {
        if let Ok(provider) = verify(path, GitProviderSource::Path) {
            return Ok(provider);
        }
    }

    for path in common_candidates() {
        if let Ok(provider) = verify(path, GitProviderSource::CommonLocation) {
            return Ok(provider);
        }
    }

    Err(format!(
        "developer Git provider is not linked\nhelp: run `provider git link /path/to/git` inside Vapor Shell, or set {GIT_ENV}=/path/to/git\nnote: normal Steam play does not require Git"
    ))
}

/// Persist an explicit developer Git provider.
///
/// # Errors
///
/// Returns filesystem or Git health-check errors.
pub(crate) fn link(installation: &InstallationPaths, path: &Path) -> Result<GitProvider, String> {
    let provider = verify(
        canonical_file(path, "Git executable")?,
        GitProviderSource::Linked,
    )?;
    let config = GitProviderConfig {
        path: provider.path.clone(),
    };
    let target = config_path(installation);
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "failed to create Git provider state directory '{}': {error}",
                parent.display()
            )
        })?;
    }
    let source = toml::to_string_pretty(&config)
        .map_err(|error| format!("failed to encode Git provider state: {error}"))?;
    fs::write(&target, source).map_err(|error| {
        format!(
            "failed to write Git provider state '{}': {error}",
            target.display()
        )
    })?;
    Ok(provider)
}

/// Clear the persisted developer Git provider.
///
/// # Errors
///
/// Returns filesystem errors.
pub(crate) fn clear(installation: &InstallationPaths) -> Result<bool, String> {
    let target = config_path(installation);
    if !target.exists() {
        return Ok(false);
    }
    fs::remove_file(&target).map_err(|error| {
        format!(
            "failed to remove Git provider state '{}': {error}",
            target.display()
        )
    })?;
    Ok(true)
}

/// Read the linked Git provider path without validating it.
///
/// # Errors
///
/// Returns read or TOML parse errors.
pub(crate) fn linked_path(installation: &InstallationPaths) -> Result<Option<PathBuf>, String> {
    let target = config_path(installation);
    if !target.is_file() {
        return Ok(None);
    }
    let source = fs::read_to_string(&target).map_err(|error| {
        format!(
            "failed to read Git provider state '{}': {error}",
            target.display()
        )
    })?;
    let config = toml::from_str::<GitProviderConfig>(&source).map_err(|error| {
        format!(
            "failed to parse Git provider state '{}': {error}",
            target.display()
        )
    })?;
    Ok(Some(config.path))
}

fn config_path(installation: &InstallationPaths) -> PathBuf {
    installation.state_dir().join(PROVIDER_FILE)
}

fn verify(path: PathBuf, source: GitProviderSource) -> Result<GitProvider, String> {
    if !is_executable(&path) {
        return Err(format!(
            "Git provider is not executable: {}",
            path.display()
        ));
    }
    let output = Command::new(&path)
        .arg("--version")
        .output()
        .map_err(|error| format!("failed to start Git '{}': {error}", path.display()))?;
    if !output.status.success() {
        return Err(format!(
            "Git provider '{}' failed health check with {}",
            path.display(),
            output.status
        ));
    }
    Ok(GitProvider { path, source })
}

fn path_candidates() -> Vec<PathBuf> {
    env::var_os("PATH")
        .into_iter()
        .flat_map(|paths| env::split_paths(&paths).collect::<Vec<_>>())
        .map(|directory| directory.join(executable("git")))
        .collect()
}

fn common_candidates() -> Vec<PathBuf> {
    if cfg!(windows) {
        [
            r"C:\Program Files\Git\cmd\git.exe",
            r"C:\Program Files\Git\bin\git.exe",
            r"C:\Program Files (x86)\Git\cmd\git.exe",
            r"C:\Program Files (x86)\Git\bin\git.exe",
        ]
        .into_iter()
        .map(PathBuf::from)
        .collect()
    } else {
        ["/usr/bin/git", "/usr/local/bin/git", "/bin/git"]
            .into_iter()
            .map(PathBuf::from)
            .collect()
    }
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

fn executable(name: &str) -> String {
    format!("{name}{}", env::consts::EXE_SUFFIX)
}

fn is_executable(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::metadata(path)
            .map(|metadata| metadata.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
    }
    #[cfg(not(unix))]
    {
        true
    }
}
