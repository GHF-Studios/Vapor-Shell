//! Private-test diagnostics capture and explicit future upload boundary.

use crate::{
    app_local_tools,
    discovery::InstallationPaths,
    manifest,
    state::{SourceContext, SourceRootKind},
};
use serde::{Deserialize, Serialize};
use std::{
    env,
    ffi::OsString,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
    time::{SystemTime, UNIX_EPOCH},
};

static RUN: OnceLock<DiagnosticsRun> = OnceLock::new();

const LATEST_FILE: &str = "latest.toml";

#[derive(Debug, Clone)]
pub(crate) struct CaptureOptions {
    pub(crate) enabled: bool,
    pub(crate) upload: bool,
}

impl CaptureOptions {
    pub(crate) fn disabled() -> Self {
        Self {
            enabled: false,
            upload: false,
        }
    }
}

pub(crate) struct UploadOptions {
    pub(crate) dry_run: bool,
}

pub(crate) struct UploadReport {
    run: LocalRun,
    transport: String,
    sent: bool,
    dry_run: bool,
}

impl UploadReport {
    pub(crate) fn run(&self) -> &LocalRun {
        &self.run
    }

    pub(crate) fn transport(&self) -> &str {
        &self.transport
    }

    pub(crate) fn sent(&self) -> bool {
        self.sent
    }

    pub(crate) fn dry_run(&self) -> bool {
        self.dry_run
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct LocalRun {
    run_id: String,
    run_dir: PathBuf,
    metadata_path: PathBuf,
    log_path: PathBuf,
}

impl LocalRun {
    pub(crate) fn run_id(&self) -> &str {
        &self.run_id
    }

    pub(crate) fn run_dir(&self) -> &Path {
        &self.run_dir
    }

    pub(crate) fn metadata_path(&self) -> &Path {
        &self.metadata_path
    }

    pub(crate) fn log_path(&self) -> &Path {
        &self.log_path
    }
}

struct DiagnosticsRun {
    local: LocalRun,
    options: CaptureOptions,
    file: Mutex<File>,
    metadata: Mutex<RunMetadata>,
}

impl DiagnosticsRun {
    fn write(&self, message: impl AsRef<str>) {
        if let Ok(mut file) = self.file.lock() {
            let _ = writeln!(file, "[{}] {}", timestamp(), redact_text(message.as_ref()));
        }
    }

    fn flush(&self) {
        if let Ok(mut file) = self.file.lock() {
            let _ = file.flush();
        }
    }

    fn update_metadata(&self, update: impl FnOnce(&mut RunMetadata)) {
        if let Ok(mut metadata) = self.metadata.lock() {
            update(&mut metadata);
            let _ = write_metadata(&self.local.metadata_path, &metadata);
            let _ = write_latest_file(
                diagnostics_root_from_run_dir(&self.local.run_dir),
                &self.local,
            );
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "kebab-case")]
struct RunMetadata {
    schema: u32,
    run: RunIdentity,
    platform: PlatformMetadata,
    paths: PathMetadata,
    startup: StartupMetadata,
    installation: InstallationMetadata,
    readiness: Option<ReadinessMetadata>,
    source: Option<SourceMetadata>,
    launch: LaunchMetadata,
    args: Vec<String>,
    steps: Vec<String>,
    errors: Vec<String>,
    upload: UploadMetadata,
    exit: Option<ExitMetadata>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "kebab-case")]
struct RunIdentity {
    id: String,
    timestamp: String,
    timestamp_unix_seconds: u64,
    short_random: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "kebab-case")]
struct PlatformMetadata {
    os: String,
    arch: String,
    family: String,
    platform: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "kebab-case")]
struct PathMetadata {
    app_root: String,
    vapor_executable: String,
    run_dir: String,
    metadata: String,
    log: String,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "kebab-case")]
struct StartupMetadata {
    mode: Option<String>,
    direct_command: Option<String>,
    startup_script: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "kebab-case")]
struct InstallationMetadata {
    identity_id: Option<String>,
    app_manifest: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "kebab-case")]
