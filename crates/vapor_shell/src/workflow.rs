//! Cargo workflows routed through the Steam-installed Vapor setup.

use crate::{
    cross_toolchain,
    discovery::{EnvironmentPaths, ensure_contained},
    workspace::{CargoProject, WorkspaceManifest},
};
use std::{
    env,
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

/// Dynamic Cargo workspace selection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectSelection {
    /// Every Cargo workspace discovered for this source root.
    All,
    /// Cargo workspaces that declare installed application binaries.
    Installable,
    /// One named Cargo workspace discovered from the active source root.
    One(String),
}

impl std::str::FromStr for ProjectSelection {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let value = value.trim();
        if value == "all" {
            return Ok(Self::All);
        }
        if value == "installable" {
            return Ok(Self::Installable);
        }
        if value.is_empty()
            || value.chars().any(|character| {
                !character.is_ascii_lowercase() && !character.is_ascii_digit() && character != '-'
            })
        {
            return Err(
                "project must be `all` or a lowercase kebab-case Cargo workspace name".to_owned(),
            );
        }
        Ok(Self::One(value.to_owned()))
    }
}

/// Supported Cargo operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CargoWorkflow {
    /// Apply Rustfmt.
    Fmt,
    /// Type-check all workspace targets.
    Check,
    /// Test all workspace targets.
    Test,
    /// Build every workspace package.
    Build,
    /// Check formatting, compile, test, lint, and build documentation.
    Validate,
}

impl CargoWorkflow {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Fmt => "fmt",
            Self::Check => "check",
            Self::Test => "test",
            Self::Build => "build",
            Self::Validate => "validate",
        }
    }
}

/// Execute a workflow for the selected Cargo workspace or all workspaces.
///
/// Build artifacts are written beneath the app root at
/// `output/dev/<project>`. The host `PATH` is retained only for non-Rust host
/// tools such as a system linker; Cargo and Rustc resolve from the installation.
///
/// # Errors
///
/// Fails when bundled Cargo is unavailable, the configured project is missing,
/// environment construction fails, or any Cargo child exits unsuccessfully.
pub fn run(
    paths: &EnvironmentPaths,
    manifest: &WorkspaceManifest,
    selection: ProjectSelection,
    workflow: CargoWorkflow,
) -> Result<(), String> {
    run_with_target(paths, manifest, selection, workflow, None)
}

/// Execute a workflow for a selected Cargo target triple.
///
/// `None` uses Cargo's host-default target and keeps the historical
/// `output/dev/<workspace>/debug` output shape. A target triple passes
/// `--target TRIPLE` to target-aware Cargo operations.
pub fn run_with_target(
    paths: &EnvironmentPaths,
    manifest: &WorkspaceManifest,
    selection: ProjectSelection,
    workflow: CargoWorkflow,
    target_triple: Option<&str>,
) -> Result<(), String> {
    validate_target_triple(target_triple)?;
    let projects = selected_projects(manifest, selection)?;
    for project in projects {
        if workflow == CargoWorkflow::Validate {
            for step in [
                ValidationStep::FmtCheck,
                ValidationStep::Check,
                ValidationStep::Test,
                ValidationStep::Clippy,
                ValidationStep::Doc,
            ] {
                run_step(paths, project, step, target_triple)?;
            }
        } else {
            run_step(
                paths,
                project,
                ValidationStep::Workflow(workflow),
                target_triple,
            )?;
        }
    }
    Ok(())
}

/// Promote configured build artifacts into the installation's target-specific
/// `bin/<target>` directory for the host target.
///
/// # Errors
///
/// Fails when a configured artifact is missing, escapes installation output,
/// or cannot replace its installed destination.
pub fn promote(paths: &EnvironmentPaths, manifest: &WorkspaceManifest) -> Result<usize, String> {
    promote_for_targets(paths, manifest, &[])
}

/// Promote configured build artifacts for one or more target triples.
///
/// An empty target list uses Cargo's host-default output directory and promotes
/// into `bin/<host-target>`. Explicit targets read Cargo's
/// `output/dev/<workspace>/<target>/debug` output and promote into
/// `bin/<target>`.
///
/// # Errors
///
/// Fails when a configured artifact is missing, escapes installation output,
/// or cannot replace its installed destination.
pub fn promote_for_targets(
    paths: &EnvironmentPaths,
    manifest: &WorkspaceManifest,
    target_triples: &[String],
) -> Result<usize, String> {
    let targets = RuntimeTarget::many(target_triples)?;
    let installation = paths.installation().root();
    let mut promoted = 0;
    for project in manifest.cargo_projects() {
        for binary in project.binaries() {
            for target in &targets {
                let filename = target.binary_file_name(binary);
                let source = target
                    .build_output_root(installation, project.name())
                    .join(&filename);
                ensure_contained(installation, &source)?;
                if !source.is_file() {
                    return Err(format!(
                        "built binary is missing for project '{}' target '{}': {}",
                        project.name(),
                        target.triple(),
                        source.display()
                    ));
                }
                let destination = installation
                    .join("bin")
                    .join(target.triple())
                    .join(&filename);
                ensure_contained(installation, &destination)?;
                promote_file(&source, &destination)?;
                println!("promoted {} -> {}", source.display(), destination.display());
                promoted += 1;
            }
        }
    }
    Ok(promoted)
}

