//! Interactive command grammar and execution.
//!
//! Static finite argument domains use Clap enums. Dynamic domains, such as
//! discovered Cargo workspace names, are validated by Vapor metadata.
//!
//! Installation commands report app-root resource paths; they never move
//! the source cursor into the Steam application directory.

use crate::{
    app_local_tools::AppToolRequirement,
    content,
    diagnostics::{self, SubmitOptions},
    discovery::{EnvironmentPaths, ensure_contained},
    distribution::StageOptions,
    documentation, git_provider, ide, manifest,
    metadata::{MetadataFormat, ResolvedMetadata, ValidationPlan},
    source as source_tools, source_registry,
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
    after_help = "Run `help COMMAND` for details on one command group.\nNormal tester launches start with `launch loo-cast`; developer work starts with installer-prepared tooling, then `source open SOURCE`."
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

    /// Inspect or link external developer providers.
    Provider {
        /// Provider operation.
        #[command(subcommand)]
        command: ProviderCommand,
    },

    /// Inspect or ship private-test launch diagnostics.
    Diagnostics {
        /// Diagnostics operation.
        #[command(subcommand)]
        command: DiagnosticsCommand,
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

/// Private-test diagnostics operations.
#[derive(Debug, Subcommand)]
pub enum DiagnosticsCommand {
    /// Report local diagnostics capture and shipping state.
    Status,
    /// Copy diagnostics into a registry checkout; `--push` also commits and pushes.
    #[command(alias = "ship")]
    Submit {
        /// Registry checkout used as the private diagnostics sink.
        #[arg(long, value_name = "PATH")]
        registry: Option<PathBuf>,
        /// Copy every local run log instead of only the current/latest run.
        #[arg(long)]
        all: bool,
        /// Commit and push diagnostics after copying them into the registry.
        #[arg(long)]
        push: bool,
        /// Preview copied files and Git actions without changing the registry.
        #[arg(long)]
        dry_run: bool,
    },
}

/// External developer provider operations.
#[derive(Debug, Subcommand)]
pub enum ProviderCommand {
    /// Inspect or link the developer Git provider.
    Git {
        /// Git provider operation.
        #[command(subcommand)]
        command: GitProviderCommand,
    },
}

/// Developer Git provider operations.
#[derive(Debug, Subcommand)]
pub enum GitProviderCommand {
    /// Report how Git is currently resolved.
    Status,
    /// Persist an explicit Git executable path.
    Link {
        /// Git executable path.
        #[arg(value_name = "PATH")]
        path: PathBuf,
    },
    /// Remove the persisted Git executable path.
    Clear,
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

/// Complete application/depot root operations.
#[derive(Debug, Subcommand)]
pub enum RootCommand {
    /// Build every project and refresh local installation outputs.
    Build {
        /// Skip rebuilding installed documentation.
        #[arg(long)]
        skip_docs: bool,
        /// Rust target triple for dry-run/custom root application binaries. May be repeated.
        #[arg(long, value_name = "TARGET")]
        target: Vec<String>,
        /// Build the manifest runtime target matrix. This is the default when declared.
        #[arg(long)]
        release_targets: bool,
        /// Build only Cargo's host target for a local smoke pass.
        #[arg(long)]
        host_only: bool,
    },
    /// Assemble and smoke-check the local application/depot package.
    Package {
        /// Runtime target triple to stage launch scripts for. May be repeated.
        #[arg(long, value_name = "TARGET")]
        target: Vec<String>,
        /// Stage the manifest runtime target matrix. This is the default when declared.
        #[arg(long)]
        release_targets: bool,
        /// Stage only the host target for a local smoke pass.
        #[arg(long)]
        host_only: bool,
    },
    /// Validate, build, stage, preview, or upload the complete Steam app/depot.
    Publish {
        /// Dedicated Steam build account. Required for real uploads.
        #[arg(long)]
        account: Option<String>,
        /// Existing non-default beta branch; defaults to the distribution manifest.
        #[arg(long)]
        branch: Option<String>,
        /// Rust target triple for dry-run/custom root application binaries. May be repeated.
        #[arg(long, value_name = "TARGET")]
        target: Vec<String>,
        /// Build and stage the manifest runtime target matrix. This is the default when declared.
        #[arg(long)]
        release_targets: bool,
        /// Build and stage only the host target for a dry-run/local smoke pass.
        #[arg(long)]
        host_only: bool,
        /// Use already-promoted app binaries for dry-run previews only.
        #[arg(long)]
        skip_build: bool,
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
        /// Build the manifest runtime target matrix. This is the default when declared.
        #[arg(long)]
        release_targets: bool,
        /// Build only Cargo's host target for a local smoke pass.
        #[arg(long)]
        host_only: bool,
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
        /// Deploy the manifest runtime target matrix. This is the default when declared.
        #[arg(long)]
        release_targets: bool,
        /// Deploy only Cargo's host target for a local smoke pass.
        #[arg(long)]
        host_only: bool,
    },
    /// Stage a content package under the app root.
    Package {
        /// Artifact ID, local name, or PublishedFileId.
        #[arg(value_name = "ARTIFACT")]
        artifact: String,
        /// Rust target triple for dry-run/custom content runtime outputs. May be repeated.
        #[arg(long, value_name = "TARGET")]
        target: Vec<String>,
        /// Package the manifest runtime target matrix. This is the default when declared.
        #[arg(long)]
        release_targets: bool,
        /// Package only the host target for a dry-run/local smoke pass.
        #[arg(long)]
        host_only: bool,
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
        /// Rust target triple for dry-run/custom content runtime outputs. May be repeated.
        #[arg(long, value_name = "TARGET")]
        target: Vec<String>,
        /// Package the manifest runtime target matrix. This is the default when declared.
        #[arg(long)]
        release_targets: bool,
        /// Package only the host target for a dry-run/local smoke pass.
        #[arg(long)]
        host_only: bool,
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
        /// Rust target triple for dry-run/custom content runtime outputs. May be repeated.
        #[arg(long, value_name = "TARGET")]
        target: Vec<String>,
        /// Package the manifest runtime target matrix. This is the default when declared.
        #[arg(long)]
        release_targets: bool,
        /// Package only the host target for a dry-run/local smoke pass.
        #[arg(long)]
        host_only: bool,
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
    /// Run `resources/vapor/vapor-scripts/<NAME>.vapor`.
    Run {
        /// Script filename stem under `resources/vapor/vapor-scripts`.
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
        ShellCommand::Docs { command } => execute_docs(command, state)?,
        ShellCommand::Ide { command } => execute_ide(command, state)?,
        ShellCommand::Root { command } => execute_root(command, state)?,
        ShellCommand::Content { command } => execute_content(command, state)?,
        ShellCommand::Script { command } => execute_script_command(command, state)?,
        ShellCommand::Provider { command } => execute_provider(command, state)?,
        ShellCommand::Diagnostics { command } => execute_diagnostics(command, state)?,
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
    diagnostics::event(format!(
        "launch loo-cast started; account={}",
        account.unwrap_or("<default>")
    ));
    let selection = match content::current_selection(state.installation())? {
        Some(selection) => selection,
        None => ensure_loo_cast_installed_and_selected(state, account)?,
    };

    println!("Play Loo-Cast");
    println!();
    println!("Status");
    println!("  Loo-Cast Packagepack: {}", selection.artifact_id());
    println!("    root: {}", selection.installed_root().display());
    diagnostics::event(format!(
        "launch selection: {} at {}",
        selection.artifact_id(),
        selection.installed_root().display()
    ));

    let reports = content::verify(state.installation(), None)?;
    diagnostics::event(format!("content verify reports: {}", reports.len()));
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
    diagnostics::event(format!(
        "engine handoff binary: {} ({runtime_target})",
        binary.display()
    ));

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
    diagnostics::event(format!("engine exited with {status}"));
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
    println!("  reinstall the Steam app if this is a normal tester install");
    println!("  vapor-installer install --app-root /path/to/steam/app");
    println!("  launch loo-cast");
    println!();
    println!("Note");
    println!(
        "  Steam Play can download the first-party Loo-Cast Packagepack after player-mode tooling is installed."
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
    diagnostics::event("launch loo-cast installing first-party packagepack from Workshop");
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
        diagnostics::event(format!("installed content: {}", report.artifact_id()));
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
    let manifest = packagepack_root.join(manifest::PACKAGEPACK_FILE_NAME);
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
    let manifest = engine_root.join(manifest::ENGINE_FILE_NAME);
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
                metadata.app_local_tools_status(),
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
                    .app_local_tools(&[AppToolRequirement::Rust])
                    .workspace(),
            )?;
            let report = if dry_run {
                ide::preview(
                    state.active_paths()?,
                    metadata.workspace_manifest()?,
                    metadata.app_local_tools_status(),
                )?
            } else {
                ide::repair(
                    state.active_paths()?,
                    metadata.workspace_manifest()?,
                    metadata.app_local_tools_status(),
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
            .app_local_tools(&[AppToolRequirement::Rust])
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
    host_only: bool,
) -> Result<Vec<String>, String> {
    let selected_modes = usize::from(!explicit_targets.is_empty())
        + usize::from(release_targets)
        + usize::from(host_only);
    if selected_modes > 1 {
        return Err("use only one of --target, --release-targets, or --host-only".to_owned());
    }
    if !explicit_targets.is_empty() {
        validate_command_runtime_targets(explicit_targets)?;
        return Ok(explicit_targets.to_vec());
    }
    if host_only {
        return Ok(Vec::new());
    }
    if release_targets && manifest.runtime_targets().is_empty() {
        return Err(
            "source manifest declares no runtime targets; add [root.runtime]/[workspace.runtime] targets or pass --target"
                .to_owned(),
        );
    }
    if !manifest.runtime_targets().is_empty() {
        return Ok(manifest.runtime_targets().to_vec());
    }
    Ok(Vec::new())
}

fn resolve_publish_runtime_targets(
    manifest: &WorkspaceManifest,
    explicit_targets: &[String],
    release_targets: bool,
    host_only: bool,
    dry_run: bool,
    command: &str,
) -> Result<Vec<String>, String> {
    if dry_run {
        return resolve_runtime_targets(manifest, explicit_targets, release_targets, host_only);
    }
    if !explicit_targets.is_empty() {
        return Err(format!(
            "real {command} requires the complete declared runtime target matrix; remove --target or use --dry-run for custom target previews"
        ));
    }
    if host_only {
        return Err(format!(
            "real {command} requires the complete declared runtime target matrix; --host-only is only for dry-run/local smoke paths"
        ));
    }
    let targets = resolve_runtime_targets(manifest, &[], release_targets, false)?;
    require_release_runtime_matrix(command, &targets)?;
    Ok(targets)
}

fn require_release_runtime_matrix(command: &str, targets: &[String]) -> Result<(), String> {
    let has_linux = targets.iter().any(|target| target.contains("linux"));
    let has_windows = targets.iter().any(|target| target.contains("windows"));
    if has_linux && has_windows {
        return Ok(());
    }
    let declared = if targets.is_empty() {
        "<none>".to_owned()
    } else {
        targets.join(", ")
    };
    Err(format!(
        "real {command} requires a declared release runtime matrix containing both Linux and Windows targets; current targets: {declared}"
    ))
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
                "runtime target must be a Rust target triple such as x86_64-pc-windows-gnullvm: {target}"
            ));
        }
        if !seen.insert(target) {
            return Err(format!("duplicate runtime target: {target}"));
        }
    }
    Ok(())
}

fn execute_docs(command: DocsCommand, state: &ShellState) -> Result<(), String> {
    match command {
        DocsCommand::Build => {
            let metadata = ResolvedMetadata::resolve(state);
            metadata.validate(
                &ValidationPlan::new("build documentation")
                    .app_local_tools(&[AppToolRequirement::Rust])
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
            skip_docs,
            target,
            release_targets,
            host_only,
        } => {
            metadata.validate(
                &ValidationPlan::new("build the Vapor application")
                    .app_local_tools(&[AppToolRequirement::Rust])
                    .workspace(),
            )?;
            let paths = state.active_paths()?;
            let workspace_manifest = metadata.workspace_manifest()?;
            let targets =
                resolve_runtime_targets(workspace_manifest, &target, release_targets, host_only)?;
            let report = refresh_root_outputs(paths, workspace_manifest, &targets, skip_docs)?;
            print_root_output_report(&report);
            println!(
                "hint: package the local app with `root package` or preview upload with `root publish --dry-run`"
            );
        }
        RootCommand::Package {
            target,
            release_targets,
            host_only,
        } => {
            metadata.validate(
                &ValidationPlan::new("package the Vapor application")
                    .app_local_tools(&[AppToolRequirement::Rust])
                    .workspace()
                    .distribution(),
            )?;
            let targets = resolve_runtime_targets(
                metadata.workspace_manifest()?,
                &target,
                release_targets,
                host_only,
            )?;
            let stage_options = StageOptions::runtime().with_runtime_targets(targets);
            let report = refresh_root_outputs(
                state.active_paths()?,
                metadata.workspace_manifest()?,
                stage_options.runtime_targets(),
                false,
            )?;
            print_root_output_report(&report);
            let distribution_manifest = metadata.distribution_manifest()?;
            let report = crate::distribution::stage_with_options(
                state.active_paths()?,
                distribution_manifest,
                stage_options.clone(),
            )?;
            steam::smoke(&report, &stage_options)?;
            println!(
                "packaged {} files at {}",
                report.files(),
                report.root().display()
            );
            println!("payload: split runtime depots");
            println!(
                "runtime targets: {}",
                stage_options.runtime_targets().join(", ")
            );
            print_staged_depots(distribution_manifest, &report);
            println!("hint: preview Steam upload with `root publish --dry-run`");
        }
        RootCommand::Publish {
            account,
            branch,
            target,
            release_targets,
            host_only,
            skip_build,
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
            if !dry_run && skip_build {
                return Err(
                    "real app publication must validate, build, and promote the complete runtime matrix; --skip-build is only for dry-run previews"
                        .to_owned(),
                );
            }
            let tool_requirements = if dry_run {
                vec![AppToolRequirement::Rust]
            } else {
                vec![
                    AppToolRequirement::Rust,
                    AppToolRequirement::CrossToolchains,
                    AppToolRequirement::SteamCmd,
                ]
            };
            metadata.validate(
                &ValidationPlan::new("publish the Vapor application")
                    .app_local_tools(&tool_requirements)
                    .workspace()
                    .distribution(),
            )?;
            let targets = resolve_publish_runtime_targets(
                metadata.workspace_manifest()?,
                &target,
                release_targets,
                host_only,
                dry_run,
                "app publication",
            )?;
            let stage_options = StageOptions::runtime().with_runtime_targets(targets.clone());
            if skip_build {
                println!("build: skipped; using already-promoted app binaries");
            } else {
                run_root_workflow_targets(
                    state.active_paths()?,
                    metadata.workspace_manifest()?,
                    CargoWorkflow::Validate,
                    &targets,
                )?;
                run_root_workflow_targets(
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
            }
            documentation::build(state.active_paths()?, metadata.workspace_manifest()?)?;
            let distribution_manifest = metadata.distribution_manifest()?;
            let publish = steam::publish(
                state.active_paths()?,
                distribution_manifest,
                steam::PublishOptions {
                    account: account.as_deref().unwrap_or("dry-run"),
                    branch: branch.as_deref(),
                    description: &description,
                    stage_options: stage_options.clone(),
                    dry_run,
                    confirmed: yes,
                },
            )?;
            println!("payload: split runtime depots");
            println!(
                "runtime targets: {}",
                stage_options.runtime_targets().join(", ")
            );
            print_staged_depots(distribution_manifest, publish.stage());
            println!("SteamPipe build script: {}", publish.script().display());
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

struct RootOutputReport {
    promoted: usize,
    docs: Option<PathBuf>,
    scripts: usize,
    launch_scripts: usize,
}

fn refresh_root_outputs(
    paths: &EnvironmentPaths,
    manifest: &WorkspaceManifest,
    targets: &[String],
    skip_docs: bool,
) -> Result<RootOutputReport, String> {
    run_root_workflow_targets(paths, manifest, CargoWorkflow::Build, targets)?;
    let promoted = workflow::promote_for_targets(paths, manifest, targets)?;
    let docs = if skip_docs {
        None
    } else {
        Some(documentation::build(paths, manifest)?)
    };
    let scripts = sync_root_asset_dir(paths, "resources/vapor/vapor-scripts")?;
    let launch_scripts = sync_root_launch_scripts(paths, targets)?;
    Ok(RootOutputReport {
        promoted,
        docs,
        scripts,
        launch_scripts,
    })
}

fn print_root_output_report(report: &RootOutputReport) {
    println!("promoted {} installation binaries", report.promoted);
    if let Some(docs) = &report.docs {
        println!("docs: {}", docs.display());
    } else {
        println!("docs: skipped");
    }
    println!("scripts: {} file(s)", report.scripts);
    println!("launch scripts: {} file(s)", report.launch_scripts);
}

fn print_staged_depots(
    manifest: &crate::distribution::DistributionManifest,
    report: &crate::distribution::StageReport,
) {
    println!("depots:");
    for depot in report.depots() {
        println!(
            "  {} ({}, {}): {}",
            manifest.application().depot_id(depot.kind()),
            depot.kind().label(),
            depot.kind().steam_os_rule(),
            depot.root().display()
        );
    }
}

fn run_workflow_targets(
    paths: &EnvironmentPaths,
    manifest: &WorkspaceManifest,
    workflow: CargoWorkflow,
    targets: &[String],
) -> Result<(), String> {
    run_selected_workflow_targets(paths, manifest, ProjectSelection::All, workflow, targets)
}

fn run_root_workflow_targets(
    paths: &EnvironmentPaths,
    manifest: &WorkspaceManifest,
    workflow: CargoWorkflow,
    targets: &[String],
) -> Result<(), String> {
    run_selected_workflow_targets(
        paths,
        manifest,
        ProjectSelection::Installable,
        workflow,
        targets,
    )
}

fn run_selected_workflow_targets(
    paths: &EnvironmentPaths,
    manifest: &WorkspaceManifest,
    selection: ProjectSelection,
    workflow: CargoWorkflow,
    targets: &[String],
) -> Result<(), String> {
    if targets.is_empty() {
        workflow::run(paths, manifest, selection, workflow)
    } else {
        for target in targets {
            workflow::run_with_target(paths, manifest, selection.clone(), workflow, Some(target))?;
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

fn sync_root_launch_scripts(paths: &EnvironmentPaths, targets: &[String]) -> Result<usize, String> {
    let source = paths.source().root().join("resources/vapor/shell-scripts");
    let target = paths.installation().root().join("bin");
    ensure_contained(paths.installation().root(), &target)?;

    if !source.exists() {
        return Ok(0);
    }

    let canonical =
        fs::canonicalize(&source).map_err(io("resolve root launch scripts", &source))?;
    ensure_contained(paths.source().root(), &canonical)?;
    let mut files = 0;
    for platform in target_platforms(targets) {
        let (relative_source, target_name) = launch_script_mapping(&platform)?;
        let script_source = canonical.join(relative_source);
        if script_source.is_file() {
            let script_target = target.join(target_name);
            fs::copy(&script_source, &script_target)
                .map_err(io("copy launch script", &script_source))?;
            make_launch_script_executable(&script_target)?;
            files += 1;
        }
    }
    Ok(files)
}

fn launch_script_mapping(platform: &str) -> Result<(&'static str, &'static str), String> {
    match platform {
        "linux" => Ok(("linux/vapor-launch.sh", "vapor-launch.sh")),
        "windows" => Ok(("windows/vapor-launch.cmd", "vapor-launch.cmd")),
        other => Err(format!(
            "no launch script mapping exists for platform '{other}'"
        )),
    }
}

fn make_launch_script_executable(path: &Path) -> Result<(), String> {
    let _ = path;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(path)
            .map_err(|error| format!("failed to inspect '{}': {error}", path.display()))?
            .permissions();
        permissions.set_mode(permissions.mode() | 0o755);
        fs::set_permissions(path, permissions)
            .map_err(|error| format!("failed to make '{}' executable: {error}", path.display()))?;
    }
    Ok(())
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
            host_only,
        } => {
            let metadata = ResolvedMetadata::resolve(state);
            metadata.validate(
                &ValidationPlan::new("build content")
                    .app_local_tools(&[AppToolRequirement::Rust])
                    .workspace(),
            )?;
            let targets = resolve_runtime_targets(
                metadata.workspace_manifest()?,
                &target,
                release_targets,
                host_only,
            )?;
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
            host_only,
        } => {
            let metadata = ResolvedMetadata::resolve(state);
            metadata.validate(
                &ValidationPlan::new("locally deploy content")
                    .app_local_tools(&[AppToolRequirement::Rust])
                    .workspace(),
            )?;
            let targets = resolve_runtime_targets(
                metadata.workspace_manifest()?,
                &target,
                release_targets,
                host_only,
            )?;
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
            host_only,
            dry_run,
        } => {
            let metadata = ResolvedMetadata::resolve(state);
            let targets = resolve_runtime_targets(
                metadata.workspace_manifest()?,
                &target,
                release_targets,
                host_only,
            )?;
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
            let reports = content::download_many(
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
            host_only,
            dry_run,
            yes,
        } => {
            let metadata = ResolvedMetadata::resolve(state);
            if !dry_run {
                metadata.validate(
                    &ValidationPlan::new("create Workshop item")
                        .app_local_tools(&[
                            AppToolRequirement::Rust,
                            AppToolRequirement::CrossToolchains,
                            AppToolRequirement::SteamCmd,
                        ])
                        .workspace(),
                )?;
            }
            let targets = resolve_publish_runtime_targets(
                metadata.workspace_manifest()?,
                &target,
                release_targets,
                host_only,
                dry_run,
                "Workshop item creation",
            )?;
            if !dry_run {
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
            }
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
                    "hint: PublishedFileId was recorded in the source content manifest; verify the Workshop item in Steam"
                );
            }
        }
        ContentCommand::Publish {
            artifacts,
            account,
            target,
            release_targets,
            host_only,
            change_note,
            dry_run,
            yes,
        } => {
            let metadata = ResolvedMetadata::resolve(state);
            if !dry_run {
                metadata.validate(
                    &ValidationPlan::new("publish Workshop items")
                        .app_local_tools(&[
                            AppToolRequirement::Rust,
                            AppToolRequirement::CrossToolchains,
                            AppToolRequirement::SteamCmd,
                        ])
                        .workspace(),
                )?;
            }
            let targets = resolve_publish_runtime_targets(
                metadata.workspace_manifest()?,
                &target,
                release_targets,
                host_only,
                dry_run,
                "Workshop publication",
            )?;
            if !dry_run {
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
            }
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

fn execute_script_command(command: ScriptCommand, state: &mut ShellState) -> Result<(), String> {
    let ScriptCommand::Run { name, dry_run } = command;
    run_script(&name, dry_run, state)
}

fn execute_provider(command: ProviderCommand, state: &ShellState) -> Result<(), String> {
    match command {
        ProviderCommand::Git { command } => execute_git_provider(command, state),
    }
}

fn execute_git_provider(command: GitProviderCommand, state: &ShellState) -> Result<(), String> {
    match command {
        GitProviderCommand::Status => {
            println!("Git Provider");
            println!();
            match git_provider::linked_path(state.installation())? {
                Some(path) => println!("Linked path: {}", path.display()),
                None => println!("Linked path: none"),
            }
            match git_provider::resolve(state.installation()) {
                Ok(provider) => {
                    println!("Status: ready");
                    println!("Path: {}", provider.path().display());
                    println!("Source: {}", provider.source().label());
                }
                Err(error) => {
                    println!("Status: not configured");
                    println!("Error: {error}");
                    println!();
                    println!("Next");
                    println!("  provider git link /path/to/git");
                }
            }
            Ok(())
        }
        GitProviderCommand::Link { path } => {
            let provider = git_provider::link(state.installation(), &path)?;
            println!("Git Provider");
            println!();
            println!("Status: linked");
            println!("Path: {}", provider.path().display());
            Ok(())
        }
        GitProviderCommand::Clear => {
            let removed = git_provider::clear(state.installation())?;
            println!("Git Provider");
            println!();
            println!(
                "Status: {}",
                if removed {
                    "linked path cleared"
                } else {
                    "no linked path was set"
                }
            );
            println!("Next");
            println!("  provider git status");
            Ok(())
        }
    }
}

fn execute_diagnostics(command: DiagnosticsCommand, state: &ShellState) -> Result<(), String> {
    match command {
        DiagnosticsCommand::Status => {
            println!("Diagnostics");
            println!();
            println!("Status");
            println!(
                "  Local logs: {}",
                diagnostics::local_directory(state.installation()).display()
            );
            match diagnostics::current_run_path()
                .or_else(|| diagnostics::latest_run_path(state.installation()))
            {
                Some(path) => println!("  Latest run: {}", path.display()),
                None => println!("  Latest run: none"),
            }
            println!(
                "  Automatic capture: {}",
                if diagnostics::auto_capture_enabled() {
                    "active"
                } else {
                    "off for this process"
                }
            );
            match diagnostics::auto_submit_setting() {
                Some(mode) => println!("  Automatic submit: {mode}"),
                None => println!("  Automatic submit: off"),
            }
            match diagnostics::registry_setting() {
                Some(path) => println!("  Registry target: {}", path.display()),
                None => println!("  Registry target: not set"),
            }
            match git_provider::resolve(state.installation()) {
                Ok(provider) => {
                    println!("  Git provider: {}", provider.path().display());
                    println!("    source: {}", provider.source().label());
                }
                Err(_) => println!("  Git provider: not linked"),
            }
            println!();
            println!("Next");
            println!("  diagnostics submit --registry /path/to/Vapor-Registry");
            println!("  diagnostics submit --registry /path/to/Vapor-Registry --push");
            println!("  provider git link /path/to/git");
        }
        DiagnosticsCommand::Submit {
            registry,
            all,
            push,
            dry_run,
        } => {
            let report = diagnostics::submit(
                state.installation(),
                &SubmitOptions {
                    registry,
                    push,
                    all,
                    dry_run,
                },
            )?;
            println!("Diagnostics Submit");
            println!();
            println!("Status");
            println!("  Registry: {}", report.registry().display());
            println!("  Target: {}", report.target_dir().display());
            println!(
                "  Logs: {}{}",
                report.logs().len(),
                if report.dry_run() { " (dry-run)" } else { "" }
            );
            for log in report.logs() {
                println!("    - {}", log.display());
            }
            if push {
                println!(
                    "  Commit: {}",
                    if report.committed() {
                        "created"
                    } else {
                        "not needed"
                    }
                );
                println!(
                    "  Push: {}",
                    if report.pushed() {
                        "complete"
                    } else {
                        "skipped"
                    }
                );
            } else {
                println!("  Push: skipped");
            }
            if report.dry_run() {
                println!();
                println!("Next");
                println!(
                    "  diagnostics submit --registry {}{}",
                    report.registry().display(),
                    if push { " --push" } else { "" }
                );
            }
        }
    }
    Ok(())
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
        diagnostics::event(format!("script {name}:{}: {line}", index + 1));
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
        diagnostics::event(format!("script {name}:{} complete", index + 1));
    }
    Ok(())
}

fn find_script(state: &ShellState, name: &str) -> Result<PathBuf, String> {
    let filename = format!("{name}.vapor");
    let mut candidates = Vec::new();
    if let Ok(paths) = state.active_paths() {
        candidates.push(
            paths
                .source()
                .root()
                .join("resources/vapor/vapor-scripts")
                .join(&filename),
        );
    }
    candidates.push(
        state
            .installation()
            .root()
            .join("resources/vapor/vapor-scripts")
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
            | ShellCommand::Diagnostics {
                command: DiagnosticsCommand::Submit {
                    push: true,
                    dry_run: false,
                    ..
                },
            }
    )
}

pub(crate) fn print_warnings(warnings: Vec<String>) {
    for warning in warnings {
        eprintln!("warning: {warning}");
    }
}
