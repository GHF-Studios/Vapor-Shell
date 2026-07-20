//! Private-test diagnostics capture and explicit shipping.

use crate::{discovery::InstallationPaths, git_provider, manifest};
use std::{
    env,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    process::Command,
    sync::{Mutex, OnceLock},
    time::{SystemTime, UNIX_EPOCH},
};

static RUN: OnceLock<DiagnosticsRun> = OnceLock::new();

const LATEST_FILE: &str = "latest.txt";

#[derive(Debug, Clone)]
pub(crate) struct CaptureOptions {
    pub(crate) enabled: bool,
    pub(crate) submit: bool,
    pub(crate) push: bool,
    pub(crate) registry: Option<PathBuf>,
}

impl CaptureOptions {
    pub(crate) fn disabled() -> Self {
        Self {
            enabled: false,
            submit: false,
            push: false,
            registry: None,
        }
    }
}

pub(crate) struct SubmitOptions {
    pub(crate) registry: Option<PathBuf>,
    pub(crate) push: bool,
    pub(crate) all: bool,
    pub(crate) dry_run: bool,
}

pub(crate) struct SubmitReport {
    registry: PathBuf,
    target_dir: PathBuf,
    logs: Vec<PathBuf>,
    committed: bool,
    pushed: bool,
    dry_run: bool,
}

impl SubmitReport {
    pub(crate) fn registry(&self) -> &Path {
        &self.registry
    }

    pub(crate) fn target_dir(&self) -> &Path {
        &self.target_dir
    }

    pub(crate) fn logs(&self) -> &[PathBuf] {
        &self.logs
    }

    pub(crate) fn committed(&self) -> bool {
        self.committed
    }

    pub(crate) fn pushed(&self) -> bool {
        self.pushed
    }

    pub(crate) fn dry_run(&self) -> bool {
        self.dry_run
    }
}

struct DiagnosticsRun {
    path: PathBuf,
    options: CaptureOptions,
    file: Mutex<File>,
}

impl DiagnosticsRun {
    fn write(&self, message: impl AsRef<str>) {
        if let Ok(mut file) = self.file.lock() {
            let _ = writeln!(file, "[{}] {}", timestamp(), message.as_ref());
        }
    }

    fn flush(&self) {
        if let Ok(mut file) = self.file.lock() {
            let _ = file.flush();
        }
    }
}

pub(crate) fn init_from_current_exe(options: CaptureOptions) {
    if !options.enabled || RUN.get().is_some() {
        return;
    }

    let Ok(executable) = env::current_exe() else {
        return;
    };
    let app_root = InstallationPaths::from_executable(&executable)
        .map(|installation| installation.root().to_path_buf())
        .ok()
        .or_else(|| fallback_app_root(&executable));
    let Some(app_root) = app_root.filter(|root| root.join(manifest::APP_FILE_NAME).is_file())
    else {
        return;
    };

    let directory = run_directory(&app_root);
    if fs::create_dir_all(&directory).is_err() {
        return;
    }
    let run_id = run_id();
    let path = directory.join(format!("run-{run_id}.log"));
    let Ok(file) = OpenOptions::new().create(true).append(true).open(&path) else {
        return;
    };
    let run = DiagnosticsRun {
        path: path.clone(),
        options,
        file: Mutex::new(file),
    };
    if RUN.set(run).is_err() {
        return;
    }
    let _ = fs::write(
        diagnostics_dir(&app_root).join(LATEST_FILE),
        format!("runs/run-{run_id}.log\n"),
    );

    event("diagnostics capture started");
    event(format!("run id: {run_id}"));
    event(format!("executable: {}", executable.display()));
    event(format!(
        "cwd: {}",
        env::current_dir()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|error| format!("unavailable ({error})"))
    ));
    event(format!("platform: {}", env::consts::OS));
    event(format!("args: {}", joined_args()));
    for key in [
        "VAPOR_STEAM_LAUNCH",
        "VAPOR_LAUNCH_MODE",
        "VAPOR_LAUNCHER_TERMINAL",
        "VAPOR_TERMINAL_RELAUNCHED",
        "VAPOR_DIAGNOSTICS",
        "VAPOR_DIAGNOSTICS_AUTO_SUBMIT",
        "VAPOR_DIAGNOSTICS_REGISTRY",
        "SteamAppId",
        "SteamGameId",
    ] {
        if let Some(value) = env::var_os(key) {
            event(format!("env {key}: {}", value.to_string_lossy()));
        }
    }
}

pub(crate) fn event(message: impl AsRef<str>) {
    if let Some(run) = RUN.get() {
        run.write(message);
    }
}

