//! Application startup and the interactive read/evaluate loop.

use crate::{
    command::{self, Control, ShellCommand},
    discovery::EnvironmentPaths,
    prompt::VaporPrompt,
    source_registry,
    state::ShellState,
    terminal, toolchain,
};
use clap::{Parser, error::ErrorKind};
use clap_repl::{ClapEditor, ReadCommandOutput};

/// Discover the containing Vapor workspace and run the interactive shell.
///
/// # Errors
///
/// Returns an error before entering the REPL when executable discovery, root
/// manifest validation, or initial state construction fails.
pub fn run() -> Result<(), String> {
    let one_shot = if std::env::args_os().len() > 1 {
        match ShellCommand::try_parse() {
            Ok(command) => Some(command),
            Err(error)
                if matches!(
                    error.kind(),
                    ErrorKind::DisplayHelp | ErrorKind::DisplayVersion
                ) =>
            {
                error
                    .print()
                    .map_err(|print_error| print_error.to_string())?;
                return Ok(());
            }
            Err(error) => return Err(error.to_string()),
        }
    } else {
        None
    };

    // Steam and desktop launchers do not normally provide an interactive
    // console. Relaunch before discovery so the child can report configuration
    // errors in the terminal it owns.
    if one_shot.is_none() && terminal::needs_relaunch() {
        terminal::relaunch()?;
        return Ok(());
    }

    let installation = EnvironmentPaths::discover_installation()?;
    let mut state = ShellState::closed(installation);
    if let Some(source) = source_registry::active_source(state.installation())? {
        match EnvironmentPaths::from_installation_and_invocation(
            state.installation().clone(),
            &source,
        ) {
            Ok(paths) => command::print_warnings(state.open_paths(paths)?),
            Err(error) => {
                eprintln!("warning: active source is invalid: {error}");
                eprintln!("hint: choose another source with `open NAME` or clear it with `close`");
            }
        }
    }

    if let Some(command) = one_shot {
        if !one_shot_allowed(&command) {
            return Err(
                "this command must run inside the interactive Vapor shell\nhelp: allowed direct facades are `vapor script run NAME`, `vapor setup ...`, `vapor content status`, `vapor sources ...`, `vapor open SOURCE`, `vapor close`, and read-only app inspection"
                    .to_owned(),
            );
        }
        command::execute(command, &mut state)?;
        return Ok(());
    }

    let toolchain = toolchain::inspect(state.installation());
    match toolchain::location_status(state.installation()) {
        Ok(toolchain::LocationStatus::Registered { .. }) => {}
        Ok(toolchain::LocationStatus::Unregistered { current }) => {
            eprintln!("notice: app root is not registered: {}", current.display());
            eprintln!("hint: review `setup status`, then choose `setup install`");
        }
        Ok(toolchain::LocationStatus::Moved { locked, current }) => {
            eprintln!("notice: app root moved and requires explicit confirmation");
            eprintln!("  previous: {}", locked.display());
            eprintln!("  current:   {}", current.display());
            eprintln!("hint: review `setup status`, then choose `setup repair`");
        }
        Err(error) => eprintln!("warning: app-root location state is invalid: {error}"),
    }
    if !toolchain.complete() {
        eprintln!("notice: Vapor setup is missing Rust, Git, or SteamCMD readiness");
        eprintln!("hint: inspect it with `setup status`, then choose `setup install`");
    }
    if !toolchain.package_complete() {
        eprintln!("notice: distributable setup package payloads are incomplete");
        eprintln!(
            "hint: inspect them with `setup package status`, then choose `setup package install`"
        );
    }

    println!("Vapor shell");
    match state.current_dir() {
        Ok(directory) => println!("Working directory: {}", directory.display()),
        Err(_) => {
            println!("Source: closed");
            println!("Hint: run `sources list`, `sources add PATH`, or `open SOURCE`");
        }
    }

    run_shell(state);
    Ok(())
}

fn one_shot_allowed(command: &ShellCommand) -> bool {
    matches!(
        command,
        ShellCommand::Script { .. }
            | ShellCommand::Setup { .. }
            | ShellCommand::Content { .. }
            | ShellCommand::Sources { .. }
            | ShellCommand::Open { .. }
            | ShellCommand::Close
            | ShellCommand::Installation
            | ShellCommand::Binaries
            | ShellCommand::Libraries
            | ShellCommand::Metadata { .. }
    )
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
