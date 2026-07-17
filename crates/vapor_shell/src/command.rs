//! Interactive command grammar and execution.
//!
//! Static finite argument domains use Clap enums. Dynamic domains, such as
//! discovered Cargo workspace names, are validated by Vapor metadata.
//!
//! Installation commands report app-root resource paths; they never move
//! the source cursor into the Steam application directory.

use crate::{
    content,
    discovery::{EnvironmentPaths, ensure_contained},
    distribution::StageOptions,
    documentation, ide,
    metadata::{MetadataFormat, ResolvedMetadata, ValidationPlan},
    setup_self::{self, SetupSelfRequirement},
    setup_self_packages, source as source_tools, source_registry,
    state::ShellState,
    steam,
    workflow::{self, CargoWorkflow, ProjectSelection},
    workspace::{SourceRootKind, WorkspaceManifest},
};
use clap::{Parser, Subcommand, ValueEnum};
use serde::Deserialize;
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

const LOO_CAST_PACKAGEPACK_ID: &str = "ghf-studios/loo-cast/loo-cast-packagepack";

#[derive(Debug, Parser)]
#[command(
    name = "vapor",
    bin_name = "vapor",
    after_help = "Run `help COMMAND` for details on one command group.\nMost project work starts with `setup self status`, then `source open SOURCE`."
)]
/// Commands accepted by the Vapor shell and its narrow host facades.
pub enum ShellCommand {
    /// Launch a playable Vapor composition.
    Launch {
        /// Launch target.
        #[command(subcommand)]
        command: LaunchCommand,
    },

