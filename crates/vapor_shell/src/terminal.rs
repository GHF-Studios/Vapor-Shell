//! Guarded terminal relaunch for GUI/Steam application starts.

use crate::discovery::InstallationPaths;
use std::{
    env,
    ffi::OsString,
    fs::{self, File, OpenOptions},
    io::{self, IsTerminal, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::SystemTime,
};

pub(crate) fn needs_relaunch() -> bool {
    env::var_os("VAPOR_TERMINAL_RELAUNCHED").is_none()
        && (!io::stdin().is_terminal() || !io::stdout().is_terminal())
}

pub(crate) fn relaunch() -> Result<(), String> {
    if !cfg!(target_os = "linux") {
        return Err("Steam Shell terminal relaunch is currently Linux/Konsole-only".to_owned());
    }

    let executable = env::current_exe().map_err(|error| error.to_string())?;
    let installation = InstallationPaths::from_executable(&executable).ok();
    let app_root = installation
        .as_ref()
        .map(|installation| installation.root().to_path_buf())
        .or_else(|| fallback_app_root(&executable));
    let executable_dir = executable.parent().map(Path::to_path_buf);
    let mut log = LaunchLog::open(app_root.as_deref());
    log.write(format!(
        "relaunch requested for '{}' with DISPLAY={:?}, WAYLAND_DISPLAY={:?}, XDG_RUNTIME_DIR={:?}, DBUS_SESSION_BUS_ADDRESS={:?}",
        executable.display(),
        env::var_os("DISPLAY"),
        env::var_os("WAYLAND_DISPLAY"),
        env::var_os("XDG_RUNTIME_DIR"),
        env::var_os("DBUS_SESSION_BUS_ADDRESS")
    ));

    if env::var_os("DISPLAY").is_none() && env::var_os("WAYLAND_DISPLAY").is_none() {
        log.write("no DISPLAY or WAYLAND_DISPLAY is set; refusing to launch Konsole");
        let suffix = log
            .path()
            .map_or_else(String::new, |path| format!("; see {}", path.display()));
        return Err(format!(
            "Vapor was started without a terminal, but no graphical display is available{suffix}"
        ));
    }

    const RUNNER: &str = r#"vapor_terminal_closing=0
trap 'vapor_terminal_closing=1' HUP TERM
if [ -n "${VAPOR_TERMINAL_LAUNCH_LOG:-}" ]; then
    printf '[runner] started in %s with tty %s\n' "$PWD" "$(tty 2>/dev/null || printf 'not-a-tty')" >> "$VAPOR_TERMINAL_LAUNCH_LOG"
fi
program=$1
shift
"$program" "$@"
status=$?
trap - HUP TERM
if [ "$vapor_terminal_closing" -ne 0 ]; then
    if [ -n "${VAPOR_TERMINAL_LAUNCH_LOG:-}" ]; then
        printf '[runner] terminal close requested; skipping fallback shell after Vapor status %s\n' "$status" >> "$VAPOR_TERMINAL_LAUNCH_LOG"
    fi
    exit "$status"
fi
if [ -n "${VAPOR_TERMINAL_LAUNCH_LOG:-}" ]; then
    printf '[runner] Vapor exited with status %s; starting interactive shell %s\n' "$status" "${SHELL:-/bin/sh}" >> "$VAPOR_TERMINAL_LAUNCH_LOG"
fi
printf '\nVapor exited with status %s.\n' "$status"
printf 'Starting an interactive shell. Close this terminal when you are done.\n\n'
"${SHELL:-/bin/sh}" -i
shell_status=$?
if [ -n "${VAPOR_TERMINAL_LAUNCH_LOG:-}" ]; then
    printf '[runner] interactive shell exited with status %s\n' "$shell_status" >> "$VAPOR_TERMINAL_LAUNCH_LOG"
fi
exit "$status""#;

    let mut command = Command::new("/usr/bin/konsole");
    command.args(["--nofork", "-p", "tabtitle=Vapor Shell"]);
    if let Some(root) = &app_root {
        command.arg("--workdir").arg(root);
        command.current_dir(root);
    }
    command
        .arg("-e")
        .args(["sh", "-c", RUNNER, "vapor-terminal"])
        .arg(&executable)
        .args(env::args_os().skip(1))
        .env("VAPOR_TERMINAL_RELAUNCHED", "1")
        .env(
            "PATH",
            terminal_path(app_root.as_deref(), executable_dir.as_deref())?,
        )
        .env("TERM", terminal_type())
        .env("COLORTERM", "truecolor")
        .env_remove("LD_PRELOAD")
        .env_remove("LD_LIBRARY_PATH")
        .env_remove("LD_AUDIT")
        .stdout(Stdio::null());
    if let Some(path) = log.path() {
        command.env("VAPOR_TERMINAL_LAUNCH_LOG", path);
    }
    if let Some(stderr) = log.stderr() {
        command.stderr(Stdio::from(stderr));
    }

    log.write("attempting terminal: /usr/bin/konsole");
    let mut child = command
        .spawn()
        .map_err(|error| format!("failed to launch /usr/bin/konsole: {error}"))?;
    let status = child
        .wait()
        .map_err(|error| format!("failed to wait for /usr/bin/konsole: {error}"))?;
    log.write(format!(
        "terminal '/usr/bin/konsole' exited after user session with {status}"
    ));
    if status.success() {
        Ok(())
    } else {
        let suffix = log
            .path()
            .map_or_else(String::new, |path| format!("; see {}", path.display()));
        Err(format!("/usr/bin/konsole exited with {status}{suffix}"))
    }
}

struct LaunchLog {
    path: Option<PathBuf>,
    file: Option<File>,
}

impl LaunchLog {
    fn open(app_root: Option<&Path>) -> Self {
        let Some(app_root) = app_root else {
            return Self {
                path: None,
                file: None,
            };
        };
        let directory = app_root.join(".vapor/logs");
        let path = directory.join("terminal-launch.log");
        let file = fs::create_dir_all(&directory).ok().and_then(|_| {
            OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path)
                .ok()
        });
        Self {
            path: file.as_ref().map(|_| path),
            file,
        }
    }

    fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    fn stderr(&self) -> Option<File> {
        self.path
            .as_ref()
            .and_then(|path| OpenOptions::new().create(true).append(true).open(path).ok())
    }

    fn write(&mut self, message: impl AsRef<str>) {
        if let Some(file) = &mut self.file {
            let _ = writeln!(file, "[{:?}] {}", SystemTime::now(), message.as_ref());
        }
    }
}