struct ReadinessMetadata {
    steamcmd: ComponentMetadata,
    rust_cargo: ComponentMetadata,
    cross_toolchains: ComponentMetadata,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "kebab-case")]
struct ComponentMetadata {
    ready: bool,
    path: String,
    missing: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "kebab-case")]
struct SourceMetadata {
    kind: String,
    identity_id: String,
    root: String,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "kebab-case")]
struct LaunchMetadata {
    target: Option<String>,
    selected_packagepack: Option<ContentSelectionMetadata>,
    engine_handoff: Option<EngineHandoffMetadata>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "kebab-case")]
struct ContentSelectionMetadata {
    artifact_id: String,
    root: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "kebab-case")]
struct EngineHandoffMetadata {
    engine_id: String,
    root: String,
    runtime_target: String,
    binary: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "kebab-case")]
struct UploadMetadata {
    requested: bool,
    transport: String,
    status: String,
    detail: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "kebab-case")]
struct ExitMetadata {
    success: bool,
    status: String,
    finished_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct LatestMetadata {
    schema: u32,
    run_id: String,
    run_dir: String,
    metadata: String,
    log: String,
    updated_at: String,
}

pub(crate) fn init_from_current_exe(options: CaptureOptions) {
    if !options.enabled || RUN.get().is_some() {
        return;
    }

    let Ok(executable) = env::current_exe() else {
        return;
    };
    let installation = InstallationPaths::from_executable(&executable).ok();
    let app_root = installation
        .as_ref()
        .map(|installation| installation.root().to_path_buf())
        .or_else(|| fallback_app_root(&executable));
    let Some(app_root) = app_root.filter(|root| root.join(manifest::APP_FILE_NAME).is_file())
    else {
        return;
    };
    let args = redacted_args(env::args_os());
    let installation_identity = installation
        .as_ref()
        .map(|installation| installation.identity_id().to_owned());
    let Ok(run) = prepare_run(&app_root, &executable, options, args, installation_identity) else {
        return;
    };
    let run_id = run.local.run_id.clone();
    let run_dir = run.local.run_dir.clone();
    if RUN.set(run).is_err() {
        return;
    }

    event("diagnostics capture started");
    event(format!("run id: {run_id}"));
    event(format!("run dir: {}", run_dir.display()));
    event(format!("executable: {}", executable.display()));
    event(format!(
        "cwd: {}",
        env::current_dir()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|error| format!("unavailable ({error})"))
    ));
    event(format!(
        "platform: {}-{}",
        env::consts::OS,
        env::consts::ARCH
    ));
    event(format!("args: {}", redacted_args(env::args_os()).join(" ")));
    for key in ["SteamAppId", "SteamGameId"] {
        if let Some(value) = env::var_os(key) {
            event(format!("env {key}: {}", value.to_string_lossy()));
        }
    }
}

pub(crate) fn record_startup_mode(mode: &str) {
    if let Some(run) = RUN.get() {
        let mode = redact_text(mode);
        run.update_metadata(|metadata| metadata.startup.mode = Some(mode));
    }
}

pub(crate) fn record_direct_command(command: impl AsRef<str>) {
    if let Some(run) = RUN.get() {
        let command = redact_text(command.as_ref());
        run.update_metadata(|metadata| metadata.startup.direct_command = Some(command));
    }
}

pub(crate) fn record_startup_script(script: &str) {
    if let Some(run) = RUN.get() {
        let script = redact_text(script);
        run.update_metadata(|metadata| metadata.startup.startup_script = Some(script));
    }
}

pub(crate) fn record_installation(installation: &InstallationPaths) {
    if let Some(run) = RUN.get() {
        let tool_status = app_local_tools::inspect(installation);
        run.update_metadata(|metadata| {
            metadata.paths.app_root = display_path(installation.root());
            metadata.paths.vapor_executable = display_path(installation.executable());
            metadata.installation.identity_id = Some(installation.identity_id().to_owned());
            metadata.installation.app_manifest =
                display_path(&installation.root().join(manifest::APP_FILE_NAME));
            metadata.readiness = Some(ReadinessMetadata {
                steamcmd: component_metadata(tool_status.steamcmd()),
                rust_cargo: component_metadata(tool_status.rust()),
                cross_toolchains: component_metadata(tool_status.cross_toolchains()),
            });
        });
    }
}

