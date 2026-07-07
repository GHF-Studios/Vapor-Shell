//! Explicit installation and inspection of app-local development tools.
//!
//! Tool packages are delivered under the app root's `packages/toolchain`. Nothing
//! is acquired from the host operating system, and normal workflow commands
//! never invoke installation implicitly.

use crate::{
    discovery::{InstallationPaths, ensure_contained},
    path_setup::{PathSetup, PathSetupReport},
};
use serde::{Deserialize, Serialize};
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

const LOCATION_LOCK: &str = "state/vapor-home.toml";

/// Relationship between the running app root and its explicitly accepted path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LocationStatus {
    /// No app location has been accepted yet.
    Unregistered {
        /// Canonical root inferred from the running executable.
        current: PathBuf,
    },
    /// The lock and running executable agree.
    Registered {
        /// Canonical accepted app root.
        path: PathBuf,
    },
    /// The lock moved with the app and still records its previous location.
    Moved {
        /// Previously accepted app root.
        locked: PathBuf,
        /// Current canonical app root.
        current: PathBuf,
    },
}

impl LocationStatus {
    /// Current app root inferred from the running executable.
    pub fn current(&self) -> &Path {
        match self {
            Self::Unregistered { current } | Self::Moved { current, .. } => current,
            Self::Registered { path } => path,
        }
    }

    /// Previously accepted path, when one exists.
    pub fn locked(&self) -> Option<&Path> {
        match self {
            Self::Registered { path } => Some(path),
            Self::Moved { locked, .. } => Some(locked),
            Self::Unregistered { .. } => None,
        }
    }

    /// Whether current and explicitly accepted paths agree.
    pub fn registered(&self) -> bool {
        matches!(self, Self::Registered { .. })
    }
}

/// Result of changing app-root location registration.
#[derive(Debug, Clone)]
pub struct LocationChange {
    status: LocationStatus,
    path_setup: PathSetupReport,
}

impl LocationChange {
    /// Post-operation location status.
    pub fn status(&self) -> &LocationStatus {
        &self.status
    }

    /// PATH registration changes made by the explicit operation.
    pub fn path_setup(&self) -> &PathSetupReport {
        &self.path_setup
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct LocationLock {
    version: u32,
    path: PathBuf,
}

/// App-local tool requirement used by command preflight checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Requirement {
    /// Rustup, Cargo, Rustc, Rustfmt, Clippy, and Rustdoc.
    Rust,
    /// Portable Git distribution.
    Git,
    /// SteamCMD distribution.
    SteamCmd,
}

impl Requirement {
    /// Human-readable tool-group name used in diagnostics.
    pub fn label(self) -> &'static str {
        match self {
            Self::Rust => "Rust toolchain",
            Self::Git => "Git",
            Self::SteamCmd => "SteamCMD",
        }
    }
}

/// Health of one required app-local tool group.
#[derive(Debug, Clone)]
pub struct ToolStatus {
    label: &'static str,
    installed: bool,
    path: PathBuf,
    missing: Vec<String>,
}

impl ToolStatus {
    /// Human-readable tool-group name.
    pub fn label(&self) -> &str {
        self.label
    }

    /// Whether every required executable is present.
    pub fn installed(&self) -> bool {
        self.installed
    }

    /// Primary expected executable or toolchain directory.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Missing executable names within this group.
    pub fn missing(&self) -> &[String] {
        &self.missing
    }
}

/// Complete status of Rust, Git, SteamCMD, and their install packages.
#[derive(Debug, Clone)]
pub struct ToolchainStatus {
    rust: ToolStatus,
    git: ToolStatus,
    steamcmd: ToolStatus,
    packages: PackageStatus,
}

impl ToolchainStatus {
    /// App-local Rust status.
    pub fn rust(&self) -> &ToolStatus {
        &self.rust
    }

    /// App-local Git status.
    pub fn git(&self) -> &ToolStatus {
        &self.git
    }

    /// App-local SteamCMD status.
    pub fn steamcmd(&self) -> &ToolStatus {
        &self.steamcmd
    }

    /// Whether every active tool is installed.
    pub fn complete(&self) -> bool {
        self.rust.installed && self.git.installed && self.steamcmd.installed
    }

    /// Whether all vendored packages required by `toolchain install` exist.
    pub fn packages_complete(&self) -> bool {
        self.packages.missing.is_empty()
    }

    /// Missing vendored package entries.
    pub fn missing_packages(&self) -> &[String] {
        &self.packages.missing
    }

    /// Root of the immutable package used by installation and repair.
    pub fn packages_root(&self) -> &Path {
        &self.packages.root
    }

