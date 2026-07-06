mod common;

use common::TestTree;
use vapor_shell::{discovery::EnvironmentPaths, workspace::WorkspaceManifest};

#[test]
fn loads_root_cargo_projects_without_treating_them_as_vapor_workspaces() {
    let installation = TestTree::new("workspace-installation");
    installation.write("Vapor.toml", "[workspace]\nid = \"example.installation\"\n");
    let executable = installation.write("bin/vapor", "binary");

    let source = TestTree::new("workspace-source");
    source.write(
        "Vapor.toml",
        r#"[workspace]
id = "example.source"

[[workspace.cargo]]
name = "shell"
manifest = "Vapor-Shell/Cargo.toml"
documentation = true
binaries = ["vapor"]
"#,
    );
    source.write(
        "Vapor-Shell/Vapor.toml",
        "[project]\nkind = \"shell\"\nid = \"example.shell\"\n",
    );
    source.write("Vapor-Shell/Cargo.toml", "[workspace]\nresolver = \"3\"\n");

    let paths = EnvironmentPaths::from_paths(&executable, source.root()).unwrap();
    let manifest = WorkspaceManifest::load(&paths).unwrap();

    assert_eq!(manifest.id(), "example.source");
    assert_eq!(manifest.cargo_projects().len(), 1);
    assert_eq!(manifest.cargo_projects()[0].name(), "shell");
    assert!(manifest.cargo_projects()[0].documentation());
    assert_eq!(manifest.cargo_projects()[0].binaries(), ["vapor"]);
}
