mod common;

use common::TestTree;
use vapor_shell::{cargo_metadata::CargoIndex, discovery::EnvironmentPaths, manifest};

fn roots(with_cargo_manifest: bool) -> (TestTree, TestTree, std::path::PathBuf) {
    let installation = TestTree::new("cargo-installation");
    installation.write(
        "App.vapor.toml",
        "[root]\nname = \"installation\"\norganization = \"example\"\n",
    );
    let executable = installation.write("bin/vapor", "binary");

    let source = TestTree::new("cargo-source");
    source.write(
        manifest::WORKSPACE_FILE_NAME,
        "[workspace]\nname = \"source\"\norganization = \"example\"\n",
    );
    if with_cargo_manifest {
        source.write("Cargo.toml", "[workspace]\nresolver = \"3\"\n");
    }

    (installation, source, executable)
}

#[test]
fn source_without_cargo_manifest_is_not_applicable() {
    let (_installation, source, executable) = roots(false);
    let paths = EnvironmentPaths::from_paths(&executable, source.root()).unwrap();

    assert!(matches!(
        CargoIndex::inspect(&paths),
        CargoIndex::NotPresent
    ));
}

#[test]
fn missing_bundled_cargo_degrades_without_blocking_source() {
    let (_installation, source, executable) = roots(true);
    let paths = EnvironmentPaths::from_paths(&executable, source.root()).unwrap();

    match CargoIndex::inspect(&paths) {
        CargoIndex::Unavailable(error) => assert!(error.contains("no bundled Cargo"), "{error}"),
        other => panic!("expected unavailable index, got {other:?}"),
    }
}

#[cfg(unix)]
#[test]
fn bundled_cargo_can_generate_a_workspace_index() {
    use std::{fs, os::unix::fs::PermissionsExt};

    let (installation, source, executable) = roots(true);
    let package_manifest = source.write(
        "crates/example/Cargo.toml",
        "[package]\nname = \"example\"\nversion = \"0.1.0\"\n",
    );
    let json = serde_json::json!({
        "workspace_root": source.root(),
        "target_directory": source.root().join("target"),
        "packages": [{
            "name": "example",
            "manifest_path": package_manifest,
            "targets": [{"name": "example", "kind": ["lib"]}]
        }]
    })
    .to_string();
    let cargo = installation.write(
        "cargo-home/bin/cargo",
        &format!("#!/bin/sh\nprintf '%s' '{json}'\n"),
    );
    let mut permissions = fs::metadata(&cargo).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&cargo, permissions).unwrap();

    let paths = EnvironmentPaths::from_paths(&executable, source.root()).unwrap();
    let CargoIndex::Loaded(metadata) = CargoIndex::inspect(&paths) else {
        panic!("expected loaded Cargo metadata");
    };

    assert_eq!(metadata.root(), source.root());
    assert_eq!(metadata.packages().len(), 1);
    assert_eq!(metadata.packages()[0].name(), "example");
    assert_eq!(metadata.packages()[0].targets()[0].kinds(), ["lib"]);
}
