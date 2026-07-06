//! Shared runtime metadata and command preflight validation.
//!
//! Metadata is resolved once from authoritative `Vapor.toml` policy and
//! replaceable runtime indexes. Commands validate only the capabilities they
//! need, then consume the same resolved manifests and tool state. Reporting a
//! broken optional capability therefore remains possible without blocking an
//! unrelated workflow.

use crate::{
    distribution::DistributionManifest,
    state::ShellState,
    toolchain::{self, LocationStatus, ToolchainStatus},
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
    source_root: PathBuf,
    workspace: Result<WorkspaceManifest, String>,
    distribution: Result<Option<DistributionManifest>, String>,
    location: Result<LocationStatus, String>,
    toolchain: ToolchainStatus,
}

impl ResolvedMetadata {
    /// Resolve source, installation, manifest, Cargo, and toolchain state.
    pub fn resolve(state: &ShellState) -> Self {
        let workspace = WorkspaceManifest::load(state.paths());
        let distribution = DistributionManifest::load_optional(state.paths());
        let location = toolchain::location_status(state.paths().installation());
        let toolchain = toolchain::inspect(state.paths().installation());
        let report = MetadataReport::new(state, &workspace, &distribution, &location, &toolchain);
        Self {
            report,
            source_root: state.paths().source().root().to_path_buf(),
            workspace,
            distribution,
            location,
            toolchain,
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
    /// installed, repaired, finalized, or otherwise changed implicitly.
    ///
    /// # Errors
    ///
    /// Returns a diagnostic explaining the first unmet requirement and the
    /// explicit command that can address it.
    pub fn validate(&self, plan: &ValidationPlan<'_>) -> Result<(), String> {
        if plan.finalized_location {
            let status = self.location.as_ref().map_err(Clone::clone)?;
            toolchain::require_finalized_status(status, plan.action)?;
        }
        if !plan.tools.is_empty() {
            toolchain::require_status(&self.toolchain, &plan.tools, plan.action)?;
        }
        if plan.workspace {
            self.workspace.as_ref().map_err(Clone::clone)?;
        }
        if plan.distribution {
            self.distribution_manifest()?;
        }
        if plan.toolchain_package && !self.toolchain.packages_complete() {
            return Err(format!(
                "cannot {}: the vendored toolchain package is incomplete at '{}'\nmissing package entries:\n  - {}\nhelp: verify the Steam app files or rebuild the bootstrap package",
                plan.action,
                self.toolchain.packages_root().display(),
                self.toolchain.missing_packages().join("\n  - ")
            ));
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
                format!(
                    "source workspace '{}' does not declare [distribution]",
                    self.source_root.display()
                )
            })
    }
}
