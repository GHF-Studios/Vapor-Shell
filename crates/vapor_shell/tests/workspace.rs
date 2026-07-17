mod common;

use common::TestTree;
use vapor_shell::{
    discovery::EnvironmentPaths,
    workspace::{WorkspaceManifest, WorkspaceProjectKind},
};

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
    assert!(manifest.runtime_targets().is_empty());
    assert_eq!(manifest.cargo_projects().len(), 1);
    assert_eq!(manifest.cargo_projects()[0].name(), "source");
    assert_eq!(
        manifest.cargo_projects()[0].manifest(),
        std::path::Path::new("Cargo.toml")
    );
    assert!(manifest.cargo_projects()[0].documentation());
    assert!(manifest.cargo_projects()[0].binaries().is_empty());
    assert!(manifest.projects().is_empty());
}

#[test]
fn loads_workspace_runtime_targets_from_manifest() {
    let installation = TestTree::new("workspace-runtime-installation");
    installation.write(
        "Vapor.toml",
        "[root]\nname = \"installation\"\norganization = \"example\"\n",
    );
    let executable = installation.write("bin/vapor", "binary");

    let source = TestTree::new("workspace-runtime-source");
    source.write(
        "Vapor.toml",
        r#"
[workspace]
name = "source"
organization = "example"

[workspace.runtime]
targets = [
    "x86_64-unknown-linux-gnu",
    "x86_64-pc-windows-msvc",
]
"#,
    );
    source.write("Cargo.toml", "[workspace]\nresolver = \"3\"\n");

    let paths = EnvironmentPaths::from_paths(&executable, source.root()).unwrap();
    let manifest = WorkspaceManifest::load(&paths).unwrap();

    assert_eq!(
        manifest.runtime_targets(),
        ["x86_64-unknown-linux-gnu", "x86_64-pc-windows-msvc"]
    );
}

#[test]
fn loads_registered_workspace_projects_from_workspace_manifest() {
    let installation = TestTree::new("workspace-projects-installation");
    installation.write(
        "Vapor.toml",
        "[root]\nname = \"installation\"\norganization = \"example\"\n",
    );
    let executable = installation.write("bin/vapor", "binary");

    let source = TestTree::new("workspace-projects-source");
    source.write(
        "Vapor.toml",
        "[workspace]\nname = \"source\"\norganization = \"example\"\n\n[[workspace.projects]]\npath = \"engine\"\n\n[[workspace.projects]]\npath = \"tools\"\n",
    );
    source.write("Cargo.toml", "[workspace]\nresolver = \"3\"\n");
    source.write(
        "engine/Vapor.toml",
        "[engine]\nname = \"engine\"\nversion = \"1.0.0\"\n",
    );
    source.write("engine/src/lib.rs", "");
    source.write("tools/Vapor.toml", "[project]\nname = \"tools\"\n");
    source.write("tools/src/lib.rs", "");

    let paths = EnvironmentPaths::from_paths(&executable, source.root()).unwrap();
    let manifest = WorkspaceManifest::load(&paths).unwrap();

    assert_eq!(manifest.projects().len(), 2);
    assert_eq!(manifest.projects()[0].id(), "example/source/engine");
    assert_eq!(manifest.projects()[0].kind().to_string(), "engine");
    assert_eq!(
        manifest.projects()[0]
            .kind()
            .content_kind()
            .unwrap()
            .to_string(),
        "engine"
    );
    assert_eq!(manifest.projects()[1].id(), "example/source/tools");
    assert_eq!(manifest.projects()[1].kind(), WorkspaceProjectKind::Project);
}

#[test]
fn missing_registered_workspace_project_is_invalid() {
    let installation = TestTree::new("workspace-missing-project-installation");
    installation.write(
        "Vapor.toml",
        "[root]\nname = \"installation\"\norganization = \"example\"\n",
    );
    let executable = installation.write("bin/vapor", "binary");

    let source = TestTree::new("workspace-missing-project-source");
    source.write(
        "Vapor.toml",
        "[workspace]\nname = \"source\"\norganization = \"example\"\n\n[[workspace.projects]]\npath = \"missing\"\n",
    );
    source.write("Cargo.toml", "[workspace]\nresolver = \"3\"\n");

    let paths = EnvironmentPaths::from_paths(&executable, source.root()).unwrap();
    let error = WorkspaceManifest::load(&paths).unwrap_err();

    assert!(
        error.contains("registered workspace project 'missing' is not a directory"),
        "{error}"
    );
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
        "[workspace]\nname = \"vapor-shell\"\norganization = \"example\"\nbinaries = [\"vapor\"]\n",
    );
    source.write("Vapor-Shell/Cargo.toml", "[workspace]\nresolver = \"3\"\n");

    let paths = EnvironmentPaths::from_paths(&executable, source.root()).unwrap();
    let manifest = WorkspaceManifest::load(&paths).unwrap();

    assert_eq!(manifest.id(), "example/vapor-root");
    assert!(manifest.runtime_targets().is_empty());
    assert_eq!(manifest.cargo_projects().len(), 1);
    assert_eq!(manifest.cargo_projects()[0].name(), "vapor-shell");
    assert_eq!(
        manifest.cargo_projects()[0].manifest(),
        std::path::Path::new("Vapor-Shell/Cargo.toml")
    );
    assert_eq!(manifest.cargo_projects()[0].binaries(), ["vapor"]);
}

#[test]
fn loads_root_runtime_targets_from_manifest() {
    let installation = TestTree::new("root-runtime-installation");
    installation.write(
        "Vapor.toml",
        "[root]\nname = \"installation\"\norganization = \"example\"\n",
    );
    let executable = installation.write("bin/vapor", "binary");

    let source = TestTree::new("root-runtime-source");
    source.write(
        "Vapor.toml",
        r#"
[root]
name = "vapor-root"
organization = "example"

[root.runtime]
targets = ["x86_64-unknown-linux-gnu", "x86_64-pc-windows-msvc"]
"#,
    );
    source.write(
        ".gitmodules",
        "[submodule \"Vapor-Shell\"]\n\tpath = Vapor-Shell\n\turl = https://example.invalid/Vapor-Shell\n",
    );
    source.write(
        "Vapor-Shell/Vapor.toml",
        "[workspace]\nname = \"vapor-shell\"\norganization = \"example\"\nbinaries = [\"vapor\"]\n",
    );
    source.write("Vapor-Shell/Cargo.toml", "[workspace]\nresolver = \"3\"\n");

    let paths = EnvironmentPaths::from_paths(&executable, source.root()).unwrap();
    let manifest = WorkspaceManifest::load(&paths).unwrap();

    assert_eq!(
        manifest.runtime_targets(),
        ["x86_64-unknown-linux-gnu", "x86_64-pc-windows-msvc"]
    );
}
