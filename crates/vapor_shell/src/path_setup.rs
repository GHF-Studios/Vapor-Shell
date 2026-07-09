//! Registration of the app-owned `bin` directory in the user's `PATH`.
//!
//! Vapor never copies an executable into a user-data directory. The actual
//! `vapor` binary remains under the movable Steam installation/app root; only a
//! marked shell-profile entry lives outside the application.

use crate::discovery::InstallationPaths;
use std::{
    env, fs,
    path::{Path, PathBuf},
};

const BLOCK_START: &str = "# >>> Vapor managed PATH >>>";
const BLOCK_END: &str = "# <<< Vapor managed PATH <<<";

/// Filesystem inputs for registering an app-owned command directory.
#[derive(Debug, Clone)]
pub struct PathSetup {
    home: PathBuf,
    binaries: PathBuf,
    shell: Option<String>,
}

/// Result of installing or inspecting PATH registration.
#[derive(Debug, Clone)]
pub struct PathSetupReport {
    command: PathBuf,
    binaries: PathBuf,
    profiles: Vec<PathBuf>,
    registered: bool,
    changed: bool,
    path_active: bool,
}

impl PathSetup {
    /// Construct an explicit registration plan.
    pub fn new(home: PathBuf, binaries: PathBuf, shell: Option<String>) -> Self {
        Self {
            home,
            binaries,
            shell,
        }
    }

    /// Construct setup before external source-workspace discovery.
    ///
    /// # Errors
    ///
    /// Fails when no user home directory is available.
    pub fn from_installation(installation: &InstallationPaths) -> Result<Self, String> {
        let home = env::var_os("HOME")
            .or_else(|| env::var_os("USERPROFILE"))
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .ok_or_else(|| "cannot register Vapor in PATH: HOME is unavailable".to_owned())?;
        let shell = env::var_os("SHELL").map(|value| value.to_string_lossy().into_owned());
        Ok(Self::new(
            home,
            installation.binaries().to_path_buf(),
            shell,
        ))
    }

    /// Add or repair the marked PATH block for the current Steam app location.
    ///
    /// # Errors
    ///
    /// Returns filesystem errors. Unmarked profile content is preserved.
    pub fn install(&self) -> Result<PathSetupReport, String> {
        self.require_command()?;
        let profiles = self.profile_paths();
        let mut changed = false;
        for profile in &profiles {
            changed |= install_profile_block(profile, &self.profile_block(profile))?;
        }
        Ok(self.report(profiles, true, changed))
    }

    /// Inspect PATH registration without changing files.
    ///
    /// # Errors
    ///
    /// Returns profile read errors.
    pub fn status(&self) -> Result<PathSetupReport, String> {
        let profiles = self.profile_paths();
        let expected = self.profile_block(
            profiles
                .first()
                .map(PathBuf::as_path)
                .unwrap_or_else(|| Path::new("")),
        );
        let registered = profiles.iter().any(|profile| {
            fs::read_to_string(profile)
                .map(|source| source.contains(&expected))
                .unwrap_or(false)
        });
        Ok(self.report(profiles, registered, false))
    }

    /// Remove only marked Vapor PATH blocks.
    ///
    /// # Errors
    ///
    /// Returns profile read or write errors.
    pub fn uninstall(&self) -> Result<PathSetupReport, String> {
        let profiles = self.profile_paths();
        let mut changed = false;
        for profile in &profiles {
            changed |= uninstall_profile_block(profile)?;
        }
        Ok(self.report(profiles, false, changed))
    }

    fn report(&self, profiles: Vec<PathBuf>, registered: bool, changed: bool) -> PathSetupReport {
        PathSetupReport {
            command: self.binaries.join(command_name()),
            binaries: self.binaries.clone(),
            path_active: current_path_contains(&self.binaries),
            profiles,
            registered,
            changed,
        }
    }

    fn require_command(&self) -> Result<(), String> {
        let command = self.binaries.join(command_name());
        if command.is_file() {
            Ok(())
        } else {
            Err(format!(
                "app-owned Vapor command is missing: {}\nhelp: rebuild or verify the Steam application before registering PATH",
                command.display()
            ))
        }
    }

