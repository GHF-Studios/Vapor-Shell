//! Human-readable rendering for resolved metadata reports.

use super::{
    CargoReport, CargoState, DistributionManifestReport, LocationReport, LocationState,
    MetadataReport, ResourceState, ToolReport, WorkspaceManifestReport,
};
use std::{fmt::Write, path::PathBuf};

impl MetadataReport {
    pub(in crate::metadata) fn render_human(&self) -> String {
        let mut output = String::new();
        writeln!(output, "Vapor metadata (schema {})", self.schema_version).unwrap();
        writeln!(output, "source:").unwrap();
        writeln!(output, "  source:    {}", self.source.source_id).unwrap();
        writeln!(output, "  root:      {}", self.source.root.display()).unwrap();
        writeln!(
            output,
            "  directory: {}",
            self.source.current_directory.display()
        )
        .unwrap();
        match &self.source.content {
            Some(content) => {
                writeln!(output, "  content:   {} ({})", content.id, content.kind).unwrap();
                writeln!(output, "  content root: {}", content.root.display()).unwrap();
            }
            None => writeln!(output, "  content:   none").unwrap(),
        }

        writeln!(output, "installation:").unwrap();
        writeln!(output, "  root:       {}", self.installation.root.display()).unwrap();
        writeln!(
            output,
            "  executable: {}",
            self.installation.executable.display()
        )
        .unwrap();
        writeln!(
            output,
            "  binaries:   {}",
            self.installation.binaries.display()
        )
        .unwrap();
        writeln!(
            output,
            "  libraries:  {}",
            optional_path(self.installation.libraries.as_ref())
        )
        .unwrap();
        writeln!(
            output,
            "  cargo:      {}",
            optional_path(self.installation.bundled_cargo.as_ref())
        )
        .unwrap();
        write_location(&mut output, &self.installation.location);

        writeln!(output, "toolchain:").unwrap();
        write_tool(&mut output, &self.toolchain.rust);
        write_tool(&mut output, &self.toolchain.git);
        write_tool(&mut output, &self.toolchain.steamcmd);
        writeln!(
            output,
            "  install package: {} ({})",
            status_word(self.toolchain.package_complete),
            self.toolchain.package_root.display()
        )
        .unwrap();
        for missing in &self.toolchain.package_missing {
            writeln!(output, "    missing: {missing}").unwrap();
        }

        writeln!(output, "manifests:").unwrap();
        write_workspace_manifest(&mut output, &self.manifests.workspace);
        write_distribution_manifest(&mut output, &self.manifests.distribution);
        write_cargo(&mut output, &self.cargo);

        writeln!(output, "diagnostics: {}", self.diagnostics.len()).unwrap();
        for diagnostic in &self.diagnostics {
            writeln!(
                output,
                "  {}[{}]: {}",
                diagnostic.level, diagnostic.scope, diagnostic.message
            )
            .unwrap();
        }
        output.pop();
        output
    }
}

fn optional_path(path: Option<&PathBuf>) -> String {
    path.map_or_else(|| "none".to_owned(), |path| path.display().to_string())
}

fn status_word(ready: bool) -> &'static str {
    if ready { "ready" } else { "missing" }
}

fn write_location(output: &mut String, location: &LocationReport) {
    match location.status {
        LocationState::Registered => {
            writeln!(
                output,
                "  location:   registered ({})",
                location.current.display()
            )
            .unwrap();
        }
        LocationState::Unregistered => {
            writeln!(
                output,
                "  location:   unregistered ({})",
                location.current.display()
            )
            .unwrap();
        }
        LocationState::Moved => {
            writeln!(output, "  location:   moved").unwrap();
            writeln!(
                output,
                "    previous: {}",
                optional_path(location.registered.as_ref())
            )
            .unwrap();
            writeln!(output, "    current:   {}", location.current.display()).unwrap();
        }
        LocationState::Invalid => {
            writeln!(
                output,
                "  location:   invalid ({})",
                location.error.as_deref().unwrap_or("unknown error")
            )
            .unwrap();
        }
    }
}

fn write_tool(output: &mut String, tool: &ToolReport) {
    writeln!(
        output,
        "  {}: {} ({})",
        tool.label,
        status_word(tool.installed),
        tool.path.display()
    )
    .unwrap();
    for missing in &tool.missing {
        writeln!(output, "    missing: {missing}").unwrap();
    }
}

fn write_workspace_manifest(output: &mut String, report: &WorkspaceManifestReport) {
    match report.status {
        ResourceState::Ready => {
            writeln!(
                output,
                "  workspace: ready ({} Cargo projects)",
                report.projects.len()
            )
            .unwrap();
            for project in &report.projects {
                writeln!(
                    output,
                    "    - {}: {}",
                    project.name,
                    project.manifest.display()
                )
                .unwrap();
            }
        }
        ResourceState::Invalid => {
            writeln!(
                output,
                "  workspace: invalid ({})",
                report.error.as_deref().unwrap_or("unknown error")
            )
            .unwrap();
        }
        ResourceState::Absent => unreachable!("workspace policy is authoritative"),
    }
}

fn write_distribution_manifest(output: &mut String, report: &DistributionManifestReport) {
    match report.status {
        ResourceState::Ready => {
            let application = report.application.as_ref().expect("ready application");
            writeln!(
                output,
                "  distribution: ready (app {}, depot {}, branch {})",
                application.app_id, application.depot_id, application.development_branch
            )
            .unwrap();
        }
        ResourceState::Absent => writeln!(output, "  distribution: not declared").unwrap(),
        ResourceState::Invalid => {
            writeln!(
                output,
                "  distribution: invalid ({})",
                report.error.as_deref().unwrap_or("unknown error")
            )
            .unwrap();
        }
    }
}

fn write_cargo(output: &mut String, report: &CargoReport) {
    writeln!(output, "cargo metadata:").unwrap();
    match report.status {
        CargoState::Absent => writeln!(output, "  status: not applicable").unwrap(),
        CargoState::Unavailable => {
            writeln!(
                output,
                "  status: unavailable ({})",
                report.error.as_deref().unwrap_or("unknown error")
            )
            .unwrap();
        }
        CargoState::Ready => {
            let workspace = report.workspace.as_ref().expect("ready Cargo workspace");
            writeln!(output, "  workspace: {}", workspace.root.display()).unwrap();
            writeln!(
                output,
                "  target:    {}",
                workspace.target_directory.display()
            )
            .unwrap();
            writeln!(output, "  packages:  {}", workspace.packages.len()).unwrap();
            for package in &workspace.packages {
                writeln!(
                    output,
                    "    - {} ({})",
                    package.name,
                    package.manifest.display()
                )
                .unwrap();
                for target in &package.targets {
                    writeln!(
                        output,
                        "      {} [{}]",
                        target.name,
                        target.kinds.join(", ")
                    )
                    .unwrap();
                }
            }
        }
    }
}
