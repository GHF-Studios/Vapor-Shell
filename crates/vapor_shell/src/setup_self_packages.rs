//! App-local distributable self-setup payload lifecycle.

use crate::discovery::ensure_contained;
use std::{
    env, fs,
    path::{Path, PathBuf},
};

const SETUP_PACKAGE: &str = "packages/setup";

#[derive(Debug, Clone)]
pub(crate) struct SetupSelfPackageStatus {
    root: PathBuf,
    missing: Vec<String>,
}

impl SetupSelfPackageStatus {
    pub(crate) fn root(&self) -> &Path {
        &self.root
    }

    pub(crate) fn complete(&self) -> bool {
        self.missing.is_empty()
    }

    pub(crate) fn missing(&self) -> &[String] {
        &self.missing
    }
}

#[derive(Debug, Clone)]
pub(crate) struct PackageInstallReport {
    changed: bool,
    status: SetupSelfPackageStatus,
}

impl PackageInstallReport {
    pub(crate) fn changed(&self) -> bool {
        self.changed
    }

    pub(crate) fn status(&self) -> &SetupSelfPackageStatus {
        &self.status
    }
}

pub(crate) fn inspect_setup_self_package(app_root: &Path) -> SetupSelfPackageStatus {
    inspect_package_at(&self_package_root(app_root))
}

pub(crate) fn validate_setup_self_package(app_root: &Path) -> Result<(), String> {
    let status = inspect_setup_self_package(app_root);
    if status.complete() {
        return Ok(());
    }
    Err(format!(
        "the distributable self-setup payload is incomplete at '{}'\nmissing package entries:\n  - {}\nhelp: populate payloads explicitly with `vapor setup self package install`\nhelp: if payloads may be stale or damaged, run `vapor setup self package repair`\nnote: `vapor setup self install` only prepares active app-local tools",
        status.root().display(),
        status.missing().join("\n  - ")
    ))
}

pub(crate) fn install_setup_self_package(
    app_root: &Path,
    repair: bool,
) -> Result<PackageInstallReport, String> {
    let before = inspect_setup_self_package(app_root);
    if before.complete() && !repair {
        return Ok(PackageInstallReport {
            changed: false,
            status: before,
        });
    }

    validate_active_setup_for_packaging(app_root)?;
    let package = self_package_root(app_root);
    ensure_contained(app_root, &package)?;
    if repair && package.exists() {
        fs::remove_dir_all(&package)
            .map_err(|error| format!("failed to reset '{}': {error}", package.display()))?;
    }
    fs::create_dir_all(&package)
        .map_err(|error| format!("failed to create '{}': {error}", package.display()))?;

    copy_tree(
        app_root,
        &app_root.join("rustup"),
        &package.join("rustup"),
        &[],
    )?;
    copy_tree(
        app_root,
        &app_root.join("rustup-home"),
        &package.join("rustup-home"),
        &[],
    )?;
    copy_tree(
        app_root,
        &app_root.join("cargo-home"),
        &package.join("cargo-home"),
        &[
            PathBuf::from("credentials"),
            PathBuf::from("credentials.toml"),
            PathBuf::from("registry/cache"),
            PathBuf::from("registry/src"),
        ],
    )?;
    copy_tree(
        app_root,
        &app_root.join("tools/git"),
        &package.join("git"),
        &[],
    )?;
    copy_tree(
        app_root,
        &app_root.join("tools/steamcmd"),
        &package.join("steamcmd"),
        &[
            PathBuf::from("config"),
            PathBuf::from("logs"),
            PathBuf::from("steamapps"),
            PathBuf::from("dumps"),
        ],
    )?;

    let status = inspect_setup_self_package(app_root);
    if !status.complete() {
        return Err(format!(
            "self-setup payload was written, but verification still fails\nmissing package entries:\n  - {}",
            status.missing().join("\n  - ")
        ));
    }
    Ok(PackageInstallReport {
        changed: true,
        status,
    })
}

pub(crate) fn copy_setup_self_package_to_active(
    app_root: &Path,
    status: &SetupSelfPackageStatus,
    repair: bool,
    active_rust_ready: bool,
    active_git_ready: bool,
    active_steamcmd_ready: bool,
) -> Result<Vec<&'static str>, String> {
    if !status.complete() {
        return Err(format!(
            "the Steam application does not contain complete self-setup payloads\nmissing package entries:\n  - {}\nhelp: repair Steam app files, or create payloads with `vapor setup self package install`\nnote: `vapor setup self install` does not create package payloads",
            status.missing().join("\n  - ")
        ));
    }

    let package = status.root();
    let mut installed = Vec::new();
    if repair || !active_rust_ready {
        copy_tree(
            app_root,
            &package.join("rustup"),
            &app_root.join("rustup"),
            &[],
        )?;
        copy_tree(
            app_root,
            &package.join("rustup-home"),
            &app_root.join("rustup-home"),
            &[],
        )?;
        copy_tree(
            app_root,
            &package.join("cargo-home"),
            &app_root.join("cargo-home"),
            &[],
        )?;
        installed.push("Rust/Cargo");
    }
    if repair || !active_git_ready {
        copy_tree(
            app_root,
            &package.join("git"),
            &app_root.join("tools/git"),
            &[],
        )?;
        installed.push("Git");
    }
    if repair || !active_steamcmd_ready {
        copy_tree(
            app_root,
            &package.join("steamcmd"),
            &app_root.join("tools/steamcmd"),
            &[],
        )?;
        installed.push("SteamCMD");
    }
    Ok(installed)
}

