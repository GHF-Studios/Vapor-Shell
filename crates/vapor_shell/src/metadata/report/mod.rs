//! Serializable metadata report model.
//!
use crate::{
    cargo_metadata::CargoIndex,
    distribution::DistributionManifest,
    state::ShellState,
    toolchain::{LocationStatus, ToolStatus, ToolchainStatus},
    workspace::WorkspaceManifest,
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
    toolchain: ToolchainReport,
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
        toolchain: &ToolchainStatus,
    ) -> Self {
        let source = SourceReport::new(state);
        let installation = InstallationReport::new(state, location);
        let toolchain_report = ToolchainReport::new(toolchain);
        let manifests = ManifestReport::new(workspace, distribution);
        let cargo = CargoReport::new(state.cargo_index());
        let diagnostics = diagnostics(location, toolchain, workspace, distribution, &cargo);
        Self {
            schema_version: 1,
            source,
            installation,
            toolchain: toolchain_report,
            manifests,
            cargo,
            diagnostics,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct SourceReport {
    workspace_id: String,
    root: PathBuf,
    current_directory: PathBuf,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<ContentReport>,
}

impl SourceReport {
    fn new(state: &ShellState) -> Self {
        Self {
            workspace_id: state.workspace().id().to_owned(),
            root: state.workspace().root().to_path_buf(),
            current_directory: state.current_dir().to_path_buf(),
            content: state.content().map(|content| ContentReport {
                id: content.id().to_owned(),
                kind: content.kind().to_string(),
                root: content.root().to_path_buf(),
            }),
        }
    }
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
        let installation = state.paths().installation();
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
    finalized: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

impl LocationReport {
    fn new(location: &Result<LocationStatus, String>) -> Self {
        match location {
            Ok(LocationStatus::Unfinalized { current }) => Self {
                status: LocationState::Unfinalized,
                current: current.clone(),
                finalized: None,
                error: None,
            },
            Ok(LocationStatus::Finalized { path }) => Self {
                status: LocationState::Finalized,
                current: path.clone(),
                finalized: Some(path.clone()),
                error: None,
            },
            Ok(LocationStatus::Moved { locked, current }) => Self {
                status: LocationState::Moved,
                current: current.clone(),
                finalized: Some(locked.clone()),
                error: None,
            },
            Err(error) => Self {
                status: LocationState::Invalid,
                current: PathBuf::new(),
                finalized: None,
                error: Some(error.clone()),
            },
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
enum LocationState {
    Unfinalized,
    Finalized,
    Moved,
    Invalid,
}

#[derive(Debug, Clone, Serialize)]
struct ToolchainReport {
    complete: bool,
    rust: ToolReport,
    git: ToolReport,
    steamcmd: ToolReport,
    package_complete: bool,
    package_root: PathBuf,
    package_missing: Vec<String>,
}

impl ToolchainReport {
    fn new(status: &ToolchainStatus) -> Self {
        Self {
            complete: status.complete(),
            rust: ToolReport::new(status.rust()),
            git: ToolReport::new(status.git()),
            steamcmd: ToolReport::new(status.steamcmd()),
            package_complete: status.packages_complete(),
            package_root: status.packages_root().to_path_buf(),
            package_missing: status.missing_packages().to_vec(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct ToolReport {
    label: String,
    installed: bool,
    path: PathBuf,
    missing: Vec<String>,
}

impl ToolReport {
    fn new(status: &ToolStatus) -> Self {
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
    projects: Vec<ProjectReport>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

impl WorkspaceManifestReport {
    fn new(workspace: &Result<WorkspaceManifest, String>) -> Self {
        match workspace {
            Ok(workspace) => Self {
                status: ResourceState::Ready,
                id: Some(workspace.id().to_owned()),
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
                error: None,
            },
            Err(error) => Self {
                status: ResourceState::Invalid,
                id: None,
                projects: Vec::new(),
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
    toolchain: &ToolchainStatus,
    workspace: &Result<WorkspaceManifest, String>,
    distribution: &Result<Option<DistributionManifest>, String>,
    cargo: &CargoReport,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    match location {
        Ok(LocationStatus::Unfinalized { current }) => diagnostics.push(Diagnostic {
            level: DiagnosticLevel::Warning,
            scope: "vapor_home",
            message: format!(
                "'{}' has not been accepted; review `vapor toolchain status`, then run `vapor toolchain finalize`",
                current.display()
            ),
        }),
        Ok(LocationStatus::Moved { locked, current }) => diagnostics.push(Diagnostic {
            level: DiagnosticLevel::Warning,
            scope: "vapor_home",
            message: format!(
                "finalized location '{}' differs from current location '{}'; explicitly finalize the move or restore the app",
                locked.display(),
                current.display()
            ),
        }),
        Err(error) => diagnostics.push(Diagnostic {
            level: DiagnosticLevel::Error,
            scope: "vapor_home",
            message: error.clone(),
        }),
        Ok(LocationStatus::Finalized { .. }) => {}
    }
    for status in [toolchain.rust(), toolchain.git(), toolchain.steamcmd()] {
        if !status.installed() {
            diagnostics.push(Diagnostic {
                level: DiagnosticLevel::Warning,
                scope: "toolchain",
                message: format!(
                    "{} is missing: {} (expected under {})",
                    status.label(),
                    status.missing().join(", "),
                    status.path().display()
                ),
            });
        }
    }
    if !toolchain.packages_complete() {
        diagnostics.push(Diagnostic {
            level: DiagnosticLevel::Warning,
            scope: "toolchain_package",
            message: format!(
                "vendored installation package is incomplete: {}",
                toolchain.missing_packages().join(", ")
            ),
        });
    }
    if let Err(error) = workspace {
        diagnostics.push(Diagnostic {
            level: DiagnosticLevel::Error,
            scope: "workspace_manifest",
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