    /// Open, index, inspect, sync, or repair authored source roots.
    Source {
        /// Source operation.
        #[command(subcommand)]
        command: SourceCommand,
    },

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

/// Playable launch targets.
#[derive(Debug, Subcommand)]
pub enum LaunchCommand {
    /// Launch the selected Loo-Cast packagepack composition, installing the first-party one when needed.
    #[command(name = "loo-cast")]
    LooCast {
        /// Steam account used when Workshop content must be downloaded.
        #[arg(long)]
        account: Option<String>,
    },
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

/// Authored source-root operations.
#[derive(Debug, Subcommand)]
pub enum SourceCommand {
    /// Create a new authored source workspace from a Vapor template.
    Init {
        /// Template to create.
        #[arg(value_enum)]
        template: SourceTemplate,
        /// Target workspace path. It must be empty or absent.
        #[arg(value_name = "PATH")]
        path: PathBuf,
        /// Owning organization namespace, such as a Steam publisher or studio slug.
        #[arg(long)]
        organization: String,
        /// Workspace slug.
        #[arg(long)]
        name: String,
        /// Steam AppID used in generated Workshop metadata. Defaults to the installed app.
        #[arg(long)]
        app_id: Option<u32>,
    },
    /// Report active source and indexed source state.
    Status,
    /// Open an indexed source name or external source path.
    Open {
        /// Indexed source name, full source ID, or filesystem path.
        #[arg(value_name = "SOURCE")]
        source: String,
    },
    /// Close the active source while keeping the app shell alive.
    Close,
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
    /// Synchronize authored source through controlled source providers.
    Sync,
    /// Inspect or repair source registration and authored metadata.
    Repair {
        /// Apply safe source metadata repairs instead of only reporting them.
        #[arg(long)]
        write: bool,
    },
}

/// Built-in source workspace templates.
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum SourceTemplate {
    /// Basic engine, game, and packagepack content workspace.
    BasicContent,
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
    Build {
        /// Rust target triple for root application binaries. May be repeated.
        #[arg(long, value_name = "TARGET")]
        target: Vec<String>,
        /// Build the release target matrix declared in `[root.runtime].targets`.
        #[arg(long)]
        release_targets: bool,
    },
    /// Build and locally deploy root binaries and docs into the Steam app root.
    Deploy {
        /// Skip rebuilding installed documentation.
        #[arg(long)]
        skip_docs: bool,
        /// Rust target triple for root application binaries. May be repeated.
        #[arg(long, value_name = "TARGET")]
        target: Vec<String>,
        /// Build the release target matrix declared in `[root.runtime].targets`.
        #[arg(long)]
        release_targets: bool,
    },
    /// Assemble and smoke-check the local application/depot package.
    Package {
        /// Include the large distributable setup/toolchain payload in the staged depot.
        #[arg(long)]
        include_setup_payload: bool,
        /// Runtime target triple to stage launchers for. May be repeated.
        #[arg(long, value_name = "TARGET")]
        target: Vec<String>,
        /// Stage the release target matrix declared in `[root.runtime].targets`.
        #[arg(long)]
        release_targets: bool,
    },
    /// Validate, build, stage, preview, or upload the complete Steam app/depot.
    Publish {
        /// Include the large distributable setup/toolchain payload in the staged depot.
        #[arg(long)]
        include_setup_payload: bool,
        /// Dedicated Steam build account. Required for real uploads.
        #[arg(long)]
        account: Option<String>,
        /// Existing non-default beta branch; defaults to the distribution manifest.
        #[arg(long)]
        branch: Option<String>,
        /// Rust target triple for root application binaries. May be repeated.
        #[arg(long, value_name = "TARGET")]
        target: Vec<String>,
        /// Build and stage the release target matrix declared in `[root.runtime].targets`.
        #[arg(long)]
        release_targets: bool,
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
    /// List source artifacts and installed content state.
    List,
    /// Validate source content metadata, dependencies, conflicts, and publication intent.
    Validate {
        /// Artifact ID, local name, or PublishedFileId. Omit to validate all.
        #[arg(value_name = "ARTIFACT")]
        artifact: Option<String>,
    },
    /// Build the active content workspace through app-local Cargo.
    Build {
        /// Rust target triple for content runtime outputs. May be repeated.
        #[arg(long, value_name = "TARGET")]
        target: Vec<String>,
        /// Build the release target matrix declared in `[workspace.runtime].targets`.
        #[arg(long)]
        release_targets: bool,
    },
    /// Build and locally install source content without Workshop publication.
    Deploy {
        /// Artifact ID, local name, or PublishedFileId.
        #[arg(value_name = "ARTIFACT_OR_WORKSHOP_ID")]
        artifact: String,
        /// Select the deployed artifact as the active packagepack.
        #[arg(long)]
        select: bool,
        /// Rust target triple for content runtime outputs. May be repeated.
        #[arg(long, value_name = "TARGET")]
        target: Vec<String>,
        /// Deploy the release target matrix declared in `[workspace.runtime].targets`.
        #[arg(long)]
        release_targets: bool,
    },
    /// Stage a content package under the app root.
    Package {
        /// Artifact ID, local name, or PublishedFileId.
        #[arg(value_name = "ARTIFACT")]
        artifact: String,
        /// Rust target triple for content runtime outputs. May be repeated for multi-platform packages.
        #[arg(long, value_name = "TARGET")]
        target: Vec<String>,
        /// Package the release target matrix declared in `[workspace.runtime].targets`.
        #[arg(long)]
        release_targets: bool,
        /// Preview package output without writing it.
        #[arg(long)]
        dry_run: bool,
    },
    /// Acquire content into the app-owned cache.
    Acquire {
        /// Artifact ID, local name, PublishedFileId, or cached Workshop ID.
        #[arg(value_name = "ARTIFACT_OR_WORKSHOP_ID")]
        target: String,
        /// Steam publishing/player account for live Workshop provider access.
        #[arg(long)]
        account: Option<String>,
    },
    /// Subscribe to or otherwise acquire content through controlled providers.
    Subscribe {
        /// Artifact ID, local name, PublishedFileId, or cached Workshop ID.
        #[arg(value_name = "ARTIFACT_OR_WORKSHOP_ID")]
        target: String,
        /// Steam account for live Workshop provider access.
        #[arg(long)]
        account: Option<String>,
    },
    /// Download content into the app-owned cache.
    Download {
        /// Artifact IDs, local names, PublishedFileIds, or cached Workshop IDs.
        #[arg(value_name = "ARTIFACT_OR_WORKSHOP_ID", required = true, num_args = 1..)]
        targets: Vec<String>,
        /// Steam account for live Workshop provider access.
        #[arg(long)]
        account: Option<String>,
    },
    /// Install source or cached content into the app root.
    Install {
        /// Artifact ID, local name, PublishedFileId, or cached Workshop ID.
        #[arg(value_name = "ARTIFACT_OR_WORKSHOP_ID")]
        target: String,
        /// Steam account used when Workshop content must be downloaded.
        #[arg(long)]
        account: Option<String>,
    },
    /// Update installed content from source or cache.
    Update {
        /// Artifact ID, local name, PublishedFileId, or cached Workshop ID. Omit to update all.
        #[arg(value_name = "ARTIFACT_OR_WORKSHOP_ID")]
        target: Option<String>,
    },
    /// Verify installed content fingerprints and receipts.
    Verify {
        /// Artifact ID, local name, PublishedFileId, or cached Workshop ID. Omit to verify all.
        #[arg(value_name = "ARTIFACT_OR_WORKSHOP_ID")]
        target: Option<String>,
    },
    /// Report the selected packagepack.
    Selected,
    /// Select an installed packagepack for play.
    Select {
        /// Installed packagepack artifact ID or PublishedFileId.
        #[arg(value_name = "ARTIFACT_OR_WORKSHOP_ID")]
        target: String,
    },
    /// Clear the selected packagepack.
    Deselect,
    /// Repair corrupted installed content from source or cache.
    Repair {
        /// Artifact ID, local name, PublishedFileId, or cached Workshop ID. Omit to repair all.
        #[arg(value_name = "ARTIFACT_OR_WORKSHOP_ID")]
        target: Option<String>,
    },
    /// Disable installed content without deleting it.
    Disable {
        /// Artifact ID, local name, PublishedFileId, or cached Workshop ID.
        #[arg(value_name = "ARTIFACT_OR_WORKSHOP_ID")]
        target: String,
    },
    /// Enable disabled content.
    Enable {
        /// Artifact ID, local name, PublishedFileId, or cached Workshop ID.
        #[arg(value_name = "ARTIFACT_OR_WORKSHOP_ID")]
        target: String,
    },
    /// Uninstall content and remove installed-state records.
    Uninstall {
        /// Artifact ID, local name, PublishedFileId, or cached Workshop ID.
        #[arg(value_name = "ARTIFACT_OR_WORKSHOP_ID")]
        target: String,
    },
    /// Preview creation of a new Workshop item.
    Create {
        /// Artifact ID or local name.
        #[arg(value_name = "ARTIFACT")]
        artifact: String,
        /// Dedicated Steam publishing account. Required for real creation.
        #[arg(long)]
        account: Option<String>,
        /// Rust target triple for content runtime outputs. May be repeated for multi-platform packages.
        #[arg(long, value_name = "TARGET")]
        target: Vec<String>,
        /// Package the release target matrix declared in `[workspace.runtime].targets`.
        #[arg(long)]
        release_targets: bool,
        /// Preview the SteamUGC create request without changing authority.
        #[arg(long)]
        dry_run: bool,
        /// Confirm the real Workshop create/upload.
        #[arg(long)]
        yes: bool,
    },
    /// Publish or preview a Workshop item update.
    Publish {
        /// Artifact IDs or local names.
        #[arg(value_name = "ARTIFACT", required = true, num_args = 1..)]
        artifacts: Vec<String>,
        /// Dedicated Steam publishing account. Required for real uploads.
        #[arg(long)]
        account: Option<String>,
        /// Rust target triple for content runtime outputs. May be repeated for multi-platform packages.
        #[arg(long, value_name = "TARGET")]
        target: Vec<String>,
        /// Package the release target matrix declared in `[workspace.runtime].targets`.
        #[arg(long)]
        release_targets: bool,
        /// Workshop update note.
        #[arg(long)]
        change_note: Option<String>,
        /// Generate package and provider preview without uploading.
        #[arg(long)]
        dry_run: bool,
        /// Confirm the real network upload.
        #[arg(long)]
        yes: bool,
    },
    /// Preview deletion or retirement of a Workshop item.
    Delete {
        /// Artifact ID, local name, or PublishedFileId.
        #[arg(value_name = "ARTIFACT_OR_WORKSHOP_ID")]
        target: String,
        /// Preview the SteamUGC delete request without changing authority.
        #[arg(long)]
        dry_run: bool,
        /// Confirm a real authority-changing delete request.
        #[arg(long)]
        yes: bool,
    },
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
        ShellCommand::Launch { command } => execute_launch(command, state)?,
        ShellCommand::Source { command } => execute_source(command, state)?,
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
        ShellCommand::Docs { command } => execute_docs(command, state)?,
        ShellCommand::Ide { command } => execute_ide(command, state)?,
        ShellCommand::Root { command } => execute_root(command, state)?,
        ShellCommand::Content { command } => execute_content(command, state)?,
        ShellCommand::Script { command } => execute_script_command(command, state)?,
        ShellCommand::Exit => return Ok(Control::Exit),
    }

    Ok(Control::Continue)
}

fn execute_launch(command: LaunchCommand, state: &ShellState) -> Result<(), String> {
    match command {
        LaunchCommand::LooCast { account } => launch_loo_cast(state, account.as_deref()),
    }
}

fn launch_loo_cast(state: &ShellState, account: Option<&str>) -> Result<(), String> {
    let selection = match content::current_selection(state.installation())? {
        Some(selection) => selection,
        None => ensure_loo_cast_installed_and_selected(state, account)?,
    };

    println!("Play Loo-Cast");
    println!();
    println!("Status");
    println!("  Loo-Cast Packagepack: {}", selection.artifact_id());
    println!("    root: {}", selection.installed_root().display());

    let reports = content::verify(state.installation(), None)?;
    if reports.is_empty() {
        println!("  Installed content: none");
        println!();
        print_loo_cast_first_run_next();
        return Ok(());
    }
    let broken = reports
        .iter()
        .filter(|report| !report.ok())
        .collect::<Vec<_>>();
    if !broken.is_empty() {
        println!("  Installed content: needs repair");
        for report in broken {
            println!("    - {}: {}", report.artifact_id(), report.detail());
        }
        println!();
        println!("Next");
        println!("  content repair");
        return Ok(());
    }
    println!("  Installed content: verified ({} item(s))", reports.len());

    let engine_id = selected_packagepack_engine_id(selection.installed_root())?;
    let layout = content::ContentLayout::new(state.installation());
    let engine_root = layout.installed().join(&engine_id);
    let runtime_target = content::host_runtime_target();
    let binary = engine_launch_binary(&engine_root, &runtime_target)?;

    println!("  Spacetime Engine: {engine_id}");
    println!("    runtime target: {runtime_target}");
    println!("    binary: {}", binary.display());
    println!();
    println!("Handoff");
    println!("  Spacetime Engine binary: {}", binary.display());

    let status = Command::new(&binary)
        .current_dir(selection.installed_root())
        .env("VAPOR_LAUNCH_TARGET", "loo-cast")
        .env("VAPOR_PACKAGEPACK_ID", selection.artifact_id())
        .env("VAPOR_PACKAGEPACK_ROOT", selection.installed_root())
        .env("VAPOR_ENGINE_ID", &engine_id)
        .env("VAPOR_ENGINE_ROOT", &engine_root)
        .env("VAPOR_RUNTIME_TARGET", &runtime_target)
        .status()
        .map_err(|error| format!("failed to launch '{}': {error}", binary.display()))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("Spacetime Engine exited with {status}"))
    }
}

