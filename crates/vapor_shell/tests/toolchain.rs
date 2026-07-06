mod common;

use common::TestTree;
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use vapor_shell::{
    discovery::EnvironmentPaths,
    path_setup::PathSetup,
    toolchain::{self, Requirement},
};

#[cfg(unix)]
#[test]
fn explicit_install_applies_all_vendored_tools_inside_the_app() {
    let installation = TestTree::new("toolchain-installation");
    installation.write("Vapor.toml", "[workspace]\nid = \"example.installation\"\n");
    let vapor_executable = write_executable(&installation, "bin/vapor");
    for path in [
        "packages/toolchain/rustup/bin/rustup",
        "packages/toolchain/rustup-home/toolchains/nightly-host/bin/cargo",
        "packages/toolchain/rustup-home/toolchains/nightly-host/bin/rustc",
        "packages/toolchain/rustup-home/toolchains/nightly-host/bin/rustfmt",
        "packages/toolchain/rustup-home/toolchains/nightly-host/bin/cargo-clippy",
        "packages/toolchain/rustup-home/toolchains/nightly-host/bin/rustdoc",
        "packages/toolchain/git/bin/git",
        "packages/toolchain/steamcmd/steamcmd",
    ] {
        write_executable(&installation, path);
    }
    installation.write("packages/toolchain/cargo-home/registry/.keep", "");

    let source = TestTree::new("toolchain-source");
    source.write("Vapor.toml", "[workspace]\nid = \"example.source\"\n");
    let paths = EnvironmentPaths::from_paths(&vapor_executable, source.root()).unwrap();

    let home = TestTree::new("toolchain-home");
    let setup = PathSetup::new(
        home.root().to_path_buf(),
        installation.root().join("bin"),
        Some("/bin/bash".to_owned()),
    );
    let finalized = toolchain::finalize_location_with_setup(paths.installation(), &setup).unwrap();
    assert!(finalized.status().finalized());

    let before = toolchain::inspect(paths.installation());
    assert!(!before.complete());
    assert!(before.packages_complete());
    let error = toolchain::require(paths.installation(), &[Requirement::Rust], "test projects")
        .unwrap_err();
    assert!(
        error.contains("will not install or repair prerequisites"),
        "{error}"
    );

    let report = toolchain::install(paths.installation(), false).unwrap();
    assert_eq!(
        report.installed_groups(),
        ["Rust toolchain", "Git", "SteamCMD"]
    );
    assert!(report.status().complete());
    assert!(installation.root().join("rustup/bin/rustup").is_file());
    assert!(installation.root().join("tools/git/bin/git").is_file());
    assert!(
        installation
            .root()
            .join("tools/steamcmd/steamcmd")
            .is_file()
    );
    assert!(
        toolchain::install(paths.installation(), false)
            .unwrap()
            .installed_groups()
            .is_empty()
    );
}

#[test]
fn moved_location_requires_explicit_refinalization() {
    let installation = TestTree::new("moved-installation");
    installation.write("Vapor.toml", "[workspace]\nid = \"example.installation\"\n");
    let executable = installation.write("bin/vapor", "binary");
    installation.write(
        "state/vapor-home.toml",
        "version = 1\npath = \"/previous/steam/library/Vapor\"\n",
    );
    let source = TestTree::new("moved-source");
    source.write("Vapor.toml", "[workspace]\nid = \"example.source\"\n");
    let paths = EnvironmentPaths::from_paths(&executable, source.root()).unwrap();

    let status = toolchain::location_status(paths.installation()).unwrap();
    assert!(matches!(status, toolchain::LocationStatus::Moved { .. }));
    let error =
        toolchain::require_finalized_location(paths.installation(), "run a workspace build")
            .unwrap_err();
    assert!(
        error.contains("no location or PATH state was changed"),
        "{error}"
    );
    assert!(error.contains("toolchain finalize"), "{error}");
}

#[cfg(unix)]
fn write_executable(tree: &TestTree, relative: &str) -> std::path::PathBuf {
    let path = tree.write(relative, "#!/bin/sh\nexit 0\n");
    let mut permissions = fs::metadata(&path).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&path, permissions).unwrap();
    path
}