#[derive(Debug, Clone)]
struct RuntimeTarget {
    triple: String,
    explicit: bool,
}

impl RuntimeTarget {
    fn many(target_triples: &[String]) -> Result<Vec<Self>, String> {
        if target_triples.is_empty() {
            return Ok(vec![Self {
                triple: host_runtime_target(),
                explicit: false,
            }]);
        }
        let mut seen = std::collections::BTreeSet::new();
        let mut targets = Vec::new();
        for target in target_triples {
            validate_target_triple(Some(target))?;
            if !seen.insert(target.clone()) {
                return Err(format!("duplicate runtime target: {target}"));
            }
            targets.push(Self {
                triple: target.clone(),
                explicit: true,
            });
        }
        Ok(targets)
    }

    fn triple(&self) -> &str {
        &self.triple
    }

    fn build_output_root(&self, installation: &Path, project_name: &str) -> PathBuf {
        let root = installation.join("output/dev").join(project_name);
        if self.explicit {
            root.join(self.triple()).join("debug")
        } else {
            root.join("debug")
        }
    }

    fn binary_file_name(&self, binary: &str) -> String {
        if self.triple.contains("windows") {
            if binary.ends_with(".exe") {
                binary.to_owned()
            } else {
                format!("{binary}.exe")
            }
        } else {
            binary.to_owned()
        }
    }
}

pub(crate) fn host_runtime_target() -> String {
    let arch = std::env::consts::ARCH;
    match (arch, std::env::consts::OS, std::env::consts::FAMILY) {
        ("x86_64", "linux", _) => "x86_64-unknown-linux-gnu".to_owned(),
        ("aarch64", "linux", _) => "aarch64-unknown-linux-gnu".to_owned(),
        ("x86_64", "windows", _) => "x86_64-pc-windows-gnullvm".to_owned(),
        ("aarch64", "windows", _) => "aarch64-pc-windows-msvc".to_owned(),
        ("x86_64", "macos", _) => "x86_64-apple-darwin".to_owned(),
        ("aarch64", "macos", _) => "aarch64-apple-darwin".to_owned(),
        _ => format!(
            "{arch}-{}-{}",
            std::env::consts::OS,
            std::env::consts::FAMILY
        ),
    }
}

fn promote_file(source: &Path, destination: &Path) -> Result<(), String> {
    let parent = destination.parent().ok_or_else(|| {
        format!(
            "binary destination has no parent: {}",
            destination.display()
        )
    })?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("failed to create '{}': {error}", parent.display()))?;
    let temporary = destination.with_extension(format!("tmp-{}", std::process::id()));
    if temporary.exists() {
        fs::remove_file(&temporary)
            .map_err(|error| format!("failed to clear '{}': {error}", temporary.display()))?;
    }
    fs::copy(source, &temporary).map_err(|error| {
        format!(
            "failed to copy '{}' to '{}': {error}",
            source.display(),
            temporary.display()
        )
    })?;
    if destination.exists() {
        fs::remove_file(destination).map_err(|error| {
            format!(
                "failed to replace installed binary '{}': {error}",
                destination.display()
            )
        })?;
    }
    fs::rename(&temporary, destination).map_err(|error| {
        format!(
            "failed to promote '{}' to '{}': {error}",
            temporary.display(),
            destination.display()
        )
    })
}

fn selected_projects(
    manifest: &WorkspaceManifest,
    selection: ProjectSelection,
) -> Result<Vec<&CargoProject>, String> {
    match selection {
        ProjectSelection::All => {
            if manifest.cargo_projects().is_empty() {
                Err(format!(
                    "source root '{}' declares no Cargo workspaces",
                    manifest.id()
                ))
            } else {
                Ok(manifest.cargo_projects().iter().collect())
            }
        }
        ProjectSelection::Installable => {
            let projects = manifest
                .cargo_projects()
                .iter()
                .filter(|project| !project.binaries().is_empty())
                .collect::<Vec<_>>();
            if projects.is_empty() {
                Err(format!(
                    "source root '{}' declares no installable Cargo workspaces",
                    manifest.id()
                ))
            } else {
                Ok(projects)
            }
        }
        ProjectSelection::One(name) => manifest
            .cargo_projects()
            .iter()
            .find(|project| project.name() == name.as_str())
            .map(|project| vec![project])
            .ok_or_else(|| format!("source root does not declare Cargo workspace '{name}'")),
    }
}

