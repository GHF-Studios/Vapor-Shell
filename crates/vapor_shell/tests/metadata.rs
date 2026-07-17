mod common;

use common::TestTree;
use vapor_shell::{
    discovery::EnvironmentPaths,
    metadata::{MetadataFormat, ResolvedMetadata, ValidationPlan},
    setup_self::SetupSelfRequirement,
    state::ShellState,
};

fn fixture() -> (TestTree, TestTree, ShellState) {
    let installation = TestTree::new("metadata-installation");
    installation.write(
        "Vapor.toml",
        "[root]\nname = \"installation\"\norganization = \"example\"\n",
    );
    let executable = installation.write("bin/vapor", "binary");

    let source = TestTree::new("metadata-source");
    source.write(
        "Vapor.toml",
        "[workspace]\nname = \"source\"\norganization = \"example\"\n",
    );
    source.write("Cargo.toml", "[workspace]\nresolver = \"3\"\n");

    let paths = EnvironmentPaths::from_paths(&executable, source.root()).unwrap();
    let state = ShellState::new(paths).unwrap();
    (installation, source, state)
}

#[test]
fn metadata_reports_partial_state_in_human_and_json_formats() {
    let (installation, source, state) = fixture();
    let metadata = ResolvedMetadata::resolve(&state);

    let human = metadata.render(MetadataFormat::Human).unwrap();
    assert!(human.contains("Metadata"), "{human}");
    assert!(human.contains("Source project: example/source"), "{human}");
    assert!(
        human.contains("Install location: not confirmed yet"),
        "{human}"
    );
    assert!(human.contains("Local tools: not installed"), "{human}");
    assert!(human.contains("Workspace manifest: ready"), "{human}");
    assert!(human.contains("Next\n  setup self install"), "{human}");

    let json = metadata.render(MetadataFormat::Json).unwrap();
    let json: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(json["schema_version"], 1);
    assert_eq!(json["source"]["source_id"], "example/source");
    assert_eq!(json["source"]["root"], source.root().display().to_string());
    assert_eq!(
        json["installation"]["root"],
        installation.root().display().to_string()
    );
    assert_eq!(json["installation"]["location"]["status"], "unregistered");
    assert!(json.get("setup_self").is_some(), "{json}");
    assert_eq!(json["setup_self"]["rust"]["label"], "Rust/Cargo");
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
        .validate(&ValidationPlan::new("build projects").registered_location())
        .unwrap_err();
    assert!(error.contains("setup self install"), "{error}");

    let error = metadata
        .validate(
            &ValidationPlan::new("authenticate").setup_self(&[SetupSelfRequirement::SteamCmd]),
        )
        .unwrap_err();
    assert!(error.contains("SteamCMD"), "{error}");

    let error = metadata
        .validate(&ValidationPlan::new("publish").distribution())
        .unwrap_err();
    assert!(error.contains("does not declare [root.steam]"), "{error}");
}
