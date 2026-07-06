//! Interactive command grammar and execution.
//!
//! Finite argument domains should use Clap enums. The current arguments are
//! paths confined by [`ShellState`] or a positive integer represented by
//! `NonZeroUsize`, so help and parser errors describe their actual contracts.
//!
//! Installation commands report replaceable resource paths; they never move
//! the source cursor into the Steam application directory.

use crate::{
    distribution, documentation,
    metadata::{MetadataFormat, ResolvedMetadata, ValidationPlan},
    state::ShellState,
    steam,
    toolchain::{self, Requirement},
    workflow::{self, CargoWorkflow, ProjectSelection},
};
use clap::{Parser, Subcommand};
use std::{
    fs,
    num::NonZeroUsize,
    path::{Path, PathBuf},
};

#[derive(Debug, Parser)]
#[command(name = "vapor", bin_name = "vapor")]
/// Commands accepted by the interactive Vapor prompt.
pub enum ShellCommand {
    /// Change directory inside this Vapor workspace; omit the path to jump to its root.
    Cd {
        /// Absolute or relative directory that must resolve inside the workspace.
        #[arg(value_name = "WORKSPACE_PATH")]
        path: Option<PathBuf>,
    },

    /// Move toward the Vapor workspace root by one or more levels.
    Up {
        /// Number of parent directories to traverse; must be at least one.
        #[arg(value_name = "LEVELS", default_value = "1")]
        levels: NonZeroUsize,
    },

    /// Print the current internal working directory.
    Pwd,

    /// List a directory inside the workspace.
    Ls {
        /// Directory to list; defaults to the current internal directory.
        #[arg(value_name = "WORKSPACE_PATH")]
        path: Option<PathBuf>,
    },

    /// Jump to the Vapor workspace root.
    Root,

    /// Print the replaceable Steam application root.
    Installation,

    /// Print the directory containing the running Vapor shell binary.
    Binaries,

    /// Print the installation's conventional library directory, when present.
    Libraries,

    /// Show resolved workspace, installation, toolchain, and Cargo metadata.
    Metadata {
        /// Output representation for people or automation.
        #[arg(long, value_enum, default_value_t)]
        format: MetadataFormat,
    },

    /// Format root Cargo projects through the Steam-installed toolchain.
    Fmt {
        /// Root project to format.
        #[arg(long, value_enum, default_value = "all")]
        project: ProjectSelection,
    },

    /// Check root Cargo projects through the Steam-installed toolchain.
    Check {
        /// Root project to check.
        #[arg(long, value_enum, default_value = "all")]
        project: ProjectSelection,
    },

    /// Test root Cargo projects through the Steam-installed toolchain.
    Test {
        /// Root project to test.
        #[arg(long, value_enum, default_value = "all")]
        project: ProjectSelection,
    },

    /// Build root Cargo projects through the Steam-installed toolchain.
    Build {
        /// Root project to build.
        #[arg(long, value_enum, default_value = "all")]
        project: ProjectSelection,
    },

    /// Run formatting, checking, tests, Clippy, and Rustdoc.
    Validate {
        /// Root project to validate.
        #[arg(long, value_enum, default_value = "all")]
        project: ProjectSelection,
    },

    /// Inspect or explicitly install app-local Rust, Git, and SteamCMD.
    Toolchain {
        /// Toolchain operation.
        #[command(subcommand)]
        command: ToolchainCommand,
    },

    /// Persist or clear the source workspace used by Steam GUI launches.
    Workspace {
        /// Workspace selection operation.
        #[command(subcommand)]
        command: WorkspaceCommand,
    },

    /// Build, locate, or open installed documentation.
    Docs {
        /// Documentation operation.
        #[command(subcommand)]
        command: DocsCommand,
    },

    /// Assemble or validate the self-hosting Steam application.
    #[command(name = "self")]
    SelfHost {
        /// Self-hosting operation.
        #[command(subcommand)]
        command: SelfCommand,
    },

    /// Run a source-controlled sequence of Vapor commands.
    Script {
        /// Script operation.
        #[command(subcommand)]
        command: ScriptCommand,
    },

    /// Authenticate or publish through installation-owned SteamCMD.
    Steam {
        /// Steam operation.
        #[command(subcommand)]
        command: SteamCommand,
    },

