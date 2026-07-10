//! Application startup and the interactive read/evaluate loop.

use crate::{
    command::{
        self, ContentCommand, Control, ScriptCommand, SetupCommand, ShellCommand, SourceCommand,
    },
    discovery::EnvironmentPaths,
    metadata::MetadataFormat,
    prompt::VaporPrompt,
    setup_self, source_registry,
    state::ShellState,
    terminal,
};
use clap::{Parser, Subcommand, error::ErrorKind};
use clap_repl::{ClapEditor, ReadCommandOutput};

enum StartupMode {
    Repl,
    Direct(ShellCommand),
    Exit,
}

/// Discover the containing Vapor workspace and run the interactive shell.
///
/// # Errors
///
/// Returns an error before entering the REPL when executable discovery, root
/// manifest validation, or initial state construction fails.
pub fn run() -> Result<(), String> {
    let startup = parse_startup()?;
    if matches!(startup, StartupMode::Exit) {
        return Ok(());
    }

    // Steam and desktop launchers do not normally provide an interactive
    // console. Relaunch before discovery so the child can report configuration
    // errors in the terminal it owns.
    if matches!(startup, StartupMode::Repl) && terminal::needs_relaunch() {
        terminal::relaunch()?;
        return Ok(());
    }

    let installation = EnvironmentPaths::discover_installation()?;
    let mut state = ShellState::closed(installation);
    open_saved_source(&mut state)?;

    if let StartupMode::Direct(command) = startup {
        command::execute(command, &mut state)?;
        return Ok(());
    }

    let setup_status = setup_self::inspect(state.installation());
    match setup_self::location_status(state.installation()) {
        Ok(setup_self::LocationStatus::Registered { .. }) => {}
        Ok(setup_self::LocationStatus::Unregistered { current }) => {
            eprintln!("notice: app root is not registered: {}", current.display());
            eprintln!("hint: review `setup self status`, then choose `setup self install`");
        }
        Ok(setup_self::LocationStatus::Moved { locked, current }) => {
            eprintln!("notice: app root moved and requires explicit confirmation");
            eprintln!("  previous: {}", locked.display());
            eprintln!("  current:   {}", current.display());
            eprintln!("hint: review `setup self status`, then choose `setup self repair`");
        }
        Err(error) => eprintln!("warning: app-root location state is invalid: {error}"),
    }
    if !setup_status.complete() {
        eprintln!("notice: Vapor setup is missing Rust, Git, or SteamCMD readiness");
        eprintln!("hint: inspect it with `setup self status`, then choose `setup self install`");
    }
    if !setup_status.package_complete() {
        eprintln!("notice: distributable self-setup payloads are incomplete");
        eprintln!(
            "hint: inspect them with `setup self package status`, then choose `setup self package install`"
        );
    }

    println!("Vapor shell");
    match state.current_dir() {
        Ok(directory) => println!("Working directory: {}", directory.display()),
        Err(_) => {
            println!("Source: closed");
            println!("Hint: run `source list`, `source add PATH`, or `source open SOURCE`");
        }
    }

    run_shell(state);
    Ok(())
}

fn parse_startup() -> Result<StartupMode, String> {
    if std::env::args_os().len() <= 1 {
        return Ok(StartupMode::Repl);
    }

    match HostCommand::try_parse() {
        Ok(command) => Ok(StartupMode::Direct(command.into_shell_command())),
        Err(error)
            if matches!(
                error.kind(),
                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion
            ) =>
        {
            error
                .print()
                .map_err(|print_error| print_error.to_string())?;
            Ok(StartupMode::Exit)
        }
        Err(host_error) => match ShellCommand::try_parse() {
            Ok(_) => Err(shell_only_error()),
            Err(shell_error)
                if matches!(
                    shell_error.kind(),
                    ErrorKind::DisplayHelp | ErrorKind::DisplayVersion
                ) =>
            {
                shell_error
                    .print()
                    .map_err(|print_error| print_error.to_string())?;
                eprintln!(
                    "note: this is shell command help; run `vapor` to enter the shell or use `vapor script run NAME`"
                );
                Ok(StartupMode::Exit)
            }
            Err(_) => Err(host_error.to_string()),
        },
    }
}

fn shell_only_error() -> String {
    "this command must run inside the interactive Vapor shell\nhelp: run `vapor` to enter the shell, or put repeatable commands in `.vapor/scripts/NAME.vapor` and run `vapor script run NAME`"
        .to_owned()
}