pub(crate) fn finish(success: bool) {
    event(format!(
        "diagnostics capture finished: {}",
        if success { "ok" } else { "error" }
    ));
    if let Some(run) = RUN.get() {
        run.flush();
    }
    if let Some(run) = RUN.get()
        && run.options.submit
    {
        let push = run.options.push;
        let options = SubmitOptions {
            registry: run.options.registry.clone(),
            push,
            all: false,
            dry_run: false,
        };
        event(format!(
            "automatic diagnostics submit requested: {}",
            if push { "push" } else { "copy" }
        ));
        match submit_current_run(&options) {
            Ok(report) => {
                event(format!(
                    "automatic diagnostics submit copied {} log(s) to {}{}",
                    report.logs().len(),
                    report.target_dir().display(),
                    if report.pushed() { " and pushed" } else { "" }
                ));
                eprintln!(
                    "diagnostics: sent {} log(s) to {}{}",
                    report.logs().len(),
                    report.target_dir().display(),
                    if report.pushed() { " and pushed" } else { "" }
                );
            }
            Err(error) => {
                event(format!("automatic diagnostics submit failed: {error}"));
                eprintln!("diagnostics: send failed: {error}");
            }
        }
        if let Some(run) = RUN.get() {
            run.flush();
        }
    }
}

pub(crate) fn local_directory(installation: &InstallationPaths) -> PathBuf {
    diagnostics_dir(installation.root())
}

pub(crate) fn current_run_path() -> Option<PathBuf> {
    RUN.get().map(|run| run.path.clone())
}

pub(crate) fn latest_run_path(installation: &InstallationPaths) -> Option<PathBuf> {
    latest_log_path(installation.root())
}

pub(crate) fn auto_capture_enabled() -> bool {
    RUN.get().is_some()
}

pub(crate) fn auto_submit_setting() -> Option<String> {
    RUN.get().and_then(|run| {
        run.options.submit.then(|| {
            if run.options.push {
                "push".to_owned()
            } else {
                "copy".to_owned()
            }
        })
    })
}

pub(crate) fn registry_setting() -> Option<PathBuf> {
    RUN.get()
        .and_then(|run| run.options.registry.clone())
        .or_else(|| env::var_os("VAPOR_DIAGNOSTICS_REGISTRY").map(PathBuf::from))
}

pub(crate) fn submit(
    installation: &InstallationPaths,
    options: &SubmitOptions,
) -> Result<SubmitReport, String> {
    if let Some(run) = RUN.get() {
        run.flush();
    }
    submit_from_installation(
        installation,
        options,
        RUN.get().map(|run| run.path.as_path()),
    )
}

fn submit_current_run(options: &SubmitOptions) -> Result<SubmitReport, String> {
    let run = RUN
        .get()
        .ok_or_else(|| "no active diagnostics run is being captured".to_owned())?;
    let installation =
        InstallationPaths::from_executable(&env::current_exe().map_err(|error| error.to_string())?)
            .map_err(|error| {
                format!("cannot resolve installation for diagnostics submit: {error}")
            })?;
    submit_from_installation(&installation, options, Some(run.path.as_path()))
}

fn submit_from_installation(
    installation: &InstallationPaths,
    options: &SubmitOptions,
    current: Option<&Path>,
) -> Result<SubmitReport, String> {
    let registry = resolve_registry(options.registry.as_deref())?;
    if !is_git_checkout(&registry) {
        return Err(format!(
            "diagnostics registry '{}' is not a Git checkout",
            registry.display()
        ));
    }
    let logs = logs_to_submit(installation, current, options.all)?;
    if logs.is_empty() {
        return Err(format!(
            "no diagnostics logs found in {}",
            run_directory(installation.root()).display()
        ));
    }
    let target_dir = registry
        .join("diagnostics")
        .join(sanitize_component(installation.identity_id()))
        .join(sanitize_component(&machine_id()))
        .join(sanitize_component(env::consts::OS));

    if !options.dry_run {
        fs::create_dir_all(&target_dir).map_err(|error| {
            format!(
                "failed to create diagnostics target '{}': {error}",
                target_dir.display()
            )
        })?;
        for log in &logs {
            let file_name = log
                .file_name()
                .ok_or_else(|| format!("diagnostics log has no file name: {}", log.display()))?;
            fs::copy(log, target_dir.join(file_name)).map_err(|error| {
                format!(
                    "failed to copy diagnostics log '{}' into '{}': {error}",
                    log.display(),
                    target_dir.display()
                )
            })?;
        }
    }

    let mut committed = false;
    let mut pushed = false;
    if options.push && !options.dry_run {
        committed = commit_diagnostics(installation, &registry)?;
        if committed {
            run_git(installation, &registry, ["push"])?;
            pushed = true;
        }
    }

    Ok(SubmitReport {
        registry,
        target_dir,
        logs,
        committed,
        pushed,
        dry_run: options.dry_run,
    })
}