    /// Status of one requested tool group.
    pub fn requirement(&self, requirement: Requirement) -> &ToolStatus {
        match requirement {
            Requirement::Rust => &self.rust,
            Requirement::Git => &self.git,
            Requirement::SteamCmd => &self.steamcmd,
        }
    }
}

#[derive(Debug, Clone)]
struct PackageStatus {
    root: PathBuf,
    missing: Vec<String>,
}

/// Result of an explicit toolchain installation or repair.
#[derive(Debug, Clone)]
pub struct InstallReport {
    installed_groups: Vec<&'static str>,
    status: ToolchainStatus,
}

/// Result of explicit toolchain removal.
#[derive(Debug, Clone)]
pub struct UninstallReport {
    removed_paths: usize,
    location: LocationChange,
}

impl UninstallReport {
    /// Number of app-local tool directories removed.
    pub fn removed_paths(&self) -> usize {
        self.removed_paths
    }

    /// PATH and location-lock changes made during removal.
    pub fn location(&self) -> &LocationChange {
        &self.location
    }
}

impl InstallReport {
    /// Tool groups whose package files were applied.
    pub fn installed_groups(&self) -> &[&'static str] {
        &self.installed_groups
    }

    /// Post-install status.
    pub fn status(&self) -> &ToolchainStatus {
        &self.status
    }
}

/// Compare the executable-derived app root with its persisted fixpoint.
///
/// # Errors
///
/// Fails when an existing lock cannot be read or parsed.
pub fn location_status(installation: &InstallationPaths) -> Result<LocationStatus, String> {
    let current = installation.root().to_path_buf();
    let path = installation.root().join(LOCATION_LOCK);
    if !path.is_file() {
        return Ok(LocationStatus::Unregistered { current });
    }
    let source = fs::read_to_string(&path)
        .map_err(|error| format!("failed to read app-root lock '{}': {error}", path.display()))?;
    let lock: LocationLock = toml::from_str(&source).map_err(|error| {
        format!(
            "failed to parse app-root lock '{}': {error}",
            path.display()
        )
    })?;
    if lock.version != 1 {
        return Err(format!(
            "unsupported app-root lock version {} in '{}'",
            lock.version,
            path.display()
        ));
    }
    if lock.path == current {
        Ok(LocationStatus::Registered { path: current })
    } else {
        Ok(LocationStatus::Moved {
            locked: lock.path,
            current,
        })
    }
}

/// Explicitly accept the executable-derived app root and register its `bin`.
///
/// # Errors
///
/// Fails when PATH registration or lock persistence fails.
pub fn register_location(installation: &InstallationPaths) -> Result<LocationChange, String> {
    let setup = PathSetup::from_installation(installation)?;
    register_location_with_setup(installation, &setup)
}

/// Explicitly accept the app root using a caller-provided PATH registration plan.
///
/// This supports controlled hosts and tests without changing the location-lock
/// semantics.
///
/// # Errors
///
/// Fails when PATH registration or lock persistence fails.
pub fn register_location_with_setup(
    installation: &InstallationPaths,
    setup: &PathSetup,
) -> Result<LocationChange, String> {
    let path_setup = setup.install()?;
    let lock_path = installation.root().join(LOCATION_LOCK);
    if let Some(parent) = lock_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "failed to create app-root state directory '{}': {error}",
                parent.display()
            )
        })?;
    }
    let lock = LocationLock {
        version: 1,
        path: installation.root().to_path_buf(),
    };
    let source = toml::to_string_pretty(&lock)
        .map_err(|error| format!("failed to encode app-root lock: {error}"))?;
    fs::write(&lock_path, source).map_err(|error| {
        format!(
            "failed to persist app-root lock '{}': {error}",
            lock_path.display()
        )
    })?;
    Ok(LocationChange {
        status: LocationStatus::Registered {
            path: installation.root().to_path_buf(),
        },
        path_setup,
    })
}

/// Explicitly remove the app-root fixpoint and marked PATH registration.
///
/// # Errors
///
/// Fails when the lock or PATH registration cannot be removed.
pub fn clear_location_registration(
    installation: &InstallationPaths,
) -> Result<LocationChange, String> {
    let setup = PathSetup::from_installation(installation)?;
    let path_setup = setup.uninstall()?;
    let lock_path = installation.root().join(LOCATION_LOCK);
    if lock_path.exists() {
        fs::remove_file(&lock_path).map_err(|error| {
            format!(
                "failed to remove app-root lock '{}': {error}",
                lock_path.display()
            )
        })?;
    }
    Ok(LocationChange {
        status: LocationStatus::Unregistered {
            current: installation.root().to_path_buf(),
        },
        path_setup,
    })
}

