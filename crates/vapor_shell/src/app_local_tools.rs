//! App-local tool readiness inspection.
//!
//! Vapor-Installer owns installation, uninstallation, player-mode bootstrap,
//! and developer-environment upgrade/downgrade. Vapor Shell only inspects the
//! resulting app-local tools so command preflight can point at the installer
//! without mutating the Steam app root.

use crate::{cross_toolchain, discovery::InstallationPaths};
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

/// App-local tool requirement used by command preflight checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppToolRequirement {
    /// Rustup, Cargo, Rustc, Rustfmt, Clippy, and Rustdoc.
    Rust,
    /// Portable Git distribution.
    Git,
    /// Portable Zig-based cross-linker wrappers.
    CrossToolchains,
    /// SteamCMD distribution.
    SteamCmd,
}

impl AppToolRequirement {
    /// Human-readable tool-group name used in diagnostics.
    pub fn label(self) -> &'static str {
        match self {
            Self::Rust => "Rust/Cargo",
            Self::Git => "Git",
            Self::CrossToolchains => "Zig/Cross",
            Self::SteamCmd => "SteamCMD",
        }
    }
}

/// Health of one required app-local tool group.
#[derive(Debug, Clone)]
pub struct AppToolComponentStatus {
    label: &'static str,
    installed: bool,
    path: PathBuf,
    missing: Vec<String>,
}

impl AppToolComponentStatus {
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

/// Complete status of active Rust, Git, cross-linker, and SteamCMD.
#[derive(Debug, Clone)]
pub struct AppToolStatus {
    rust: AppToolComponentStatus,
    git: AppToolComponentStatus,
    cross: AppToolComponentStatus,
    steamcmd: AppToolComponentStatus,
}

impl AppToolStatus {
    /// App-local Rust status.
    pub fn rust(&self) -> &AppToolComponentStatus {
        &self.rust
    }

    /// App-local Git status.
    pub fn git(&self) -> &AppToolComponentStatus {
        &self.git
    }

    /// App-local portable cross-linker status.
    pub fn cross_toolchains(&self) -> &AppToolComponentStatus {
        &self.cross
    }

    /// App-local SteamCMD status.
    pub fn steamcmd(&self) -> &AppToolComponentStatus {
        &self.steamcmd
    }

    /// Whether every developer tool group is installed.
    pub fn complete(&self) -> bool {
        self.rust.installed && self.git.installed && self.cross.installed && self.steamcmd.installed
    }

    /// Status of one requested tool group.
    pub fn requirement(&self, requirement: AppToolRequirement) -> &AppToolComponentStatus {
        match requirement {
            AppToolRequirement::Rust => &self.rust,
            AppToolRequirement::Git => &self.git,
            AppToolRequirement::CrossToolchains => &self.cross,
            AppToolRequirement::SteamCmd => &self.steamcmd,
        }
    }
}

/// Inspect installer-managed app-local tools.
pub fn inspect(installation: &InstallationPaths) -> AppToolStatus {
    inspect_root(installation.root())
}

/// Require selected tools without attempting installation.
///
/// # Errors
///
/// Returns a diagnostic naming missing components and explicit installer
/// commands.
pub fn require(
    installation: &InstallationPaths,
    requirements: &[AppToolRequirement],
    action: &str,
) -> Result<(), String> {
    let status = inspect(installation);
    require_status(&status, requirements, action)
}

/// Require selected tools using an already-resolved app-local tool status.
///
/// # Errors
///
/// Returns a diagnostic naming missing components and explicit installer
/// commands.
pub fn require_status(
    status: &AppToolStatus,
    requirements: &[AppToolRequirement],
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
        "cannot {action}: the app-local {} {} not installed\n{}\nhelp: player-mode tooling uses `vapor-installer install --app-root <app-root>`\nhelp: development tooling uses `vapor-installer dev-env install --app-root <app-root>`\nnote: this command will not install prerequisites automatically",
        missing
            .iter()
            .map(|requirement| requirement.label())
            .collect::<Vec<_>>()
            .join(", "),
        if missing.len() == 1 { "is" } else { "are" },
        format_missing_selected(status, &missing)
    ))
}

fn inspect_root(root: &Path) -> AppToolStatus {
    let rustup = root.join("rustup/bin").join(executable("rustup"));
    let toolchains = root.join("rustup-home/toolchains");
    let (rust_bin, rust_missing) = inspect_rust(&toolchains, Some(root));
    let mut missing = rust_missing;
    if !is_executable(&rustup) {
        missing.push(format!("rustup (expected at {})", rustup.display()));
    }
    let rust = AppToolComponentStatus {
        label: "Rust/Cargo",
        installed: missing.is_empty(),
        path: rust_bin.unwrap_or(toolchains),
        missing,
    };

    let git_paths = git_candidates(root);
    let git_path = git_paths
        .iter()
        .find(|path| is_healthy_git(path, root))
        .cloned()
        .unwrap_or_else(|| preferred_git_path(root));
    let git_delegates_to_system = git_paths.iter().any(|path| is_delegating_git_script(path));
    let git_installed = git_paths.iter().any(|path| is_healthy_git(path, root));
    let git = AppToolComponentStatus {
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
    let steamcmd = AppToolComponentStatus {
        label: "SteamCMD",
        installed: steam_installed,
        path: steam_path,
        missing: if steam_installed {
            Vec::new()
        } else {
            vec!["steamcmd".to_owned()]
        },
    };

    let cross_status = cross_toolchain::inspect(root);
    let cross = AppToolComponentStatus {
        label: "Zig/Cross",
        installed: cross_status.installed,
        path: cross_status.path,
        missing: cross_status.missing,
    };

    AppToolStatus {
        rust,
        git,
        cross,
        steamcmd,
    }
}

fn preferred_git_path(root: &Path) -> PathBuf {
    if cfg!(target_os = "windows") {
        root.join("tools/git/cmd").join(executable("git"))
    } else {
        root.join("tools/git/bin").join(executable("git"))
    }
}

fn git_candidates(root: &Path) -> Vec<PathBuf> {
    vec![
        root.join("tools/git/bin").join(executable("git")),
        root.join("tools/git/cmd").join(executable("git")),
    ]
}

fn is_healthy_git(path: &Path, root: &Path) -> bool {
    !is_delegating_git_script(path) && is_healthy_executable(path, root)
}

fn is_delegating_git_script(path: &Path) -> bool {
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

fn format_missing_selected(status: &AppToolStatus, requirements: &[AppToolRequirement]) -> String {
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
