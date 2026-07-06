mod common;

use common::TestTree;
use vapor_shell::{
    discovery::EnvironmentPaths,
    metadata::{MetadataFormat, ResolvedMetadata, ValidationPlan},
    state::ShellState,
    toolchain::Requirement,
};

fn fixture() -> (TestTree, TestTree, ShellState) {
    let installation = TestTree::new("metadata-installation");
    installation.write("Vapor.toml", "[workspace]\nid = \"example.installation\"\n");
    let executable = installation.write("bin/vapor", "binary");

    let source = TestTree::new("metadata-source");
    source.write("Vapor.toml", "[workspace]\nid = \"example.source\"\n");

    let paths = EnvironmentPaths::from_paths(&executable, source.root()).unwrap();
    let state = ShellState::new(paths).unwrap();
    (installation, source, state)
}

#[test]
fn metadata_reports_partial_state_in_human_and_json_formats() {
    let (installation, source, state) = fixture();
    let metadata = ResolvedMetadata::resolve(&state);

    let human = metadata.render(MetadataFormat::Human).unwrap();
    assert!(human.contains("workspace: example.source"), "{human}");
    assert!(human.contains("location:   unfinalized"), "{human}");
    assert!(human.contains("distribution: not declared"), "{human}");
    assert!(human.contains("Rust toolchain: missing"), "{human}");

    let json = metadata.render(MetadataFormat::Json).unwrap();
    let json: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(json["schema_version"], 1);
    assert_eq!(json["source"]["workspace_id"], "example.source");
    assert_eq!(json["source"]["root"], source.root().display().to_string());
    assert_eq!(
        json["installation"]["root"],
        installation.root().display().to_string()
    );
    assert_eq!(json["installation"]["location"]["status"], "unfinalized");
    assert_eq!(json["manifests"]["distribution"]["status"], "absent");
    assert!(json["diagnostics"].as_array().unwrap().len() >= 4);
}

#[test]
fn validation_plans_check_only_requested_capabilities() {
    let (_installation, _source, state) = fixture();
    let metadata = ResolvedMetadata::resolve(&state);

    metadata
        .validate(&ValidationPlan::new("inspect metadata").workspace())
        .unwrap();

    let error = metadata
        .validate(&ValidationPlan::new("build projects").finalized_location())
        .unwrap_err();
    assert!(error.contains("toolchain finalize"), "{error}");

    let error = metadata
        .validate(&ValidationPlan::new("authenticate").tools(&[Requirement::SteamCmd]))
        .unwrap_err();
    assert!(error.contains("SteamCMD"), "{error}");

    let error = metadata
        .validate(&ValidationPlan::new("publish").distribution())
        .unwrap_err();
    assert!(error.contains("does not declare [distribution]"), "{error}");
}
