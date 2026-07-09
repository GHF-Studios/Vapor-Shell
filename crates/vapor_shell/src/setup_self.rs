//! Explicit installation and inspection of the app-local Vapor setup.
//!
//! `setup self install` creates the one mandatory app-local tool bundle inside the
//! Steam app root. Normal workflow commands never invoke installation
//! implicitly.

use crate::{
    discovery::{InstallationPaths, ensure_contained},
    path_setup::{PathSetup, PathSetupReport},
    setup_self_packages,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

const LOCATION_LOCK: &str = "vapor-home.toml";
const RUSTUP_INIT_X86_64_LINUX: &str =
    "https://static.rust-lang.org/rustup/dist/x86_64-unknown-linux-gnu/rustup-init";
const RUSTUP_INIT_AARCH64_LINUX: &str =
    "https://static.rust-lang.org/rustup/dist/aarch64-unknown-linux-gnu/rustup-init";
const STEAMCMD_LINUX: &str =
    "https://steamcdn-a.akamaihd.net/client/installer/steamcmd_linux.tar.gz";

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
pub enum SetupSelfRequirement {
    /// Rustup, Cargo, Rustc, Rustfmt, Clippy, and Rustdoc.
    Rust,
    /// Portable Git distribution.
    Git,
    /// SteamCMD distribution.
    SteamCmd,
}

impl SetupSelfRequirement {
    /// Human-readable tool-group name used in diagnostics.
    pub fn label(self) -> &'static str {
        match self {
            Self::Rust => "Rust/Cargo",
            Self::Git => "Git",
            Self::SteamCmd => "SteamCMD",
        }
    }
}

/// Health of one required app-local tool group.
#[derive(Debug, Clone)]
pub struct SetupSelfComponentStatus {
    label: &'static str,
    installed: bool,
    path: PathBuf,
    missing: Vec<String>,
}

impl SetupSelfComponentStatus {
    /// Human-readable tool-group name.
    pub fn label(&self) -> &str {
        self.label
    }

    /// Whether every required executable is present.
    pub fn installed(&self) -> bool {
        self.installed
    }

    /// Primary expected executable or setup directory.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Missing executable names within this group.
    pub fn missing(&self) -> &[String] {
        &self.missing
    }
}

/// Complete status of active Rust, Git, SteamCMD, and self-setup payloads.
#[derive(Debug, Clone)]
pub struct SetupSelfStatus {
    rust: SetupSelfComponentStatus,
    git: SetupSelfComponentStatus,
    steamcmd: SetupSelfComponentStatus,
    package: setup_self_packages::SetupSelfPackageStatus,
}

impl SetupSelfStatus {
    /// App-local Rust status.
    pub fn rust(&self) -> &SetupSelfComponentStatus {
        &self.rust
    }

    /// App-local Git status.
    pub fn git(&self) -> &SetupSelfComponentStatus {
        &self.git
    }

    /// App-local SteamCMD status.
    pub fn steamcmd(&self) -> &SetupSelfComponentStatus {
        &self.steamcmd
    }

    /// Whether every active tool is installed.
    pub fn complete(&self) -> bool {
        self.rust.installed && self.git.installed && self.steamcmd.installed
    }

    /// Whether the distributable self-setup payload required for app staging exists.
    pub fn package_complete(&self) -> bool {
        self.package.complete()
    }

    /// Root of the distributable self-setup payload content.
    pub fn package_root(&self) -> &Path {
        self.package.root()
    }

    /// Missing self-setup payload entries.
    pub fn missing_package_entries(&self) -> &[String] {
        self.package.missing()
    }

    /// Status of one requested tool group.
    pub fn requirement(&self, requirement: SetupSelfRequirement) -> &SetupSelfComponentStatus {
        match requirement {
            SetupSelfRequirement::Rust => &self.rust,
            SetupSelfRequirement::Git => &self.git,
            SetupSelfRequirement::SteamCmd => &self.steamcmd,
        }
    }
}

/// Result of an explicit self-setup installation or repair.
#[derive(Debug, Clone)]
pub struct InstallReport {
    installed_groups: Vec<&'static str>,
    status: SetupSelfStatus,
}

