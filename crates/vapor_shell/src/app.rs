//! Application startup and the interactive read/evaluate loop.

use crate::{
    app_local_tools,
    command::{
        self, ContentCommand, Control, DiagnosticsCommand, LaunchCommand, ProviderCommand,
        RootCommand, ScriptCommand, ShellCommand, SourceCommand,
    },
    diagnostics::{self, CaptureOptions},
    discovery::EnvironmentPaths,
    metadata::MetadataFormat,
    prompt::VaporPrompt,
    source_registry,
    state::ShellState,
};
use clap::{Parser, Subcommand, error::ErrorKind};
use clap_repl::{ClapEditor, ReadCommandOutput};
use std::path::{Path, PathBuf};

enum StartupMode {
    Repl { startup_script: Option<String> },
    Direct(ShellCommand),
    Exit,
}

struct Startup {
    mode: StartupMode,
    diagnostics: CaptureOptions,
}

/// Discover the containing Vapor workspace and run the interactive shell.
///
/// # Errors
///
/// Returns an error before entering the REPL when executable discovery, root
/// manifest validation, or initial state construction fails.
pub fn run() -> Result<(), String> {
    let startup = parse_startup()?;
    if matches!(startup.mode, StartupMode::Exit) {
        return Ok(());
    }

    diagnostics::init_from_current_exe(startup.diagnostics);
    let result = run_inner(startup.mode);
    if let Err(error) = &result {
        diagnostics::event(format!("run error: {error}"));
    }
    diagnostics::finish(result.is_ok());
    result
}

