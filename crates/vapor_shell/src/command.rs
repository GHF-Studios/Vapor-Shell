//! Interactive command grammar and execution.
//!
//! Static finite argument domains use Clap enums. Dynamic domains, such as
//! discovered Cargo workspace names, are validated by Vapor metadata.
//!
//! Installation commands report app-root resource paths; they never move
//! the source cursor into the Steam application directory.

use crate::{
    discovery::EnvironmentPaths,
    documentation, ide,
    metadata::{MetadataFormat, ResolvedMetadata, ValidationPlan},
    setup_self::{self, SetupSelfRequirement},
    setup_self_packages, source_registry,
    state::ShellState,
    steam,
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

    /// Open an indexed source name or external source path.
    Open {
        /// Indexed source name, full source ID, or filesystem path.
        #[arg(value_name = "SOURCE")]
        source: String,
    },

    /// Close the active source while keeping the app shell alive.
    Close,

    /// Print the Steam installation/app root.
    Installation,

    /// Print the directory containing the running Vapor shell binary.
    Binaries,

    /// Print the installation's conventional library directory, when present.
    Libraries,

    /// Show resolved source, installation, setup, and Cargo metadata.
    Metadata {
        /// Output representation for people or automation.
        #[arg(long, value_enum, default_value_t)]
        format: MetadataFormat,
    },

    /// Format Cargo workspaces through app-local Rust/Cargo.
    Fmt {
        /// Cargo workspace name to format, or `all`.
        #[arg(long, value_name = "PROJECT", default_value = "all")]
        project: ProjectSelection,
    },

    /// Check Cargo workspaces through app-local Rust/Cargo.
    Check {
        /// Cargo workspace name to check, or `all`.
        #[arg(long, value_name = "PROJECT", default_value = "all")]
        project: ProjectSelection,
    },

    /// Test Cargo workspaces through app-local Rust/Cargo.
    Test {
        /// Cargo workspace name to test, or `all`.
        #[arg(long, value_name = "PROJECT", default_value = "all")]
        project: ProjectSelection,
    },

    /// Build Cargo workspaces through app-local Rust/Cargo.
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

    /// Inspect or repair Vapor setup domains.
    Setup {
        /// Setup operation.
        #[command(subcommand)]
        command: SetupCommand,
    },

    /// Manage the app-local index of external source roots.
    Sources {
        /// Source index operation.
        #[command(subcommand)]
        command: SourcesCommand,
    },

    /// Build, locate, or open installed documentation.
    Docs {
        /// Documentation operation.
        #[command(subcommand)]
        command: DocsCommand,
    },

    /// Inspect or repair project-local IDE settings.
    Ide {
        /// IDE setup operation.
        #[command(subcommand)]
        command: IdeCommand,
    },

    /// Build, package, or publish the application/depot root.
    Root {
        /// Root application operation.
        #[command(subcommand)]
        command: RootCommand,
    },

    /// Inspect or operate on typed Workshop/content nodes.
    Content {
        /// Content operation.
        #[command(subcommand)]
        command: ContentCommand,
    },

    /// Run a source-controlled sequence of Vapor commands.
    Script {
        /// Script operation.
        #[command(subcommand)]
        command: ScriptCommand,
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

/// App-local source index operations.
#[derive(Debug, Subcommand)]
pub enum SourcesCommand {
    /// List indexed source roots.
    List,
    /// Index a source root without opening it.
    Add {
        /// Source path; defaults to the process working directory.
        #[arg(value_name = "SOURCE_PATH")]
        path: Option<PathBuf>,
    },
    /// Remove an indexed source by local name or full ID.
    Remove {
        /// Indexed source name or full source ID.
        #[arg(value_name = "SOURCE")]
        source: String,
    },
}

/// Project-local IDE setup operations.
#[derive(Debug, Subcommand)]
pub enum IdeCommand {
    /// Report RustRover/JetBrains project-local setup state.
    Status,
    /// Write project-local settings for app-local Rust/Cargo.
    Repair {
        /// Preview IDE files without writing them.
        #[arg(long)]
        dry_run: bool,
    },
}

/// Vapor setup domain operations.
#[derive(Debug, Subcommand)]
pub enum SetupCommand {
    /// Manage this installed Vapor app environment.
    #[command(name = "self")]
    Self_ {
        /// Self-setup operation.
        #[command(subcommand)]
        command: SetupSelfCommand,
    },
}

/// Installed app self-setup lifecycle operations.
#[derive(Debug, Subcommand)]
pub enum SetupSelfCommand {
    /// Report app-root, PATH, app-local tool, and package-payload readiness.
    Status,
    /// Install missing self-setup components inside the app root.
    Install {
        /// Preview registration and file changes without applying them.
        #[arg(long)]
        dry_run: bool,
    },
    /// Remove app-local self-setup components, PATH registration, and location state.
    Uninstall {
        /// Preview removal without deleting files or registration state.
        #[arg(long)]
        dry_run: bool,
    },
    /// Reapply or reacquire self-setup components inside the app root.
    Repair {
        /// Preview registration and repair changes without applying them.
        #[arg(long)]
        dry_run: bool,
    },
    /// Manage distributable self-setup payloads used for app/depot staging.
    Package {
        /// Self-setup package payload operation.
        #[command(subcommand)]
        command: SetupSelfPackageCommand,
    },
}

/// Distributable self-setup package payload operations.
#[derive(Debug, Subcommand)]
pub enum SetupSelfPackageCommand {
    /// Report distributable self-setup payload readiness.
    Status,
    /// Populate missing distributable self-setup payloads from active tools.
    Install {
        /// Preview package writes without changing files.
        #[arg(long)]
        dry_run: bool,
    },
    /// Rebuild distributable self-setup payloads from active tools.
    Repair {
        /// Preview package rebuild without changing files.
        #[arg(long)]
        dry_run: bool,
    },
}

/// Complete application/depot root operations.
#[derive(Debug, Subcommand)]
pub enum RootCommand {
    /// Build every project and promote declared binaries into the installation.
    Build,
    /// Assemble and smoke-check the local application/depot package.
    Package,
    /// Validate, build, stage, preview, or upload the complete Steam app/depot.
    Publish {
        /// Dedicated Steam build account. Required for real uploads.
        #[arg(long)]
        account: Option<String>,
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

/// Typed content-node operations.
#[derive(Debug, Subcommand)]
pub enum ContentCommand {
    /// Report the active Workshop/content node.
    Status,
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
            let target = match path {
                Some(path) => path,
                None => state.active_paths()?.source().root().to_path_buf(),
            };
            print_warnings(state.change_directory(&target)?);
        }
        ShellCommand::Up { levels } => {
            print_warnings(state.move_up(levels.get())?);
        }
        ShellCommand::Pwd => println!("{}", state.current_dir()?.display()),
        ShellCommand::Ls { path } => {
            list_directory(state, path.as_deref().unwrap_or_else(|| Path::new(".")))?;
        }
        ShellCommand::Open { source } => execute_open(&source, state)?,
        ShellCommand::Close => execute_close(state)?,
        ShellCommand::Installation => {
            println!("{}", state.installation().root().display());
        }
        ShellCommand::Binaries => {
            println!("{}", state.installation().binaries().display());
        }
        ShellCommand::Libraries => {
            let target = state.installation().libraries().ok_or_else(|| {
                format!(
                    "Steam installation '{}' has no library directory",
                    state.installation().root().display()
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
        ShellCommand::Setup { command } => execute_setup(command, state)?,
        ShellCommand::Sources { command } => execute_sources(command, state)?,
        ShellCommand::Docs { command } => execute_docs(command, state)?,
        ShellCommand::Ide { command } => execute_ide(command, state)?,
        ShellCommand::Root { command } => execute_root(command, state)?,
        ShellCommand::Content { command } => execute_content(command, state)?,
        ShellCommand::Script { command } => execute_script_command(command, state)?,
        ShellCommand::Exit => return Ok(Control::Exit),
    }

    Ok(Control::Continue)
}

fn execute_open(source: &str, state: &mut ShellState) -> Result<(), String> {
    let path = source_registry::resolve_target(state.installation(), source)?;
    let paths =
        EnvironmentPaths::from_installation_and_invocation(state.installation().clone(), &path)?;
    let (name, entry) = source_registry::add(state.installation(), paths.source())?;
    source_registry::set_active(state.installation(), paths.source())?;
    print_warnings(state.open_paths(paths)?);
    println!("opened source {name}: {}", entry.path().display());
    println!("  id: {}", entry.id());
    println!("hint: inspect it with `metadata`, then run `validate`");
    Ok(())
}

fn execute_close(state: &mut ShellState) -> Result<(), String> {
    source_registry::clear_active(state.installation())?;
    state.close_source();
    println!("closed active source");
    println!("hint: open another source with `open NAME` or inspect sources with `sources list`");
    Ok(())
}

fn execute_sources(command: SourcesCommand, state: &ShellState) -> Result<(), String> {
    match command {
        SourcesCommand::List => {
            let registry = source_registry::load(state.installation())?;
            if registry.sources().is_empty() {
                println!("no indexed Vapor sources");
                println!("hint: add one with `sources add PATH`");
            } else {
                for (name, entry) in registry.sources() {
                    println!("{name}: {}", entry.path().display());
                    println!("  id: {}", entry.id());
                }
            }
        }
        SourcesCommand::Add { path } => {
            let path = path
                .unwrap_or(std::env::current_dir().map_err(|error| {
                    format!("failed to read the invocation directory: {error}")
                })?);
            let paths = EnvironmentPaths::from_installation_and_invocation(
                state.installation().clone(),
                &path,
            )?;
            let (name, entry) = source_registry::add(state.installation(), paths.source())?;
            println!("indexed source {name}: {}", entry.path().display());
            println!("  id: {}", entry.id());
            println!("hint: open it with `open {name}`");
        }
        SourcesCommand::Remove { source } => {
            if let Some(name) = source_registry::remove(state.installation(), &source)? {
                println!("removed indexed source: {name}");
            } else {
                println!("source was not indexed: {source}");
            }
        }
    }
    Ok(())
}

fn execute_ide(command: IdeCommand, state: &ShellState) -> Result<(), String> {
    let metadata = ResolvedMetadata::resolve(state);
    match command {
        IdeCommand::Status => {
            metadata.validate(&ValidationPlan::new("inspect IDE setup").workspace())?;
            let status = ide::inspect(
                state.active_paths()?,
                metadata.workspace_manifest()?,
                metadata.setup_self_status(),
            )?;
            print_ide_status(&status);
            if status.complete() {
                println!("hint: IDE settings are current; next run `vapor validate`");
            } else {
                println!(
                    "hint: preview project-local IDE repair with `vapor ide repair --dry-run`"
                );
            }
        }
        IdeCommand::Repair { dry_run } => {
            metadata.validate(
                &ValidationPlan::new("repair IDE setup")
                    .registered_location()
                    .setup_self(&[SetupSelfRequirement::Rust])
                    .workspace(),
            )?;
            let report = if dry_run {
                ide::preview(
                    state.active_paths()?,
                    metadata.workspace_manifest()?,
                    metadata.setup_self_status(),
                )?
            } else {
                ide::repair(
                    state.active_paths()?,
                    metadata.workspace_manifest()?,
                    metadata.setup_self_status(),
                )?
            };
            print_ide_status(report.status());
            if report.written().is_empty() {
                println!(
                    "{}: IDE settings are already current",
                    if dry_run { "dry-run" } else { "repair" }
                );
            } else {
                for path in report.written() {
                    println!(
                        "{}: {}",
                        if dry_run { "would write" } else { "wrote" },
                        path.display()
                    );
                }
            }
            if dry_run {
                println!("dry-run: no IDE files were changed");
                println!("hint: apply with `vapor ide repair`");
            } else {
                println!("hint: restart or refresh RustRover so project settings are reloaded");
            }
        }
    }
    Ok(())
}

fn print_ide_status(status: &ide::IdeStatus) {
    println!("source root: {}", status.source_root().display());
    println!("IDE directory: {}", status.idea_dir().display());
    println!("Rust/Cargo bin: {}", status.rust_bin().display());
    match status.stdlib_source() {
        Some(path) => println!("Rust standard library source: {}", path.display()),
        None => println!(
            "Rust standard library source: missing\nhint: include rust-src in the app-local Rust package for full IDE indexing"
        ),
    }
    for file in status.files() {
        let state = match file.state() {
            ide::IdeFileState::Missing => "missing",
            ide::IdeFileState::Outdated => "outdated",
            ide::IdeFileState::Current => "current",
        };
        println!("{state}: {}", file.path().display());
    }
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
            .setup_self(&[SetupSelfRequirement::Rust, SetupSelfRequirement::Git])
            .workspace(),
    )?;
    workflow::run(
        state.active_paths()?,
        metadata.workspace_manifest()?,
        project,
        command,
    )?;
    println!("hint: {}", command.next_hint());
    Ok(())
}

fn execute_setup(command: SetupCommand, state: &mut ShellState) -> Result<(), String> {
    match command {
        SetupCommand::Self_ { command } => execute_setup_self(command, state),
    }
}

fn execute_setup_self(command: SetupSelfCommand, state: &mut ShellState) -> Result<(), String> {
    let installation = state.installation();
    match command {
        SetupSelfCommand::Status => {
            let location = setup_self::location_status(installation)?;
            print_location_status(&location);
            let status = setup_self::inspect(installation);
            print_tool_status(status.rust());
            print_tool_status(status.git());
            print_tool_status(status.steamcmd());
            print_package_status(&status);
            if !location.registered() {
                println!("hint: accept this app root explicitly with `vapor setup self install`");
            } else if status.complete() {
                println!(
                    "hint: self-setup is ready; package depot payloads with `vapor setup self package install`"
                );
            } else {
                println!(
                    "hint: install missing self-setup components with `vapor setup self install`"
                );
            }
        }
        SetupSelfCommand::Install { dry_run } => {
            if dry_run {
                preview_setup_self_install(installation, false)?;
                return Ok(());
            }
            let change = setup_self::register_location(installation)?;
            print_location_status(change.status());
            let report = setup_self::install(installation)?;
            state.refresh_cargo_index();
            if report.installed_groups().is_empty() {
                println!("self-setup is already installed; no files changed");
            } else {
                println!("installed: {}", report.installed_groups().join(", "));
            }
            print_path_hint(change.path_setup());
            println!("hint: confirm with `vapor setup self status`, then run `vapor validate`");
        }
        SetupSelfCommand::Repair { dry_run } => {
            if dry_run {
                preview_setup_self_install(installation, true)?;
                return Ok(());
            }
            let change = setup_self::register_location(installation)?;
            print_location_status(change.status());
            let report = setup_self::repair(installation)?;
            state.refresh_cargo_index();
            if report.installed_groups().is_empty() {
                println!("self-setup repair found all components already installed");
            } else {
                println!("repaired: {}", report.installed_groups().join(", "));
            }
            print_path_hint(change.path_setup());
            println!("hint: confirm with `vapor setup self status`, then run `vapor validate`");
        }
        SetupSelfCommand::Uninstall { dry_run } => {
            if dry_run {
                preview_setup_self_uninstall(installation)?;
                return Ok(());
            }
            let report = setup_self::uninstall(installation)?;
            state.refresh_cargo_index();
            print_location_status(report.location().status());
            println!(
                "removed {} app-local tool directories",
                report.removed_paths()
            );
            print_path_hint(report.location().path_setup());
            println!("hint: reinstall later with `vapor setup self install`");
        }
        SetupSelfCommand::Package { command } => match command {
            SetupSelfPackageCommand::Status => {
                let status = setup_self::inspect(installation);
                print_package_status(&status);
                if status.package_complete() {
                    println!("hint: assemble the app package with `vapor root package`");
                } else {
                    println!(
                        "hint: populate self-setup payloads with `vapor setup self package install`"
                    );
                }
            }
            SetupSelfPackageCommand::Install { dry_run } => {
                execute_setup_self_package(false, dry_run, state)?;
            }
            SetupSelfPackageCommand::Repair { dry_run } => {
                execute_setup_self_package(true, dry_run, state)?;
            }
        },
    }
    Ok(())
}

fn preview_setup_self_install(
    installation: &crate::discovery::InstallationPaths,
    repair: bool,
) -> Result<(), String> {
    let location = setup_self::location_status(installation)?;
    let status = setup_self::inspect(installation);
    println!(
        "dry-run: would {} Vapor self-setup",
        if repair { "repair" } else { "install" }
    );
    print_location_status(&location);
    println!("would accept app root: {}", installation.root().display());
    println!(
        "would ensure PATH directory: {}",
        installation.binaries().display()
    );
    print_tool_action(status.rust(), repair);
    print_tool_action(status.git(), repair);
    print_tool_action(status.steamcmd(), repair);
    print_package_status(&status);
    println!(
        "would download Rust through rustup-init into {} and {} when Rust is missing or repair is requested",
        installation.root().join("rustup-home").display(),
        installation.root().join("cargo-home").display()
    );
    println!(
        "would apply app-owned Git from {} when complete self-setup payloads exist",
        status.package_root().display()
    );
    println!("would require tools/git/bin/git to be a real app-owned Git executable");
    println!(
        "would download and extract SteamCMD into {} when SteamCMD is missing or repair is requested",
        installation.root().join("tools/steamcmd").display()
    );
    println!("dry-run: no files, PATH registration, or app-root lock were changed");
    Ok(())
}

fn preview_setup_self_uninstall(
    installation: &crate::discovery::InstallationPaths,
) -> Result<(), String> {
    let location = setup_self::location_status(installation)?;
    let status = setup_self::inspect(installation);
    println!("dry-run: would uninstall Vapor self-setup");
    print_location_status(&location);
    for path in [
        installation.root().join("rustup"),
        installation.root().join("rustup-home"),
        installation.root().join("cargo-home"),
        installation.root().join("tools/git"),
        installation.root().join("tools/steamcmd"),
    ] {
        println!(
            "would remove {}: {}",
            if path.exists() {
                "present path"
            } else {
                "absent path"
            },
            path.display()
        );
    }
    println!("would clear app-root location lock and PATH registration");
    print_tool_status(status.rust());
    print_tool_status(status.git());
    print_tool_status(status.steamcmd());
    print_package_status(&status);
    println!("dry-run: no files, PATH registration, or app-root lock were changed");
    Ok(())
}

fn print_tool_action(status: &setup_self::SetupSelfComponentStatus, repair: bool) {
    let action = if repair {
        "reapply"
    } else if status.installed() {
        "keep"
    } else {
        "install"
    };
    println!("would {action}: {}", status.label());
    println!("  path: {}", status.path().display());
    for missing in status.missing() {
        println!("  missing: {missing}");
    }
}

fn print_location_status(status: &setup_self::LocationStatus) {
    match status {
        setup_self::LocationStatus::Unregistered { current } => {
            println!("app root: unregistered");
            println!("  current:   {}", current.display());
        }
        setup_self::LocationStatus::Registered { path } => {
            println!("app root: registered");
            println!("  path:      {}", path.display());
        }
        setup_self::LocationStatus::Moved { locked, current } => {
            println!("app root: moved (confirmation required)");
            println!("  previous:  {}", locked.display());
            println!("  current:   {}", current.display());
        }
    }
}

fn print_path_hint(report: &crate::path_setup::PathSetupReport) {
    println!("PATH command: {}", report.command().display());
    println!("PATH directory: {}", report.binaries().display());
    for profile in report.profiles() {
        println!("PATH profile: {}", profile.display());
    }
    if report.changed() || !report.path_active() {
        println!("hint: open a new terminal to apply PATH changes");
    }
}

fn print_tool_status(status: &setup_self::SetupSelfComponentStatus) {
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

fn print_package_status(status: &setup_self::SetupSelfStatus) {
    println!(
        "self-setup payload: {}",
        if status.package_complete() {
            "ready"
        } else {
            "missing"
        }
    );
    println!("  path: {}", status.package_root().display());
    for missing in status.missing_package_entries() {
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
                    .setup_self(&[SetupSelfRequirement::Rust])
                    .workspace(),
            )?;
            println!(
                "{}",
                documentation::build(state.active_paths()?, metadata.workspace_manifest()?)?
                    .display()
            );
            println!("hint: open it with `vapor docs open`");
        }
        DocsCommand::Path { topic } => println!(
            "{}",
            documentation::path(state.active_paths()?, topic.as_deref())?.display()
        ),
        DocsCommand::Open { topic } => println!(
            "{}",
            documentation::open(state.active_paths()?, topic.as_deref())?.display()
        ),
    }
    Ok(())
}

fn execute_root(command: RootCommand, state: &ShellState) -> Result<(), String> {
    let metadata = ResolvedMetadata::resolve(state);
    match command {
        RootCommand::Build => {
            metadata.validate(
                &ValidationPlan::new("rebuild the Vapor application")
                    .registered_location()
                    .setup_self(&[SetupSelfRequirement::Rust, SetupSelfRequirement::Git])
                    .workspace(),
            )?;
            workflow::run(
                state.active_paths()?,
                metadata.workspace_manifest()?,
                ProjectSelection::All,
                CargoWorkflow::Build,
            )?;
            let promoted =
                workflow::promote(state.active_paths()?, metadata.workspace_manifest()?)?;
            println!("promoted {promoted} installation binaries");
            println!("hint: assemble the app package with `vapor root package`");
        }
        RootCommand::Package => {
            metadata.validate(
                &ValidationPlan::new("package the Vapor application")
                    .registered_location()
                    .setup_self(&[
                        SetupSelfRequirement::Rust,
                        SetupSelfRequirement::Git,
                        SetupSelfRequirement::SteamCmd,
                    ])
                    .workspace()
                    .distribution(),
            )?;
            setup_self_packages::validate_setup_self_package(state.installation().root())?;
            documentation::build(state.active_paths()?, metadata.workspace_manifest()?)?;
            let report = crate::distribution::stage(
                state.active_paths()?,
                metadata.distribution_manifest()?,
            )?;
            steam::smoke(report.root())?;
            println!(
                "packaged {} files at {}",
                report.files(),
                report.root().display()
            );
            println!("hint: preview Steam upload with `vapor root publish --dry-run`");
        }
        RootCommand::Publish {
            account,
            branch,
            description,
            dry_run,
            yes,
        } => {
            if !dry_run && account.as_deref().is_none_or(str::is_empty) {
                return Err(
                    "real app publication requires --account ACCOUNT after reviewing --dry-run"
                        .to_owned(),
                );
            }
            metadata.validate(
                &ValidationPlan::new("publish the Vapor application")
                    .registered_location()
                    .setup_self(&[
                        SetupSelfRequirement::Rust,
                        SetupSelfRequirement::Git,
                        SetupSelfRequirement::SteamCmd,
                    ])
                    .workspace()
                    .distribution(),
            )?;
            setup_self_packages::validate_setup_self_package(state.installation().root())?;
            workflow::run(
                state.active_paths()?,
                metadata.workspace_manifest()?,
                ProjectSelection::All,
                CargoWorkflow::Validate,
            )?;
            workflow::run(
                state.active_paths()?,
                metadata.workspace_manifest()?,
                ProjectSelection::All,
                CargoWorkflow::Build,
            )?;
            let promoted =
                workflow::promote(state.active_paths()?, metadata.workspace_manifest()?)?;
            println!("promoted {promoted} installation binaries");
            documentation::build(state.active_paths()?, metadata.workspace_manifest()?)?;
            let script = steam::publish(
                state.active_paths()?,
                metadata.distribution_manifest()?,
                account.as_deref().unwrap_or("dry-run"),
                branch.as_deref(),
                &description,
                dry_run,
                yes,
            )?;
            println!("SteamPipe build script: {}", script.display());
            if dry_run {
                println!(
                    "hint: review the staged app, then run `vapor root publish --account ACCOUNT --yes`"
                );
            } else {
                println!("hint: Steam accepted the app upload; verify the target beta branch");
            }
        }
    }
    Ok(())
}

fn execute_content(command: ContentCommand, state: &ShellState) -> Result<(), String> {
    match command {
        ContentCommand::Status => {
            if let Some(content) = state.content() {
                println!("content: {}", content.id());
                println!("  kind: {}", content.kind());
                println!("  root: {}", content.root().display());
            } else {
                println!("content: none");
                println!(
                    "hint: cd into a typed content node, or use metadata to inspect the source root"
                );
            }
        }
    }
    Ok(())
}

fn execute_setup_self_package(
    repair: bool,
    dry_run: bool,
    state: &ShellState,
) -> Result<(), String> {
    let action = if repair {
        "repair self-setup payloads"
    } else {
        "install self-setup payloads"
    };
    let location = setup_self::location_status(state.installation())?;
    setup_self::require_registered_status(&location, action)?;
    let setup_status = setup_self::inspect(state.installation());
    setup_self_packages::validate_active_setup_for_packaging(state.installation().root())?;
    if dry_run {
        println!(
            "dry-run: would {} distributable self-setup payloads",
            if repair { "repair" } else { "install" }
        );
        print_package_status(&setup_status);
        println!(
            "would copy active tools into {}",
            setup_status.package_root().display()
        );
        println!("dry-run: no package files were changed");
    } else {
        let report =
            setup_self_packages::install_setup_self_package(state.installation().root(), repair)?;
        if report.changed() {
            println!(
                "{} self-setup payload at {}",
                if repair { "repaired" } else { "installed" },
                report.status().root().display()
            );
        } else {
            println!(
                "self-setup payload is already installed at {}",
                report.status().root().display()
            );
        }
        println!("hint: assemble the app package with `vapor root package`");
    }
    Ok(())
}

fn execute_script_command(command: ScriptCommand, state: &mut ShellState) -> Result<(), String> {
    let ScriptCommand::Run { name, dry_run } = command;
    if name.contains('/') || name.contains('\\') || name == "." || name == ".." {
        return Err("script name must be a simple filename stem".to_owned());
    }
    let path = state
        .active_paths()?
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
                "scripts may not invoke scripts, exit the host shell, authenticate Steam, perform real publishes, or apply IDE repairs"
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
            | ShellCommand::Root {
                command: RootCommand::Publish { dry_run: false, .. }
            }
            | ShellCommand::Ide {
                command: IdeCommand::Repair { dry_run: false },
            }
    )
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
