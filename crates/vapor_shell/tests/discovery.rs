mod common;

use common::TestTree;
use std::fs;
use vapor_shell::{
    discovery::{EnvironmentPaths, InstallationPaths, SourceWorkspace},
    manifest,
};

fn installation_fixture() -> (TestTree, std::path::PathBuf) {
    let tree = TestTree::new("installation");
    tree.write(
        manifest::FILE_NAME,
        "[root]\nname = \"installation\"\norganization = \"example\"\n",
    );
    fs::create_dir_all(tree.root().join("lib")).expect("lib should be created");
    tree.write("cargo-home/bin/cargo", "fake cargo");
    let executable = tree.write("bin/vapor", "binary");
    (tree, executable)
}

#[test]
fn discovers_disjoint_installation_and_source_roots() {
    let (installation, executable) = installation_fixture();
    let source = TestTree::new("source");
    source.write(
        manifest::FILE_NAME,
        "[workspace]\nname = \"source\"\norganization = \"example\"\n",
    );
    source.write("games/example/Vapor.toml", "[game]\nname = \"game\"\n");
    fs::create_dir_all(source.root().join("games/example/src")).unwrap();

    let paths = EnvironmentPaths::from_paths(&executable, &source.root().join("games/example/src"))
        .expect("discovery should succeed");

    assert_eq!(paths.installation().root(), installation.root());
    assert_eq!(paths.source().root(), source.root());
    assert_eq!(
        paths.source().invocation(),
        source.root().join("games/example/src")
    );
    assert!(paths.installation().cargo().is_some());
}

#[test]
fn permits_same_identity_when_installation_and_source_are_disjoint() {
    let (installation, executable) = installation_fixture();
    let source = TestTree::new("same-identity-source");
    source.write(
        manifest::FILE_NAME,
        "[workspace]\nname = \"installation\"\norganization = \"example\"\n",
    );

    let paths = EnvironmentPaths::from_paths(&executable, source.root()).unwrap();

    assert_eq!(paths.installation().root(), installation.root());
    assert_eq!(paths.source().root(), source.root());
    assert_eq!(
        paths.installation().identity_id(),
        paths.source().identity_id()
    );
}

#[test]
fn discovers_cargo_inside_the_managed_rustup_toolchain() {
    let installation = TestTree::new("toolchain-installation");
    installation.write(
        manifest::FILE_NAME,
        "[root]\nname = \"installation\"\norganization = \"example\"\n",
    );
    let cargo = installation.write(
        "rustup-home/toolchains/nightly-test-host/bin/cargo",
        "fake cargo",
    );
    let executable = installation.write("bin/vapor", "binary");

    let paths = InstallationPaths::from_executable(&executable).unwrap();

    assert_eq!(paths.cargo(), Some(cargo.as_path()));
}

#[test]
fn installation_discovery_ignores_manifest_beside_bin_vapor() {
    let installation = TestTree::new("installation-bin-marker");
    installation.write(
        manifest::FILE_NAME,
        "[root]\nname = \"installation\"\norganization = \"example\"\n",
    );
    installation.write("bin/Vapor.toml", "");
    let executable = installation.write("bin/vapor", "binary");

    let paths = InstallationPaths::from_executable(&executable).unwrap();

    assert_eq!(paths.root(), installation.root());
    assert_eq!(paths.binaries(), installation.root().join("bin"));
}

#[test]
fn installation_discovery_requires_manifest_at_app_root() {
    let installation = TestTree::new("installation-missing-root-marker");
    installation.write(
        "bin/Vapor.toml",
        "[root]\nname = \"wrong\"\norganization = \"example\"\n",
    );
    let executable = installation.write("bin/vapor", "binary");

    let error = InstallationPaths::from_executable(&executable).unwrap_err();

    assert!(error.contains("missing its root manifest"), "{error}");
    assert!(error.contains("expected:"), "{error}");
    assert!(error.contains("Vapor.toml"), "{error}");
}

#[test]
fn installation_discovery_rejects_vapor_at_app_root() {
    let installation = TestTree::new("installation-root-binary");
    installation.write(
        manifest::FILE_NAME,
        "[root]\nname = \"installation\"\norganization = \"example\"\n",
    );
    let executable = installation.write("vapor", "binary");

    let error = InstallationPaths::from_executable(&executable).unwrap_err();

    assert!(
        error.contains("not laid out as an installed Vapor application"),
        "{error}"
    );
    assert!(error.contains("expected command:"), "{error}");
    assert!(
        error.contains(&installation.root().join("bin/vapor").display().to_string()),
        "{error}"
    );
}

#[test]
fn rejects_source_inside_the_installation() {
    let (installation, executable) = installation_fixture();
    fs::create_dir_all(installation.root().join("source")).unwrap();

    let error =
        EnvironmentPaths::from_paths(&executable, &installation.root().join("source")).unwrap_err();

    assert!(
        error.contains("no external source root is selected"),
        "{error}"
    );
}

#[test]
fn rejects_source_build_as_a_candidate_app_root() {
    let tree = TestTree::new("source-build");
    tree.write(
        manifest::FILE_NAME,
        "[workspace]\nname = \"source\"\norganization = \"example\"\n",
    );
    let executable = tree.write("target/debug/vapor", "binary");

    let error = InstallationPaths::from_executable(&executable).unwrap_err();
    assert!(
        error.contains("not laid out as an installed Vapor application"),
        "{error}"
    );
    assert!(error.contains("source-built target/debug/vapor"), "{error}");
}

#[test]
fn escalates_from_shell_repo_to_containing_vapor_root() {
    let (_installation, executable) = installation_fixture();
    let source = TestTree::new("superproject-source");
    source.write(
        manifest::FILE_NAME,
        "[root]\nname = \"vapor-root\"\norganization = \"example\"\n",
    );
    source.write(
        "Vapor-Shell/Vapor.toml",
        "[workspace]\nname = \"vapor-shell\"\norganization = \"example\"\n",
    );
    fs::create_dir_all(source.root().join("Vapor-Shell/crates/vapor_shell")).unwrap();

    let paths = EnvironmentPaths::from_paths(
        &executable,
        &source.root().join("Vapor-Shell/crates/vapor_shell"),
    )
    .expect("containing root should be selected");

    assert_eq!(paths.source().root(), source.root());
}

#[test]
fn rejects_standalone_project_repo() {
    let (_installation, executable) = installation_fixture();
    let installation = InstallationPaths::from_executable(&executable).unwrap();
    let source = TestTree::new("standalone-shell-source");
    source.write(manifest::FILE_NAME, "[project]\nname = \"vapor-shell\"\n");

    let error = SourceWorkspace::from_invocation(source.root(), &installation).unwrap_err();

    assert!(
        error.contains("cannot be the source root identity"),
        "{error}"
    );
}

#[test]
fn rejects_installation_whose_highest_marker_is_content() {
    let tree = TestTree::new("content-installation");
    tree.write(manifest::FILE_NAME, "[engine]\nname = \"engine\"\n");
    let executable = tree.write("bin/vapor", "binary");

    let error = InstallationPaths::from_executable(&executable).unwrap_err();
    assert!(
        error.contains("cannot be the source root identity"),
        "{error}"
    );
}

#[test]
fn rejects_invocation_outside_a_source_root() {
    let (_installation, executable) = installation_fixture();
    let installation = InstallationPaths::from_executable(&executable).unwrap();
    let source = TestTree::new("no-source-workspace");

    let error = SourceWorkspace::from_invocation(source.root(), &installation).unwrap_err();
    assert!(
        error.contains("not inside an external Vapor source root"),
        "{error}"
    );
}
