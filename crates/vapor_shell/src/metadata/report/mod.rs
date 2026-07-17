//! Serializable metadata report model.
//!
use crate::{
    cargo_metadata::CargoIndex,
    distribution::DistributionManifest,
    setup_self::{LocationStatus, SetupSelfComponentStatus, SetupSelfStatus},
    state::ShellState,
    workspace::{SourceRootKind, WorkspaceManifest},
};
use serde::Serialize;
use std::path::PathBuf;

mod render;

/// Serializable metadata returned by the public metadata facade.
#[derive(Debug, Clone, Serialize)]
pub struct MetadataReport {
    schema_version: u32,
    source: SourceReport,
    installation: InstallationReport,
    setup_self: SetupSelfReport,
    manifests: ManifestReport,
    cargo: CargoReport,
    diagnostics: Vec<Diagnostic>,
}

impl MetadataReport {
    pub(super) fn new(
        state: &ShellState,
        workspace: &Result<WorkspaceManifest, String>,
        distribution: &Result<Option<DistributionManifest>, String>,
        location: &Result<LocationStatus, String>,
        setup_self: &SetupSelfStatus,
    ) -> Self {
        let source = SourceReport::new(state);
        let installation = InstallationReport::new(state, location);
        let setup_self_report = SetupSelfReport::new(setup_self);
        let manifests = ManifestReport::new(workspace, distribution);
        let cargo = CargoReport::new(state.cargo_index());
        let diagnostics = diagnostics(location, setup_self, workspace, distribution, &cargo);
        Self {
            schema_version: 1,
            source,
            installation,
            setup_self: setup_self_report,
            manifests,
            cargo,
            diagnostics,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct SourceReport {
    status: SourceState,
    #[serde(skip_serializing_if = "Option::is_none")]
    source_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    root: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    current_directory: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<ContentReport>,
}

impl SourceReport {
    fn new(state: &ShellState) -> Self {
        let source = state.source();
        Self {
            status: if source.is_some() {
                SourceState::Open
            } else {
                SourceState::Closed
            },
            source_id: source.map(|source| source.id().to_owned()),
            root: source.map(|source| source.root().to_path_buf()),
            current_directory: state.current_dir().ok().map(PathBuf::from),
            content: state.content().map(|content| ContentReport {
                id: content.id().to_owned(),
                kind: content.kind().to_string(),
                root: content.root().to_path_buf(),
            }),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
enum SourceState {
    Open,
    Closed,
}

#[derive(Debug, Clone, Serialize)]
struct ContentReport {
    id: String,
    kind: String,
    root: PathBuf,
}

#[derive(Debug, Clone, Serialize)]
struct InstallationReport {
    root: PathBuf,
    executable: PathBuf,
    binaries: PathBuf,
    #[serde(skip_serializing_if = "Option::is_none")]
    libraries: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    bundled_cargo: Option<PathBuf>,
    location: LocationReport,
}

impl InstallationReport {
    fn new(state: &ShellState, location: &Result<LocationStatus, String>) -> Self {
        let installation = state.installation();
        Self {
            root: installation.root().to_path_buf(),
            executable: installation.executable().to_path_buf(),
            binaries: installation.binaries().to_path_buf(),
            libraries: installation.libraries().map(PathBuf::from),
            bundled_cargo: installation.bundled_cargo(),
            location: LocationReport::new(location),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct LocationReport {
    status: LocationState,
    current: PathBuf,
    #[serde(skip_serializing_if = "Option::is_none")]
    registered: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

impl LocationReport {
    fn new(location: &Result<LocationStatus, String>) -> Self {
        match location {
            Ok(LocationStatus::Unregistered { current }) => Self {
                status: LocationState::Unregistered,
                current: current.clone(),
                registered: None,
                error: None,
            },
            Ok(LocationStatus::Registered { path }) => Self {
                status: LocationState::Registered,
                current: path.clone(),
                registered: Some(path.clone()),
                error: None,
            },
            Ok(LocationStatus::Moved { locked, current }) => Self {
                status: LocationState::Moved,
                current: current.clone(),
                registered: Some(locked.clone()),
                error: None,
            },
            Err(error) => Self {
                status: LocationState::Invalid,
                current: PathBuf::new(),
                registered: None,
                error: Some(error.clone()),
            },
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
enum LocationState {
    Unregistered,
    Registered,
    Moved,
    Invalid,
}

#[derive(Debug, Clone, Serialize)]
struct SetupSelfReport {
    complete: bool,
    rust: SetupSelfComponentReport,
    git: SetupSelfComponentReport,
    steamcmd: SetupSelfComponentReport,
    package: PackageReport,
}

impl SetupSelfReport {
    fn new(status: &SetupSelfStatus) -> Self {
        Self {
            complete: status.complete(),
            rust: SetupSelfComponentReport::new(status.rust()),
            git: SetupSelfComponentReport::new(status.git()),
            steamcmd: SetupSelfComponentReport::new(status.steamcmd()),
            package: PackageReport {
                complete: status.package_complete(),
                root: status.package_root().to_path_buf(),
                missing: status.missing_package_entries().to_vec(),
            },
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct SetupSelfComponentReport {
    label: String,
    installed: bool,
    path: PathBuf,
    missing: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct PackageReport {
    complete: bool,
    root: PathBuf,
    missing: Vec<String>,
}

impl SetupSelfComponentReport {
    fn new(status: &SetupSelfComponentStatus) -> Self {
        Self {
            label: status.label().to_owned(),
            installed: status.installed(),
            path: status.path().to_path_buf(),
            missing: status.missing().to_vec(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct ManifestReport {
    workspace: WorkspaceManifestReport,
    distribution: DistributionManifestReport,
}

impl ManifestReport {
    fn new(
        workspace: &Result<WorkspaceManifest, String>,
        distribution: &Result<Option<DistributionManifest>, String>,
    ) -> Self {
        Self {
            workspace: WorkspaceManifestReport::new(workspace),
            distribution: DistributionManifestReport::new(distribution),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct WorkspaceManifestReport {
    status: ResourceState,
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    kind: Option<String>,
    projects: Vec<ProjectReport>,
    registered_projects: Vec<RegisteredProjectReport>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

impl WorkspaceManifestReport {
    fn new(workspace: &Result<WorkspaceManifest, String>) -> Self {
        match workspace {
            Ok(workspace) => Self {
                status: ResourceState::Ready,
                id: Some(workspace.id().to_owned()),
                kind: Some(
                    match workspace.kind() {
                        SourceRootKind::Root => "application-root",
                        SourceRootKind::Workspace => "workspace",
                    }
                    .to_owned(),
                ),
                projects: workspace
                    .cargo_projects()
                    .iter()
                    .map(|project| ProjectReport {
                        name: project.name().to_owned(),
                        manifest: project.manifest().to_path_buf(),
                        documentation: project.documentation(),
                        binaries: project.binaries().to_vec(),
                    })
                    .collect(),
                registered_projects: workspace
                    .projects()
                    .iter()
                    .map(|project| RegisteredProjectReport {
                        id: project.id().to_owned(),
                        name: project.name().to_owned(),
                        kind: project.kind().to_string(),
                        path: project.path().to_path_buf(),
                        manifest: project.manifest().to_path_buf(),
                    })
                    .collect(),
                error: None,
            },
            Err(error) => Self {
                status: ResourceState::Invalid,
                id: None,
                kind: None,
                projects: Vec::new(),
                registered_projects: Vec::new(),
                error: Some(error.clone()),
            },
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct ProjectReport {
    name: String,
    manifest: PathBuf,
    documentation: bool,
    binaries: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct RegisteredProjectReport {
    id: String,
    name: String,
    kind: String,
    path: PathBuf,
    manifest: PathBuf,
}

#[derive(Debug, Clone, Serialize)]
struct DistributionManifestReport {
    status: ResourceState,
    #[serde(skip_serializing_if = "Option::is_none")]
    application: Option<ApplicationReport>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

impl DistributionManifestReport {
    fn new(distribution: &Result<Option<DistributionManifest>, String>) -> Self {
        match distribution {
            Ok(Some(distribution)) => Self {
                status: ResourceState::Ready,
                application: Some(ApplicationReport {
                    app_id: distribution.application().app_id(),
                    depot_id: distribution.application().depot_id(),
                    development_branch: distribution.application().development_branch().to_owned(),
                }),
                error: None,
            },
            Ok(None) => Self {
                status: ResourceState::Absent,
                application: None,
                error: None,
            },
            Err(error) => Self {
                status: ResourceState::Invalid,
                application: None,
                error: Some(error.clone()),
            },
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct ApplicationReport {
    app_id: u32,
    depot_id: u32,
    development_branch: String,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
enum ResourceState {
    Ready,
    Absent,
    Invalid,
}

#[derive(Debug, Clone, Serialize)]
struct CargoReport {
    status: CargoState,
    #[serde(skip_serializing_if = "Option::is_none")]
    workspace: Option<CargoWorkspaceReport>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

impl CargoReport {
    fn new(index: &CargoIndex) -> Self {
        match index {
            CargoIndex::NotPresent => Self {
                status: CargoState::Absent,
                workspace: None,
                error: None,
            },
            CargoIndex::Unavailable(error) => Self {
                status: CargoState::Unavailable,
                workspace: None,
                error: Some(error.clone()),
            },
            CargoIndex::Loaded(workspace) => Self {
                status: CargoState::Ready,
                workspace: Some(CargoWorkspaceReport {
                    root: workspace.root().to_path_buf(),
                    target_directory: workspace.target_directory().to_path_buf(),
                    packages: workspace
                        .packages()
                        .iter()
                        .map(|package| CargoPackageReport {
                            name: package.name().to_owned(),
                            manifest: package.manifest_path().to_path_buf(),
                            targets: package
                                .targets()
                                .iter()
                                .map(|target| CargoTargetReport {
                                    name: target.name().to_owned(),
                                    kinds: target.kinds().to_vec(),
                                })
                                .collect(),
                        })
                        .collect(),
                }),
                error: None,
            },
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
enum CargoState {
    Ready,
    Absent,
    Unavailable,
}

#[derive(Debug, Clone, Serialize)]
struct CargoWorkspaceReport {
    root: PathBuf,
    target_directory: PathBuf,
    packages: Vec<CargoPackageReport>,
}

#[derive(Debug, Clone, Serialize)]
struct CargoPackageReport {
    name: String,
    manifest: PathBuf,
    targets: Vec<CargoTargetReport>,
}

#[derive(Debug, Clone, Serialize)]
struct CargoTargetReport {
    name: String,
    kinds: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct Diagnostic {
    level: DiagnosticLevel,
    scope: &'static str,
    message: String,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
enum DiagnosticLevel {
    Warning,
    Error,
}

impl std::fmt::Display for DiagnosticLevel {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Warning => "warning",
            Self::Error => "error",
        })
    }
}

fn diagnostics(
    location: &Result<LocationStatus, String>,
    setup_self: &SetupSelfStatus,
    workspace: &Result<WorkspaceManifest, String>,
    distribution: &Result<Option<DistributionManifest>, String>,
    cargo: &CargoReport,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    match location {
        Ok(LocationStatus::Unregistered { current }) => diagnostics.push(Diagnostic {
            level: DiagnosticLevel::Warning,
            scope: "vapor_home",
            message: format!(
                "'{}' has not been accepted; review `vapor setup self status`, then run `vapor setup self install`",
                current.display()
            ),
        }),
        Ok(LocationStatus::Moved { locked, current }) => diagnostics.push(Diagnostic {
            level: DiagnosticLevel::Warning,
            scope: "vapor_home",
            message: format!(
                "accepted location '{}' differs from current location '{}'; run `vapor setup self repair` if the move was intentional, or restore the app",
                locked.display(),
                current.display()
            ),
        }),
        Err(error) => diagnostics.push(Diagnostic {
            level: DiagnosticLevel::Error,
            scope: "vapor_home",
            message: error.clone(),
        }),
        Ok(LocationStatus::Registered { .. }) => {}
    }
    for status in [setup_self.rust(), setup_self.git(), setup_self.steamcmd()] {
        if !status.installed() {
            diagnostics.push(Diagnostic {
                level: DiagnosticLevel::Warning,
                scope: "setup_self",
                message: format!(
                    "{} is incomplete at {}: missing {}",
                    status.label(),
                    status.path().display(),
                    status.missing().join(", ")
                ),
            });
        }
    }
    if !setup_self.package_complete() {
        diagnostics.push(Diagnostic {
            level: DiagnosticLevel::Warning,
            scope: "setup_self_package",
            message: format!(
                "distributable self-setup payload is incomplete: {}; run `vapor setup self package install` after active tools are healthy",
                setup_self.missing_package_entries().join(", ")
            ),
        });
    }
    if let Err(error) = workspace {
        diagnostics.push(Diagnostic {
            level: DiagnosticLevel::Error,
            scope: if error.contains("no Vapor source is open") {
                "source"
            } else {
                "workspace_manifest"
            },
            message: error.clone(),
        });
    }
    if let Err(error) = distribution {
        diagnostics.push(Diagnostic {
            level: DiagnosticLevel::Error,
            scope: "distribution_manifest",
            message: error.clone(),
        });
    }
    if let Some(error) = &cargo.error {
        diagnostics.push(Diagnostic {
            level: DiagnosticLevel::Warning,
            scope: "cargo_metadata",
            message: error.clone(),
        });
    }
    diagnostics
}
