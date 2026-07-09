mod common;

use common::TestTree;
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use vapor_shell::{
    discovery::EnvironmentPaths,
    workflow::{self, CargoWorkflow, ProjectSelection},
    workspace::WorkspaceManifest,
};

#[cfg(unix)]
#[test]
fn test_workflow_uses_installed_cargo_and_app_owned_output() {
    let installation = TestTree::new("workflow-installation");
    installation.write(
        "Vapor.toml",
        "[root]\nname = \"installation\"\norganization = \"example\"\n",
    );
    let executable = installation.write("bin/vapor", "binary");
    let cargo = installation.write(
        "rustup-home/toolchains/test-host/bin/cargo",
        "#!/bin/sh\nprintf '%s\\n' \"$@\" > \"$VAPOR_HOME/workflow-args\"\nprintf '%s\\n' \"$CARGO_HOME\" > \"$VAPOR_HOME/workflow-cargo-home\"\nprintf '%s\\n' \"$CARGO_TARGET_DIR\" > \"$VAPOR_HOME/workflow-target\"\n",
    );
    let mut permissions = fs::metadata(&cargo).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&cargo, permissions).unwrap();

    let source = TestTree::new("workflow-source");
    source.write(
        "Vapor.toml",
        "[workspace]\nname = \"workflow-source\"\norganization = \"example\"\n",
    );
    let cargo_manifest = source.write("Cargo.toml", "[workspace]\nresolver = \"3\"\n");

    let paths = EnvironmentPaths::from_paths(&executable, source.root()).unwrap();
    let manifest = WorkspaceManifest::load(&paths).unwrap();
    workflow::run(
        &paths,
        &manifest,
        ProjectSelection::One("workflow-source".to_owned()),
        CargoWorkflow::Test,
    )
    .unwrap();

    let args = fs::read_to_string(installation.root().join("workflow-args")).unwrap();
    assert!(args.contains("test\n--workspace\n--all-targets"), "{args}");
    assert!(
        args.contains(&cargo_manifest.display().to_string()),
        "{args}"
    );
    assert_eq!(
        fs::read_to_string(installation.root().join("workflow-cargo-home")).unwrap(),
        format!("{}\n", installation.root().join("cargo-home").display())
    );
    assert_eq!(
        fs::read_to_string(installation.root().join("workflow-target")).unwrap(),
        format!(
            "{}\n",
            installation
                .root()
                .join("output/dev/workflow-source")
                .display()
        )
    );
}