fn commit_diagnostics(installation: &InstallationPaths, registry: &Path) -> Result<bool, String> {
    run_git(installation, registry, ["add", "diagnostics"])?;
    let mut command = git_command(installation)?;
    let status = command
        .current_dir(registry)
        .args(["diff", "--cached", "--quiet"])
        .status()
        .map_err(|error| {
            format!(
                "failed to run Git diff in '{}': {error}",
                registry.display()
            )
        })?;
    if status.success() {
        return Ok(false);
    }
    run_git(
        installation,
        registry,
        [
            "commit",
            "-m",
            &format!("Upload Vapor diagnostics {}", machine_id()),
        ],
    )?;
    Ok(true)
}

fn run_git<const N: usize>(
    installation: &InstallationPaths,
    registry: &Path,
    args: [&str; N],
) -> Result<(), String> {
    let mut command = git_command(installation)?;
    let status = command
        .current_dir(registry)
        .args(args)
        .status()
        .map_err(|error| format!("failed to run Git in '{}': {error}", registry.display()))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "Git exited with {status} in '{}'",
            registry.display()
        ))
    }
}

fn git_command(installation: &InstallationPaths) -> Result<Command, String> {
    git_provider::command(installation).map_err(|error| {
        format!(
            "cannot push diagnostics: {error}\nhelp: pass `--registry /path/to/Vapor-Registry` and link Git with `provider git link /path/to/git`"
        )
    })
}

fn is_git_checkout(path: &Path) -> bool {
    path.join(".git").exists()
}

fn logs_to_submit(
    installation: &InstallationPaths,
    current: Option<&Path>,
    all: bool,
) -> Result<Vec<PathBuf>, String> {
    if all {
        let directory = run_directory(installation.root());
        if !directory.is_dir() {
            return Ok(Vec::new());
        }
        let mut logs = fs::read_dir(&directory)
            .map_err(|error| {
                format!(
                    "failed to read diagnostics log directory '{}': {error}",
                    directory.display()
                )
            })?
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|extension| extension == "log"))
            .collect::<Vec<_>>();
        logs.sort();
        return Ok(logs);
    }
    if let Some(path) = current.filter(|path| path.is_file()) {
        return Ok(vec![path.to_path_buf()]);
    }
    if let Some(path) = latest_log_path(installation.root()) {
        return Ok(vec![path]);
    }
    Ok(Vec::new())
}

fn resolve_registry(explicit: Option<&Path>) -> Result<PathBuf, String> {
    let path = explicit
        .map(Path::to_path_buf)
        .or_else(|| env::var_os("VAPOR_DIAGNOSTICS_REGISTRY").map(PathBuf::from))
        .ok_or_else(|| {
            "diagnostics registry is not set\nhelp: pass `--registry /path/to/Vapor-Registry` or set VAPOR_DIAGNOSTICS_REGISTRY\nnote: normal Steam installs no longer create an app-local registry checkout"
                .to_owned()
        })?;
    fs::canonicalize(&path).map_err(|error| {
        format!(
            "failed to resolve diagnostics registry '{}': {error}",
            path.display()
        )
    })
}

fn latest_log_path(app_root: &Path) -> Option<PathBuf> {
    let latest = diagnostics_dir(app_root).join(LATEST_FILE);
    let mut source = String::new();
    File::open(&latest).ok()?.read_to_string(&mut source).ok()?;
    let path = PathBuf::from(source.trim());
    let path = if path.is_absolute() {
        path
    } else {
        diagnostics_dir(app_root).join(path)
    };
    path.is_file().then_some(path)
}

fn diagnostics_dir(app_root: &Path) -> PathBuf {
    app_root.join(".vapor/diagnostics")
}

fn run_directory(app_root: &Path) -> PathBuf {
    diagnostics_dir(app_root).join("runs")
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

fn timestamp() -> String {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}.{:09}Z", duration.as_secs(), duration.subsec_nanos())
}

fn run_id() -> String {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    format!(
        "{}-{:09}-{}",
        duration.as_secs(),
        duration.subsec_nanos(),
        std::process::id()
    )
}

fn joined_args() -> String {
    env::args_os()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join(" ")
}

fn machine_id() -> String {
    env::var("VAPOR_DIAGNOSTICS_MACHINE")
        .or_else(|_| env::var("COMPUTERNAME"))
        .or_else(|_| env::var("HOSTNAME"))
        .unwrap_or_else(|_| "unknown-machine".to_owned())
}

fn sanitize_component(value: &str) -> String {
    let mut sanitized = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.') {
                character
            } else {
                '-'
            }
        })
        .collect::<String>();
    while sanitized.contains("--") {
        sanitized = sanitized.replace("--", "-");
    }
    let sanitized = sanitized.trim_matches('-');
    if sanitized.is_empty() {
        "unknown".to_owned()
    } else {
        sanitized.to_owned()
    }
}
