//! Build, locate, and open installed Vapor documentation.

use crate::{
    discovery::{EnvironmentPaths, ensure_contained},
    workflow,
    workspace::WorkspaceManifest,
};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

/// Build Rustdoc for every workspace declared by the distribution manifest.
pub fn build(paths: &EnvironmentPaths, manifest: &WorkspaceManifest) -> Result<PathBuf, String> {
    let cargo = paths
        .installation()
        .bundled_cargo()
        .ok_or_else(|| "bundled Cargo is unavailable".to_owned())?;
    let docs_root = paths.installation().root().join("docs");
    ensure_contained(paths.installation().root(), &docs_root)?;
    if docs_root.exists() {
        fs::remove_dir_all(&docs_root).map_err(|e| e.to_string())?;
    }
    fs::create_dir_all(&docs_root).map_err(|e| e.to_string())?;

    for section in manifest
        .cargo_projects()
        .iter()
        .filter(|project| project.documentation())
    {
        let cargo_manifest = paths.source().root().join(section.manifest());
        let target = paths
            .installation()
            .root()
            .join("output/docs")
            .join(section.name());
        ensure_contained(paths.installation().root(), &target)?;
        if target.exists() {
            fs::remove_dir_all(&target)
                .map_err(|error| format!("failed to clear '{}': {error}", target.display()))?;
        }
        let status = Command::new(&cargo)
            .args(["doc", "--workspace", "--no-deps", "--manifest-path"])
            .arg(&cargo_manifest)
            .env("VAPOR_HOME", paths.installation().root())
            .env("CARGO_HOME", paths.installation().root().join("cargo-home"))
            .env(
                "RUSTUP_HOME",
                paths.installation().root().join("rustup-home"),
            )
            .env("PATH", workflow::managed_path(paths)?)
            .env_remove("RUSTC_WRAPPER")
            .env("CARGO_TARGET_DIR", &target)
            .current_dir(paths.source().root())
            .status()
            .map_err(|error| format!("failed to build {} docs: {error}", section.name()))?;
        if !status.success() {
            return Err(format!(
                "documentation build for '{}' failed with {status}",
                section.name()
            ));
        }
        let section_root = docs_root.join(section.name());
        copy_tree(&target.join("doc"), &section_root)?;
        copy_linked_markdown_docs(&cargo_manifest, &section_root)?;
        write_section_index(&section_root, section.name())?;
    }
    write_index(&docs_root, manifest)?;
    Ok(docs_root)
}

/// Resolve an installed documentation section or the aggregate index.
pub fn path(paths: &EnvironmentPaths, topic: Option<&str>) -> Result<PathBuf, String> {
    let root = paths.installation().root().join("docs");
    let candidate = topic.map_or_else(
        || root.join("index.html"),
        |name| root.join(name).join("index.html"),
    );
    if candidate.is_file() {
        Ok(candidate)
    } else {
        Err(format!(
            "documentation is not built: {}",
            candidate.display()
        ))
    }
}

/// Open documentation without blocking the Vapor command loop.
pub fn open(paths: &EnvironmentPaths, topic: Option<&str>) -> Result<PathBuf, String> {
    let document = path(paths, topic)?;
    let mut command = if cfg!(target_os = "windows") {
        let mut command = Command::new("cmd");
        command.args(["/C", "start", ""]);
        command
    } else if cfg!(target_os = "macos") {
        Command::new("open")
    } else {
        Command::new("xdg-open")
    };
    command
        .arg(&document)
        .spawn()
        .map_err(|error| format!("failed to open docs: {error}"))?;
    Ok(document)
}

fn copy_tree(source: &Path, target: &Path) -> Result<(), String> {
    fs::create_dir_all(target).map_err(|e| e.to_string())?;
    for entry in
        fs::read_dir(source).map_err(|e| format!("failed to read '{}': {e}", source.display()))?
    {
        let entry = entry.map_err(|e| e.to_string())?;
        let destination = target.join(entry.file_name());
        if entry.file_type().map_err(|e| e.to_string())?.is_dir() {
            copy_tree(&entry.path(), &destination)?;
        } else {
            fs::copy(entry.path(), destination).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

fn copy_linked_markdown_docs(cargo_manifest: &Path, section_root: &Path) -> Result<(), String> {
    let Some(project_root) = cargo_manifest.parent() else {
        return Ok(());
    };
    let source_docs = project_root.join("docs");
    if !source_docs.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(section_root)
        .map_err(|e| format!("failed to read '{}': {e}", section_root.display()))?
    {
        let entry = entry.map_err(|e| e.to_string())?;
        if !entry.file_type().map_err(|e| e.to_string())?.is_dir() {
            continue;
        }
        let crate_root = entry.path();
        if crate_root.join("index.html").is_file() {
            copy_tree(&source_docs, &crate_root.join("docs"))?;
        }
    }
    Ok(())
}

fn write_index(root: &Path, manifest: &WorkspaceManifest) -> Result<(), String> {
    let links = manifest
        .cargo_projects()
        .iter()
        .filter(|project| project.documentation())
        .map(|section| {
            format!(
                "<li><a href=\"{0}/index.html\">{0}</a></li>",
                section.name()
            )
        })
        .collect::<String>();
    fs::write(root.join("index.html"), format!("<!doctype html><title>Vapor documentation</title><h1>Vapor documentation</h1><ul>{links}</ul>"))
        .map_err(|e| e.to_string())
}

fn write_section_index(root: &Path, name: &str) -> Result<(), String> {
    let mut crates = fs::read_dir(root)
        .map_err(|e| {
            format!(
                "failed to read documentation root '{}': {e}",
                root.display()
            )
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("failed to read documentation entry: {e}"))?;
    crates.sort_by_key(|entry| entry.file_name());
    let links = crates
        .into_iter()
        .filter_map(|entry| {
            let path = entry.path();
            let file_name = entry.file_name().to_string_lossy().into_owned();
            entry
                .file_type()
                .ok()
                .filter(|kind| kind.is_dir())
                .and_then(|_| path.join("index.html").is_file().then_some(file_name))
        })
        .map(|crate_name| {
            format!(
                "<li><a href=\"{0}/index.html\">{0}</a></li>",
                html_escape(&crate_name)
            )
        })
        .collect::<String>();
    let title = html_escape(name);
    fs::write(
        root.join("index.html"),
        format!(
            "<!doctype html><title>{title} documentation</title><h1>{title} documentation</h1><ul>{links}</ul>"
        ),
    )
    .map_err(|e| e.to_string())
}

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
