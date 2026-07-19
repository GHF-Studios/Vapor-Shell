//! Human-readable rendering for resolved metadata reports.

use super::{CargoState, Diagnostic, MetadataReport, ResourceState, SourceState};
use std::fmt::Write;

impl MetadataReport {
    pub(in crate::metadata) fn render_human(&self) -> String {
        let mut output = String::new();
        writeln!(output, "Metadata").unwrap();
        writeln!(output).unwrap();
        writeln!(output, "Status").unwrap();
        write_source(&mut output, self);
        write_installation(&mut output, self);
        write_manifests(&mut output, self);
        write_cargo(&mut output, self);
        write_diagnostics(&mut output, self);
        writeln!(output).unwrap();
        writeln!(output, "Next").unwrap();
        writeln!(output, "  {}", next_command(self)).unwrap();
        output.pop();
        output
    }
}

fn write_source(output: &mut String, report: &MetadataReport) {
    match report.source.status {
        SourceState::Open => {
            writeln!(
                output,
                "  Source project: {}",
                report.source.source_id.as_deref().unwrap_or("unknown")
            )
            .unwrap();
            if let Some(kind) = report.manifests.workspace.kind.as_deref() {
                writeln!(output, "    type: {}", source_kind_label(kind)).unwrap();
            }
            if let Some(root) = &report.source.root {
                writeln!(output, "    root: {}", root.display()).unwrap();
            }
            if let Some(content) = &report.source.content {
                writeln!(output, "    content: {} ({})", content.id, content.kind).unwrap();
            }
        }
        SourceState::Closed => {
            writeln!(output, "  Source project: none open").unwrap();
        }
    }
}

fn write_installation(output: &mut String, report: &MetadataReport) {
    writeln!(output, "  App root: {}", report.installation.root.display()).unwrap();
    writeln!(
        output,
        "  Development tools: {}",
        if report.app_local_tools.complete {
            "ready"
        } else {
            "not installed"
        }
    )
    .unwrap();
}

fn write_manifests(output: &mut String, report: &MetadataReport) {
    match report.manifests.workspace.status {
        ResourceState::Ready => {
            writeln!(
                output,
                "  Workspace manifest: ready ({} Cargo workspace{}, {} registered project{})",
                report.manifests.workspace.projects.len(),
                plural(report.manifests.workspace.projects.len()),
                report.manifests.workspace.registered_projects.len(),
                plural(report.manifests.workspace.registered_projects.len())
            )
            .unwrap();
            if !report.manifests.workspace.projects.is_empty() {
                writeln!(output, "    Cargo workspaces:").unwrap();
                for project in &report.manifests.workspace.projects {
                    writeln!(output, "      - {}", project.name).unwrap();
                }
            }
            if !report.manifests.workspace.registered_projects.is_empty() {
                writeln!(output, "    Vapor projects:").unwrap();
                for project in &report.manifests.workspace.registered_projects {
                    writeln!(output, "      - {} ({})", project.name, project.kind).unwrap();
                }
            }
        }
        ResourceState::Absent => writeln!(output, "  Workspace manifest: not open").unwrap(),
        ResourceState::Invalid => {
            writeln!(
                output,
                "  Workspace manifest: invalid ({})",
                report
                    .manifests
                    .workspace
                    .error
                    .as_deref()
                    .unwrap_or("unknown error")
            )
            .unwrap();
        }
    }

    match report.manifests.distribution.status {
        ResourceState::Ready => {
            let application = report
                .manifests
                .distribution
                .application
                .as_ref()
                .expect("ready application");
            writeln!(
                output,
                "  Steam app: {} / branch {}",
                application.app_id, application.development_branch
            )
            .unwrap();
            for depot in &application.depots {
                writeln!(
                    output,
                    "    depot {}: {} ({})",
                    depot.kind, depot.depot_id, depot.steam_os_rule
                )
                .unwrap();
            }
        }
        ResourceState::Absent => {}
        ResourceState::Invalid => {
            writeln!(
                output,
                "  Steam app: invalid ({})",
                report
                    .manifests
                    .distribution
                    .error
                    .as_deref()
                    .unwrap_or("unknown error")
            )
            .unwrap();
        }
    }
}

fn write_cargo(output: &mut String, report: &MetadataReport) {
    match report.cargo.status {
        CargoState::Ready => {
            let workspace = report
                .cargo
                .workspace
                .as_ref()
                .expect("ready Cargo workspace");
            writeln!(
                output,
                "  Cargo metadata: ready ({} package{})",
                workspace.packages.len(),
                plural(workspace.packages.len())
            )
            .unwrap();
        }
        CargoState::Absent => writeln!(output, "  Cargo metadata: not available").unwrap(),
        CargoState::Unavailable => {
            writeln!(
                output,
                "  Cargo metadata: unavailable ({})",
                report.cargo.error.as_deref().unwrap_or("unknown error")
            )
            .unwrap();
        }
    }
}

fn write_diagnostics(output: &mut String, report: &MetadataReport) {
    let diagnostics = human_diagnostics(report);
    if diagnostics.is_empty() {
        writeln!(output, "  Diagnostics: none for normal use").unwrap();
        return;
    }
    writeln!(
        output,
        "  Diagnostics: {} issue{}",
        diagnostics.len(),
        plural(diagnostics.len())
    )
    .unwrap();
    for diagnostic in diagnostics {
        writeln!(
            output,
            "    - {}[{}]: {}",
            diagnostic.level, diagnostic.scope, diagnostic.message
        )
        .unwrap();
    }
}

fn next_command(report: &MetadataReport) -> &'static str {
    if matches!(report.source.status, SourceState::Closed) {
        return "source open /path/to/source";
    }
    if !report.app_local_tools.complete {
        return "vapor-installer dev-env install --app-root <app-root>";
    }
    if matches!(report.manifests.workspace.status, ResourceState::Invalid) {
        return "source status";
    }
    "validate"
}

fn human_diagnostics(report: &MetadataReport) -> Vec<&Diagnostic> {
    report.diagnostics.iter().collect()
}

fn source_kind_label(kind: &str) -> &str {
    match kind {
        "application-root" => "application root",
        "workspace" => "workspace",
        other => other,
    }
}

fn plural(count: usize) -> &'static str {
    if count == 1 { "" } else { "s" }
}
