//! Shared runtime metadata and command preflight validation.
//!
//! Metadata is resolved once from authoritative `Vapor.toml` policy and
//! app-owned runtime indexes. Commands validate only the capabilities they
//! need, then consume the same resolved manifests and tool state. Reporting a
//! broken capability therefore remains possible without blocking an unrelated
//! workflow.

use crate::{
    distribution::DistributionManifest,
    setup::{self, LocationStatus, SetupStatus},
    state::ShellState,
    workspace::WorkspaceManifest,
};
use clap::ValueEnum;
use std::path::PathBuf;

mod report;
mod validation;

pub use report::MetadataReport;
pub use validation::ValidationPlan;

/// Output representation supported by `vapor metadata`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, ValueEnum)]
pub enum MetadataFormat {
    /// Concise output intended for people.
    #[default]
    Human,
    /// Stable structured output intended for scripts and agents.
    Json,
}

/// One coherent snapshot of the active Vapor environment.
#[derive(Debug, Clone)]
pub struct ResolvedMetadata {
    report: MetadataReport,
    source_root: Option<PathBuf>,
    workspace: Result<WorkspaceManifest, String>,
    distribution: Result<Option<DistributionManifest>, String>,
    location: Result<LocationStatus, String>,
    setup: SetupStatus,
}

impl ResolvedMetadata {
    /// Resolve source, installation, manifest, Cargo, and setup state.
    pub fn resolve(state: &ShellState) -> Self {
        let active_paths = state.active_paths();
        let workspace = active_paths.as_ref().map_or_else(
            |error| Err(error.clone()),
            |paths| WorkspaceManifest::load(paths),
        );
        let distribution = active_paths.as_ref().map_or_else(
            |error| Err(error.clone()),
            |paths| DistributionManifest::load_optional(paths),
        );
        let location = setup::location_status(state.installation());
        let setup_status = setup::inspect(state.installation());
        let report =
            MetadataReport::new(state, &workspace, &distribution, &location, &setup_status);
        Self {
            report,
            source_root: state
                .active_paths()
                .ok()
                .map(|paths| paths.source().root().to_path_buf()),
            workspace,
            distribution,
            location,
            setup: setup_status,
        }
    }

    /// Serializable, side-effect-free report derived from this snapshot.
    pub fn report(&self) -> &MetadataReport {
        &self.report
    }

    /// Render the complete snapshot in the requested representation.
    ///
    /// # Errors
    ///
    /// JSON rendering fails only if the serializable report cannot be encoded.
    pub fn render(&self, format: MetadataFormat) -> Result<String, String> {
        match format {
            MetadataFormat::Human => Ok(self.report.render_human()),
            MetadataFormat::Json => serde_json::to_string_pretty(&self.report)
                .map_err(|error| format!("failed to encode Vapor metadata as JSON: {error}")),
        }
    }

    /// Validate the capabilities required by one command.
    ///
    /// Checks stop at the first actionable prerequisite. No missing resource is
    /// installed, repaired, accepted, or otherwise changed implicitly.
    ///
    /// # Errors
    ///
    /// Returns a diagnostic explaining the first unmet requirement and the
    /// explicit command that can address it.
    pub fn validate(&self, plan: &ValidationPlan<'_>) -> Result<(), String> {
        if plan.registered_location {
            let status = self.location.as_ref().map_err(Clone::clone)?;
            setup::require_registered_status(status, plan.action)?;
        }
        if !plan.setup.is_empty() {
            setup::require_status(&self.setup, &plan.setup, plan.action)?;
        }
        if plan.workspace {
            self.workspace.as_ref().map_err(Clone::clone)?;
        }
        if plan.distribution {
            self.distribution_manifest()?;
        }
        Ok(())
    }

    /// Validated root workspace policy from this snapshot.
    ///
    /// # Errors
    ///
    /// Returns the workspace parsing or policy error captured during resolution.
    pub fn workspace_manifest(&self) -> Result<&WorkspaceManifest, String> {
        self.workspace.as_ref().map_err(Clone::clone)
    }

    /// App-local Rust, Git, and SteamCMD status from this snapshot.
    pub fn setup_status(&self) -> &SetupStatus {
        &self.setup
    }

    /// Validated Steam distribution policy from this snapshot.
    ///
    /// # Errors
    ///
    /// Returns the parsing error or explains that this workspace does not
    /// declare a self-hosting distribution.
    pub fn distribution_manifest(&self) -> Result<&DistributionManifest, String> {
        self.distribution
            .as_ref()
            .map_err(Clone::clone)?
            .as_ref()
            .ok_or_else(|| {
                if let Some(source_root) = &self.source_root {
                    format!(
                        "source root '{}' does not declare [root.steam]",
                        source_root.display()
                    )
                } else {
                    "cannot load distribution policy: no Vapor source is open\nhelp: open an application source root with `open NAME` or `open PATH`"
                        .to_owned()
                }
            })
    }
}
