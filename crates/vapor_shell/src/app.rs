//! Application startup and the interactive read/evaluate loop.

use crate::{
    command::{self, Control, ShellCommand},
    discovery::EnvironmentPaths,
    prompt::VaporPrompt,
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

    let paths = EnvironmentPaths::discover()?;
    let mut state = ShellState::new(paths)?;
    command::print_warnings(state.refresh_context());

    if let Some(command) = one_shot {
        if !matches!(command, ShellCommand::Script { .. }) {
            return Err(
                "one-shot commands are disabled; run `vapor` for the interactive shell or use `vapor script run NAME`"
                    .to_owned(),
            );
        }
        command::execute(command, &mut state)?;
        return Ok(());
    }

    let toolchain = toolchain::inspect(state.paths().installation());
    match toolchain::location_status(state.paths().installation()) {
        Ok(toolchain::LocationStatus::Registered { .. }) => {}
        Ok(toolchain::LocationStatus::Unregistered { current }) => {
            eprintln!("notice: app root is not registered: {}", current.display());
            eprintln!("hint: review `toolchain status`, then choose `toolchain install`");
        }
        Ok(toolchain::LocationStatus::Moved { locked, current }) => {
            eprintln!("notice: app root moved and requires explicit confirmation");
            eprintln!("  previous: {}", locked.display());
            eprintln!("  current:   {}", current.display());
            eprintln!("hint: review `toolchain status`, then choose `toolchain repair`");
        }
        Err(error) => eprintln!("warning: app-root location state is invalid: {error}"),
    }
    if !toolchain.complete() {
        eprintln!("notice: the app-local Rust, Git, and SteamCMD toolchain is not complete");
        if toolchain.packages_complete() {
            eprintln!("hint: inspect it with `toolchain status`, then choose `toolchain install`");
        } else {
            eprintln!("hint: vendored packages are incomplete; verify the Steam app files");
        }
    }

    println!("Vapor shell");
    println!("Working directory: {}", state.current_dir().display());

    run_shell(state);
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