pub(crate) fn record_source_context(source: Option<&SourceContext>) {
    if let Some(run) = RUN.get() {
        run.update_metadata(|metadata| {
            metadata.source = source.map(|source| SourceMetadata {
                kind: match source.kind() {
                    SourceRootKind::Root => "root",
                    SourceRootKind::Workspace => "workspace",
                }
                .to_owned(),
                identity_id: source.id().to_owned(),
                root: display_path(source.root()),
            });
        });
    }
}

pub(crate) fn record_launch_target(target: &str) {
    if let Some(run) = RUN.get() {
        let target = redact_text(target);
        run.update_metadata(|metadata| metadata.launch.target = Some(target));
    }
}

pub(crate) fn record_selected_packagepack(artifact_id: &str, root: &Path) {
    if let Some(run) = RUN.get() {
        let artifact_id = redact_text(artifact_id);
        let root = display_path(root);
        run.update_metadata(|metadata| {
            metadata.launch.selected_packagepack =
                Some(ContentSelectionMetadata { artifact_id, root });
        });
    }
}

pub(crate) fn record_engine_handoff(
    engine_id: &str,
    root: &Path,
    runtime_target: &str,
    binary: &Path,
) {
    if let Some(run) = RUN.get() {
        let engine_id = redact_text(engine_id);
        let root = display_path(root);
        let runtime_target = redact_text(runtime_target);
        let binary = display_path(binary);
        run.update_metadata(|metadata| {
            metadata.launch.engine_handoff = Some(EngineHandoffMetadata {
                engine_id,
                root,
                runtime_target,
                binary,
            });
        });
    }
}

pub(crate) fn step(message: impl AsRef<str>) {
    let message = redact_text(message.as_ref());
    event(format!("step: {message}"));
    if let Some(run) = RUN.get() {
        run.update_metadata(|metadata| {
            metadata.steps.push(message);
            trim_vec(&mut metadata.steps, 200);
        });
    }
}

pub(crate) fn record_error(message: impl AsRef<str>) {
    let message = redact_text(message.as_ref());
    event(format!("error: {message}"));
    if let Some(run) = RUN.get() {
        run.update_metadata(|metadata| {
            metadata.errors.push(message);
            trim_vec(&mut metadata.errors, 100);
        });
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
        run.update_metadata(|metadata| {
            metadata.exit = Some(ExitMetadata {
                success,
                status: if success { "ok" } else { "error" }.to_owned(),
                finished_at: timestamp(),
            });
        });
        run.flush();
        eprintln!(
            "diagnostics: captured run {} at {}",
            run.local.run_id(),
            run.local.run_dir().display()
        );

        if run.options.upload {
            let transport = transport::default_transport();
            if !transport.configured() {
                let detail = transport.not_configured_message();
                run.update_metadata(|metadata| {
                    metadata.upload.status = "not-configured".to_owned();
                    metadata.upload.detail = Some(detail.clone());
                });
                event(format!("diagnostics upload skipped: {detail}"));
                eprintln!("diagnostics: upload skipped: {detail}");
                eprintln!(
                    "diagnostics: local run is at {}",
                    run.local.run_dir().display()
                );
            } else if let Err(error) = upload_local_run(&run.local, false, transport.as_ref()) {
                run.update_metadata(|metadata| {
                    metadata.upload.status = "failed".to_owned();
                    metadata.upload.detail = Some(error.clone());
                });
                event(format!("diagnostics upload failed: {error}"));
                eprintln!("diagnostics: upload failed: {error}");
                eprintln!(
                    "diagnostics: local run is at {}",
                    run.local.run_dir().display()
                );
            } else {
                run.update_metadata(|metadata| {
                    metadata.upload.status = "sent".to_owned();
                    metadata.upload.detail = None;
                });
                event("diagnostics upload sent");
                eprintln!("diagnostics: sent run {}", run.local.run_id());
            }
        }
        run.flush();
    }
}