pub(crate) fn is_delegating_git_script(path: &Path) -> bool {
    let Ok(metadata) = fs::metadata(path) else {
        return false;
    };
    if metadata.len() > 4096 {
        return false;
    }
    let Ok(source) = fs::read_to_string(path) else {
        return false;
    };
    source.starts_with("#!")
        && source.contains("exec")
        && (source.contains("/usr/bin/git") || source.contains(" git"))
}

fn self_package_root(app_root: &Path) -> PathBuf {
    app_root.join(SETUP_PACKAGE)
}

fn inspect_package_at(package: &Path) -> SetupSelfPackageStatus {
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
    missing.extend(inspect_package_tools(package));
    missing.sort();
    missing.dedup();
    SetupSelfPackageStatus {
        root: package.to_path_buf(),
        missing,
    }
}

fn inspect_package_tools(package: &Path) -> Vec<String> {
    let mut missing = Vec::new();
    if !is_executable(&package.join("rustup/bin").join(executable("rustup"))) {
        missing.push(format!("rustup/bin/{}", executable("rustup")));
    }
    missing.extend(
        inspect_rust_bins(&package.join("rustup-home/toolchains"))
            .into_iter()
            .map(|name| format!("rustup-home/toolchains/*/bin/{}", executable(&name))),
    );
    let git = package.join("git/bin").join(executable("git"));
    if !is_executable(&git) {
        missing.push(format!("git/bin/{}", executable("git")));
    } else if is_delegating_git_script(&git) {
        missing.push("git/bin/git (replace with app-owned Git executable)".to_owned());
    }
    if !steam_candidates(&package.join("steamcmd"))
        .iter()
        .any(|path| is_executable(path))
    {
        missing.push("steamcmd/steamcmd[.sh|.exe]".to_owned());
    }
    missing
}

pub(crate) fn validate_active_setup_for_packaging(app_root: &Path) -> Result<(), String> {
    let mut missing = Vec::new();
    for directory in [
        "rustup",
        "rustup-home",
        "cargo-home",
        "cargo-home/registry",
        "tools/git",
        "tools/steamcmd",
    ] {
        if !app_root.join(directory).is_dir() {
            missing.push(directory.to_owned());
        }
    }
    if !is_executable(&app_root.join("rustup/bin").join(executable("rustup"))) {
        missing.push(format!("rustup/bin/{}", executable("rustup")));
    }
    missing.extend(
        inspect_rust_bins(&app_root.join("rustup-home/toolchains"))
            .into_iter()
            .map(|name| format!("rustup-home/toolchains/*/bin/{}", executable(&name))),
    );
    let git = app_root.join("tools/git/bin").join(executable("git"));
    if !is_executable(&git) {
        missing.push(format!("tools/git/bin/{}", executable("git")));
    } else if is_delegating_git_script(&git) {
        missing.push("tools/git/bin/git (replace with app-owned Git executable)".to_owned());
    }
    if !steam_candidates(&app_root.join("tools/steamcmd"))
        .iter()
        .any(|path| is_executable(path))
    {
        missing.push("tools/steamcmd/steamcmd[.sh|.exe]".to_owned());
    }
    if missing.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "cannot populate self-setup payloads: active app-local tools are incomplete\nmissing active entries:\n  - {}\nhelp: run `vapor setup self status` and repair active tools first\nnote: replace any delegating Git script with a real app-owned Git distribution",
            missing.join("\n  - ")
        ))
    }
}

fn inspect_rust_bins(toolchains: &Path) -> Vec<String> {
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
            .filter(|name| !is_executable(&bin.join(executable(name))))
            .map(|name| (*name).to_owned())
            .collect::<Vec<_>>();
        if missing.is_empty() {
            return Vec::new();
        }
    }
    required.iter().map(|name| (*name).to_owned()).collect()
}

fn copy_tree(
    app_root: &Path,
    source: &Path,
    destination: &Path,
    exclusions: &[PathBuf],
) -> Result<(), String> {
    ensure_contained(app_root, source)?;
    ensure_contained(app_root, destination)?;
    copy_tree_entry(source, destination, source, exclusions)
}

fn copy_tree_entry(
    source: &Path,
    destination: &Path,
    item_root: &Path,
    exclusions: &[PathBuf],
) -> Result<(), String> {
    let relative = source.strip_prefix(item_root).unwrap_or(Path::new(""));
    if exclusions
        .iter()
        .any(|excluded| relative.starts_with(excluded))
    {
        return Ok(());
    }
    let canonical = fs::canonicalize(source).map_err(|error| {
        format!(
            "failed to resolve self-setup payload '{}': {error}",
            source.display()
        )
    })?;
    ensure_contained(item_root, &canonical)?;
    let metadata = fs::metadata(&canonical).map_err(|error| {
        format!(
            "failed to inspect self-setup payload '{}': {error}",
            source.display()
        )
    })?;
    if metadata.is_dir() {
        fs::create_dir_all(destination)
            .map_err(|error| format!("failed to create '{}': {error}", destination.display()))?;
        for entry in fs::read_dir(&canonical)
            .map_err(|error| format!("failed to read '{}': {error}", canonical.display()))?
        {
            let entry = entry.map_err(|error| format!("failed to read package entry: {error}"))?;
            copy_tree_entry(
                &entry.path(),
                &destination.join(entry.file_name()),
                item_root,
                exclusions,
            )?;
        }
    } else if metadata.is_file() {
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("failed to create '{}': {error}", parent.display()))?;
        }
        fs::copy(&canonical, destination).map_err(|error| {
            format!(
                "failed to copy '{}' to '{}': {error}",
                canonical.display(),
                destination.display()
            )
        })?;
    }
    Ok(())
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