fn print_loo_cast_first_run() {
    println!("Play Loo-Cast");
    println!();
    println!("Status");
    println!("  Loo-Cast Packagepack: not installed");
    println!("  Spacetime Engine handoff: unavailable");
    println!();
    print_loo_cast_first_run_next();
}

fn print_loo_cast_first_run_next() {
    println!("Next");
    println!("  open Vapor Shell");
    println!("  setup self install");
    println!("  launch loo-cast");
    println!();
    println!("Note");
    println!(
        "  Steam Play can download the first-party Loo-Cast Packagepack after setup is installed."
    );
}

fn ensure_loo_cast_installed_and_selected(
    state: &ShellState,
    account: Option<&str>,
) -> Result<content::PackagepackSelection, String> {
    if let Ok(selection) =
        content::select_packagepack(state.installation(), LOO_CAST_PACKAGEPACK_ID)
    {
        return Ok(selection);
    }

    println!("Play Loo-Cast");
    println!();
    println!("Status");
    println!("  Loo-Cast Packagepack: not installed");
    println!("  Workshop: downloading first-party Loo-Cast Packagepack and dependencies");
    let reports = match content::install_with_account(
        state.installation(),
        None,
        LOO_CAST_PACKAGEPACK_ID,
        account,
    ) {
        Ok(reports) => reports,
        Err(error) if error.contains("SteamCMD is not installed") => {
            print_loo_cast_first_run();
            return Err(error);
        }
        Err(error) => return Err(error),
    };
    for report in &reports {
        println!("    installed: {}", report.artifact_id());
    }
    let selection = content::select_packagepack(state.installation(), LOO_CAST_PACKAGEPACK_ID)?;
    println!(
        "  Selected Loo-Cast Packagepack: {}",
        selection.artifact_id()
    );
    println!();
    Ok(selection)
}

fn selected_packagepack_engine_id(packagepack_root: &Path) -> Result<String, String> {
    let manifest = packagepack_root.join("Vapor.toml");
    let source = fs::read_to_string(&manifest)
        .map_err(|error| format!("failed to read '{}': {error}", manifest.display()))?;
    let parsed: LaunchManifest = toml::from_str(&source)
        .map_err(|error| format!("failed to parse '{}': {error}", manifest.display()))?;
    let packagepack = parsed.packagepack.ok_or_else(|| {
        format!(
            "selected packagepack manifest '{}' has no [packagepack] section",
            manifest.display()
        )
    })?;
    packagepack
        .dependencies
        .into_iter()
        .find(|dependency| dependency.relationship == "engine")
        .map(|dependency| dependency.id)
        .ok_or_else(|| {
            format!(
                "selected packagepack '{}' has no required engine dependency",
                manifest.display()
            )
        })
}

fn engine_launch_binary(engine_root: &Path, runtime_target: &str) -> Result<PathBuf, String> {
    let manifest = engine_root.join("Vapor.toml");
    let source = fs::read_to_string(&manifest)
        .map_err(|error| format!("failed to read '{}': {error}", manifest.display()))?;
    let parsed: LaunchManifest = toml::from_str(&source)
        .map_err(|error| format!("failed to parse '{}': {error}", manifest.display()))?;
    let engine = parsed.engine.ok_or_else(|| {
        format!(
            "selected packagepack engine manifest '{}' has no [engine] section",
            manifest.display()
        )
    })?;
    let runtime = engine
        .runtime
        .iter()
        .find(|runtime| runtime.target == runtime_target)
        .ok_or_else(|| {
            format!(
                "selected packagepack engine '{}' has no runtime payload for {runtime_target}\nhelp: install content built for this platform",
                manifest.display()
            )
        })?;
    let binary_name = runtime.binaries.first().ok_or_else(|| {
        format!(
            "selected packagepack engine '{}' declares no runtime binary for {runtime_target}",
            manifest.display()
        )
    })?;
    let binary = engine_root
        .join("bin")
        .join(runtime_target)
        .join(binary_name);
    if binary.is_file() {
        Ok(binary)
    } else {
        Err(format!(
            "selected packagepack engine binary is missing: {}\nhelp: deploy or install content with runtime outputs",
            binary.display()
        ))
    }
}

