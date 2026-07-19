mod common;

use common::TestTree;
use vapor_shell::{
    app_local_tools::AppToolRequirement,
    discovery::EnvironmentPaths,
    metadata::{MetadataFormat, ResolvedMetadata, ValidationPlan},
    state::ShellState,
};

fn sample_metadata_state() -> (TestTree, TestTree, ShellState) {
    let installation = TestTree::new("metadata-installation");
    installation.write(
        "App.vapor.toml",
        "[root]\nname = \"installation\"\norganization = \"example\"\n",
    );
    let executable = installation.write("bin/vapor", "binary");

    let source = TestTree::new("metadata-source");
    source.write(
        "Workspace.vapor.toml",
        "[workspace]\nname = \"source\"\norganization = \"example\"\n",
    );
    source.write("Cargo.toml", "[workspace]\nresolver = \"3\"\n");

    let paths = EnvironmentPaths::from_paths(&executable, source.root()).unwrap();
    let state = ShellState::new(paths).unwrap();
    (installation, source, state)
}

#[test]
fn metadata_reports_partial_state_in_human_and_json_formats() {
    let (installation, source, state) = sample_metadata_state();
    let metadata = ResolvedMetadata::resolve(&state);

    let human = metadata.render(MetadataFormat::Human).unwrap();
    assert!(human.contains("Metadata"), "{human}");
    assert!(human.contains("Source project: example/source"), "{human}");
    assert!(human.contains("App root:"), "{human}");
    assert!(
        human.contains("Development tools: not installed"),
        "{human}"
    );
    assert!(human.contains("Workspace manifest: ready"), "{human}");
    assert!(
        human.contains("Next\n  vapor-installer dev-env install --app-root <app-root>"),
        "{human}"
    );

    let json = metadata.render(MetadataFormat::Json).unwrap();
    let json: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(json["schema_version"], 1);
    assert_eq!(json["source"]["source_id"], "example/source");
    assert_eq!(json["source"]["root"], source.root().display().to_string());
    assert_eq!(
        json["installation"]["root"],
        installation.root().display().to_string()
    );
    assert!(json["installation"].get("location").is_none(), "{json}");
    assert!(json.get("app_local_tools").is_some(), "{json}");
    assert_eq!(json["app_local_tools"]["rust"]["label"], "Rust/Cargo");
    assert_eq!(json["manifests"]["distribution"]["status"], "absent");
    assert!(json["diagnostics"].as_array().unwrap().len() >= 4);
}

#[test]
fn validation_plans_check_only_requested_capabilities() {
    let (_installation, _source, state) = sample_metadata_state();
    let metadata = ResolvedMetadata::resolve(&state);

    metadata
        .validate(&ValidationPlan::new("inspect metadata").workspace())
        .unwrap();

    let error = metadata
        .validate(
            &ValidationPlan::new("build projects").app_local_tools(&[AppToolRequirement::Rust]),
        )
        .unwrap_err();
    assert!(error.contains("vapor-installer dev-env install"), "{error}");

    let error = metadata
        .validate(
            &ValidationPlan::new("authenticate").app_local_tools(&[AppToolRequirement::SteamCmd]),
        )
        .unwrap_err();
    assert!(error.contains("SteamCMD"), "{error}");

    let error = metadata
        .validate(&ValidationPlan::new("publish").distribution())
        .unwrap_err();
    assert!(error.contains("does not declare [root.steam]"), "{error}");
}
