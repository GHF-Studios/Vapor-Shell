//! Application startup and the interactive read/evaluate loop.

use crate::{
    command::{
        self, ContentCommand, Control, LaunchCommand, RootCommand, ScriptCommand, SetupCommand,
        ShellCommand, SourceCommand,
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
    Repl { startup_script: Option<String> },
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
    if startup.needs_terminal() && terminal::needs_relaunch() {
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

    let StartupMode::Repl { startup_script } = startup else {
        unreachable!("direct and exit startup modes returned before the REPL")
    };
    if let Some(script) = startup_script {
        print_startup_script_header(&script);
        if let Err(error) = command::run_script(&script, false, &mut state) {
            eprintln!("error: {error}");
        }
        println!();
        println!("The Vapor shell is still open. Use `help` for commands or `exit` to close it.");
        println!();
    } else {
        print_startup_overview(&state);
    }

    run_shell(state);
    Ok(())
}

fn print_startup_script_header(script: &str) {
    println!("Vapor Shell");
    println!();
    println!("Startup script");
    println!("  {script}");
    println!();
}

fn print_startup_overview(state: &ShellState) {
    let installation = state.installation();
    let location = setup_self::location_status(installation);
    let setup_status = setup_self::inspect(installation);

    println!("Vapor Shell");
    println!();
    println!("Status");
    match &location {
        Ok(setup_self::LocationStatus::Registered { .. }) => {
            println!("  Install location: confirmed");
        }
        Ok(setup_self::LocationStatus::Unregistered { .. }) => {
            println!("  Install location: not confirmed yet");
        }
        Ok(setup_self::LocationStatus::Moved { locked, current }) => {
            println!("  Install location: changed");
            println!("    previous: {}", locked.display());
            println!("    current:  {}", current.display());
        }
        Err(error) => {
            println!("  Install location: needs attention ({error})");
        }
    }
    if setup_status.complete() {
        println!("  Local tools: ready");
    } else {
        println!("  Local tools: not installed");
    }
    match state.source() {
        Some(source) => {
            println!("  Source project: {}", source.id());
            println!("    root: {}", source.root().display());
        }
        None => println!("  Source project: none open"),
    }

    println!();
    println!("Next");
    if matches!(&location, Ok(setup_self::LocationStatus::Moved { .. })) {
        println!("  setup self repair");
    } else if location.is_err() {
        println!("  setup self status");
    } else if !matches!(&location, Ok(setup_self::LocationStatus::Registered { .. }))
        || !setup_status.complete()
    {
        println!("  setup self install");
        println!();
        println!("Then");
        if state.source().is_none() {
            println!("  source open /path/to/source");
        } else {
            println!("  validate");
        }
    } else if state.source().is_none() {
        println!("  source open /path/to/source");
    } else {
        println!("  validate");
    }

    println!();
    println!("Use `help` for commands. Use `exit` to close Vapor.");
    println!();
}

fn parse_startup() -> Result<StartupMode, String> {
    if std::env::args_os().len() <= 1 {
        return Ok(StartupMode::Repl {
            startup_script: None,
        });
    }

    match HostCommand::try_parse() {
        Ok(command) => command.into_startup_mode(),
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
    about = "Open Vapor Shell or run a setup/source command",
    after_help = "Run `vapor` with no command to enter the interactive Shell.\nUse `vapor --startup-script NAME` to run an app/source script before the prompt.\nUse setup commands to prepare this Steam install.\nUse source commands to choose the project you want Vapor to work with."
)]
struct HostCommand {
    /// Run `.vapor/scripts/<NAME>.vapor` before entering the interactive shell.
    #[arg(long, value_name = "NAME")]
    startup_script: Option<String>,
    #[command(subcommand)]
    command: Option<HostSubcommand>,
}

#[derive(Debug, Subcommand)]
enum HostSubcommand {
    /// Launch a playable Vapor composition.
    Launch {
        #[command(subcommand)]
        command: LaunchCommand,
    },
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
    /// Run content lifecycle operations against the remembered/open source.
    Content {
        #[command(subcommand)]
        command: ContentCommand,
    },
    /// Run root app/depot operations against the remembered/open source.
    Root {
        #[command(subcommand)]
        command: RootCommand,
    },
    /// Run a source-controlled Vapor script.
    Script {
        #[command(subcommand)]
        command: ScriptCommand,
    },
}

impl HostCommand {
    fn into_startup_mode(self) -> Result<StartupMode, String> {
        match (self.startup_script, self.command) {
            (Some(script), None) => Ok(StartupMode::Repl {
                startup_script: Some(script),
            }),
            (None, Some(command)) => Ok(StartupMode::Direct(command.into_shell_command())),
            (None, None) => Ok(StartupMode::Repl {
                startup_script: None,
            }),
            (Some(_), Some(_)) => Err(
                "--startup-script enters the interactive shell and cannot be combined with a one-shot command"
                    .to_owned(),
            ),
        }
    }
}

impl HostSubcommand {
    fn into_shell_command(self) -> ShellCommand {
        match self {
            HostSubcommand::Setup { command } => ShellCommand::Setup { command },
            HostSubcommand::Launch { command } => ShellCommand::Launch { command },
            HostSubcommand::Source { command } => ShellCommand::Source { command },
            HostSubcommand::Installation => ShellCommand::Installation,
            HostSubcommand::Binaries => ShellCommand::Binaries,
            HostSubcommand::Libraries => ShellCommand::Libraries,
            HostSubcommand::Metadata { format } => ShellCommand::Metadata { format },
            HostSubcommand::Content { command } => ShellCommand::Content { command },
            HostSubcommand::Root { command } => ShellCommand::Root { command },
            HostSubcommand::Script { command } => ShellCommand::Script { command },
        }
    }
}

impl StartupMode {
    fn needs_terminal(&self) -> bool {
        matches!(
            self,
            Self::Repl { .. }
                | Self::Direct(ShellCommand::Launch {
                    command: LaunchCommand::LooCast { .. }
                })
        )
    }
}

fn open_saved_source(state: &mut ShellState) -> Result<(), String> {
    if let Some(source) = source_registry::active_source(state.installation())? {
        match EnvironmentPaths::from_installation_and_source_path(
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