/// Result of explicit setup removal.
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
    /// Tool groups installed or repaired during this run.
    pub fn installed_groups(&self) -> &[&'static str] {
        &self.installed_groups
    }

    /// Post-install status.
    pub fn status(&self) -> &SetupSelfStatus {
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
    let path = installation.state_dir().join(LOCATION_LOCK);
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
    let lock_path = installation.state_dir().join(LOCATION_LOCK);
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
    let lock_path = installation.state_dir().join(LOCATION_LOCK);
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
            "cannot {action}: the app root has not been accepted\n  current: {}\nhelp: review this location with `vapor setup self status`\nhelp: accept it explicitly with `vapor setup self install`\nnote: no location or PATH state was changed",
            current.display()
        )),
        LocationStatus::Moved { locked, current } => Err(format!(
            "cannot {action}: the app root no longer matches its accepted location\n  previous: {}\n  current:  {}\nhelp: if this move was intentional, run `vapor setup self repair`\nhelp: otherwise move the Steam app back or verify its library location\nnote: no location or PATH state was changed",
            locked.display(),
            current.display()
        )),
    }
}

/// Inspect the mandatory app-local setup.
pub fn inspect(installation: &InstallationPaths) -> SetupSelfStatus {
    inspect_root(installation.root())
}

/// Install missing tools.
///
/// # Errors
///
/// Fails when the app location is not accepted, acquisition fails, a path
/// escapes the installation, or verification remains incomplete.
pub fn install(installation: &InstallationPaths) -> Result<InstallReport, String> {
    apply_setup_self_install(installation, false)
}

/// Reinstall every app-local tool group.
///
/// # Errors
///
/// Fails under the same conditions as [`install`].
pub fn repair(installation: &InstallationPaths) -> Result<InstallReport, String> {
    apply_setup_self_install(installation, true)
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

/// Install missing tools, or reacquire every tool when `repair` is true.
///
/// # Errors
///
/// Fails when acquisition fails, a path escapes the installation, copying fails,
/// or post-install verification remains incomplete.
fn apply_setup_self_install(
    installation: &InstallationPaths,
    repair: bool,
) -> Result<InstallReport, String> {
    require_registered_location(installation, "install setup")?;
    let before = inspect(installation);

    let root = installation.root();
    let installed_groups = if before.package_complete() {
        setup_self_packages::copy_setup_self_package_to_active(
            root,
            &before.package,
            repair,
            before.rust.installed(),
            before.git.installed(),
            before.steamcmd.installed(),
        )?
    } else {
        bootstrap_tools(root, &before, repair)?
    };

    let status = inspect(installation);
    if !status.complete() {
        return Err(format!(
            "self-setup installation completed, but verification still fails\n{}",
            format_missing(&status)
        ));
    }
    Ok(InstallReport {
        installed_groups,
        status,
    })
}

fn bootstrap_tools(
    root: &Path,
    before: &SetupSelfStatus,
    repair: bool,
) -> Result<Vec<&'static str>, String> {
    if !cfg!(target_os = "linux") {
        return Err("self-setup installation is currently implemented only for Linux".to_owned());
    }
    let mut installed_groups = Vec::new();
    if repair || !before.rust.installed() {
        bootstrap_rust(root)?;
        installed_groups.push("Rust/Cargo");
    }
    if repair || !before.git.installed() {
        bootstrap_git(root)?;
        installed_groups.push("Git");
    }
    if repair || !before.steamcmd.installed() {
        bootstrap_steamcmd(root)?;
        installed_groups.push("SteamCMD");
    }
    Ok(installed_groups)
}

fn bootstrap_rust(root: &Path) -> Result<(), String> {
    let downloads = downloads_dir(root)?;
    let rustup_init = downloads.join("rustup-init");
    download(rustup_init_url()?, &rustup_init)?;
    make_executable(&rustup_init)?;
    let status = Command::new(&rustup_init)
        .args([
            "-y",
            "--no-modify-path",
            "--profile",
            "default",
            "--default-toolchain",
            "stable",
        ])
        .env("RUSTUP_HOME", root.join("rustup-home"))
        .env("CARGO_HOME", root.join("cargo-home"))
        .status()
        .map_err(|error| format!("failed to start rustup-init: {error}"))?;
    if !status.success() {
        return Err(format!("rustup-init exited with {status}"));
    }
    let source = root.join("cargo-home/bin").join(executable("rustup"));
    let target = root.join("rustup/bin").join(executable("rustup"));
    copy_file(root, &source, &target)?;
    Ok(())
}