#[derive(Debug, Deserialize)]
struct LaunchManifest {
    packagepack: Option<LaunchContent>,
    engine: Option<LaunchContent>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct LaunchContent {
    #[serde(default)]
    runtime: Vec<LaunchRuntime>,
    #[serde(default)]
    dependencies: Vec<LaunchReference>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct LaunchRuntime {
    target: String,
    #[serde(default)]
    binaries: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct LaunchReference {
    id: String,
    #[serde(default = "default_launch_relationship")]
    relationship: String,
}

fn default_launch_relationship() -> String {
    "dependency".to_owned()
}

fn execute_source(command: SourceCommand, state: &mut ShellState) -> Result<(), String> {
    match command {
        SourceCommand::Init {
            template,
            path,
            organization,
            name,
            app_id,
        } => execute_source_init(template, path, organization, name, app_id, state),
        SourceCommand::Status => execute_source_status(state),
        SourceCommand::Open { source } => execute_source_open(&source, state),
        SourceCommand::Close => execute_source_close(state),
        SourceCommand::List => execute_source_list(state),
        SourceCommand::Add { path } => execute_source_add(state, path),
        SourceCommand::Remove { source } => execute_source_remove(state, &source),
        SourceCommand::Sync => {
            let source = state.active_paths()?.source();
            println!("source: {}", source.identity_id());
            println!("  root: {}", source.root().display());
            println!(
                "sync: not yet applied; controlled Git/provider synchronization will attach here"
            );
            println!("hint: inspect source state with `source status`");
            Ok(())
        }
        SourceCommand::Repair { write } => {
            let registry = source_registry::load(state.installation())?;
            let metadata_repair = state
                .active_paths()
                .ok()
                .and_then(|paths| source_tools::repair_source_metadata(paths, write).ok());
            println!("Source Repair");
            println!();
            println!("Status");
            println!("  Saved sources: {}", registry.sources().len());
            if let Some(source) = state.source() {
                println!("  Source project: {}", source.id());
                println!("    root: {}", source.root().display());
                match WorkspaceManifest::load(state.active_paths()?) {
                    Ok(manifest) => {
                        println!("  Registered Vapor projects: {}", manifest.projects().len());
                        print_registered_projects(&manifest);
                    }
                    Err(error) => {
                        println!("  Workspace manifest: invalid");
                        println!("    {error}");
                    }
                }
                if let Some(report) = &metadata_repair {
                    if report.actions().is_empty() {
                        println!("  Metadata repairs: none");
                    } else {
                        println!(
                            "  Metadata repairs: {} {}",
                            report.actions().len(),
                            if write { "applied" } else { "available" }
                        );
                        for action in report.actions() {
                            println!(
                                "    - [{}] {} -> workshop-id {}",
                                action.section(),
                                action.reference_id(),
                                action.workshop_id()
                            );
                            println!("      manifest: {}", action.manifest().display());
                        }
                    }
                }
            } else {
                println!("  Source project: none open");
            }
            println!();
            println!("Next");
            if state.source().is_some() {
                if metadata_repair
                    .as_ref()
                    .is_some_and(|report| !report.actions().is_empty() && !write)
                {
                    println!("  source repair --write");
                } else {
                    println!("  source status");
                }
            } else if registry.sources().is_empty() {
                println!("  source open /path/to/source");
            } else {
                println!("  source list");
            }
            Ok(())
        }
    }
}

fn execute_source_init(
    template: SourceTemplate,
    path: PathBuf,
    organization: String,
    name: String,
    app_id: Option<u32>,
    state: &mut ShellState,
) -> Result<(), String> {
    let target = if path.is_absolute() {
        path
    } else if let Ok(current) = state.current_dir() {
        current.join(path)
    } else {
        std::env::current_dir()
            .map_err(|error| format!("failed to read invocation directory: {error}"))?
            .join(path)
    };
    let app_id = match app_id {
        Some(app_id) => app_id,
        None => source_tools::installation_app_id(state.installation())?,
    };
    let report = match template {
        SourceTemplate::BasicContent => {
            source_tools::init_basic_content(source_tools::BasicContentInit {
                path: target,
                organization,
                name,
                app_id,
            })?
        }
    };
    let paths = EnvironmentPaths::from_installation_and_source_path(
        state.installation().clone(),
        report.root(),
    )?;
    let (name, entry) = source_registry::add(state.installation(), paths.source())?;
    source_registry::set_active(state.installation(), paths.source())?;
    print_warnings(state.open_paths(paths)?);

    println!("Source Created");
    println!();
    println!("Status");
    println!("  Template: basic-content");
    println!("  Source project: {name}");
    println!("    id: {}", entry.id());
    println!("    root: {}", entry.path().display());
    println!("  Workspace: {}", report.workspace_id());
    println!("  Packagepack: {}", report.packagepack_id());
    println!();
    println!("Next");
    println!("  content validate");
    println!();
    println!("Then");
    println!("  content deploy {} --select", report.packagepack_id());
    Ok(())
}

fn execute_source_status(state: &ShellState) -> Result<(), String> {
    let registry = source_registry::load(state.installation())?;
    println!("Source Status");
    println!();
    println!("Status");
    if let Some(source) = state.source() {
        println!("  Source project: open");
        println!("    id: {}", source.id());
        println!("    root: {}", source.root().display());
        println!("    current: {}", state.current_dir()?.display());
        match WorkspaceManifest::load(state.active_paths()?) {
            Ok(manifest) => {
                println!("  Cargo workspaces: {}", manifest.cargo_projects().len());
                println!("  Registered Vapor projects: {}", manifest.projects().len());
                print_registered_projects(&manifest);
            }
            Err(error) => {
                println!("  Workspace manifest: invalid");
                println!("    {error}");
            }
        }
    } else {
        println!("  Source project: none open");
    }
    println!("  Saved sources: {}", registry.sources().len());
    println!();
    println!("Next");
    if state.source().is_some() {
        println!("  metadata");
    } else if registry.sources().is_empty() {
        println!("  source open /path/to/source");
    } else {
        println!("  source list");
    }
    Ok(())
}

fn print_registered_projects(manifest: &WorkspaceManifest) {
    for project in manifest.projects() {
        println!("    - {} ({})", project.id(), project.kind());
        println!("      path: {}", project.path().display());
    }
}

fn execute_source_open(source: &str, state: &mut ShellState) -> Result<(), String> {
    let path = source_registry::resolve_target(state.installation(), source)?;
    let paths =
        EnvironmentPaths::from_installation_and_source_path(state.installation().clone(), &path)?;
    let (name, entry) = source_registry::add(state.installation(), paths.source())?;
    source_registry::set_active(state.installation(), paths.source())?;
    print_warnings(state.open_paths(paths)?);
    println!("Source Opened");
    println!();
    println!("Status");
    println!("  Source project: {name}");
    println!("    id: {}", entry.id());
    println!("    root: {}", entry.path().display());
    println!();
    println!("Next");
    println!("  metadata");
    println!();
    println!("Then");
    println!("  validate");
    Ok(())
}

fn execute_source_close(state: &mut ShellState) -> Result<(), String> {
    source_registry::clear_active(state.installation())?;
    state.close_source();
    println!("Source Closed");
    println!();
    println!("Next");
    println!("  source list");
    Ok(())
}

fn execute_source_list(state: &ShellState) -> Result<(), String> {
    let registry = source_registry::load(state.installation())?;
    println!("Saved Sources");
    println!();
    if registry.sources().is_empty() {
        println!("Status");
        println!("  Saved sources: none");
        println!();
        println!("Next");
        println!("  source open /path/to/source");
    } else {
        println!("Status");
        for (name, entry) in registry.sources() {
            println!("  {name}");
            println!("    id: {}", entry.id());
            println!("    root: {}", entry.path().display());
        }
        println!();
        println!("Next");
        println!("  source open NAME");
    }
    Ok(())
}

fn execute_source_add(state: &ShellState, path: Option<PathBuf>) -> Result<(), String> {
    let path = path.unwrap_or(
        std::env::current_dir()
            .map_err(|error| format!("failed to read the invocation directory: {error}"))?,
    );
    let paths =
        EnvironmentPaths::from_installation_and_source_path(state.installation().clone(), &path)?;
    let (name, entry) = source_registry::add(state.installation(), paths.source())?;
    println!("Source Saved");
    println!();
    println!("Status");
    println!("  Source project: {name}");
    println!("    id: {}", entry.id());
    println!("    root: {}", entry.path().display());
    println!();
    println!("Next");
    println!("  source open {name}");
    Ok(())
}

fn execute_source_remove(state: &ShellState, source: &str) -> Result<(), String> {
    if let Some(name) = source_registry::remove(state.installation(), source)? {
        println!("removed indexed source: {name}");
    } else {
        println!("source was not indexed: {source}");
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
                println!("hint: IDE settings are current; next run `validate`");
            } else {
                println!("hint: preview project-local IDE repair with `ide repair --dry-run`");
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
                println!("hint: apply with `ide repair`");
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
    let paths = state.active_paths()?;
    let manifest = metadata.workspace_manifest()?;
    workflow::run(paths, manifest, project, command)?;
    print_workflow_complete(command, paths, manifest);
    Ok(())
}

fn print_workflow_complete(
    command: CargoWorkflow,
    paths: &EnvironmentPaths,
    manifest: &WorkspaceManifest,
) {
    println!();
    println!("Workflow Complete");
    println!();
    println!("Status");
    println!("  {}: passed", command.label());
    println!("  Source type: {}", source_type_label(manifest, paths));
    println!();
    println!("Next");
    println!("  {}", workflow_next_command(command, paths, manifest));
}

fn source_type_label(manifest: &WorkspaceManifest, paths: &EnvironmentPaths) -> &'static str {
    match manifest.kind() {
        SourceRootKind::Root => "application root",
        SourceRootKind::Workspace if workspace_has_content(paths) => "content workspace",
        SourceRootKind::Workspace => "workspace",
    }
}

fn workflow_next_command(
    command: CargoWorkflow,
    paths: &EnvironmentPaths,
    manifest: &WorkspaceManifest,
) -> &'static str {
    match (manifest.kind(), workspace_has_content(paths), command) {
        (_, _, CargoWorkflow::Fmt) => "test",
        (_, _, CargoWorkflow::Check) => "test",
        (_, _, CargoWorkflow::Test) => "validate",
        (SourceRootKind::Root, _, CargoWorkflow::Build | CargoWorkflow::Validate) => "root build",
        (SourceRootKind::Workspace, true, CargoWorkflow::Build) => "content package ARTIFACT",
        (SourceRootKind::Workspace, true, CargoWorkflow::Validate) => "content validate",
        (SourceRootKind::Workspace, false, CargoWorkflow::Build) => "validate",
        (SourceRootKind::Workspace, false, CargoWorkflow::Validate) => "build",
    }
}

fn workspace_has_content(paths: &EnvironmentPaths) -> bool {
    content::discover(paths)
        .map(|catalog| !catalog.artifacts().is_empty())
        .unwrap_or(false)
}

fn resolve_runtime_targets(
    manifest: &WorkspaceManifest,
    explicit_targets: &[String],
    release_targets: bool,
) -> Result<Vec<String>, String> {
    if release_targets && !explicit_targets.is_empty() {
        return Err("use either --target or --release-targets, not both".to_owned());
    }
    if release_targets {
        if manifest.runtime_targets().is_empty() {
            return Err(
                "source Vapor.toml declares no runtime targets; add [root.runtime]/[workspace.runtime] targets or pass --target"
                    .to_owned(),
            );
        }
        Ok(manifest.runtime_targets().to_vec())
    } else {
        validate_command_runtime_targets(explicit_targets)?;
        Ok(explicit_targets.to_vec())
    }
}

fn validate_command_runtime_targets(targets: &[String]) -> Result<(), String> {
    let mut seen = BTreeSet::new();
    for target in targets {
        let valid = !target.is_empty()
            && target.chars().all(|character| {
                character.is_ascii_alphanumeric() || matches!(character, '-' | '_')
            });
        if !valid {
            return Err(format!(
                "runtime target must be a Rust target triple such as x86_64-pc-windows-msvc: {target}"
            ));
        }
        if !seen.insert(target) {
            return Err(format!("duplicate runtime target: {target}"));
        }
    }
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
            let status = setup_self::inspect(installation);
            println!("Setup Status");
            println!();
            println!("Status");
            match &location {
                setup_self::LocationStatus::Registered { path } => {
                    println!("  Install location: confirmed");
                    println!("    root: {}", path.display());
                }
                setup_self::LocationStatus::Unregistered { current } => {
                    println!("  Install location: not confirmed yet");
                    println!("    root: {}", current.display());
                }
                setup_self::LocationStatus::Moved { locked, current } => {
                    println!("  Install location: changed");
                    println!("    previous: {}", locked.display());
                    println!("    current:  {}", current.display());
                }
            }
            println!(
                "  Local tools: {}",
                if status.complete() {
                    "ready"
                } else {
                    "not installed"
                }
            );
            print_tool_summary(status.rust());
            print_tool_summary(status.git());
            print_tool_summary(status.steamcmd());
            println!(
                "  Bootstrap payload: {}",
                if status.package_complete() {
                    "packaged"
                } else {
                    "not packaged"
                }
            );
            println!("    only needed when publishing a stacked setup depot");
            println!();
            println!("Next");
            if matches!(location, setup_self::LocationStatus::Moved { .. }) {
                println!("  setup self repair");
            } else if !location.registered() {
                println!("  setup self install");
            } else if status.complete() {
                println!("  source open /path/to/source");
            } else {
                println!("  setup self install");
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
            println!(
                "hint: confirm with `vapor setup self status`; then enter the shell and run `validate`"
            );
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
            println!(
                "hint: confirm with `vapor setup self status`; then enter the shell and run `validate`"
            );
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
                    println!("hint: enter the shell and run `root package`");
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
    println!(
        "would otherwise import a real host Git binary into {} and replace delegating scripts",
        installation.root().join("tools/git").display()
    );
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

fn print_tool_summary(status: &setup_self::SetupSelfComponentStatus) {
    println!(
        "    {}: {}",
        status.label(),
        if status.installed() {
            "ready"
        } else {
            "missing"
        }
    );
    if !status.installed() {
        println!("      missing: {}", status.missing().join(", "));
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
            println!("hint: open it with `docs open`");
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
        RootCommand::Build {
            target,
            release_targets,
        } => {
            metadata.validate(
                &ValidationPlan::new("rebuild the Vapor application")
                    .registered_location()
                    .setup_self(&[SetupSelfRequirement::Rust, SetupSelfRequirement::Git])
                    .workspace(),
            )?;
            let targets =
                resolve_runtime_targets(metadata.workspace_manifest()?, &target, release_targets)?;
            run_workflow_targets(
                state.active_paths()?,
                metadata.workspace_manifest()?,
                CargoWorkflow::Build,
                &targets,
            )?;
            let promoted = workflow::promote_for_targets(
                state.active_paths()?,
                metadata.workspace_manifest()?,
                &targets,
            )?;
            println!("promoted {promoted} installation binaries");
            println!("hint: assemble the app package with `root package`");
        }
        RootCommand::Deploy {
            skip_docs,
            target,
            release_targets,
        } => {
            metadata.validate(
                &ValidationPlan::new("locally deploy the Vapor application")
                    .registered_location()
                    .setup_self(&[SetupSelfRequirement::Rust, SetupSelfRequirement::Git])
                    .workspace(),
            )?;
            let targets =
                resolve_runtime_targets(metadata.workspace_manifest()?, &target, release_targets)?;
            run_workflow_targets(
                state.active_paths()?,
                metadata.workspace_manifest()?,
                CargoWorkflow::Build,
                &targets,
            )?;
            let promoted = workflow::promote_for_targets(
                state.active_paths()?,
                metadata.workspace_manifest()?,
                &targets,
            )?;
            println!("promoted {promoted} installation binaries");
            if skip_docs {
                println!("docs: skipped");
            } else {
                let docs =
                    documentation::build(state.active_paths()?, metadata.workspace_manifest()?)?;
                println!("docs: {}", docs.display());
            }
            let scripts = sync_root_asset_dir(state.active_paths()?, ".vapor/scripts")?;
            let launchers = sync_root_launchers(state.active_paths()?, &targets)?;
            println!("scripts: {scripts} file(s)");
            println!("launchers: {launchers} file(s)");
            println!(
                "hint: package the local app with `root package` or preview upload with `root publish --dry-run`"
            );
        }
        RootCommand::Package {
            include_setup_payload,
            target,
            release_targets,
        } => {
            metadata.validate(
                &ValidationPlan::new("package the Vapor application")
                    .registered_location()
                    .setup_self(&[SetupSelfRequirement::Rust, SetupSelfRequirement::Git])
                    .workspace()
                    .distribution(),
            )?;
            let targets =
                resolve_runtime_targets(metadata.workspace_manifest()?, &target, release_targets)?;
            let stage_options = if include_setup_payload {
                setup_self_packages::validate_setup_self_package(state.installation().root())?;
                StageOptions::with_setup_payload()
            } else {
                StageOptions::runtime()
            }
            .with_runtime_targets(targets);
            documentation::build(state.active_paths()?, metadata.workspace_manifest()?)?;
            let report = crate::distribution::stage_with_options(
                state.active_paths()?,
                metadata.distribution_manifest()?,
                stage_options.clone(),
            )?;
            steam::smoke(report.root(), &stage_options)?;
            println!(
                "packaged {} files at {}",
                report.files(),
                report.root().display()
            );
            println!(
                "payload: {}",
                if stage_options.includes_setup_payload() {
                    "runtime plus setup/toolchain"
                } else {
                    "runtime only"
                }
            );
            println!(
                "runtime targets: {}",
                stage_options.runtime_targets().join(", ")
            );
            println!("hint: preview Steam upload with `root publish --dry-run`");
        }
        RootCommand::Publish {
            include_setup_payload,
            account,
            branch,
            target,
            release_targets,
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
            let setup_requirements = if dry_run {
                vec![SetupSelfRequirement::Rust, SetupSelfRequirement::Git]
            } else {
                vec![
                    SetupSelfRequirement::Rust,
                    SetupSelfRequirement::Git,
                    SetupSelfRequirement::SteamCmd,
                ]
            };
            metadata.validate(
                &ValidationPlan::new("publish the Vapor application")
                    .registered_location()
                    .setup_self(&setup_requirements)
                    .workspace()
                    .distribution(),
            )?;
            let targets =
                resolve_runtime_targets(metadata.workspace_manifest()?, &target, release_targets)?;
            let stage_options = if include_setup_payload {
                setup_self_packages::validate_setup_self_package(state.installation().root())?;
                StageOptions::with_setup_payload()
            } else {
                StageOptions::runtime()
            }
            .with_runtime_targets(targets.clone());
            run_workflow_targets(
                state.active_paths()?,
                metadata.workspace_manifest()?,
                CargoWorkflow::Validate,
                &targets,
            )?;
            run_workflow_targets(
                state.active_paths()?,
                metadata.workspace_manifest()?,
                CargoWorkflow::Build,
                &targets,
            )?;
            let promoted = workflow::promote_for_targets(
                state.active_paths()?,
                metadata.workspace_manifest()?,
                &targets,
            )?;
            println!("promoted {promoted} installation binaries");
            documentation::build(state.active_paths()?, metadata.workspace_manifest()?)?;
            let script = steam::publish(
                state.active_paths()?,
                metadata.distribution_manifest()?,
                steam::PublishOptions {
                    account: account.as_deref().unwrap_or("dry-run"),
                    branch: branch.as_deref(),
                    description: &description,
                    stage_options: stage_options.clone(),
                    dry_run,
                    confirmed: yes,
                },
            )?;
            println!(
                "payload: {}",
                if stage_options.includes_setup_payload() {
                    "runtime plus setup/toolchain"
                } else {
                    "runtime only"
                }
            );
            println!(
                "runtime targets: {}",
                stage_options.runtime_targets().join(", ")
            );
            println!("SteamPipe build script: {}", script.display());
            if dry_run {
                println!(
                    "hint: review the staged app, then run `root publish --account ACCOUNT --yes`"
                );
            } else {
                println!("hint: Steam accepted the app upload; verify the target beta branch");
            }
        }
    }
    Ok(())
}

fn run_workflow_targets(
    paths: &EnvironmentPaths,
    manifest: &WorkspaceManifest,
    workflow: CargoWorkflow,
    targets: &[String],
) -> Result<(), String> {
    if targets.is_empty() {
        workflow::run(paths, manifest, ProjectSelection::All, workflow)
    } else {
        for target in targets {
            workflow::run_with_target(
                paths,
                manifest,
                ProjectSelection::All,
                workflow,
                Some(target),
            )?;
        }
        Ok(())
    }
}

fn io<'a>(action: &'static str, path: &'a Path) -> impl FnOnce(std::io::Error) -> String + 'a {
    move |error| format!("failed to {action} '{}': {error}", path.display())
}

fn sync_root_asset_dir(paths: &EnvironmentPaths, relative: &str) -> Result<usize, String> {
    let source = paths.source().root().join(relative);
    let target = paths.installation().root().join(relative);
    ensure_contained(paths.installation().root(), &target)?;

    if target.exists() {
        fs::remove_dir_all(&target).map_err(io("reset installed scripts", &target))?;
    }
    if !source.exists() {
        return Ok(0);
    }

    let canonical = fs::canonicalize(&source).map_err(io("resolve root scripts", &source))?;
    ensure_contained(paths.source().root(), &canonical)?;
    copy_script_tree(&canonical, &target)
}

fn sync_root_launchers(paths: &EnvironmentPaths, targets: &[String]) -> Result<usize, String> {
    let source = paths.source().root().join(".vapor/launch");
    let target = paths.installation().root().join(".vapor/launch");
    ensure_contained(paths.installation().root(), &target)?;

    if target.exists() {
        fs::remove_dir_all(&target).map_err(io("reset installed launchers", &target))?;
    }
    if !source.exists() {
        return Ok(0);
    }

    let canonical = fs::canonicalize(&source).map_err(io("resolve root launchers", &source))?;
    ensure_contained(paths.source().root(), &canonical)?;
    let mut files = 0;
    for platform in target_platforms(targets) {
        let platform_source = canonical.join(&platform);
        if platform_source.exists() {
            files += copy_script_tree(&platform_source, &target.join(platform))?;
        }
    }
    Ok(files)
}

fn target_platforms(targets: &[String]) -> BTreeSet<String> {
    let mut platforms = BTreeSet::new();
    let selected = if targets.is_empty() {
        vec![workflow::host_runtime_target()]
    } else {
        targets.to_vec()
    };
    for target in selected {
        if target.contains("linux") {
            platforms.insert("linux".to_owned());
        } else if target.contains("windows") {
            platforms.insert("windows".to_owned());
        }
    }
    platforms
}

fn copy_script_tree(source: &Path, target: &Path) -> Result<usize, String> {
    fs::create_dir_all(target).map_err(io("create script directory", target))?;
    let mut files = 0;
    for entry in fs::read_dir(source).map_err(io("read script directory", source))? {
        let entry = entry.map_err(|error| error.to_string())?;
        let source_path = entry.path();
        let target_path = target.join(entry.file_name());
        let file_type = entry.file_type().map_err(|error| error.to_string())?;
        if file_type.is_dir() {
            files += copy_script_tree(&source_path, &target_path)?;
        } else if file_type.is_file() {
            if let Some(parent) = target_path.parent() {
                fs::create_dir_all(parent).map_err(io("create script parent", parent))?;
            }
            fs::copy(&source_path, &target_path).map_err(io("copy script", &source_path))?;
            files += 1;
        }
    }
    Ok(files)
}

fn execute_content(command: ContentCommand, state: &ShellState) -> Result<(), String> {
    match command {
        ContentCommand::Status => {
            let layout = content::ContentLayout::new(state.installation());
            println!("Content Status");
            println!();
            println!("Status");
            if let Some(content) = state.content() {
                println!("  Current content: {}", content.id());
                println!("    kind: {}", content.kind());
                println!("    root: {}", content.root().display());
            } else {
                println!("  Current content: none selected by directory");
            }
            println!("  Installed content root: {}", layout.installed().display());
            println!("  Content cache: {}", layout.cache().display());
            println!("  Content state: {}", layout.state().display());
            println!();
            println!("Next");
            println!("  content list");
        }
        ContentCommand::List => {
            println!("Content Library");
            println!();
            println!("Status");
            let mut source_count = None;
            if let Ok(paths) = state.active_paths() {
                let catalog = content::discover(paths)?;
                source_count = Some(catalog.artifacts().len());
                if catalog.artifacts().is_empty() {
                    println!("  Source content: none");
                } else {
                    println!("  Source content: {} item(s)", catalog.artifacts().len());
                    println!();
                    println!("Source Content");
                    for artifact in catalog.artifacts() {
                        print_artifact(artifact);
                    }
                }
            } else {
                println!("  Source content: unavailable because no source is open");
            }
            let installed = content::installed_index(state.installation())?;
            println!();
            if installed.is_empty() {
                println!("Installed Content");
                println!("  none");
            } else {
                println!("Installed Content");
                for id in installed {
                    println!("  {id}");
                }
            }
            println!();
            println!("Next");
            match source_count {
                Some(count) if count > 0 => println!("  content validate"),
                Some(_) => println!("  source open /path/to/content-workspace"),
                None => println!("  source open /path/to/content-workspace"),
            }
        }
        ContentCommand::Validate { artifact } => {
            let report = content::validate(state.active_paths()?, artifact.as_deref())?;
            println!("Content Validate");
            println!();
            println!("Status");
            println!("  Checked content: {} item(s)", report.checked().len());
            for id in report.checked() {
                println!("    - {id}");
            }
            println!();
            println!("Diagnostics");
            if report.diagnostics().is_empty() {
                println!("  none");
            } else {
                for diagnostic in report.diagnostics() {
                    println!("  - {diagnostic}");
                }
            }
            println!();
            println!("Next");
            println!("  content package ARTIFACT");
        }
        ContentCommand::Build {
            target,
            release_targets,
        } => {
            let metadata = ResolvedMetadata::resolve(state);
            metadata.validate(
                &ValidationPlan::new("build content")
                    .registered_location()
                    .setup_self(&[SetupSelfRequirement::Rust, SetupSelfRequirement::Git])
                    .workspace(),
            )?;
            let targets =
                resolve_runtime_targets(metadata.workspace_manifest()?, &target, release_targets)?;
            run_workflow_targets(
                state.active_paths()?,
                metadata.workspace_manifest()?,
                CargoWorkflow::Build,
                &targets,
            )?;
            println!("hint: stage content with `content package ARTIFACT`");
        }
        ContentCommand::Deploy {
            artifact,
            select,
            target,
            release_targets,
        } => {
            let metadata = ResolvedMetadata::resolve(state);
            metadata.validate(
                &ValidationPlan::new("locally deploy content")
                    .registered_location()
                    .setup_self(&[SetupSelfRequirement::Rust, SetupSelfRequirement::Git])
                    .workspace(),
            )?;
            let targets =
                resolve_runtime_targets(metadata.workspace_manifest()?, &target, release_targets)?;
            run_workflow_targets(
                state.active_paths()?,
                metadata.workspace_manifest()?,
                CargoWorkflow::Build,
                &targets,
            )?;
            let reports = content::install_for_targets(
                state.installation(),
                Some(state.active_paths()?),
                &artifact,
                &targets,
            )?;
            for report in &reports {
                println!("deployed: {}", report.artifact_id());
                println!("  installed: {}", report.installed_root().display());
                print_fingerprint(report.fingerprint());
                println!("  receipt: {}", report.receipt().display());
            }
            if select {
                let selection = select_deployed_packagepack(state, &artifact, &reports)?;
                println!("selected packagepack: {}", selection.artifact_id());
                println!("  installed: {}", selection.installed_root().display());
                print_fingerprint(selection.fingerprint());
            } else {
                println!(
                    "hint: select a packagepack with `content select ARTIFACT_OR_WORKSHOP_ID`"
                );
            }
        }
        ContentCommand::Package {
            artifact,
            target,
            release_targets,
            dry_run,
        } => {
            let metadata = ResolvedMetadata::resolve(state);
            let targets =
                resolve_runtime_targets(metadata.workspace_manifest()?, &target, release_targets)?;
            let report =
                content::package_for_targets(state.active_paths()?, &artifact, dry_run, &targets)?;
            print_package_report(&report);
            if dry_run {
                println!("dry-run: no package files were changed");
            } else {
                println!(
                    "hint: cache it with `content acquire {}` or preview Workshop upload with `content publish {} --dry-run`",
                    report.artifact_id(),
                    report.artifact_id()
                );
            }
        }
        ContentCommand::Acquire { target, account } => {
            let report = content::acquire(
                state.installation(),
                state.active_paths().ok(),
                &target,
                account.as_deref(),
            )?;
            println!("acquired: {}", report.artifact_id());
            println!("  cache: {}", report.cache_root().display());
            print_fingerprint(report.fingerprint());
            println!("  receipt: {}", report.receipt().display());
            println!(
                "hint: install it with `content install {}`",
                report.artifact_id()
            );
        }
        ContentCommand::Subscribe { target, account } => {
            let report = content::acquire(
                state.installation(),
                state.active_paths().ok(),
                &target,
                account.as_deref(),
            )?;
            println!("subscribed/acquired: {}", report.artifact_id());
            println!("  cache: {}", report.cache_root().display());
            print_fingerprint(report.fingerprint());
            println!("  receipt: {}", report.receipt().display());
            println!(
                "hint: install it with `content install {}`",
                report.artifact_id()
            );
        }
        ContentCommand::Download { targets, account } => {
            let reports = content::acquire_many(
                state.installation(),
                state.active_paths().ok(),
                &targets,
                account.as_deref(),
            )?;
            for report in &reports {
                println!("downloaded/cached: {}", report.artifact_id());
                println!("  cache: {}", report.cache_root().display());
                print_fingerprint(report.fingerprint());
                println!("  receipt: {}", report.receipt().display());
            }
            if reports.len() == 1 {
                println!(
                    "hint: install it with `content install {}`",
                    reports[0].artifact_id()
                );
            } else {
                println!(
                    "hint: install a packagepack with `content install ARTIFACT_OR_WORKSHOP_ID`"
                );
            }
        }
        ContentCommand::Install { target, account } => {
            let reports = content::install_with_account(
                state.installation(),
                state.active_paths().ok(),
                &target,
                account.as_deref(),
            )?;
            print_install_reports(&reports);
            println!("hint: verify installed content with `content verify {target}`");
        }
        ContentCommand::Update { target } => {
            let reports = content::update(
                state.installation(),
                state.active_paths().ok(),
                target.as_deref(),
            )?;
            print_install_reports(&reports);
            println!("hint: verify updated content with `content verify`");
        }
        ContentCommand::Verify { target } => {
            let reports = content::verify(state.installation(), target.as_deref())?;
            if reports.is_empty() {
                println!("Content Verify");
                println!();
                println!("Status");
                println!("  Installed content: none");
                println!();
                println!("Next");
                println!("  content list");
                return Ok(());
            }
            println!("Content Verify");
            println!();
            println!("Status");
            for report in &reports {
                println!(
                    "  {}: {}",
                    report.artifact_id(),
                    if report.ok() { "ok" } else { "corrupt" }
                );
                println!("    detail: {}", report.detail());
                if let Some(expected) = report.expected() {
                    println!("    expected: {}", expected.hash());
                }
                if let Some(observed) = report.observed() {
                    println!("    observed: {}", observed.hash());
                }
            }
            if reports.iter().any(|report| !report.ok()) {
                println!();
                println!("Next");
                println!("  content repair");
            }
        }
        ContentCommand::Selected => match content::current_selection(state.installation())? {
            Some(selection) => {
                println!("selected packagepack: {}", selection.artifact_id());
                println!("  root: {}", selection.installed_root().display());
                print_fingerprint(selection.fingerprint());
            }
            None => {
                println!("selected packagepack: none");
                println!("hint: select one with `content select ARTIFACT`");
            }
        },
        ContentCommand::Select { target } => {
            let selection = content::select_packagepack(state.installation(), &target)?;
            println!("selected packagepack: {}", selection.artifact_id());
            println!("  root: {}", selection.installed_root().display());
            print_fingerprint(selection.fingerprint());
        }
        ContentCommand::Deselect => {
            content::clear_selection(state.installation())?;
            println!("selected packagepack: none");
        }
        ContentCommand::Repair { target } => {
            let reports = content::repair(
                state.installation(),
                state.active_paths().ok(),
                target.as_deref(),
            )?;
            if reports.is_empty() {
                println!("content repair: all checked content already verified");
            } else {
                print_install_reports(&reports);
            }
        }
        ContentCommand::Disable { target } => {
            let report = content::disable(state.installation(), &target)?;
            println!("disabled: {}", report.artifact_id());
            println!("  retained at: {}", report.installed_root().display());
            println!("  receipt: {}", report.receipt().display());
        }
        ContentCommand::Enable { target } => {
            let report = content::enable(state.installation(), &target)?;
            println!("enabled: {}", report.artifact_id());
            println!("  installed at: {}", report.installed_root().display());
            println!("  receipt: {}", report.receipt().display());
        }
        ContentCommand::Uninstall { target } => {
            let report = content::uninstall(state.installation(), &target)?;
            println!("uninstalled: {}", report.artifact_id());
            println!("  removed artifact root: {}", report.removed());
            println!("  receipt: {}", report.receipt().display());
        }
        ContentCommand::Create {
            artifact,
            account,
            target,
            release_targets,
            dry_run,
            yes,
        } => {
            let metadata = ResolvedMetadata::resolve(state);
            let targets =
                resolve_runtime_targets(metadata.workspace_manifest()?, &target, release_targets)?;
            let report = content::create_workshop_item_for_targets(
                state.active_paths()?,
                &artifact,
                account.as_deref(),
                &targets,
                dry_run,
                yes,
            )?;
            println!("Workshop create: {}", report.artifact_id());
            if let Some(script) = report.script() {
                println!("  provider script: {}", script.display());
            }
            if let Some(published_file_id) = report.published_file_id() {
                println!("  published-file-id: {published_file_id}");
            }
            println!("  receipt: {}", report.receipt().display());
            if dry_run {
                println!("dry-run: no Workshop item was created");
                println!(
                    "hint: review the package and provider script, then run `content create {} --account ACCOUNT --yes` manually in the shell",
                    report.artifact_id()
                );
            } else if report.uploaded() {
                println!(
                    "hint: PublishedFileId was recorded in the source Vapor.toml; verify the Workshop item in Steam"
                );
            }
        }
        ContentCommand::Publish {
            artifacts,
            account,
            target,
            release_targets,
            change_note,
            dry_run,
            yes,
        } => {
            let metadata = ResolvedMetadata::resolve(state);
            let targets =
                resolve_runtime_targets(metadata.workspace_manifest()?, &target, release_targets)?;
            let reports = content::publish_workshop_items_for_targets(
                state.active_paths()?,
                &artifacts,
                account.as_deref(),
                &targets,
                change_note.as_deref(),
                dry_run,
                yes,
            )?;
            for report in &reports {
                println!("Workshop publish: {}", report.artifact_id());
                if let Some(script) = report.script() {
                    println!("  provider script: {}", script.display());
                }
                println!("  receipt: {}", report.receipt().display());
            }
            if dry_run {
                println!("dry-run: no Workshop upload was performed");
                println!(
                    "hint: review the package and provider scripts, then run `content publish ARTIFACT... --account ACCOUNT --yes` manually in the shell"
                );
            } else if reports.iter().any(|report| report.uploaded()) {
                println!(
                    "hint: verify the Workshop item in Steam and run `content acquire`/`content install` for a local roundtrip"
                );
            }
        }
        ContentCommand::Delete {
            target,
            dry_run,
            yes,
        } => {
            let report =
                content::delete_workshop_item(state.installation(), &target, dry_run, yes)?;
            println!("Workshop delete: {}", report.artifact_id());
            println!("  receipt: {}", report.receipt().display());
            if dry_run {
                println!("dry-run: no Workshop item was deleted");
            }
        }
    }
    Ok(())
}

fn select_deployed_packagepack(
    state: &ShellState,
    target: &str,
    reports: &[content::InstallReport],
) -> Result<content::PackagepackSelection, String> {
    match content::select_packagepack(state.installation(), target) {
        Ok(selection) => Ok(selection),
        Err(target_error) => {
            for report in reports {
                if let Ok(selection) =
                    content::select_packagepack(state.installation(), report.artifact_id())
                {
                    return Ok(selection);
                }
            }
            Err(target_error)
        }
    }
}

fn print_artifact(artifact: &content::ContentArtifact) {
    println!("  {} ({})", artifact.id(), artifact.kind());
    println!("    root: {}", artifact.root().display());
    if let Some(version) = artifact.version() {
        println!("    version: {version}");
    }
    if let Some(workshop_id) = artifact.workshop().published_file_id() {
        println!("    workshop: {workshop_id}");
    }
    for dependency in artifact.dependencies() {
        println!(
            "    {}: {}{}",
            dependency.relationship(),
            dependency.id(),
            if dependency.optional() {
                " (optional)"
            } else {
                ""
            }
        );
    }
    for conflict in artifact.conflicts() {
        println!("    conflict: {}", conflict.id());
    }
}

fn print_package_report(report: &content::PackageReport) {
    println!(
        "{}: {}",
        if report.dry_run() {
            "would package"
        } else {
            "packaged"
        },
        report.artifact_id()
    );
    println!("  artifact root: {}", report.root().display());
    println!("  runtime target: {}", report.runtime_target());
    print_fingerprint(report.fingerprint());
    if let Some(receipt) = report.receipt() {
        println!("  receipt: {}", receipt.display());
    }
}

fn print_install_reports(reports: &[content::InstallReport]) {
    for report in reports {
        println!("installed: {}", report.artifact_id());
        println!("  root: {}", report.installed_root().display());
        print_fingerprint(report.fingerprint());
        println!("  receipt: {}", report.receipt().display());
    }
}

fn print_fingerprint(fingerprint: &content::Fingerprint) {
    println!(
        "  fingerprint: {} {} ({} files, {} bytes)",
        fingerprint.algorithm(),
        fingerprint.hash(),
        fingerprint.files(),
        fingerprint.bytes()
    );
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
        println!("hint: enter the shell and run `root package`");
    }
    Ok(())
}

fn execute_script_command(command: ScriptCommand, state: &mut ShellState) -> Result<(), String> {
    let ScriptCommand::Run { name, dry_run } = command;
    run_script(&name, dry_run, state)
}

pub(crate) fn run_script(name: &str, dry_run: bool, state: &mut ShellState) -> Result<(), String> {
    if name.contains('/') || name.contains('\\') || name == "." || name == ".." {
        return Err("script name must be a simple filename stem".to_owned());
    }
    let path = find_script(state, name)?;
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
                "scripts may not invoke scripts, exit the host shell, perform real publishes, delete Workshop items, or apply IDE repairs"
                    .to_owned(),
            );
        }
        execute(parsed, state)?;
    }
    Ok(())
}

