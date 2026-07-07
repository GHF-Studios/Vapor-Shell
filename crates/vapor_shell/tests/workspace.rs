mod common;

use common::TestTree;
use vapor_shell::{discovery::EnvironmentPaths, workspace::WorkspaceManifest};

#[test]
fn loads_normal_source_workspace_from_root_cargo_manifest() {
    let installation = TestTree::new("workspace-installation");
    installation.write(
        "Vapor.toml",
        "[root]\nname = \"installation\"\norganization = \"example\"\n",
    );
    let executable = installation.write("bin/vapor", "binary");

    let source = TestTree::new("workspace-source");
    source.write(
        "Vapor.toml",
        "[workspace]\nname = \"source\"\norganization = \"example\"\n",
    );
    source.write("Cargo.toml", "[workspace]\nresolver = \"3\"\n");

    let paths = EnvironmentPaths::from_paths(&executable, source.root()).unwrap();
    let manifest = WorkspaceManifest::load(&paths).unwrap();

    assert_eq!(manifest.id(), "example/source");
    assert_eq!(manifest.name(), "source");
    assert_eq!(manifest.organization(), "example");
    assert_eq!(manifest.cargo_projects().len(), 1);
    assert_eq!(manifest.cargo_projects()[0].name(), "source");
    assert_eq!(
        manifest.cargo_projects()[0].manifest(),
        std::path::Path::new("Cargo.toml")
    );
    assert!(manifest.cargo_projects()[0].documentation());
    assert!(manifest.cargo_projects()[0].binaries().is_empty());
}

#[test]
fn loads_root_source_cargo_workspaces_from_direct_submodules() {
    let installation = TestTree::new("root-workspace-installation");
    installation.write(
        "Vapor.toml",
        "[root]\nname = \"installation\"\norganization = \"example\"\n",
    );
    let executable = installation.write("bin/vapor", "binary");

    let source = TestTree::new("root-workspace-source");
    source.write(
        "Vapor.toml",
        "[root]\nname = \"vapor-root\"\norganization = \"example\"\n",
    );
    source.write(
        ".gitmodules",
        "[submodule \"Vapor-Shell\"]\n\tpath = Vapor-Shell\n\turl = https://example.invalid/Vapor-Shell\n",
    );
    source.write(
        "Vapor-Shell/Vapor.toml",
        "[workspace]\nname = \"vapor-shell\"\norganization = \"example\"\n",
    );
    source.write("Vapor-Shell/Cargo.toml", "[workspace]\nresolver = \"3\"\n");

    let paths = EnvironmentPaths::from_paths(&executable, source.root()).unwrap();
    let manifest = WorkspaceManifest::load(&paths).unwrap();

    assert_eq!(manifest.id(), "example/vapor-root");
    assert_eq!(manifest.cargo_projects().len(), 1);
    assert_eq!(manifest.cargo_projects()[0].name(), "vapor-shell");
    assert_eq!(
        manifest.cargo_projects()[0].manifest(),
        std::path::Path::new("Vapor-Shell/Cargo.toml")
    );
}