fn bootstrap_steamcmd(root: &Path) -> Result<(), String> {
    let archive = downloads_dir(root)?.join("steamcmd_linux.tar.gz");
    download(STEAMCMD_LINUX, &archive)?;
    let target = root.join("tools/steamcmd");
    ensure_contained(root, &target)?;
    fs::create_dir_all(&target)
        .map_err(|error| format!("failed to create '{}': {error}", target.display()))?;
    let status = Command::new("tar")
        .args(["-xzf"])
        .arg(&archive)
        .arg("-C")
        .arg(&target)
        .status()
        .map_err(|error| format!("failed to start tar for SteamCMD archive: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("SteamCMD archive extraction exited with {status}"))
    }
}

fn bootstrap_git(root: &Path) -> Result<(), String> {
    let host = find_host_git(root)?;
    let target = root.join("tools/git");
    ensure_contained(root, &target)?;
    if target.exists() {
        fs::remove_dir_all(&target)
            .map_err(|error| format!("failed to reset '{}': {error}", target.display()))?;
    }

    let bin = target.join("bin");
    fs::create_dir_all(&bin)
        .map_err(|error| format!("failed to create '{}': {error}", bin.display()))?;

    if let Some(exec_path) = host.exec_path {
        copy_external_tree(root, &exec_path, &target.join("libexec/git-core"))?;
    }
    let app_git = target.join("libexec/git-core").join(executable("git"));
    if !is_executable(&app_git) {
        copy_external_file(root, &host.binary, &app_git)?;
    }
    if let Some(templates) = host.templates {
        copy_external_tree(root, &templates, &target.join("share/git-core/templates"))?;
    }
    write_git_launcher(root, &target)?;
    Ok(())
}

#[derive(Debug)]
struct HostGit {
    binary: PathBuf,
    exec_path: Option<PathBuf>,
    templates: Option<PathBuf>,
}

fn find_host_git(root: &Path) -> Result<HostGit, String> {
    let mut candidates = host_git_candidates();
    candidates.sort();
    candidates.dedup();

    let mut inspected = BTreeSet::new();
    let mut rejected = Vec::new();
    for candidate in candidates {
        if !is_executable(&candidate) {
            continue;
        }
        let canonical = match fs::canonicalize(&candidate) {
            Ok(path) => path,
            Err(error) => {
                rejected.push(format!("{} ({error})", candidate.display()));
                continue;
            }
        };
        if !inspected.insert(canonical.clone()) || path_is_inside(&canonical, root) {
            continue;
        }
        if setup_self_packages::is_delegating_git_script(&canonical) {
            rejected.push(format!(
                "{} (delegates to another Git)",
                canonical.display()
            ));
            continue;
        }
        let version = Command::new(&canonical).arg("--version").output();
        if !version.is_ok_and(|output| output.status.success()) {
            rejected.push(format!("{} (`git --version` failed)", canonical.display()));
            continue;
        }
        let exec_path = git_stdout_path(&canonical, "--exec-path");
        return Ok(HostGit {
            binary: canonical,
            templates: git_template_path(exec_path.as_deref()),
            exec_path,
        });
    }

    let detail = if rejected.is_empty() {
        "no executable Git candidate was found on PATH or in common system locations".to_owned()
    } else {
        format!("rejected candidates:\n  - {}", rejected.join("\n  - "))
    };
    Err(format!(
        "cannot install Git: no usable host Git is available to import\n{detail}\nhelp: install Git with the operating-system package manager, then run `vapor setup self install`\nnote: Vapor imports a real Git binary into tools/git; it will not install a wrapper that delegates to system Git"
    ))
}

fn host_git_candidates() -> Vec<PathBuf> {
    let mut candidates = env::var_os("PATH")
        .into_iter()
        .flat_map(|paths| env::split_paths(&paths).collect::<Vec<_>>())
        .map(|directory| directory.join(executable("git")))
        .collect::<Vec<_>>();
    if cfg!(target_os = "linux") {
        candidates.extend([
            PathBuf::from("/usr/bin/git"),
            PathBuf::from("/usr/local/bin/git"),
            PathBuf::from("/bin/git"),
        ]);
    }
    candidates
}

fn git_stdout_path(git: &Path, arg: &str) -> Option<PathBuf> {
    let output = Command::new(git).arg(arg).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8(output.stdout).ok()?;
    let value = stdout.trim();
    (!value.is_empty()).then(|| PathBuf::from(value))
}

fn git_template_path(exec_path: Option<&Path>) -> Option<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(exec_path) = exec_path
        && let Some(prefix) = exec_path.parent().and_then(Path::parent)
    {
        candidates.push(prefix.join("share/git-core/templates"));
    }
    candidates.extend([
        PathBuf::from("/usr/share/git-core/templates"),
        PathBuf::from("/usr/local/share/git-core/templates"),
    ]);
    candidates.into_iter().find(|path| path.is_dir())
}