#[derive(Debug, Parser)]
#[command(
    name = "vapor",
    bin_name = "vapor",
    about = "Open the Vapor shell or run a narrow host-level facade",
    after_help = "Run `vapor` with no command to enter the Vapor shell.\nThe shell owns source context, setup state, and command authority.\nSource workflows belong in the shell or in `.vapor/scripts/NAME.vapor`."
)]
struct HostCommand {
    #[command(subcommand)]
    command: HostSubcommand,
}

#[derive(Debug, Subcommand)]
enum HostSubcommand {
    /// Inspect or repair app-local setup.
    Setup {
        #[command(subcommand)]
        command: SetupCommand,
    },
    /// Manage authored source roots.
    Source {
        #[command(subcommand)]
        command: SourceCommand,
    },
    /// Print the Steam installation/app root.
    Installation,
    /// Print the app-local binary directory.
    Binaries,
    /// Print the app-local library directory.
    Libraries,
    /// Report resolved metadata for the remembered/open source.
    Metadata {
        #[arg(long, value_enum, default_value_t)]
        format: MetadataFormat,
    },
    /// Report the remembered/open source's active content node.
    Content {
        #[command(subcommand)]
        command: HostContentCommand,
    },
    /// Run a source-controlled Vapor script.
    Script {
        #[command(subcommand)]
        command: ScriptCommand,
    },
}

#[derive(Debug, Subcommand)]
enum HostContentCommand {
    /// Report the remembered/open source's active content node.
    Status,
    /// List source and installed content without mutating state.
    List,
    /// Verify installed content fingerprints and receipts.
    Verify {
        /// Artifact ID, local name, PublishedFileId, or cached Workshop ID. Omit to verify all.
        #[arg(value_name = "ARTIFACT_OR_WORKSHOP_ID")]
        target: Option<String>,
    },
}

impl HostContentCommand {
    fn into_content_command(self) -> ContentCommand {
        match self {
            Self::Status => ContentCommand::Status,
            Self::List => ContentCommand::List,
            Self::Verify { target } => ContentCommand::Verify { target },
        }
    }
}

impl HostCommand {
    fn into_shell_command(self) -> ShellCommand {
        match self.command {
            HostSubcommand::Setup { command } => ShellCommand::Setup { command },
            HostSubcommand::Source { command } => ShellCommand::Source { command },
            HostSubcommand::Installation => ShellCommand::Installation,
            HostSubcommand::Binaries => ShellCommand::Binaries,
            HostSubcommand::Libraries => ShellCommand::Libraries,
            HostSubcommand::Metadata { format } => ShellCommand::Metadata { format },
            HostSubcommand::Content { command } => ShellCommand::Content {
                command: command.into_content_command(),
            },
            HostSubcommand::Script { command } => ShellCommand::Script { command },
        }
    }
}

fn open_saved_source(state: &mut ShellState) -> Result<(), String> {
    if let Some(source) = source_registry::active_source(state.installation())? {
        match EnvironmentPaths::from_installation_and_invocation(
            state.installation().clone(),
            &source,
        ) {
            Ok(paths) => command::print_warnings(state.open_paths(paths)?),
            Err(error) => {
                eprintln!("warning: active source is invalid: {error}");
                eprintln!(
                    "hint: choose another source with `source open NAME` or clear it with `source close`"
                );
            }
        }
    }
    Ok(())
}

fn run_shell(mut state: ShellState) {
    let mut editor = ClapEditor::<ShellCommand>::builder()
        .with_prompt(prompt_for(&state))
        .build();

    loop {
        editor.set_prompt(prompt_for(&state));

        match editor.read_command() {
            ReadCommandOutput::Command(command) => match command::execute(command, &mut state) {
                Ok(Control::Continue) => {}
                Ok(Control::Exit) => break,
                Err(error) => eprintln!("error: {error}"),
            },
            ReadCommandOutput::EmptyLine | ReadCommandOutput::CtrlC => {}
            ReadCommandOutput::CtrlD => break,
            ReadCommandOutput::ClapError(error) => {
                let _ = error.print();
            }
            ReadCommandOutput::ShlexError => {
                eprintln!("error: input contains invalid or unclosed quoting");
            }
            ReadCommandOutput::ReedlineError(error) => {
                eprintln!("error: terminal input failed: {error}");
                break;
            }
        }
    }
}

fn prompt_for(state: &ShellState) -> Box<VaporPrompt> {
    Box::new(VaporPrompt::new(state.prompt_context()))
}
