//! Serializable metadata report model.
//!
use crate::{
    app_local_tools::{AppToolComponentStatus, AppToolStatus},
    cargo_metadata::CargoIndex,
    distribution::{DistributionManifest, SteamDepotKind},
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
    app_local_tools: AppToolReport,
    manifests: ManifestReport,
    cargo: CargoReport,
    diagnostics: Vec<Diagnostic>,
}

impl MetadataReport {
    pub(super) fn new(
        state: &ShellState,
        workspace: &Result<WorkspaceManifest, String>,
        distribution: &Result<Option<DistributionManifest>, String>,
        app_local_tools: &AppToolStatus,
    ) -> Self {
        let source = SourceReport::new(state);
        let installation = InstallationReport::new(state);
        let app_local_tools_report = AppToolReport::new(app_local_tools);
        let manifests = ManifestReport::new(workspace, distribution);
        let cargo = CargoReport::new(state.cargo_index());
        let diagnostics = diagnostics(app_local_tools, workspace, distribution, &cargo);
        Self {
            schema_version: 1,
            source,
            installation,
            app_local_tools: app_local_tools_report,
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
}

impl InstallationReport {
    fn new(state: &ShellState) -> Self {
        let installation = state.installation();
        Self {
            root: installation.root().to_path_buf(),
            executable: installation.executable().to_path_buf(),
            binaries: installation.binaries().to_path_buf(),
            libraries: installation.libraries().map(PathBuf::from),
            bundled_cargo: installation.bundled_cargo(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct AppToolReport {
    complete: bool,
    rust: AppToolComponentReport,
    cross_toolchains: AppToolComponentReport,
    steamcmd: AppToolComponentReport,
}

impl AppToolReport {
    fn new(status: &AppToolStatus) -> Self {
        Self {
            complete: status.complete(),
            rust: AppToolComponentReport::new(status.rust()),
            cross_toolchains: AppToolComponentReport::new(status.cross_toolchains()),
            steamcmd: AppToolComponentReport::new(status.steamcmd()),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct AppToolComponentReport {
    label: String,
    installed: bool,
    path: PathBuf,
    missing: Vec<String>,
}

impl AppToolComponentReport {
    fn new(status: &AppToolComponentStatus) -> Self {
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
                    depots: [
                        SteamDepotKind::Common,
                        SteamDepotKind::Linux,
                        SteamDepotKind::Windows,
                    ]
                    .into_iter()
                    .map(|kind| SteamDepotReport {
                        kind: kind.label().to_owned(),
                        depot_id: distribution.application().depot_id(kind),
                        steam_os_rule: kind.steam_os_rule().to_owned(),
                    })
                    .collect(),
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
    depots: Vec<SteamDepotReport>,
    development_branch: String,
}

#[derive(Debug, Clone, Serialize)]
struct SteamDepotReport {
    kind: String,
    depot_id: u32,
    steam_os_rule: String,
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
    app_local_tools: &AppToolStatus,
    workspace: &Result<WorkspaceManifest, String>,
    distribution: &Result<Option<DistributionManifest>, String>,
    cargo: &CargoReport,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for status in [
        app_local_tools.rust(),
        app_local_tools.cross_toolchains(),
        app_local_tools.steamcmd(),
    ] {
        if !status.installed() {
            diagnostics.push(Diagnostic {
                level: DiagnosticLevel::Warning,
                scope: "app_local_tools",
                message: format!(
                    "{} is incomplete at {}: missing {}",
                    status.label(),
                    status.path().display(),
                    status.missing().join(", ")
                ),
            });
        }
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
