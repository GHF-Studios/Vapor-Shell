//! Guarded terminal relaunch for GUI/Steam application starts.

use std::{
    env,
    io::{self, IsTerminal},
    process::Command,
};

pub(crate) fn needs_relaunch() -> bool {
    env::var_os("VAPOR_TERMINAL_RELAUNCHED").is_none()
        && (!io::stdin().is_terminal() || !io::stdout().is_terminal())
}

pub(crate) fn relaunch() -> Result<(), String> {
    let executable = env::current_exe().map_err(|error| error.to_string())?;
    let mut attempts: Vec<Command> = Vec::new();
    if cfg!(target_os = "windows") {
        let mut command = Command::new("cmd");
        command.args(["/C", "start", ""]).arg(&executable);
        attempts.push(command);
    } else if cfg!(target_os = "macos") {
        let mut command = Command::new("open");
        command.args(["-a", "Terminal"]).arg(&executable);
        attempts.push(command);
    } else {
        const RUNNER: &str = r#""$1"
status=$?
if [ "$status" -ne 0 ]; then
    printf '\nVapor exited with status %s. Press Enter to close this terminal.\n' "$status"
    read -r _
fi
exit "$status""#;
        for (program, separator) in [
            ("x-terminal-emulator", "-e"),
            ("konsole", "-e"),
            ("gnome-terminal", "--"),
            ("xterm", "-e"),
        ] {
            let mut command = Command::new(program);
            command
                .arg(separator)
                .args(["sh", "-c", RUNNER, "vapor-terminal"])
                .arg(&executable);
            attempts.push(command);
        }
    }
    for mut command in attempts {
        command.env("VAPOR_TERMINAL_RELAUNCHED", "1");
        if command.spawn().is_ok() {
            return Ok(());
        }
    }
    Err(
        "Vapor was started without a terminal and no supported terminal emulator could be launched"
            .to_owned(),
    )
}
