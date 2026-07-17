//! SteamCMD-backed beta-branch SteamPipe publishing.

use crate::{
    discovery::{EnvironmentPaths, InstallationPaths},
    distribution::{self, DistributionManifest, StageOptions},
};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

/// Locate the installation-owned SteamCMD executable.
pub fn executable(paths: &EnvironmentPaths) -> Result<PathBuf, String> {
    executable_for_installation(paths.installation())
}

/// Locate the installation-owned SteamCMD executable from an app root.
pub fn executable_for_installation(installation: &InstallationPaths) -> Result<PathBuf, String> {
    let root = installation.root();
    let names = if cfg!(windows) {
        vec!["steamcmd.exe"]
    } else {
        vec!["steamcmd", "steamcmd.sh"]
    };
    [root.join("tools/steamcmd"), root.join("steam/steamcmd")]
        .into_iter()
        .flat_map(|directory| names.iter().map(move |name| directory.join(name)))
        .find(|path| path.is_file())
        .ok_or_else(|| "SteamCMD is not installed in the Vapor app root".to_owned())
}

/// Options for a SteamPipe publication attempt.
#[derive(Debug, Clone)]
pub struct PublishOptions<'a> {
    /// Dedicated Steam build account.
    pub account: &'a str,
    /// Target beta branch, or the manifest default.
    pub branch: Option<&'a str>,
    /// Internal Steam build description.
    pub description: &'a str,
    /// Staged payload mode.
    pub stage_options: StageOptions,
    /// Whether to generate a preview VDF without uploading.
    pub dry_run: bool,
    /// Whether a real upload has been explicitly confirmed.
    pub confirmed: bool,
}

/// Stage, validate, and publish the development beta through SteamCMD.
pub fn publish(
    paths: &EnvironmentPaths,
    manifest: &DistributionManifest,
    options: PublishOptions<'_>,
) -> Result<PathBuf, String> {
    let branch = options
        .branch
        .unwrap_or(manifest.application().development_branch());
    if branch == "default" || branch.trim().is_empty() {
        return Err("automatic publishing requires a non-default beta branch".to_owned());
    }
    if !options.dry_run && !options.confirmed {
        return Err("publishing requires --yes after reviewing --dry-run".to_owned());
    }
    let stage = distribution::stage_with_options(paths, manifest, options.stage_options.clone())?;
    smoke(stage.root(), &options.stage_options)?;
    let script = write_build_script(
        paths,
        manifest,
        stage.root(),
        branch,
        options.description,
        options.dry_run,
    )?;
    if options.dry_run {
        return Ok(script);
    }
    let steamcmd = executable(paths)?;
    let status = Command::new(&steamcmd)
        .args(["+login", options.account, "+run_app_build"])
        .arg(&script)
        .arg("+quit")
        .current_dir(steamcmd.parent().expect("SteamCMD has a parent"))
        .status()
        .map_err(|error| format!("failed to start SteamCMD: {error}"))?;
    if status.success() {
        Ok(script)
    } else {
        Err(format!("Steam publish exited with {status}"))
    }
}

/// Validate essential staged self-hosting inputs.
pub fn smoke(stage: &Path, options: &StageOptions) -> Result<(), String> {
    for required in ["Vapor.toml", "bin", "docs"] {
        let path = stage.join(required);
        if !path.exists() {
            return Err(format!(
                "staged application is missing required path: {}",
                path.display()
            ));
        }
    }
    if options.includes_setup_payload() {
        for required in [
            "packages/setup",
            "packages/setup/rustup",
            "packages/setup/rustup-home",
            "packages/setup/cargo-home",
            "packages/setup/git",
            "packages/setup/steamcmd",
        ] {
            let path = stage.join(required);
            if !path.exists() {
                return Err(format!(
                    "staged application is missing required path: {}",
                    path.display()
                ));
            }
        }
    }
    let linux_shell = stage.join("bin/x86_64-unknown-linux-gnu/vapor");
    let windows_shell = stage.join("bin/x86_64-pc-windows-gnullvm/vapor.exe");
    let legacy_shell = stage.join(format!("bin/vapor{}", std::env::consts::EXE_SUFFIX));
    if !linux_shell.is_file() && !windows_shell.is_file() && !legacy_shell.is_file() {
        return Err("staged application has no vapor binary".to_owned());
    }
    if stage.join(".vapor/launch/linux/vapor.sh").is_file() && !linux_shell.is_file() {
        return Err(format!(
            "staged Linux launch wrapper has no target Vapor binary: {}",
            linux_shell.display()
        ));
    }
    if stage.join(".vapor/launch/windows/vapor.cmd").is_file() && !windows_shell.is_file() {
        return Err(format!(
            "staged Windows launch wrapper has no target Vapor binary: {}",
            windows_shell.display()
        ));
    }
    for forbidden in [
        "packages/setup/cargo-home/credentials",
        "packages/setup/cargo-home/credentials.toml",
        "packages/setup/cargo-home/registry/cache",
        "packages/setup/cargo-home/registry/src",
        "packages/setup/steamcmd/config",
        "packages/setup/steamcmd/logs",
        "packages/setup/steamcmd/steamapps",
        "packages/setup/steamcmd/dumps",
    ] {
        let path = stage.join(forbidden);
        if path.exists() {
            return Err(format!(
                "staged application includes SteamCMD session state: {}",
                path.display()
            ));
        }
    }
    Ok(())
}

fn write_build_script(
    paths: &EnvironmentPaths,
    manifest: &DistributionManifest,
    content: &Path,
    branch: &str,
    description: &str,
    preview: bool,
) -> Result<PathBuf, String> {
    let scripts = paths.installation().root().join("output/root/scripts");
    let output = paths.installation().root().join("output/root/steam-build");
    fs::create_dir_all(&scripts).map_err(|e| e.to_string())?;
    fs::create_dir_all(&output).map_err(|e| e.to_string())?;
    let path = scripts.join(format!("app_build_{}.vdf", manifest.application().app_id()));
    let preview = if preview {
        "    \"Preview\" \"1\"\n"
    } else {
        ""
    };
    let vdf = format!(
        "\"AppBuild\"\n{{\n    \"AppID\" \"{}\"\n    \"Desc\" \"{}\"\n{preview}    \"SetLive\" \"{}\"\n    \"ContentRoot\" \"{}\"\n    \"BuildOutput\" \"{}\"\n    \"Depots\"\n    {{\n        \"{}\"\n        {{\n            \"FileMapping\" {{ \"LocalPath\" \"*\" \"DepotPath\" \".\" \"recursive\" \"1\" }}\n        }}\n    }}\n}}\n",
        manifest.application().app_id(),
        escape(description),
        escape(branch),
        escape(&content.display().to_string()),
        escape(&output.display().to_string()),
        manifest.application().depot_id()
    );
    fs::write(&path, vdf).map_err(|e| e.to_string())?;
    Ok(path)
}

fn escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}