fn write_git_launcher(root: &Path, git_root: &Path) -> Result<(), String> {
    let launcher = git_root.join("bin").join(executable("git"));
    ensure_contained(root, &launcher)?;
    let source = "#!/bin/sh\nset -eu\nself_dir=$(CDPATH= cd -- \"$(dirname -- \"$0\")\" && pwd)\ngit_root=$(CDPATH= cd -- \"$self_dir/..\" && pwd)\nif [ -d \"$git_root/libexec/git-core\" ]; then\n    GIT_EXEC_PATH=\"$git_root/libexec/git-core\"\n    export GIT_EXEC_PATH\nfi\nif [ -d \"$git_root/share/git-core/templates\" ]; then\n    GIT_TEMPLATE_DIR=\"$git_root/share/git-core/templates\"\n    export GIT_TEMPLATE_DIR\nfi\nexec \"$git_root/libexec/git-core/git\" \"$@\"\n";
    fs::write(&launcher, source)
        .map_err(|error| format!("failed to write '{}': {error}", launcher.display()))?;
    make_executable(&launcher)
}

fn copy_external_file(root: &Path, source: &Path, target: &Path) -> Result<(), String> {
    ensure_contained(root, target)?;
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create '{}': {error}", parent.display()))?;
    }
    fs::copy(source, target).map_err(|error| {
        format!(
            "failed to copy host file '{}' to '{}': {error}",
            source.display(),
            target.display()
        )
    })?;
    make_executable(target)
}

fn copy_external_tree(root: &Path, source: &Path, destination: &Path) -> Result<(), String> {
    let canonical = fs::canonicalize(source).map_err(|error| {
        format!(
            "failed to resolve host Git path '{}': {error}",
            source.display()
        )
    })?;
    copy_external_tree_entry(root, &canonical, destination)
}

fn copy_external_tree_entry(root: &Path, source: &Path, destination: &Path) -> Result<(), String> {
    ensure_contained(root, destination)?;
    let metadata = fs::metadata(source).map_err(|error| {
        format!(
            "failed to inspect host Git path '{}': {error}",
            source.display()
        )
    })?;
    if metadata.is_dir() {
        fs::create_dir_all(destination)
            .map_err(|error| format!("failed to create '{}': {error}", destination.display()))?;
        for entry in fs::read_dir(source).map_err(|error| {
            format!(
                "failed to read host Git path '{}': {error}",
                source.display()
            )
        })? {
            let entry = entry.map_err(|error| format!("failed to read host Git entry: {error}"))?;
            copy_external_tree_entry(root, &entry.path(), &destination.join(entry.file_name()))?;
        }
    } else if metadata.is_file() {
        copy_external_file(root, source, destination)?;
    }
    Ok(())
}

fn path_is_inside(path: &Path, root: &Path) -> bool {
    fs::canonicalize(root).is_ok_and(|root| path.starts_with(root))
}

fn rustup_init_url() -> Result<&'static str, String> {
    match (env::consts::OS, env::consts::ARCH) {
        ("linux", "x86_64") => Ok(RUSTUP_INIT_X86_64_LINUX),
        ("linux", "aarch64") => Ok(RUSTUP_INIT_AARCH64_LINUX),
        (os, arch) => Err(format!(
            "Rust/Cargo self-setup installation is not configured for {arch}-{os}"
        )),
    }
}

fn downloads_dir(root: &Path) -> Result<PathBuf, String> {
    let path = root.join(".vapor/downloads");
    ensure_contained(root, &path)?;
    fs::create_dir_all(&path).map_err(|error| {
        format!(
            "failed to create downloads directory '{}': {error}",
            path.display()
        )
    })?;
    Ok(path)
}

fn download(url: &str, destination: &Path) -> Result<(), String> {
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create '{}': {error}", parent.display()))?;
    }
    let curl_status = Command::new("curl")
        .args(["--proto", "=https", "--tlsv1.2", "-fL", "-o"])
        .arg(destination)
        .arg(url)
        .status();
    match curl_status {
        Ok(status) if status.success() => return Ok(()),
        Ok(status) => {
            eprintln!("warning: curl download exited with {status}; trying wget");
        }
        Err(error) => {
            eprintln!("warning: failed to start curl: {error}; trying wget");
        }
    }
    let status = Command::new("wget")
        .arg("-O")
        .arg(destination)
        .arg(url)
        .status()
        .map_err(|error| format!("failed to start curl or wget for '{url}': {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "failed to download '{url}': wget exited with {status}"
        ))
    }
}