    /// Exit the Vapor sub-shell.
    #[command(alias = "quit")]
    Exit,
}

/// Documentation operations.
#[derive(Debug, Subcommand)]
pub enum DocsCommand {
    /// Build all declared Rustdoc into the installation.
    Build,
    /// Print an installed documentation path.
    Path {
        /// Optional documentation section.
        topic: Option<String>,
    },
    /// Open installed documentation without blocking the shell.
    Open {
        /// Optional documentation section.
        topic: Option<String>,
    },
}

/// Persisted source-workspace selection.
#[derive(Debug, Subcommand)]
pub enum WorkspaceCommand {
    /// Remember the current source root for future Steam launches.
    Remember,
    /// Remove the remembered source root.
    Forget,
}

/// App-local toolchain lifecycle operations.
#[derive(Debug, Subcommand)]
pub enum ToolchainCommand {
    /// Report VAPOR_HOME, active tools, and vendored install packages.
    Status,
    /// Explicitly accept the current VAPOR_HOME and update PATH registration.
    Finalize,
    /// Install missing Rust, Git, and SteamCMD packages inside the app root.
    Install {
        /// Reapply every package while preserving extra mutable tool state.
        #[arg(long)]
        repair: bool,
    },
    /// Remove the VAPOR_HOME fixpoint and marked PATH registration.
    Unlock,
}

/// Self-hosting application operations.
#[derive(Debug, Subcommand)]
pub enum SelfCommand {
    /// Build every project and promote declared binaries into the installation.
    Rebuild,
    /// Build docs and assemble a clean allowlisted depot tree.
    Stage,
    /// Validate the currently staged application.
    Smoke,
}

/// Vapor command-script operations.
#[derive(Debug, Subcommand)]
pub enum ScriptCommand {
    /// Run `.vapor/scripts/<NAME>.vapor`.
    Run {
        /// Script filename stem under `.vapor/scripts`.
        name: String,
        /// Print commands without executing them.
        #[arg(long)]
        plan: bool,
    },
}