pub(crate) fn local_directory(installation: &InstallationPaths) -> PathBuf {
    diagnostics_dir(installation.root())
}

pub(crate) fn current_run() -> Option<LocalRun> {
    RUN.get().map(|run| run.local.clone())
}

pub(crate) fn latest_run(installation: &InstallationPaths) -> Option<LocalRun> {
    latest_local_run(installation.root())
}

pub(crate) fn auto_capture_enabled() -> bool {
    RUN.get().is_some()
}

pub(crate) fn upload_setting() -> Option<String> {
    RUN.get().and_then(|run| {
        run.options
            .upload
            .then(|| transport::default_transport().name().to_owned())
    })
}

pub(crate) fn upload(
    installation: &InstallationPaths,
    options: &UploadOptions,
) -> Result<UploadReport, String> {
    if let Some(run) = RUN.get() {
        run.flush();
    }
    let run = current_run()
        .or_else(|| latest_run(installation))
        .ok_or_else(|| {
            format!(
                "no diagnostics runs found in {}",
                run_directory(installation.root()).display()
            )
        })?;
    let transport = transport::default_transport();
    if options.dry_run {
        return Ok(UploadReport {
            run,
            transport: transport.name().to_owned(),
            sent: false,
            dry_run: true,
        });
    }
    upload_local_run(&run, false, transport.as_ref())?;
    Ok(UploadReport {
        run,
        transport: transport.name().to_owned(),
        sent: true,
        dry_run: false,
    })
}

fn upload_local_run(
    run: &LocalRun,
    dry_run: bool,
    transport: &dyn transport::DiagnosticsTransport,
) -> Result<(), String> {
    transport.upload(run, dry_run)
}

fn prepare_run(
    app_root: &Path,
    executable: &Path,
    options: CaptureOptions,
    args: Vec<String>,
    installation_identity: Option<String>,
) -> Result<DiagnosticsRun, String> {
    let now = now_parts();
    let date = utc_date(now.seconds);
    let platform = platform_label();
    let short_random = short_run_token(now);
    let run_id = format!(
        "{}-{}-{}",
        now.seconds,
        sanitize_component(&platform),
        short_random
    );
    let run_dir = run_directory(app_root).join(date).join(format!(
        "{}-{}-{}",
        now.seconds,
        sanitize_component(&platform),
        sanitize_component(&short_random)
    ));
    fs::create_dir_all(&run_dir).map_err(|error| {
        format!(
            "failed to create diagnostics run directory '{}': {error}",
            run_dir.display()
        )
    })?;
    let metadata_path = run_dir.join("metadata.toml");
    let log_path = run_dir.join("vapor.log");
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .map_err(|error| {
            format!(
                "failed to open diagnostics log '{}': {error}",
                log_path.display()
            )
        })?;

    let local = LocalRun {
        run_id: run_id.clone(),
        run_dir: run_dir.clone(),
        metadata_path: metadata_path.clone(),
        log_path: log_path.clone(),
    };
    let metadata = RunMetadata {
        schema: 1,
        run: RunIdentity {
            id: run_id,
            timestamp: format_timestamp(now),
            timestamp_unix_seconds: now.seconds,
            short_random,
        },
        platform: PlatformMetadata {
            os: env::consts::OS.to_owned(),
            arch: env::consts::ARCH.to_owned(),
            family: env::consts::FAMILY.to_owned(),
            platform,
        },
        paths: PathMetadata {
            app_root: display_path(app_root),
            vapor_executable: display_path(executable),
            run_dir: display_path(&run_dir),
            metadata: display_path(&metadata_path),
            log: display_path(&log_path),
        },
        startup: StartupMetadata::default(),
        installation: InstallationMetadata {
            identity_id: installation_identity,
            app_manifest: display_path(&app_root.join(manifest::APP_FILE_NAME)),
        },
        readiness: None,
        source: None,
        launch: LaunchMetadata::default(),
        args,
        steps: Vec::new(),
        errors: Vec::new(),
        upload: UploadMetadata {
            requested: options.upload,
            transport: transport::default_transport().name().to_owned(),
            status: if options.upload {
                "requested".to_owned()
            } else {
                "not-requested".to_owned()
            },
            detail: None,
        },
        exit: None,
    };
    write_metadata(&metadata_path, &metadata)?;
    write_latest_file(diagnostics_root_from_run_dir(&run_dir), &local)?;
    Ok(DiagnosticsRun {
        local,
        options,
        file: Mutex::new(file),
        metadata: Mutex::new(metadata),
    })
}

