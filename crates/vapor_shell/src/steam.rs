//! SteamCMD terminal handoff and beta-branch SteamPipe publishing.

use crate::{
    discovery::EnvironmentPaths,
    distribution::{self, DistributionManifest},
    toolchain,
};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

/// Locate the installation-owned SteamCMD executable.
pub fn executable(paths: &EnvironmentPaths) -> Result<PathBuf, String> {
    let root = paths.installation().root();
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

/// Temporarily hand the terminal to SteamCMD for interactive authentication.
pub fn login(paths: &EnvironmentPaths, account: &str) -> Result<(), String> {
    let steamcmd = executable(paths)?;
    let status = Command::new(&steamcmd)
        .args(["+login", account, "+quit"])
        .current_dir(steamcmd.parent().expect("SteamCMD has a parent"))
        .status()
        .map_err(|error| format!("failed to start SteamCMD: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("SteamCMD login exited with {status}"))
    }
}

/// Stage, validate, and publish the development beta through SteamCMD.
pub fn publish(
    paths: &EnvironmentPaths,
    manifest: &DistributionManifest,
    account: &str,
    branch: Option<&str>,
    description: &str,
    dry_run: bool,
    confirmed: bool,
) -> Result<PathBuf, String> {
    let branch = branch.unwrap_or(manifest.application().development_branch());
    if branch == "default" || branch.trim().is_empty() {
        return Err("automatic publishing requires a non-default beta branch".to_owned());
    }
    if !dry_run && !confirmed {
        return Err("publishing requires --yes after reviewing --dry-run".to_owned());
    }
    let stage = distribution::stage(paths, manifest)?;
    smoke(stage.root())?;
    let script = write_build_script(paths, manifest, stage.root(), branch, description, dry_run)?;
    if dry_run {
        return Ok(script);
    }
    let steamcmd = executable(paths)?;
    let status = Command::new(&steamcmd)
        .args(["+login", account, "+run_app_build"])
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
pub fn smoke(stage: &Path) -> Result<(), String> {
    for required in ["Vapor.toml", "bin", "docs", "packages/toolchain"] {
        let path = stage.join(required);
        if !path.exists() {
            return Err(format!(
                "staged application is missing required path: {}",
                path.display()
            ));
        }
    }
    toolchain::validate_package(&stage.join("packages/toolchain"))?;
    let has_shell = fs::read_dir(stage.join("bin"))
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
        .any(|entry| entry.file_name().to_string_lossy().starts_with("vapor"));
    if !has_shell {
        return Err("staged application has no vapor binary".to_owned());
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