fn find_script(state: &ShellState, name: &str) -> Result<PathBuf, String> {
    let filename = format!("{name}.vapor");
    let mut candidates = Vec::new();
    if let Ok(paths) = state.active_paths() {
        candidates.push(paths.source().root().join(".vapor/scripts").join(&filename));
    }
    candidates.push(
        state
            .installation()
            .root()
            .join(".vapor/scripts")
            .join(filename),
    );

    for path in &candidates {
        if path.is_file() {
            return Ok(path.clone());
        }
    }

    let searched = candidates
        .iter()
        .map(|path| format!("  - {}", path.display()))
        .collect::<Vec<_>>()
        .join("\n");
    Err(format!("script not found: {name}\nsearched:\n{searched}"))
}

fn script_command_allowed(command: &ShellCommand) -> bool {
    !matches!(
        command,
        ShellCommand::Script { .. }
            | ShellCommand::Exit
            | ShellCommand::Root {
                command: RootCommand::Publish { dry_run: false, .. }
            }
            | ShellCommand::Content {
                command: ContentCommand::Create { dry_run: false, .. },
            }
            | ShellCommand::Content {
                command: ContentCommand::Publish { dry_run: false, .. },
            }
            | ShellCommand::Content {
                command: ContentCommand::Delete { dry_run: false, .. },
            }
            | ShellCommand::Ide {
                command: IdeCommand::Repair { dry_run: false },
            }
    )
}

pub(crate) fn print_warnings(warnings: Vec<String>) {
    for warning in warnings {
        eprintln!("warning: {warning}");
    }
}
