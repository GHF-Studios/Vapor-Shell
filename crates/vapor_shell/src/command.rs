//! Interactive command grammar and execution.
//!
//! Static finite argument domains use Clap enums. Dynamic domains, such as
//! discovered Cargo workspace names, are validated by Vapor metadata.
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
    /// Change directory inside this Vapor source root; omit the path to jump to its root.
    Cd {
        /// Absolute or relative directory that must resolve inside the source root.
        #[arg(value_name = "SOURCE_PATH")]
        path: Option<PathBuf>,
    },

    /// Move toward the Vapor source root by one or more levels.
    Up {
        /// Number of parent directories to traverse; must be at least one.
        #[arg(value_name = "LEVELS", default_value = "1")]
        levels: NonZeroUsize,
    },

    /// Print the current internal working directory.
    Pwd,

    /// List a directory inside the source root.
    Ls {
        /// Directory to list; defaults to the current internal directory.
        #[arg(value_name = "SOURCE_PATH")]
        path: Option<PathBuf>,
    },

    /// Jump to the Vapor source root.
    Root,

    /// Print the replaceable Steam application root.
    Installation,

    /// Print the directory containing the running Vapor shell binary.
    Binaries,

    /// Print the installation's conventional library directory, when present.
    Libraries,

    /// Show resolved source, installation, toolchain, and Cargo metadata.
    Metadata {
        /// Output representation for people or automation.
        #[arg(long, value_enum, default_value_t)]
        format: MetadataFormat,
    },

    /// Format Cargo workspaces through the Steam-installed toolchain.
    Fmt {
        /// Cargo workspace name to format, or `all`.
        #[arg(long, value_name = "PROJECT", default_value = "all")]
        project: ProjectSelection,
    },

    /// Check Cargo workspaces through the Steam-installed toolchain.
    Check {
        /// Cargo workspace name to check, or `all`.
        #[arg(long, value_name = "PROJECT", default_value = "all")]
        project: ProjectSelection,
    },

    /// Test Cargo workspaces through the Steam-installed toolchain.
    Test {
        /// Cargo workspace name to test, or `all`.
        #[arg(long, value_name = "PROJECT", default_value = "all")]
        project: ProjectSelection,
    },

    /// Build Cargo workspaces through the Steam-installed toolchain.
    Build {
        /// Cargo workspace name to build, or `all`.
        #[arg(long, value_name = "PROJECT", default_value = "all")]
        project: ProjectSelection,
    },

    /// Run formatting, checking, tests, Clippy, and Rustdoc.
    Validate {
        /// Cargo workspace name to validate, or `all`.
        #[arg(long, value_name = "PROJECT", default_value = "all")]
        project: ProjectSelection,
    },

    /// Inspect, install, repair, or remove app-local Rust, Git, and SteamCMD.
    Toolchain {
        /// Toolchain operation.
        #[command(subcommand)]
        command: ToolchainCommand,
    },

    /// Persist or clear the source root used by Steam GUI launches.
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

/// Persisted source-root selection.
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
    /// Report the app root, active tools, and vendored install packages.
    Status,
    /// Install missing Rust, Git, and SteamCMD packages inside the app root.
    Install,
    /// Remove app-local Rust, Git, SteamCMD, PATH registration, and location state.
    Uninstall,
    /// Reapply vendored Rust, Git, and SteamCMD packages inside the app root.
    Repair,
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
        dry_run: bool,
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
        dry_run: bool,
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
        &ValidationPlan::new("change the remembered source root").registered_location(),
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
                "remembered source root: {}",
                state.paths().source().root().display()
            );
            println!("hint: next inspect prerequisites with `vapor toolchain status`");
        }
        WorkspaceCommand::Forget => {
            if path.exists() {
                fs::remove_file(&path).map_err(|e| e.to_string())?;
            }
            println!("forgot remembered source root");
            println!(
                "hint: run Vapor inside another source root and use `vapor workspace remember`"
            );
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
            .registered_location()
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
            if !location.registered() {
                println!("hint: accept this app root explicitly with `vapor toolchain install`");
            } else if status.complete() {
                println!("hint: toolchain is ready; next run `vapor validate`");
            } else if status.packages_complete() {
                println!("hint: install explicitly with `vapor toolchain install`");
            } else {
                println!("hint: verify the Steam app files before attempting installation");
            }
        }
        ToolchainCommand::Install => {
            let change = toolchain::register_location(installation)?;
            print_location_status(change.status());
            let report = toolchain::install(installation)?;
            state.refresh_cargo_index();
            if report.installed_groups().is_empty() {
                println!("toolchain is already installed; no files changed");
            } else {
                println!("installed: {}", report.installed_groups().join(", "));
            }
            print_path_hint(change.path_setup());
            println!("hint: confirm with `vapor toolchain status`, then run `vapor validate`");
        }
        ToolchainCommand::Repair => {
            let change = toolchain::register_location(installation)?;
            print_location_status(change.status());
            let report = toolchain::repair(installation)?;
            state.refresh_cargo_index();
            if report.installed_groups().is_empty() {
                println!("toolchain repair found all groups already installed");
            } else {
                println!("repaired: {}", report.installed_groups().join(", "));
            }
            print_path_hint(change.path_setup());
            println!("hint: confirm with `vapor toolchain status`, then run `vapor validate`");
        }
        ToolchainCommand::Uninstall => {
            let report = toolchain::uninstall(installation)?;
            state.refresh_cargo_index();
            print_location_status(report.location().status());
            println!(
                "removed {} app-local tool directories",
                report.removed_paths()
            );
            print_path_hint(report.location().path_setup());
            println!("hint: reinstall later with `vapor toolchain install`");
        }
    }
    Ok(())
}