fn terminal_type() -> OsString {
    env::var_os("TERM")
        .filter(|term| term != "dumb")
        .unwrap_or_else(|| OsString::from("xterm-256color"))
}

fn terminal_path(
    app_root: Option<&Path>,
    executable_dir: Option<&Path>,
) -> Result<OsString, String> {
    let mut entries = Vec::new();
    if let Some(directory) = executable_dir {
        push_unique_path(&mut entries, directory.to_path_buf());
    }
    if let Some(root) = app_root {
        push_unique_path(&mut entries, root.join("bin"));
    }
    if let Some(existing) = env::var_os("PATH") {
        for path in env::split_paths(&existing) {
            push_unique_path(&mut entries, path);
        }
    }
    for fallback in ["/usr/local/bin", "/usr/bin", "/bin"] {
        push_unique_path(&mut entries, PathBuf::from(fallback));
    }
    env::join_paths(entries).map_err(|error| format!("failed to construct terminal PATH: {error}"))
}

fn fallback_app_root(executable: &Path) -> Option<PathBuf> {
    let directory = executable.parent()?;
    if directory.file_name().is_some_and(|name| name == "bin") {
        return directory.parent().map(Path::to_path_buf);
    }
    if directory
        .parent()
        .and_then(Path::file_name)
        .is_some_and(|name| name == "bin")
    {
        return directory
            .parent()
            .and_then(Path::parent)
            .map(Path::to_path_buf);
    }
    directory.parent().map(Path::to_path_buf)
}

fn push_unique_path(entries: &mut Vec<PathBuf>, path: PathBuf) {
    if !entries.iter().any(|entry| entry == &path) {
        entries.push(path);
    }
}
