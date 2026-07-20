//! Steam-facing terminal entrypoint for Vapor.
//!
//! This binary deliberately does not understand Vapor launch modes. Steam
//! starts this executable, it opens the platform terminal, forwards every
//! argument to the existing `bin/vapor-launch.*` script, waits for that
//! terminal to close, and exits with the terminal status.

#![cfg_attr(windows, windows_subsystem = "windows")]

use std::{
    env,
    ffi::OsString,
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    process::{Command, ExitStatus, Stdio},
    time::SystemTime,
};

fn main() {
    let status = match run() {
        Ok(status) => status.code().unwrap_or(1),
        Err(error) => {
            let _ = writeln!(std::io::stderr(), "vapor-entrypoint: {error}");
            1
        }
    };
    std::process::exit(status);
}

fn run() -> Result<ExitStatus, String> {
    let executable = env::current_exe()
        .map_err(|error| format!("failed to resolve vapor-entrypoint executable: {error}"))?;
    let app_root = discover_app_root(&executable).ok_or_else(|| {
        format!(
            "could not discover app root from executable '{}'",
            executable.display()
        )
    })?;
    let mut log = EntryLog::open(&app_root);
    let arguments: Vec<OsString> = env::args_os().skip(1).collect();
    log.write(format!(
        "entrypoint executable={} app_root={} args={:?}",
        executable.display(),
        app_root.display(),
        arguments
    ));

    let script = platform_script(&app_root);
    if !script.is_file() {
        let message = format!("launch script is missing: {}", script.display());
        log.write(&message);
        return Err(message);
    }

    launch_terminal(&app_root, &script, &arguments, &mut log)
}

fn discover_app_root(executable: &Path) -> Option<PathBuf> {
    let executable = fs::canonicalize(executable).ok()?;
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
    None
}

fn platform_script(app_root: &Path) -> PathBuf {
    #[cfg(windows)]
    {
        app_root.join("bin").join("vapor-launch.cmd")
    }
    #[cfg(not(windows))]
    {
        app_root.join("bin").join("vapor-launch.sh")
    }
}

#[cfg(target_os = "linux")]
fn launch_terminal(
    app_root: &Path,
    script: &Path,
    arguments: &[OsString],
    log: &mut EntryLog,
) -> Result<ExitStatus, String> {
    if env::var_os("DISPLAY").is_none() && env::var_os("WAYLAND_DISPLAY").is_none() {
        let message = "Konsole launch requires DISPLAY or WAYLAND_DISPLAY".to_owned();
        log.write(&message);
        return Err(message);
    }

    if is_steam_pressure_vessel() && Path::new("/run/host/usr/bin/konsole").is_file() {
        if let Some(loader) = host_loader() {
            log.write(format!(
                "launching host Konsole through loader {}",
                loader.display()
            ));
            let mut command = Command::new(loader);
            command
                .arg("--library-path")
                .arg(host_library_path())
                .arg("/run/host/usr/bin/konsole")
                .args(["--nofork", "--hold", "-p", "tabtitle=Vapor"])
                .arg("--workdir")
                .arg(app_root)
                .arg("-e")
                .arg("/usr/bin/env")
                .arg(format!("PATH={}", linux_child_path(app_root)))
                .arg(format!("VAPOR_APP_ROOT={}", app_root.display()))
                .arg("VAPOR_STEAM_LAUNCH=1")
                .arg("VAPOR_TERMINAL_RELAUNCHED=1")
                .arg("VAPOR_LAUNCHER_TERMINAL=1")
                .arg("VAPOR_LAUNCHER_HOLD_ON_EXIT=1")
                .arg(format!("VAPOR_ENTRYPOINT_LOG={}", log.path().display()))
                .arg(script)
                .args(arguments)
                .current_dir(app_root)
                .env_remove("LD_LIBRARY_PATH")
                .env_remove("LD_PRELOAD")
                .env_remove("LD_AUDIT")
                .env_remove("STEAM_RUNTIME_LIBRARY_PATH")
                .env(
                    "PATH",
                    format!(
                        "/run/host/usr/local/bin:/run/host/usr/bin:/run/host/bin:{}",
                        env::var_os("PATH")
                            .unwrap_or_else(|| OsString::from("/usr/bin:/bin"))
                            .to_string_lossy()
                    ),
                )
                .env(
                    "QT_PLUGIN_PATH",
                    "/run/host/usr/lib/qt6/plugins:/run/host/usr/lib/x86_64-linux-gnu/qt6/plugins",
                )
                .env(
                    "XDG_DATA_DIRS",
                    "/run/host/usr/local/share:/run/host/usr/share:/usr/share",
                );
            return wait_for_terminal(command, "host Konsole", log);
        }
        log.write("host Konsole exists but no /run/host dynamic loader was found");
    }

    let konsole = if Path::new("/usr/bin/konsole").is_file() {
        PathBuf::from("/usr/bin/konsole")
    } else {
        PathBuf::from("konsole")
    };
    log.write(format!("launching Konsole through {}", konsole.display()));
    let mut command = Command::new(konsole);
    command
        .args(["--nofork", "--hold", "-p", "tabtitle=Vapor"])
        .arg("--workdir")
        .arg(app_root)
        .arg("-e")
        .arg(script)
        .args(arguments)
        .current_dir(app_root);
    configure_child_environment(&mut command, app_root, log);
    wait_for_terminal(command, "Konsole", log)
}