/// Require explicit acceptance of the current app root.
///
/// # Errors
///
/// Explains an unregistered or moved location without changing it.
pub fn require_registered_location(
    installation: &InstallationPaths,
    action: &str,
) -> Result<(), String> {
    let status = location_status(installation)?;
    require_registered_status(&status, action)
}

/// Require an already-resolved app-root status to be accepted.
///
/// # Errors
///
/// Explains an unregistered or moved location without changing it.
pub fn require_registered_status(status: &LocationStatus, action: &str) -> Result<(), String> {
    match status {
        LocationStatus::Registered { .. } => Ok(()),
        LocationStatus::Unregistered { current } => Err(format!(
            "cannot {action}: the app root has not been accepted\n  current: {}\nhelp: review this location with `vapor toolchain status`\nhelp: accept it explicitly with `vapor toolchain install`\nnote: no location or PATH state was changed",
            current.display()
        )),
        LocationStatus::Moved { locked, current } => Err(format!(
            "cannot {action}: the app root no longer matches its accepted location\n  previous: {}\n  current:  {}\nhelp: if this move was intentional, run `vapor toolchain repair`\nhelp: otherwise move the Steam app back or verify its library location\nnote: no location or PATH state was changed",
            locked.display(),
            current.display()
        )),
    }
}

/// Inspect active tools and vendored install packages.
pub fn inspect(installation: &InstallationPaths) -> ToolchainStatus {
    inspect_root(installation.root())
}

/// Install missing tools.
///
/// # Errors
///
/// Fails when the app location is not accepted, packages are incomplete, a path
/// escapes the installation, copying fails, or verification remains incomplete.
pub fn install(installation: &InstallationPaths) -> Result<InstallReport, String> {
    apply_packages(installation, false)
}

/// Reapply every vendored tool package.
///
/// # Errors
///
/// Fails under the same conditions as [`install`].
pub fn repair(installation: &InstallationPaths) -> Result<InstallReport, String> {
    apply_packages(installation, true)
}

/// Remove app-local Rust/Cargo, Git, SteamCMD, PATH registration, and location lock.
///
/// # Errors
///
/// Fails when a managed path escapes the app root or removal cannot complete.
pub fn uninstall(installation: &InstallationPaths) -> Result<UninstallReport, String> {
    let root = installation.root();
    let mut removed_paths = 0;
    for relative in [
        "rustup",
        "rustup-home",
        "cargo-home",
        "tools/git",
        "tools/steamcmd",
    ] {
        let path = root.join(relative);
        ensure_contained(root, &path)?;
        if path.exists() {
            fs::remove_dir_all(&path)
                .map_err(|error| format!("failed to remove '{}': {error}", path.display()))?;
            removed_paths += 1;
        }
    }
    let location = clear_location_registration(installation)?;
    Ok(UninstallReport {
        removed_paths,
        location,
    })
}

/// Install missing tools, or reapply every package when `repair` is true.
///
/// Packages are copied only within the Steam application root. Existing extra
/// state, including SteamCMD authentication data, is preserved.
///
/// # Errors
///
/// Fails when a vendored package is incomplete, a path escapes the installation,
/// copying fails, or post-install verification remains incomplete.
fn apply_packages(installation: &InstallationPaths, repair: bool) -> Result<InstallReport, String> {
    require_registered_location(installation, "install the toolchain")?;
    let before = inspect(installation);
    if !before.packages_complete() {
        return Err(format!(
            "the Steam application does not contain a complete toolchain package\nmissing package entries:\n  - {}\nhelp: verify the app's files in Steam; bootstrap builds must populate '{}'",
            before.missing_packages().join("\n  - "),
            before.packages.root.display()
        ));
    }

    let root = installation.root();
    let package = &before.packages.root;
    let mut installed_groups = Vec::new();
    if repair || !before.rust.installed() {
        copy_package(root, &package.join("rustup"), &root.join("rustup"))?;
        copy_package(
            root,
            &package.join("rustup-home"),
            &root.join("rustup-home"),
        )?;
        copy_package(root, &package.join("cargo-home"), &root.join("cargo-home"))?;
        installed_groups.push("Rust toolchain");
    }
    if repair || !before.git.installed() {
        copy_package(root, &package.join("git"), &root.join("tools/git"))?;
        installed_groups.push("Git");
    }
    if repair || !before.steamcmd.installed() {
        copy_package(
            root,
            &package.join("steamcmd"),
            &root.join("tools/steamcmd"),
        )?;
        installed_groups.push("SteamCMD");
    }

    let status = inspect(installation);
    if !status.complete() {
        return Err(format!(
            "toolchain package application completed, but verification still fails\n{}",
            format_missing(&status)
        ));
    }
    Ok(InstallReport {
        installed_groups,
        status,
    })
}