    fn profile_paths(&self) -> Vec<PathBuf> {
        if cfg!(windows) {
            return Vec::new();
        }
        let shell = self
            .shell
            .as_deref()
            .and_then(|value| Path::new(value).file_name())
            .and_then(|value| value.to_str());
        match shell {
            Some("bash") => vec![self.home.join(".profile"), self.home.join(".bashrc")],
            Some("zsh") => vec![self.home.join(".zprofile"), self.home.join(".zshrc")],
            Some("fish") => vec![self.home.join(".config/fish/conf.d/vapor.fish")],
            _ => vec![self.home.join(".profile")],
        }
    }

    fn profile_block(&self, profile: &Path) -> String {
        let path = self.binaries.to_string_lossy();
        if profile
            .extension()
            .is_some_and(|extension| extension == "fish")
        {
            format!(
                "{BLOCK_START}\nfish_add_path --prepend --global '{}'\n{BLOCK_END}\n",
                shell_quote(&path)
            )
        } else {
            format!(
                "{BLOCK_START}\nVAPOR_BIN='{}'\ncase \":$PATH:\" in\n    *\":$VAPOR_BIN:\"*) ;;\n    *) export PATH=\"$VAPOR_BIN${{PATH:+:$PATH}}\" ;;\nesac\nunset VAPOR_BIN\n{BLOCK_END}\n",
                shell_quote(&path)
            )
        }
    }
}

impl PathSetupReport {
    /// App-owned executable reached through PATH.
    pub fn command(&self) -> &Path {
        &self.command
    }

    /// App-owned directory added to PATH.
    pub fn binaries(&self) -> &Path {
        &self.binaries
    }

    /// Shell profiles governed by this setup.
    pub fn profiles(&self) -> &[PathBuf] {
        &self.profiles
    }

    /// Whether the current app location is registered in a profile.
    pub fn registered(&self) -> bool {
        self.registered
    }

    /// Whether this operation changed a profile.
    pub fn changed(&self) -> bool {
        self.changed
    }

    /// Whether the app `bin` directory is active in this process's PATH.
    pub fn path_active(&self) -> bool {
        self.path_active
    }
}

fn command_name() -> &'static str {
    if cfg!(windows) { "vapor.exe" } else { "vapor" }
}

fn install_profile_block(path: &Path, block: &str) -> Result<bool, String> {
    let existing = if path.is_file() {
        fs::read_to_string(path)
            .map_err(|error| format!("failed to read '{}': {error}", path.display()))?
    } else {
        String::new()
    };
    let without = remove_block(&existing);
    let separator = if without.is_empty() || without.ends_with('\n') {
        ""
    } else {
        "\n"
    };
    let updated = format!("{without}{separator}{block}");
    if updated == existing {
        return Ok(false);
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create '{}': {error}", parent.display()))?;
    }
    fs::write(path, updated)
        .map_err(|error| format!("failed to write '{}': {error}", path.display()))?;
    Ok(true)
}

fn uninstall_profile_block(path: &Path) -> Result<bool, String> {
    if !path.is_file() {
        return Ok(false);
    }
    let existing = fs::read_to_string(path)
        .map_err(|error| format!("failed to read '{}': {error}", path.display()))?;
    let updated = remove_block(&existing);
    if updated == existing {
        return Ok(false);
    }
    fs::write(path, updated)
        .map_err(|error| format!("failed to write '{}': {error}", path.display()))?;
    Ok(true)
}

fn remove_block(source: &str) -> String {
    let Some(start) = source.find(BLOCK_START) else {
        return source.to_owned();
    };
    let Some(relative_end) = source[start..].find(BLOCK_END) else {
        return source.to_owned();
    };
    let end = start + relative_end + BLOCK_END.len();
    let end = if source[end..].starts_with('\n') {
        end + 1
    } else {
        end
    };
    format!("{}{}", &source[..start], &source[end..])
}

fn shell_quote(value: &str) -> String {
    value.replace('\'', "'\"'\"'")
}

fn current_path_contains(directory: &Path) -> bool {
    env::var_os("PATH")
        .map(|path| env::split_paths(&path).any(|entry| entry == directory))
        .unwrap_or(false)
}