fn copy_file(root: &Path, source: &Path, target: &Path) -> Result<(), String> {
    ensure_contained(root, source)?;
    ensure_contained(root, target)?;
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create '{}': {error}", parent.display()))?;
    }
    fs::copy(source, target).map_err(|error| {
        format!(
            "failed to copy '{}' to '{}': {error}",
            source.display(),
            target.display()
        )
    })?;
    make_executable(target)
}

fn make_executable(path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(path)
            .map_err(|error| format!("failed to inspect '{}': {error}", path.display()))?
            .permissions();
        permissions.set_mode(permissions.mode() | 0o755);
        fs::set_permissions(path, permissions)
            .map_err(|error| format!("failed to make '{}' executable: {error}", path.display()))?;
    }
    Ok(())
}

/// Require selected tools without attempting repair or installation.
///
/// # Errors
///
/// Returns a diagnostic naming missing components and explicit next commands.
pub fn require(
    installation: &InstallationPaths,
    requirements: &[SetupSelfRequirement],
    action: &str,
) -> Result<(), String> {
    let status = inspect(installation);
    require_status(&status, requirements, action)
}

/// Require selected tools using an already-resolved self-setup status.
///
/// # Errors
///
/// Returns a diagnostic naming missing components and explicit next commands.
pub fn require_status(
    status: &SetupSelfStatus,
    requirements: &[SetupSelfRequirement],
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
        "cannot {action}: the app-local {} {} not installed\n{}\nhelp: inspect setup with `vapor setup self status`\nhelp: install setup explicitly with `vapor setup self install`\nnote: this command will not install or repair prerequisites automatically",
        missing
            .iter()
            .map(|requirement| requirement.label())
            .collect::<Vec<_>>()
            .join(", "),
        if missing.len() == 1 { "is" } else { "are" },
        format_missing_selected(status, &missing)
    ))
}

fn inspect_root(root: &Path) -> SetupSelfStatus {
    let rustup = root.join("rustup/bin").join(executable("rustup"));
    let toolchains = root.join("rustup-home/toolchains");
    let (rust_bin, rust_missing) = inspect_rust(&toolchains, Some(root));
    let mut missing = rust_missing;
    if !is_healthy_executable(&rustup, root) {
        missing.push(format!("rustup (expected at {})", rustup.display()));
    }
    let rust = SetupSelfComponentStatus {
        label: "Rust/Cargo",
        installed: missing.is_empty(),
        path: rust_bin.unwrap_or(toolchains),
        missing,
    };

    let git_path = root.join("tools/git/bin").join(executable("git"));
    let git_delegates_to_system = setup_self_packages::is_delegating_git_script(&git_path);
    let git_installed = !git_delegates_to_system && is_healthy_executable(&git_path, root);
    let git = SetupSelfComponentStatus {
        label: "Git",
        installed: git_installed,
        path: git_path,
        missing: if git_installed {
            Vec::new()
        } else if git_delegates_to_system {
            vec!["app-owned Git executable (replace delegating script)".to_owned()]
        } else {
            vec!["git".to_owned()]
        },
    };

    let steam_path = steam_executable(root);
    let steam_installed = is_executable(&steam_path);
    let steamcmd = SetupSelfComponentStatus {
        label: "SteamCMD",
        installed: steam_installed,
        path: steam_path,
        missing: if steam_installed {
            Vec::new()
        } else {
            vec!["steamcmd".to_owned()]
        },
    };

    SetupSelfStatus {
        rust,
        git,
        steamcmd,
        package: setup_self_packages::inspect_setup_self_package(root),
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

fn format_missing(status: &SetupSelfStatus) -> String {
    format_missing_selected(
        status,
        &[
            SetupSelfRequirement::Rust,
            SetupSelfRequirement::Git,
            SetupSelfRequirement::SteamCmd,
        ],
    )
}

fn format_missing_selected(
    status: &SetupSelfStatus,
    requirements: &[SetupSelfRequirement],
) -> String {
    requirements
        .iter()
        .filter_map(|requirement| {
            let tool = status.requirement(*requirement);
            (!tool.installed()).then(|| {
                format!(
                    "  - {}: missing {} (primary path {})",
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