/// Require selected tools without attempting repair or installation.
///
/// # Errors
///
/// Returns a diagnostic naming missing components and explicit next commands.
pub fn require(
    installation: &InstallationPaths,
    requirements: &[Requirement],
    action: &str,
) -> Result<(), String> {
    let status = inspect(installation);
    require_status(&status, requirements, action)
}

/// Require selected tools using an already-resolved toolchain status.
///
/// # Errors
///
/// Returns a diagnostic naming missing components and explicit next commands.
pub fn require_status(
    status: &ToolchainStatus,
    requirements: &[Requirement],
    action: &str,
) -> Result<(), String> {
    let missing = requirements
        .iter()
        .copied()
        .filter(|requirement| !status.requirement(*requirement).installed())
        .collect::<Vec<_>>();
    if missing.is_empty() {
        return Ok(());
    }
    Err(format!(
        "cannot {action}: the app-local {} {} not installed\n{}\nhelp: inspect the installation with `vapor toolchain status`\nhelp: install the vendored tools explicitly with `vapor toolchain install`\nnote: this command will not install or repair prerequisites automatically",
        missing
            .iter()
            .map(|requirement| requirement.label())
            .collect::<Vec<_>>()
            .join(", "),
        if missing.len() == 1 { "is" } else { "are" },
        format_missing_selected(status, &missing)
    ))
}

/// Validate the immutable package used for fresh app-local installations.
///
/// # Errors
///
/// Lists missing package directories or executables.
pub fn validate_package(package: &Path) -> Result<(), String> {
    let status = inspect_package_at(package);
    if status.missing.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "vendored toolchain package is incomplete at '{}'\nmissing package entries:\n  - {}\nhelp: verify the Steam app files or rebuild the bootstrap package",
            package.display(),
            status.missing.join("\n  - ")
        ))
    }
}

fn inspect_root(root: &Path) -> ToolchainStatus {
    let rustup = root.join("rustup/bin").join(executable("rustup"));
    let toolchains = root.join("rustup-home/toolchains");
    let (rust_bin, rust_missing) = inspect_rust(&toolchains, Some(root));
    let mut missing = rust_missing;
    if !is_healthy_executable(&rustup, root) {
        missing.push("rustup".to_owned());
    }
    let rust = ToolStatus {
        label: "Rust toolchain",
        installed: missing.is_empty(),
        path: rust_bin.unwrap_or(toolchains),
        missing,
    };

    let git_path = root.join("tools/git/bin").join(executable("git"));
    let git_installed = is_healthy_executable(&git_path, root);
    let git = ToolStatus {
        label: "Git",
        installed: git_installed,
        path: git_path,
        missing: if git_installed {
            Vec::new()
        } else {
            vec!["git".to_owned()]
        },
    };

    let steam_path = steam_executable(root);
    let steam_installed = is_executable(&steam_path);
    let steamcmd = ToolStatus {
        label: "SteamCMD",
        installed: steam_installed,
        path: steam_path,
        missing: if steam_installed {
            Vec::new()
        } else {
            vec!["steamcmd".to_owned()]
        },
    };

    ToolchainStatus {
        rust,
        git,
        steamcmd,
        packages: inspect_packages(root),
    }
}

fn inspect_rust(toolchains: &Path, active_root: Option<&Path>) -> (Option<PathBuf>, Vec<String>) {
    let required = ["cargo", "rustc", "rustfmt", "cargo-clippy", "rustdoc"];
    let mut candidates = fs::read_dir(toolchains)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
        .map(|entry| entry.path().join("bin"))
        .collect::<Vec<_>>();
    candidates.sort();
    for bin in &candidates {
        let missing = required
            .iter()
            .filter(|name| {
                let path = bin.join(executable(name));
                active_root.map_or_else(
                    || !is_executable(&path),
                    |root| !is_healthy_executable(&path, root),
                )
            })
            .map(|name| (*name).to_owned())
            .collect::<Vec<_>>();
        if missing.is_empty() {
            return (Some(bin.clone()), Vec::new());
        }
    }
    (
        candidates.into_iter().next(),
        required.iter().map(|name| (*name).to_owned()).collect(),
    )
}

fn inspect_packages(root: &Path) -> PackageStatus {
    inspect_package_at(&root.join("packages/toolchain"))
}