fn print_location_status(status: &toolchain::LocationStatus) {
    match status {
        toolchain::LocationStatus::Unregistered { current } => {
            println!("app root: unregistered");
            println!("  current:   {}", current.display());
        }
        toolchain::LocationStatus::Registered { path } => {
            println!("app root: registered");
            println!("  path:      {}", path.display());
        }
        toolchain::LocationStatus::Moved { locked, current } => {
            println!("app root: moved (confirmation required)");
            println!("  previous:  {}", locked.display());
            println!("  current:   {}", current.display());
        }
    }
}

fn print_path_hint(report: &crate::path_setup::PathSetupReport) {
    println!("PATH directory: {}", report.binaries().display());
    if report.changed() || !report.path_active() {
        println!("hint: open a new terminal to apply PATH changes");
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
                    .registered_location()
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
                    .registered_location()
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
                    .registered_location()
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
                    .registered_location(),
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
                "hint: preview publication with `vapor steam publish --account ACCOUNT --dry-run`"
            );
        }
    }
    Ok(())
}

fn execute_script_command(command: ScriptCommand, state: &mut ShellState) -> Result<(), String> {
    let ScriptCommand::Run { name, dry_run } = command;
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
        if dry_run {
            continue;
        }
        let args = shlex::split(line)
            .ok_or_else(|| format!("invalid quoting at {}:{}", path.display(), index + 1))?;
        let parsed = ShellCommand::try_parse_from(
            std::iter::once("vapor").chain(args.iter().map(String::as_str)),
        )
        .map_err(|error| error.to_string())?;
        if !script_command_allowed(&parsed) {
            return Err(
                "scripts may not invoke scripts, exit the host shell, authenticate Steam, or perform real publishes"
                    .to_owned(),
            );
        }
        execute(parsed, state)?;
    }
    Ok(())
}

fn script_command_allowed(command: &ShellCommand) -> bool {
    !matches!(
        command,
        ShellCommand::Script { .. }
            | ShellCommand::Exit
            | ShellCommand::Steam {
                command: SteamCommand::Login { .. }
            }
            | ShellCommand::Steam {
                command: SteamCommand::Publish { dry_run: false, .. }
            }
    )
}

fn execute_steam(command: SteamCommand, state: &ShellState) -> Result<(), String> {
    let metadata = ResolvedMetadata::resolve(state);
    match command {
        SteamCommand::Login { account } => {
            metadata.validate(
                &ValidationPlan::new("authenticate SteamCMD")
                    .registered_location()
                    .tools(&[Requirement::SteamCmd]),
            )?;
            steam::login(state.paths(), &account)?;
            println!(
                "hint: authentication returned successfully; preview with `vapor steam publish --account {account} --dry-run`"
            );
            Ok(())
        }
        SteamCommand::Publish {
            account,
            branch,
            description,
            dry_run,
            yes,
        } => {
            metadata.validate(
                &ValidationPlan::new("publish the Vapor application")
                    .registered_location()
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
                dry_run,
                yes,
            )?;
            println!("SteamPipe build script: {}", script.display());
            if dry_run {
                println!(
                    "hint: review the staged build, then rerun with `--yes` and without `--dry-run`"
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