#[cfg(windows)]
fn launch_terminal(
    app_root: &Path,
    script: &Path,
    arguments: &[OsString],
    log: &mut EntryLog,
) -> Result<ExitStatus, String> {
    let shell = env::var_os("ComSpec").unwrap_or_else(|| OsString::from("cmd.exe"));
    log.write(format!(
        "launching command prompt through {}",
        PathBuf::from(&shell).display()
    ));
    let mut command = Command::new(shell);
    command
        .args(["/D", "/K", "call"])
        .arg(script)
        .args(arguments)
        .current_dir(app_root);
    configure_child_environment(&mut command, app_root, log);
    wait_for_terminal(command, "Command Prompt", log)
}

#[cfg(not(any(target_os = "linux", windows)))]
fn launch_terminal(
    _app_root: &Path,
    _script: &Path,
    _arguments: &[OsString],
    log: &mut EntryLog,
) -> Result<ExitStatus, String> {
    let message = format!(
        "vapor-entrypoint has no terminal adapter for {}",
        env::consts::OS
    );
    log.write(&message);
    Err(message)
}

fn configure_child_environment(command: &mut Command, app_root: &Path, log: &EntryLog) {
    command
        .env("VAPOR_APP_ROOT", app_root)
        .env("VAPOR_STEAM_LAUNCH", "1")
        .env("VAPOR_TERMINAL_RELAUNCHED", "1")
        .env("VAPOR_LAUNCHER_TERMINAL", "1")
        .env("VAPOR_LAUNCHER_HOLD_ON_EXIT", "1")
        .env("VAPOR_ENTRYPOINT_LOG", log.path());
}

fn wait_for_terminal(
    mut command: Command,
    label: &str,
    log: &mut EntryLog,
) -> Result<ExitStatus, String> {
    if let Some(stderr) = log.stderr() {
        command.stderr(Stdio::from(stderr));
    }
    log.write(format!("waiting for {label}"));
    let status = command
        .status()
        .map_err(|error| format!("failed to launch {label}: {error}"))?;
    log.write(format!("{label} exited with {status}"));
    Ok(status)
}

#[cfg(target_os = "linux")]
fn is_steam_pressure_vessel() -> bool {
    env::var_os("PRESSURE_VESSEL_RUNTIME").is_some()
        || env::var_os("container").is_some_and(|container| container == "pressure-vessel")
}

#[cfg(target_os = "linux")]
fn host_loader() -> Option<PathBuf> {
    [
        "/run/host/lib64/ld-linux-x86-64.so.2",
        "/run/host/usr/lib64/ld-linux-x86-64.so.2",
        "/run/host/lib/ld-linux-x86-64.so.2",
        "/run/host/usr/lib/ld-linux-x86-64.so.2",
        "/run/host/lib/x86_64-linux-gnu/ld-linux-x86-64.so.2",
        "/run/host/usr/lib/x86_64-linux-gnu/ld-linux-x86-64.so.2",
    ]
    .iter()
    .map(PathBuf::from)
    .find(|candidate| candidate.is_file())
}

#[cfg(target_os = "linux")]
fn host_library_path() -> &'static std::ffi::OsStr {
    std::ffi::OsStr::new(
        "/run/host/usr/lib:/run/host/usr/lib64:/run/host/usr/lib/x86_64-linux-gnu:/run/host/usr/lib/pulseaudio:/run/host/usr/lib/libproxy:/run/host/usr/lib/qt6/plugins:/run/host/usr/lib/x86_64-linux-gnu/qt6/plugins:/run/host/lib:/run/host/lib64:/run/host/lib/x86_64-linux-gnu",
    )
}

#[cfg(target_os = "linux")]
fn linux_child_path(app_root: &Path) -> String {
    let mut paths = vec![
        app_root.join("bin/x86_64-unknown-linux-gnu"),
        app_root.join("cargo-home/bin"),
        app_root.join("rustup/bin"),
        app_root.join("tools/steamcmd"),
        PathBuf::from("/usr/bin"),
        PathBuf::from("/bin"),
    ];
    if let Some(existing) = env::var_os("PATH") {
        paths.extend(env::split_paths(&existing));
    }
    env::join_paths(paths)
        .unwrap_or_else(|_| OsString::from("/usr/bin:/bin"))
        .to_string_lossy()
        .into_owned()
}

struct EntryLog {
    path: PathBuf,
    file: Option<File>,
}

impl EntryLog {
    fn open(app_root: &Path) -> Self {
        let directory = app_root.join(".vapor/logs");
        let path = directory.join("entrypoint.log");
        let file = fs::create_dir_all(&directory).ok().and_then(|_| {
            OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path)
                .ok()
        });
        Self { path, file }
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn stderr(&self) -> Option<File> {
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .ok()
    }

    fn write(&mut self, message: impl AsRef<str>) {
        if let Some(file) = &mut self.file {
            let _ = writeln!(file, "[{:?}] {}", SystemTime::now(), message.as_ref());
        }
    }
}