fn inspect_package_at(package: &Path) -> PackageStatus {
    let mut missing = Vec::new();
    for directory in [
        "rustup",
        "rustup-home",
        "cargo-home",
        "cargo-home/registry",
        "git",
        "steamcmd",
    ] {
        if !package.join(directory).is_dir() {
            missing.push(directory.to_owned());
        }
    }
    let package_status = inspect_package_tools(package);
    missing.extend(package_status);
    missing.sort();
    missing.dedup();
    PackageStatus {
        root: package.to_path_buf(),
        missing,
    }
}

fn inspect_package_tools(package: &Path) -> Vec<String> {
    let mut missing = Vec::new();
    if !is_executable(&package.join("rustup/bin").join(executable("rustup"))) {
        missing.push(format!("rustup/bin/{}", executable("rustup")));
    }
    let (_, rust_missing) = inspect_rust(&package.join("rustup-home/toolchains"), None);
    missing.extend(
        rust_missing
            .into_iter()
            .map(|name| format!("rustup-home/toolchains/*/bin/{}", executable(&name))),
    );
    if !is_executable(&package.join("git/bin").join(executable("git"))) {
        missing.push(format!("git/bin/{}", executable("git")));
    }
    if !steam_candidates(&package.join("steamcmd"))
        .iter()
        .any(|path| is_executable(path))
    {
        missing.push("steamcmd/steamcmd[.sh|.exe]".to_owned());
    }
    missing
}

fn copy_package(installation: &Path, source: &Path, destination: &Path) -> Result<(), String> {
    ensure_contained(installation, source)?;
    ensure_contained(installation, destination)?;
    copy_tree(source, destination, source)
}

fn copy_tree(source: &Path, destination: &Path, package_root: &Path) -> Result<(), String> {
    let canonical = fs::canonicalize(source)
        .map_err(|error| format!("failed to resolve package '{}': {error}", source.display()))?;
    ensure_contained(package_root, &canonical)?;
    let metadata = fs::metadata(&canonical)
        .map_err(|error| format!("failed to inspect package '{}': {error}", source.display()))?;
    if metadata.is_dir() {
        fs::create_dir_all(destination).map_err(|error| {
            format!(
                "failed to create tool directory '{}': {error}",
                destination.display()
            )
        })?;
        for entry in fs::read_dir(&canonical)
            .map_err(|error| format!("failed to read package '{}': {error}", canonical.display()))?
        {
            let entry = entry.map_err(|error| format!("failed to read package entry: {error}"))?;
            copy_tree(
                &entry.path(),
                &destination.join(entry.file_name()),
                package_root,
            )?;
        }
    } else if metadata.is_file() {
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                format!(
                    "failed to create tool directory '{}': {error}",
                    parent.display()
                )
            })?;
        }
        fs::copy(&canonical, destination).map_err(|error| {
            format!(
                "failed to install '{}' at '{}': {error}",
                canonical.display(),
                destination.display()
            )
        })?;
    }
    Ok(())
}

fn format_missing(status: &ToolchainStatus) -> String {
    format_missing_selected(
        status,
        &[Requirement::Rust, Requirement::Git, Requirement::SteamCmd],
    )
}

fn format_missing_selected(status: &ToolchainStatus, requirements: &[Requirement]) -> String {
    requirements
        .iter()
        .filter_map(|requirement| {
            let tool = status.requirement(*requirement);
            (!tool.installed()).then(|| {
                format!(
                    "  - {}: missing {} (expected under {})",
                    tool.label(),
                    tool.missing().join(", "),
                    tool.path().display()
                )
            })
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn steam_executable(root: &Path) -> PathBuf {
    let directory = root.join("tools/steamcmd");
    steam_candidates(&directory)
        .into_iter()
        .find(|path| is_executable(path))
        .unwrap_or_else(|| directory.join(executable("steamcmd")))
}

fn steam_candidates(directory: &Path) -> Vec<PathBuf> {
    if cfg!(windows) {
        vec![directory.join("steamcmd.exe")]
    } else {
        vec![directory.join("steamcmd"), directory.join("steamcmd.sh")]
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
        fs::metadata(path).is_ok_and(|metadata| metadata.permissions().mode() & 0o111 != 0)
    }
    #[cfg(not(unix))]
    {
        true
    }
}

fn is_healthy_executable(path: &Path, root: &Path) -> bool {
    is_executable(path)
        && Command::new(path)
            .arg("--version")
            .env("VAPOR_HOME", root)
            .env("CARGO_HOME", root.join("cargo-home"))
            .env("RUSTUP_HOME", root.join("rustup-home"))
            .output()
            .is_ok_and(|output| output.status.success())
}