fn run_inner(startup: StartupMode) -> Result<(), String> {
    diagnostics::event(format!("startup mode: {}", startup.diagnostic_label()));

    let installation = EnvironmentPaths::discover_installation()?;
    diagnostics::event(format!(
        "installation: {} ({})",
        installation.root().display(),
        installation.identity_id()
    ));
    let mut state = ShellState::closed(installation);
    open_saved_source(&mut state)?;

    if let StartupMode::Direct(command) = startup {
        diagnostics::event(format!("direct command: {command:?}"));
        command::execute(command, &mut state)?;
        return Ok(());
    }

    let StartupMode::Repl { startup_script } = startup else {
        unreachable!("direct and exit startup modes returned before the REPL")
    };
    if let Some(script) = startup_script {
        print_startup_script_header(&script);
        diagnostics::event(format!("startup script: {script}"));
        if let Err(error) = command::run_script(&script, false, &mut state) {
            diagnostics::event(format!("startup script error: {error}"));
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
    let tool_status = app_local_tools::inspect(installation);

    println!("Vapor Shell");
    println!();
    println!("Status");
    println!("  App root: {}", installation.root().display());
    let runtime_tools_ready = tool_status.steamcmd().installed();
    println!(
        "  Runtime tools: {}",
        if runtime_tools_ready {
            "ready"
        } else {
            "not installed"
        }
    );
    println!(
        "  Development tools: {}",
        if tool_status.complete() {
            "ready"
        } else {
            "not installed"
        }
    );
    if let Some(failure) = std::env::var_os("VAPOR_INSTALLER_INSTALL_FAILED") {
        println!("  Installer: failed");
        println!("    error: {}", failure.to_string_lossy());
        if let Some(log) = std::env::var_os("VAPOR_INSTALLER_LOG") {
            println!("    log: {}", log.to_string_lossy());
        }
        println!(
            "    command: vapor-installer install --app-root {}",
            display_command_argument(installation.root())
        );
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
    if std::env::var_os("VAPOR_INSTALLER_INSTALL_FAILED").is_some() {
        println!("  reinstall the Steam app, or run the installer command shown above");
    } else if !runtime_tools_ready {
        println!(
            "  vapor-installer install --app-root {}",
            display_command_argument(installation.root())
        );
    } else if !tool_status.complete() && state.source().is_some() {
        println!(
            "  vapor-installer dev-env install --app-root {}",
            display_command_argument(installation.root())
        );
    } else if state.source().is_none() {
        println!("  launch loo-cast");
    } else {
        println!("  validate");
    }

    println!();
    println!("Use `help` for commands. Use `exit` to close Vapor.");
    println!();
}

fn display_command_argument(path: &Path) -> String {
    shell_quote(&path.display().to_string())
}

#[cfg(windows)]
fn shell_quote(value: &str) -> String {
    if value.is_empty()
        || value.bytes().any(|byte| {
            matches!(
                byte,
                b' ' | b'\t' | b'\r' | b'\n' | b'(' | b')' | b'&' | b'|' | b'<' | b'>' | b'^'
            )
        })
    {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_owned()
    }
}

#[cfg(not(windows))]
fn shell_quote(value: &str) -> String {
    let safe = !value.is_empty()
        && value
            .bytes()
            .all(|byte| matches!(byte, b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'_' | b'@' | b'%' | b'+' | b'=' | b':' | b',' | b'.' | b'/' | b'-'));
    if safe {
        value.to_owned()
    } else {
        format!("'{}'", value.replace('\'', "'\\''"))
    }
}

fn parse_startup() -> Result<Startup, String> {
    if std::env::args_os().len() <= 1 {
        return Ok(Startup {
            mode: StartupMode::Repl {
                startup_script: None,
            },
            diagnostics: CaptureOptions::disabled(),
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
            Ok(Startup {
                mode: StartupMode::Exit,
                diagnostics: CaptureOptions::disabled(),
            })
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
                Ok(Startup {
                    mode: StartupMode::Exit,
                    diagnostics: CaptureOptions::disabled(),
                })
            }
            Err(_) => Err(host_error.to_string()),
        },
    }
}

fn shell_only_error() -> String {
    "this command must run inside the interactive Vapor shell\nhelp: run `vapor` to enter the shell, or put repeatable commands in `resources/vapor/vapor-scripts/NAME.vapor` and run `vapor script run NAME`"
        .to_owned()
}

#[derive(Debug, Parser)]
#[command(
    name = "vapor",
    bin_name = "vapor",
    about = "Open Vapor Shell or run a launch/source command",
    after_help = "Run `vapor` with no command to enter the interactive Shell.\nUse `vapor --startup-script NAME` to run an app/source script before the prompt.\nUse Vapor Installer for player-mode install/uninstall and development tooling.\nUse source commands to choose the project you want Vapor to work with."
)]
struct HostCommand {
    /// Capture this run and send it through the private diagnostics path on exit.
    #[arg(long, global = true)]
    send_diagnostics: bool,
    /// Registry checkout used by `--send-diagnostics`.
    #[arg(long, global = true, value_name = "PATH")]
    diagnostics_registry: Option<PathBuf>,
    /// Copy diagnostics into the registry without committing or pushing.
    #[arg(long, global = true)]
    diagnostics_copy_only: bool,
    /// Run `resources/vapor/vapor-scripts/<NAME>.vapor` before entering the interactive shell.
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
    /// Inspect or link external developer providers.
    Provider {
        #[command(subcommand)]
        command: ProviderCommand,
    },
    /// Inspect or ship private-test launch diagnostics.
    Diagnostics {
        #[command(subcommand)]
        command: DiagnosticsCommand,
    },
}

impl HostCommand {
    fn into_startup_mode(self) -> Result<Startup, String> {
        let diagnostics = CaptureOptions {
            enabled: self.send_diagnostics,
            submit: self.send_diagnostics,
            push: self.send_diagnostics && !self.diagnostics_copy_only,
            registry: self.diagnostics_registry,
        };
        match (self.startup_script, self.command) {
            (Some(script), None) => Ok(Startup {
                mode: StartupMode::Repl {
                    startup_script: Some(script),
                },
                diagnostics,
            }),
            (None, Some(command)) => Ok(Startup {
                mode: StartupMode::Direct(command.into_shell_command()),
                diagnostics,
            }),
            (None, None) => Ok(Startup {
                mode: StartupMode::Repl {
                    startup_script: None,
                },
                diagnostics,
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
            HostSubcommand::Launch { command } => ShellCommand::Launch { command },
            HostSubcommand::Source { command } => ShellCommand::Source { command },
            HostSubcommand::Installation => ShellCommand::Installation,
            HostSubcommand::Binaries => ShellCommand::Binaries,
            HostSubcommand::Libraries => ShellCommand::Libraries,
            HostSubcommand::Metadata { format } => ShellCommand::Metadata { format },
            HostSubcommand::Content { command } => ShellCommand::Content { command },
            HostSubcommand::Root { command } => ShellCommand::Root { command },
            HostSubcommand::Script { command } => ShellCommand::Script { command },
            HostSubcommand::Provider { command } => ShellCommand::Provider { command },
            HostSubcommand::Diagnostics { command } => ShellCommand::Diagnostics { command },
        }
    }
}

impl StartupMode {
    fn diagnostic_label(&self) -> &'static str {
        match self {
            Self::Repl {
                startup_script: Some(_),
            } => "repl-with-startup-script",
            Self::Repl {
                startup_script: None,
            } => "repl",
            Self::Direct(_) => "direct",
            Self::Exit => "exit",
        }
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
            ReadCommandOutput::Command(command) => {
                let summary = format!("{command:?}");
                match command::execute(command, &mut state) {
                    Ok(Control::Continue) => {
                        diagnostics::event(format!("shell command ok: {summary}"));
                    }
                    Ok(Control::Exit) => {
                        diagnostics::event("shell command requested exit");
                        break;
                    }
                    Err(error) => {
                        diagnostics::event(format!("shell command error: {summary}: {error}"));
                        eprintln!("error: {error}");
                    }
                }
            }
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