#[derive(Clone, Copy)]
enum ValidationStep {
    Workflow(CargoWorkflow),
    FmtCheck,
    Check,
    Test,
    Clippy,
    Doc,
}

impl ValidationStep {
    fn label(self) -> &'static str {
        match self {
            Self::Workflow(workflow) => workflow.label(),
            Self::FmtCheck => "fmt --check",
            Self::Check => "check",
            Self::Test => "test",
            Self::Clippy => "clippy",
            Self::Doc => "doc",
        }
    }

    fn args(self) -> (&'static [&'static str], &'static [&'static str]) {
        match self {
            Self::Workflow(CargoWorkflow::Fmt) => (&["fmt", "--all"], &[]),
            Self::Workflow(CargoWorkflow::Check) | Self::Check => {
                (&["check", "--workspace", "--all-targets", "--locked"], &[])
            }
            Self::Workflow(CargoWorkflow::Test) | Self::Test => {
                (&["test", "--workspace", "--all-targets", "--locked"], &[])
            }
            Self::Workflow(CargoWorkflow::Build) => (&["build", "--workspace", "--locked"], &[]),
            Self::Workflow(CargoWorkflow::Validate) => unreachable!("validate expands into steps"),
            Self::FmtCheck => (&["fmt", "--all"], &["--check"]),
            Self::Clippy => (
                &["clippy", "--workspace", "--all-targets", "--locked"],
                &["-D", "warnings"],
            ),
            Self::Doc => (&["doc", "--workspace", "--no-deps", "--locked"], &[]),
        }
    }
}

fn run_step(
    paths: &EnvironmentPaths,
    project: &CargoProject,
    step: ValidationStep,
    target_triple: Option<&str>,
) -> Result<(), String> {
    let cargo = paths
        .installation()
        .bundled_cargo()
        .ok_or_else(|| "Steam installation has no bundled Cargo executable".to_owned())?;
    let manifest = paths.source().root().join(project.manifest());
    let working_directory = manifest
        .parent()
        .ok_or_else(|| format!("Cargo manifest has no parent: {}", manifest.display()))?;
    let installation = paths.installation().root();
    let target = installation.join("output/dev").join(project.name());

    println!("==> {}: {}", project.name(), step.label());
    let mut command = Command::new(&cargo);
    let (cargo_args, tool_args) = step.args();
    command
        .args(cargo_args)
        .args(["--manifest-path"])
        .arg(&manifest)
        .current_dir(working_directory)
        .env("VAPOR_HOME", installation)
        .env("CARGO_HOME", installation.join("cargo-home"))
        .env("RUSTUP_HOME", installation.join("rustup-home"))
        .env("CARGO_TARGET_DIR", target)
        .env("PATH", managed_path(paths)?)
        .env_remove("RUSTC_WRAPPER");
    if step.accepts_target()
        && let Some(target) = target_triple
    {
        cross_toolchain::configure_linker_env(&mut command, installation, target)?;
        command.args(["--target", target]);
    }
    if !tool_args.is_empty() {
        command.arg("--").args(tool_args);
    }
    if matches!(step, ValidationStep::Doc) {
        command.env("RUSTDOCFLAGS", "-D warnings");
    }

    let status = command.status().map_err(|error| {
        format!(
            "failed to run {} for '{}': {error}",
            step.label(),
            project.name()
        )
    })?;
    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "{} failed for project '{}' with {status}",
            step.label(),
            project.name()
        ))
    }
}

impl ValidationStep {
    fn accepts_target(self) -> bool {
        !matches!(self, Self::Workflow(CargoWorkflow::Fmt) | Self::FmtCheck)
    }
}

fn validate_target_triple(target: Option<&str>) -> Result<(), String> {
    let Some(target) = target else {
        return Ok(());
    };
    let valid = !target.is_empty()
        && target
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'));
    if valid {
        Ok(())
    } else {
        Err(format!(
            "target triple must be a Rust target triple such as x86_64-pc-windows-gnullvm: {target}"
        ))
    }
}

pub(crate) fn managed_path(paths: &EnvironmentPaths) -> Result<OsString, String> {
    let installation = paths.installation();
    let root = installation.root();
    let mut entries = Vec::<PathBuf>::new();
    let cargo = installation.bundled_cargo();
    if let Some(parent) = cargo.as_deref().and_then(|cargo| cargo.parent()) {
        entries.push(parent.to_path_buf());
    }
    entries.extend([
        root.join("tools/git/cmd"),
        root.join("tools/git/bin"),
        root.join("tools/cross/bin"),
        root.join("tools/zig"),
        root.join("tools/llvm-mingw/bin"),
        root.join("cargo-home/bin"),
        root.join("rustup/bin"),
        root.join("tools/steamcmd"),
        root.join("bin"),
    ]);
    if let Some(existing) = env::var_os("PATH") {
        entries.extend(env::split_paths(&existing));
    }
    env::join_paths(entries).map_err(|error| format!("failed to construct Vapor PATH: {error}"))
}