/// SteamCMD operations.
#[derive(Debug, Subcommand)]
pub enum SteamCommand {
    /// Hand the terminal to SteamCMD for interactive login.
    Login {
        /// Dedicated Steam build account.
        #[arg(long)]
        account: String,
    },
    /// Stage and publish to a non-default Steam beta branch.
    Publish {
        /// Dedicated Steam build account.
        #[arg(long)]
        account: String,
        /// Existing non-default beta branch; defaults to the distribution manifest.
        #[arg(long)]
        branch: Option<String>,
        /// Internal Steam build description.
        #[arg(long, default_value = "Vapor development build")]
        description: String,
        /// Generate staging and a preview VDF without uploading.
        #[arg(long)]
        plan: bool,
        /// Confirm the real network upload.
        #[arg(long)]
        yes: bool,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Whether the application loop should read another command.
pub enum Control {
    /// Read another command.
    Continue,
    /// End the interactive session.
    Exit,
}

/// Execute one parsed command against session state.
///
/// # Errors
///
/// Returns navigation, filesystem, or missing-installation-resource errors.
pub fn execute(command: ShellCommand, state: &mut ShellState) -> Result<Control, String> {
    match command {
        ShellCommand::Cd { path } => {
            let target = path.unwrap_or_else(|| state.paths().source().root().to_path_buf());
            print_warnings(state.change_directory(&target)?);
        }
        ShellCommand::Up { levels } => {
            print_warnings(state.move_up(levels.get())?);
        }
        ShellCommand::Pwd => println!("{}", state.current_dir().display()),
        ShellCommand::Ls { path } => {
            list_directory(state, path.as_deref().unwrap_or_else(|| Path::new(".")))?;
        }
        ShellCommand::Root => {
            let target = state.paths().source().root().to_path_buf();
            print_warnings(state.change_directory_to(target)?);
        }
        ShellCommand::Installation => {
            println!("{}", state.paths().installation().root().display());
        }
        ShellCommand::Binaries => {
            println!("{}", state.paths().installation().binaries().display());
        }
        ShellCommand::Libraries => {
            let target = state.paths().installation().libraries().ok_or_else(|| {
                format!(
                    "Steam installation '{}' has no library directory",
                    state.paths().installation().root().display()
                )
            })?;
            println!("{}", target.display());
        }
        ShellCommand::Metadata { format } => {
            println!("{}", ResolvedMetadata::resolve(state).render(format)?);
        }
        ShellCommand::Fmt { project } => execute_workflow(CargoWorkflow::Fmt, project, state)?,
        ShellCommand::Check { project } => {
            execute_workflow(CargoWorkflow::Check, project, state)?;
        }
        ShellCommand::Test { project } => execute_workflow(CargoWorkflow::Test, project, state)?,
        ShellCommand::Build { project } => {
            execute_workflow(CargoWorkflow::Build, project, state)?;
        }
        ShellCommand::Validate { project } => {
            execute_workflow(CargoWorkflow::Validate, project, state)?;
        }
        ShellCommand::Toolchain { command } => execute_toolchain(command, state)?,
        ShellCommand::Workspace { command } => execute_workspace(command, state)?,
        ShellCommand::Docs { command } => execute_docs(command, state)?,
        ShellCommand::SelfHost { command } => execute_self(command, state)?,
        ShellCommand::Script { command } => execute_script_command(command, state)?,
        ShellCommand::Steam { command } => execute_steam(command, state)?,
        ShellCommand::Exit => return Ok(Control::Exit),
    }

    Ok(Control::Continue)
}

fn execute_workspace(command: WorkspaceCommand, state: &ShellState) -> Result<(), String> {
    let metadata = ResolvedMetadata::resolve(state);
    metadata.validate(
        &ValidationPlan::new("change the remembered source workspace").finalized_location(),
    )?;
    let path = state
        .paths()
        .installation()
        .root()
        .join("state/source-workspace");
    match command {
        WorkspaceCommand::Remember => {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            fs::write(&path, state.paths().source().root().display().to_string())
                .map_err(|e| e.to_string())?;
            println!(
                "remembered source workspace: {}",
                state.paths().source().root().display()
            );
            println!("hint: next inspect prerequisites with `vapor toolchain status`");
        }
        WorkspaceCommand::Forget => {
            if path.exists() {
                fs::remove_file(&path).map_err(|e| e.to_string())?;
            }
            println!("forgot remembered source workspace");
            println!("hint: run Vapor inside another workspace and use `vapor workspace remember`");
        }
    }
    Ok(())
}

fn execute_workflow(
    command: CargoWorkflow,
    project: ProjectSelection,
    state: &ShellState,
) -> Result<(), String> {
    let metadata = ResolvedMetadata::resolve(state);
    metadata.validate(
        &ValidationPlan::new(command.label())
            .finalized_location()
            .tools(&[Requirement::Rust, Requirement::Git])
            .workspace(),
    )?;
    workflow::run(
        state.paths(),
        metadata.workspace_manifest()?,
        project,
        command,
    )?;
    println!("hint: {}", command.next_hint());
    Ok(())
}

fn execute_toolchain(command: ToolchainCommand, state: &mut ShellState) -> Result<(), String> {
    let installation = state.paths().installation();
    match command {
        ToolchainCommand::Status => {
            let location = toolchain::location_status(installation)?;
            print_location_status(&location);
            let status = toolchain::inspect(installation);
            print_tool_status(status.rust());
            print_tool_status(status.git());
            print_tool_status(status.steamcmd());
            println!(
                "vendored packages: {}",
                if status.packages_complete() {
                    "complete"
                } else {
                    "incomplete"
                }
            );
            for missing in status.missing_packages() {
                println!("  missing: {missing}");
            }
            if !location.finalized() {
                println!("hint: review and accept VAPOR_HOME with `vapor toolchain finalize`");
            } else if status.complete() {
                println!("hint: toolchain is ready; next run `vapor validate`");
            } else if status.packages_complete() {
                println!("hint: install explicitly with `vapor toolchain install`");
            } else {
                println!("hint: verify the Steam app files before attempting installation");
            }
        }
        ToolchainCommand::Finalize => {
            let change = toolchain::finalize_location(installation)?;
            print_location_status(change.status());
            println!(
                "PATH directory: {}",
                change.path_setup().binaries().display()
            );
            if change.path_setup().changed() || !change.path_setup().path_active() {
                println!("hint: open a new terminal to apply the finalized PATH");
            } else {
                println!("hint: VAPOR_HOME is finalized; next run `vapor toolchain install`");
            }
        }
        ToolchainCommand::Install { repair } => {
            let report = toolchain::install(installation, repair)?;
            state.refresh_cargo_index();
            if report.installed_groups().is_empty() {
                println!("toolchain is already installed; no files changed");
            } else {
                println!("installed: {}", report.installed_groups().join(", "));
            }
            println!("hint: confirm with `vapor toolchain status`, then run `vapor validate`");
        }
        ToolchainCommand::Unlock => {
            let change = toolchain::unlock_location(installation)?;
            print_location_status(change.status());
            println!("hint: open a new terminal to remove the old PATH entry");
        }
    }
    Ok(())
}

fn print_location_status(status: &toolchain::LocationStatus) {
    match status {
        toolchain::LocationStatus::Unfinalized { current } => {
            println!("VAPOR_HOME: unfinalized");
            println!("  current:   {}", current.display());
        }
        toolchain::LocationStatus::Finalized { path } => {
            println!("VAPOR_HOME: finalized");
            println!("  path:      {}", path.display());
        }
        toolchain::LocationStatus::Moved { locked, current } => {
            println!("VAPOR_HOME: moved (confirmation required)");
            println!("  finalized: {}", locked.display());
            println!("  current:   {}", current.display());
        }
    }
}

fn print_tool_status(status: &toolchain::ToolStatus) {
    println!(
        "{}: {}",
        status.label(),
        if status.installed() {
            "installed"
        } else {
            "missing"
        }
    );
    println!("  path: {}", status.path().display());
    for missing in status.missing() {
        println!("  missing: {missing}");
    }
}

fn execute_docs(command: DocsCommand, state: &ShellState) -> Result<(), String> {
    match command {
        DocsCommand::Build => {
            let metadata = ResolvedMetadata::resolve(state);
            metadata.validate(
                &ValidationPlan::new("build documentation")
                    .finalized_location()
                    .tools(&[Requirement::Rust])
                    .workspace(),
            )?;
            println!(
                "{}",
                documentation::build(state.paths(), metadata.workspace_manifest()?)?.display()
            );
            println!("hint: open it with `vapor docs open`");
        }
        DocsCommand::Path { topic } => println!(
            "{}",
            documentation::path(state.paths(), topic.as_deref())?.display()
        ),
        DocsCommand::Open { topic } => println!(
            "{}",
            documentation::open(state.paths(), topic.as_deref())?.display()
        ),
    }
    Ok(())
}

fn execute_self(command: SelfCommand, state: &ShellState) -> Result<(), String> {
    let metadata = ResolvedMetadata::resolve(state);
    match command {
        SelfCommand::Rebuild => {
            metadata.validate(
                &ValidationPlan::new("rebuild the Vapor application")
                    .finalized_location()
                    .tools(&[Requirement::Rust, Requirement::Git])
                    .workspace(),
            )?;
            workflow::run(
                state.paths(),
                metadata.workspace_manifest()?,
                ProjectSelection::All,
                CargoWorkflow::Build,
            )?;
            let promoted = workflow::promote(state.paths(), metadata.workspace_manifest()?)?;
            println!("promoted {promoted} installation binaries");
            println!("hint: next run `vapor self stage` or preview a Steam publish");
        }
        SelfCommand::Stage => {
            metadata.validate(
                &ValidationPlan::new("stage the Vapor application")
                    .finalized_location()
                    .tools(&[Requirement::Rust])
                    .workspace()
                    .distribution()
                    .toolchain_package(),
            )?;
            documentation::build(state.paths(), metadata.workspace_manifest()?)?;
            let report = distribution::stage(state.paths(), metadata.distribution_manifest()?)?;
            println!(
                "staged {} files at {}",
                report.files(),
                report.root().display()
            );
            println!("hint: validate staging with `vapor self smoke`");
        }
        SelfCommand::Smoke => {
            metadata.validate(
                &ValidationPlan::new("smoke-test the staged Vapor application")
                    .finalized_location(),
            )?;
            let root = state
                .paths()
                .installation()
                .root()
                .join("output/root/content");
            steam::smoke(&root)?;
            println!(
                "staged application passed smoke validation: {}",
                root.display()
            );
            println!(
                "hint: preview publication with `vapor steam publish --account ACCOUNT --plan`"
            );
        }
    }
    Ok(())
}

fn execute_script_command(command: ScriptCommand, state: &mut ShellState) -> Result<(), String> {
    let ScriptCommand::Run { name, plan } = command;
    if name.contains('/') || name.contains('\\') || name == "." || name == ".." {
        return Err("script name must be a simple filename stem".to_owned());
    }
    let path = state
        .paths()
        .source()
        .root()
        .join(".vapor/scripts")
        .join(format!("{name}.vapor"));
    let source = fs::read_to_string(&path)
        .map_err(|error| format!("failed to read '{}': {error}", path.display()))?;
    for (index, line) in source.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        println!("{}:{}: {line}", path.display(), index + 1);
        if plan {
            continue;
        }
        let args = shlex::split(line)
            .ok_or_else(|| format!("invalid quoting at {}:{}", path.display(), index + 1))?;
        let parsed = ShellCommand::try_parse_from(
            std::iter::once("vapor").chain(args.iter().map(String::as_str)),
        )
        .map_err(|error| error.to_string())?;
        if matches!(parsed, ShellCommand::Script { .. } | ShellCommand::Exit) {
            return Err("scripts may not invoke scripts or exit the host shell".to_owned());
        }
        execute(parsed, state)?;
    }
    Ok(())
}

fn execute_steam(command: SteamCommand, state: &ShellState) -> Result<(), String> {
    let metadata = ResolvedMetadata::resolve(state);
    match command {
        SteamCommand::Login { account } => {
            metadata.validate(
                &ValidationPlan::new("authenticate SteamCMD")
                    .finalized_location()
                    .tools(&[Requirement::SteamCmd]),
            )?;
            steam::login(state.paths(), &account)?;
            println!(
                "hint: authentication returned successfully; preview with `vapor steam publish --account {account} --plan`"
            );
            Ok(())
        }
        SteamCommand::Publish {
            account,
            branch,
            description,
            plan,
            yes,
        } => {
            metadata.validate(
                &ValidationPlan::new("publish the Vapor application")
                    .finalized_location()
                    .tools(&[Requirement::Rust, Requirement::Git, Requirement::SteamCmd])
                    .workspace()
                    .distribution()
                    .toolchain_package(),
            )?;
            workflow::run(
                state.paths(),
                metadata.workspace_manifest()?,
                ProjectSelection::All,
                CargoWorkflow::Validate,
            )?;
            workflow::run(
                state.paths(),
                metadata.workspace_manifest()?,
                ProjectSelection::All,
                CargoWorkflow::Build,
            )?;
            workflow::promote(state.paths(), metadata.workspace_manifest()?)?;
            documentation::build(state.paths(), metadata.workspace_manifest()?)?;
            let script = steam::publish(
                state.paths(),
                metadata.distribution_manifest()?,
                &account,
                branch.as_deref(),
                &description,
                plan,
                yes,
            )?;
            println!("SteamPipe build script: {}", script.display());
            if plan {
                println!(
                    "hint: review the staged build, then rerun with `--yes` and without `--plan`"
                );
            } else {
                println!("hint: SteamCMD accepted the build; verify the vapor-dev branch in Steam");
            }
            Ok(())
        }
    }
}

fn list_directory(state: &ShellState, target: &Path) -> Result<(), String> {
    let directory = state.resolve_directory(target)?;
    let mut entries = fs::read_dir(&directory)
        .map_err(|error| format!("failed to list '{}': {error}", directory.display()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("failed to read directory entry: {error}"))?;
    entries.sort_by_key(|entry| entry.file_name());

    for entry in entries {
        let file_type = entry
            .file_type()
            .map_err(|error| format!("failed to inspect '{}': {error}", entry.path().display()))?;
        let suffix = if file_type.is_dir() { "/" } else { "" };
        println!("{}{}", entry.file_name().to_string_lossy(), suffix);
    }
    Ok(())
}

pub(crate) fn print_warnings(warnings: Vec<String>) {
    for warning in warnings {
        eprintln!("warning: {warning}");
    }
}
