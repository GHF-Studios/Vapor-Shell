//! Project-local IDE configuration managed by explicit Vapor commands.
//!
//! This module intentionally writes only files under the selected source
//! root's `.idea` directory. It does not edit global JetBrains settings because
//! those files mix durable preferences with volatile window, workspace, and AI
//! session state.

use crate::{
    app_local_tools::AppToolStatus,
    discovery::{EnvironmentPaths, ensure_contained},
    workspace::WorkspaceManifest,
};
use std::{
    fs,
    path::{Path, PathBuf},
};

const CARGO_PROJECTS_FILE: &str = "cargoProjects.xml";
const RUST_SETTINGS_FILE: &str = "rust.xml";
const VAPOR_SETTINGS_FILE: &str = "vapor.xml";

const CARGO_PROJECTS_TEMPLATE: &str = include_str!("../templates/ide/cargoProjects.xml");
const RUST_TEMPLATE: &str = include_str!("../templates/ide/rust.xml");
const VAPOR_TEMPLATE: &str = include_str!("../templates/ide/vapor.xml");

#[derive(Debug, Clone)]
pub(crate) struct IdeStatus {
    source_root: PathBuf,
    idea_dir: PathBuf,
    rust_bin: PathBuf,
    stdlib_source: Option<PathBuf>,
    files: Vec<IdeFileStatus>,
}

impl IdeStatus {
    pub(crate) fn source_root(&self) -> &Path {
        &self.source_root
    }

    pub(crate) fn idea_dir(&self) -> &Path {
        &self.idea_dir
    }

    pub(crate) fn rust_bin(&self) -> &Path {
        &self.rust_bin
    }

    pub(crate) fn stdlib_source(&self) -> Option<&Path> {
        self.stdlib_source.as_deref()
    }

    pub(crate) fn files(&self) -> &[IdeFileStatus] {
        &self.files
    }

    pub(crate) fn complete(&self) -> bool {
        self.files.iter().all(IdeFileStatus::current)
    }
}

#[derive(Debug, Clone)]
pub(crate) struct IdeFileStatus {
    path: PathBuf,
    state: IdeFileState,
}

impl IdeFileStatus {
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn state(&self) -> IdeFileState {
        self.state
    }