fn component_metadata(status: &app_local_tools::AppToolComponentStatus) -> ComponentMetadata {
    ComponentMetadata {
        ready: status.installed(),
        path: display_path(status.path()),
        missing: status
            .missing()
            .iter()
            .map(|item| redact_text(item))
            .collect(),
    }
}

fn latest_local_run(app_root: &Path) -> Option<LocalRun> {
    let latest = diagnostics_dir(app_root).join(LATEST_FILE);
    let mut source = String::new();
    File::open(&latest).ok()?.read_to_string(&mut source).ok()?;
    let latest: LatestMetadata = toml::from_str(&source).ok()?;
    let run = LocalRun {
        run_id: latest.run_id,
        run_dir: PathBuf::from(latest.run_dir),
        metadata_path: PathBuf::from(latest.metadata),
        log_path: PathBuf::from(latest.log),
    };
    run.log_path.is_file().then_some(run)
}

fn write_metadata(path: &Path, metadata: &RunMetadata) -> Result<(), String> {
    let source = toml::to_string_pretty(metadata)
        .map_err(|error| format!("failed to serialize diagnostics metadata: {error}"))?;
    fs::write(path, source).map_err(|error| {
        format!(
            "failed to write diagnostics metadata '{}': {error}",
            path.display()
        )
    })
}

fn write_latest_file(diagnostics_root: Option<&Path>, run: &LocalRun) -> Result<(), String> {
    let Some(root) = diagnostics_root else {
        return Ok(());
    };
    fs::create_dir_all(root).map_err(|error| {
        format!(
            "failed to create diagnostics directory '{}': {error}",
            root.display()
        )
    })?;
    let latest = LatestMetadata {
        schema: 1,
        run_id: run.run_id.clone(),
        run_dir: display_path(&run.run_dir),
        metadata: display_path(&run.metadata_path),
        log: display_path(&run.log_path),
        updated_at: timestamp(),
    };
    let source = toml::to_string_pretty(&latest)
        .map_err(|error| format!("failed to serialize diagnostics latest pointer: {error}"))?;
    fs::write(root.join(LATEST_FILE), source).map_err(|error| {
        format!(
            "failed to write diagnostics latest pointer '{}': {error}",
            root.join(LATEST_FILE).display()
        )
    })
}