    fn current(&self) -> bool {
        self.state == IdeFileState::Current
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum IdeFileState {
    Missing,
    Outdated,
    Current,
}

#[derive(Debug, Clone)]
pub(crate) struct IdeRepairReport {
    written: Vec<PathBuf>,
    status: IdeStatus,
}

impl IdeRepairReport {
    pub(crate) fn written(&self) -> &[PathBuf] {
        &self.written
    }

    pub(crate) fn status(&self) -> &IdeStatus {
        &self.status
    }
}

struct IdePlan {
    root: PathBuf,
    idea_dir: PathBuf,
    rust_bin: PathBuf,
    stdlib_source: Option<PathBuf>,
    files: Vec<IdeFile>,
}

struct IdeFile {
    path: PathBuf,
    contents: String,
}

/// Inspect project-local IDE files without changing them.
pub(crate) fn inspect(
    paths: &EnvironmentPaths,
    manifest: &WorkspaceManifest,
    app_tools: &AppToolStatus,
) -> Result<IdeStatus, String> {
    let plan = build_plan(paths, manifest, app_tools)?;
    plan.status()
}

/// Preview project-local IDE files that would be written.
pub(crate) fn preview(
    paths: &EnvironmentPaths,
    manifest: &WorkspaceManifest,
    app_tools: &AppToolStatus,
) -> Result<IdeRepairReport, String> {
    let plan = build_plan(paths, manifest, app_tools)?;
    let written = plan
        .files
        .iter()
        .filter(|file| !file_current(file))
        .map(|file| file.path.clone())
        .collect();
    Ok(IdeRepairReport {
        written,
        status: plan.status()?,
    })
}

/// Write project-local IDE files under the selected source root's `.idea`.
pub(crate) fn repair(
    paths: &EnvironmentPaths,
    manifest: &WorkspaceManifest,
    app_tools: &AppToolStatus,
) -> Result<IdeRepairReport, String> {
    let plan = build_plan(paths, manifest, app_tools)?;
    fs::create_dir_all(&plan.idea_dir).map_err(|error| {
        format!(
            "failed to create IDE settings directory '{}': {error}",
            plan.idea_dir.display()
        )
    })?;
    let mut written = Vec::new();
    for file in &plan.files {
        ensure_contained(&plan.root, &file.path)?;
        if file_current(file) {
            continue;
        }
        fs::write(&file.path, &file.contents)
            .map_err(|error| format!("failed to write '{}': {error}", file.path.display()))?;
        written.push(file.path.clone());
    }
    Ok(IdeRepairReport {
        written,
        status: plan.status()?,
    })
}

fn build_plan(
    paths: &EnvironmentPaths,
    manifest: &WorkspaceManifest,
    app_tools: &AppToolStatus,
) -> Result<IdePlan, String> {
    let root = paths.source().root().to_path_buf();
    let idea_dir = root.join(".idea");
    let rust_bin = app_tools.rust().path().to_path_buf();
    let stdlib_source = rust_stdlib_source(&rust_bin).filter(|path| path.is_dir());
    let files = vec![
        IdeFile {
            path: idea_dir.join(CARGO_PROJECTS_FILE),
            contents: cargo_projects_xml(manifest),
        },
        IdeFile {
            path: idea_dir.join(RUST_SETTINGS_FILE),
            contents: rust_xml(&rust_bin, stdlib_source.as_deref()),
        },
        IdeFile {
            path: idea_dir.join(VAPOR_SETTINGS_FILE),
            contents: vapor_xml(paths, app_tools, stdlib_source.as_deref())?,
        },
    ];
    Ok(IdePlan {
        root,
        idea_dir,
        rust_bin,
        stdlib_source,
        files,
    })
}

impl IdePlan {
    fn status(&self) -> Result<IdeStatus, String> {
        let files = self
            .files
            .iter()
            .map(|file| {
                Ok(IdeFileStatus {
                    path: file.path.clone(),
                    state: file_state(file)?,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        Ok(IdeStatus {
            source_root: self.root.clone(),
            idea_dir: self.idea_dir.clone(),
            rust_bin: self.rust_bin.clone(),
            stdlib_source: self.stdlib_source.clone(),
            files,
        })
    }
}

fn cargo_projects_xml(manifest: &WorkspaceManifest) -> String {
    let mut cargo_projects = String::new();
    for project in manifest.cargo_projects() {
        cargo_projects.push_str("    <cargoProject FILE=\"$PROJECT_DIR$/");
        cargo_projects.push_str(&xml_escape(&project.manifest().to_string_lossy()));
        cargo_projects.push_str("\" />\n");
    }
    CARGO_PROJECTS_TEMPLATE.replace("{{cargo_projects}}", &cargo_projects)
}

fn rust_xml(rust_bin: &Path, stdlib_source: Option<&Path>) -> String {
    RUST_TEMPLATE
        .replace(
            "{{rust_bin_directory}}",
            &xml_escape(&rust_bin.to_string_lossy()),
        )
        .replace(
            "{{stdlib_option}}",
            &xml_option("explicitPathToStdlib", stdlib_source),
        )
}

fn vapor_xml(
    paths: &EnvironmentPaths,
    app_tools: &AppToolStatus,
    stdlib_source: Option<&Path>,
) -> Result<String, String> {
    let root = paths.installation().root();
    let cargo = paths
        .installation()
        .bundled_cargo()
        .ok_or_else(|| "cannot configure IDE: bundled Cargo is missing".to_owned())?;
    let rustc = app_tools.rust().path().join(executable("rustc"));
    let rustup = root.join("rustup/bin").join(executable("rustup"));
    Ok(VAPOR_TEMPLATE
        .replace("{{source_id}}", &xml_escape(paths.source().identity_id()))
        .replace("{{app_root}}", &xml_escape(&root.to_string_lossy()))
        .replace(
            "{{cargo_home}}",
            &xml_escape(&root.join("cargo-home").to_string_lossy()),
        )
        .replace(
            "{{rustup_home}}",
            &xml_escape(&root.join("rustup-home").to_string_lossy()),
        )
        .replace("{{cargo_path}}", &xml_escape(&cargo.to_string_lossy()))
        .replace("{{rustc_path}}", &xml_escape(&rustc.to_string_lossy()))
        .replace("{{rustup_path}}", &xml_escape(&rustup.to_string_lossy()))
        .replace(
            "{{stdlib_option}}",
            &xml_option("rustStdlibSource", stdlib_source),
        ))
}

fn xml_option(name: &str, path: Option<&Path>) -> String {
    path.map_or_else(String::new, |path| {
        format!(
            "    <option name=\"{}\" value=\"{}\" />\n",
            xml_escape(name),
            xml_escape(&path.to_string_lossy())
        )
    })
}

fn file_state(file: &IdeFile) -> Result<IdeFileState, String> {
    if !file.path.is_file() {
        return Ok(IdeFileState::Missing);
    }
    let current = fs::read_to_string(&file.path)
        .map_err(|error| format!("failed to read '{}': {error}", file.path.display()))?;
    if current == file.contents {
        Ok(IdeFileState::Current)
    } else {
        Ok(IdeFileState::Outdated)
    }
}

fn file_current(file: &IdeFile) -> bool {
    matches!(file_state(file), Ok(IdeFileState::Current))
}

fn rust_stdlib_source(rust_bin: &Path) -> Option<PathBuf> {
    rust_bin
        .parent()
        .map(|setup| setup.join("lib/rustlib/src/rust/library"))
}

fn executable(name: &str) -> String {
    format!("{name}{}", std::env::consts::EXE_SUFFIX)
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