fn diagnostics_root_from_run_dir(run_dir: &Path) -> Option<&Path> {
    run_dir.parent()?.parent()?.parent()
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

#[derive(Debug, Clone, Copy)]
struct TimeParts {
    seconds: u64,
    nanos: u32,
}

fn now_parts() -> TimeParts {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    TimeParts {
        seconds: duration.as_secs(),
        nanos: duration.subsec_nanos(),
    }
}

fn timestamp() -> String {
    format_timestamp(now_parts())
}

fn format_timestamp(time: TimeParts) -> String {
    format!("{}.{:09}Z", time.seconds, time.nanos)
}

fn utc_date(seconds: u64) -> String {
    let days = (seconds / 86_400) as i64;
    let (year, month, day) = civil_from_days(days);
    format!("{year:04}-{month:02}-{day:02}")
}

fn civil_from_days(days_since_unix_epoch: i64) -> (i32, u32, u32) {
    let z = days_since_unix_epoch + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let mut year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    year += if month <= 2 { 1 } else { 0 };
    (year as i32, month as u32, day as u32)
}

fn platform_label() -> String {
    format!("{}-{}", env::consts::OS, env::consts::ARCH)
}

fn short_run_token(now: TimeParts) -> String {
    let mut bytes = [0_u8; 4];
    if File::open("/dev/urandom")
        .and_then(|mut file| file.read_exact(&mut bytes))
        .is_ok()
    {
        return format!(
            "{:02x}{:02x}{:02x}{:02x}",
            bytes[0], bytes[1], bytes[2], bytes[3]
        );
    }

    sanitize_component(&format!("{:x}{:x}", now.nanos, std::process::id()))
}

fn redacted_args(args: impl IntoIterator<Item = OsString>) -> Vec<String> {
    redact_words(
        args.into_iter()
            .map(|arg| arg.to_string_lossy().into_owned()),
    )
}

fn redact_text(text: &str) -> String {
    redact_words(text.split_whitespace().map(ToOwned::to_owned)).join(" ")
}

fn redact_words(words: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut redact_next = false;
    let mut redacted = Vec::new();
    for word in words {
        if redact_next {
            redacted.push("<redacted>".to_owned());
            redact_next = false;
            continue;
        }
        let (value, consumes_next) = redact_word(&word);
        redacted.push(value);
        redact_next = consumes_next;
    }
    redacted
}

fn redact_word(word: &str) -> (String, bool) {
    if let Some((name, value)) = word.split_once('=') {
        if is_sensitive_name(name.trim_start_matches('-')) {
            return (format!("{name}=<redacted>"), false);
        }
        if is_sensitive_name(value) {
            return (format!("{name}=<redacted>"), false);
        }
    }
    if let Some(name) = word.strip_suffix(':')
        && is_sensitive_name(name.trim_start_matches('-'))
    {
        return (format!("{name}:<redacted>"), true);
    }
    let name = word.trim_start_matches('-');
    if word.starts_with('-') && is_sensitive_name(name) {
        return (word.to_owned(), true);
    }
    (word.to_owned(), false)
}

fn is_sensitive_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    if [
        "password",
        "passwd",
        "token",
        "secret",
        "credential",
        "credentials",
        "cookie",
        "authorization",
        "refresh_token",
        "access_token",
        "authticket",
        "auth_ticket",
        "sessionticket",
        "session_ticket",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
    {
        return true;
    }
    lower
        .split(|character: char| !character.is_ascii_alphanumeric())
        .any(|part| matches!(part, "key" | "auth" | "ticket"))
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

fn display_path(path: &Path) -> String {
    path.display().to_string()
}

fn trim_vec(values: &mut Vec<String>, max: usize) {
    if values.len() > max {
        values.drain(0..(values.len() - max));
    }
}

mod transport {
    use super::LocalRun;

    pub(super) trait DiagnosticsTransport {
        fn name(&self) -> &'static str;
        fn configured(&self) -> bool;
        fn not_configured_message(&self) -> String;
        fn upload(&self, run: &LocalRun, dry_run: bool) -> Result<(), String>;
    }

    pub(super) fn default_transport() -> Box<dyn DiagnosticsTransport> {
        Box::new(FutureHttpServerTransport)
    }

    struct FutureHttpServerTransport;

    impl DiagnosticsTransport for FutureHttpServerTransport {
        fn name(&self) -> &'static str {
            "future-http-server"
        }

        fn configured(&self) -> bool {
            false
        }

        fn not_configured_message(&self) -> String {
            "diagnostics upload transport is not configured in this build; future server upload will attach here"
                .to_owned()
        }

        fn upload(&self, run: &LocalRun, dry_run: bool) -> Result<(), String> {
            if dry_run {
                return Ok(());
            }
            Err(format!(
                "{}; local run is at {}",
                self.not_configured_message(),
                run.run_dir().display()
            ))
        }
    }
}
