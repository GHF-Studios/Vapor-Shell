mod common;

use common::TestTree;
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use vapor_shell::{
    command::{self, SetupCommand, SetupPackageCommand, ShellCommand},
    discovery::EnvironmentPaths,
    path_setup::PathSetup,
    state::ShellState,
    toolchain::{self, Requirement},
};

#[cfg(unix)]
#[test]
fn active_app_local_tools_satisfy_toolchain_preflight() {
    let installation = TestTree::new("toolchain-installation");
    installation.write(
        "Vapor.toml",
        "[root]\nname = \"installation\"\norganization = \"example\"\n",
    );
    let vapor_executable = write_executable(&installation, "bin/vapor");
    for path in [
        "rustup/bin/rustup",
        "rustup-home/toolchains/nightly-host/bin/cargo",
        "rustup-home/toolchains/nightly-host/bin/rustc",
        "rustup-home/toolchains/nightly-host/bin/rustfmt",
        "rustup-home/toolchains/nightly-host/bin/cargo-clippy",
        "rustup-home/toolchains/nightly-host/bin/rustdoc",
        "tools/git/bin/git",
        "tools/steamcmd/steamcmd",
    ] {
        write_executable(&installation, path);
    }
    installation.write("cargo-home/registry/.keep", "");

    let source = TestTree::new("toolchain-source");
    source.write(
        "Vapor.toml",
        "[workspace]\nname = \"source\"\norganization = \"example\"\n",
    );
    let paths = EnvironmentPaths::from_paths(&vapor_executable, source.root()).unwrap();

    let home = TestTree::new("toolchain-home");
    let setup = PathSetup::new(
        home.root().to_path_buf(),
        installation.root().join("bin"),
        Some("/bin/bash".to_owned()),
    );
    let registered = toolchain::register_location_with_setup(paths.installation(), &setup).unwrap();
    assert!(registered.status().registered());

    let before = toolchain::inspect(paths.installation());
    assert!(before.complete());
    toolchain::require(
        paths.installation(),
        &[Requirement::Rust, Requirement::Git, Requirement::SteamCmd],
        "test projects",
    )
    .unwrap();
}

#[cfg(unix)]
#[test]
fn host_git_wrapper_is_not_a_valid_app_owned_git() {
    let installation = TestTree::new("toolchain-git-wrapper");
    installation.write(
        "Vapor.toml",
        "[root]\nname = \"installation\"\norganization = \"example\"\n",
    );
    write_executable(&installation, "bin/vapor");
    for path in [
        "rustup/bin/rustup",
        "rustup-home/toolchains/nightly-host/bin/cargo",
        "rustup-home/toolchains/nightly-host/bin/rustc",
        "rustup-home/toolchains/nightly-host/bin/rustfmt",
        "rustup-home/toolchains/nightly-host/bin/cargo-clippy",
        "rustup-home/toolchains/nightly-host/bin/rustdoc",
        "tools/steamcmd/steamcmd",
    ] {
        write_executable(&installation, path);
    }
    write_host_git_wrapper(&installation, "tools/git/bin/git");

    let executable = installation.root().join("bin/vapor");
    let source = TestTree::new("toolchain-git-wrapper-source");
    source.write(
        "Vapor.toml",
        "[workspace]\nname = \"source\"\norganization = \"example\"\n",
    );
    let paths = EnvironmentPaths::from_paths(&executable, source.root()).unwrap();
    let status = toolchain::inspect(paths.installation());

    assert!(!status.git().installed());
    assert!(
        status
            .git()
            .missing()
            .iter()
            .any(|entry| entry.contains("host Git wrapper")),
        "{:?}",
        status.git().missing()
    );
}

#[cfg(unix)]
#[test]
fn setup_package_install_populates_package_content_without_auth_state() {
    let installation = TestTree::new("setup-package-installation");
    installation.write(
        "Vapor.toml",
        "[root]\nname = \"installation\"\norganization = \"example\"\n",
    );
    let vapor_executable = write_executable(&installation, "bin/vapor");
    for path in [
        "rustup/bin/rustup",
        "rustup-home/toolchains/nightly-host/bin/cargo",
        "rustup-home/toolchains/nightly-host/bin/rustc",
        "rustup-home/toolchains/nightly-host/bin/rustfmt",
        "rustup-home/toolchains/nightly-host/bin/cargo-clippy",
        "rustup-home/toolchains/nightly-host/bin/rustdoc",
        "tools/git/bin/git",
        "tools/steamcmd/steamcmd",
    ] {
        write_executable(&installation, path);
    }
    installation.write("cargo-home/registry/.keep", "");
    installation.write("cargo-home/credentials.toml", "SECRET");
    installation.write("cargo-home/registry/cache/secret.crate", "SECRET");
    installation.write("cargo-home/registry/src/secret.rs", "SECRET");
    installation.write("tools/steamcmd/config/config.vdf", "SECRET");
    installation.write("tools/steamcmd/logs/steamcmd.log", "SECRET");
    installation.write("tools/steamcmd/steamapps/appmanifest_1.acf", "SECRET");

    let source = TestTree::new("setup-package-source");
    source.write(
        "Vapor.toml",
        "[workspace]\nname = \"source\"\norganization = \"example\"\n",
    );
    source.write("Cargo.toml", "[workspace]\nresolver = \"3\"\n");
    let paths = EnvironmentPaths::from_paths(&vapor_executable, source.root()).unwrap();
    let home = TestTree::new("setup-package-home");
    let setup = PathSetup::new(
        home.root().to_path_buf(),
        installation.root().join("bin"),
        Some("/bin/bash".to_owned()),
    );
    toolchain::register_location_with_setup(paths.installation(), &setup).unwrap();
    let mut state = ShellState::new(paths).unwrap();

    command::execute(
        ShellCommand::Setup {
            command: SetupCommand::Package {
                command: SetupPackageCommand::Install { dry_run: false },
            },
        },
        &mut state,
    )
    .unwrap();

    let package = installation.root().join("packages/toolchain");
    assert!(package.join("rustup/bin/rustup").is_file());
    assert!(
        package
            .join("rustup-home/toolchains/nightly-host/bin/cargo")
            .is_file()
    );
    assert!(package.join("git/bin/git").is_file());
    assert!(package.join("steamcmd/steamcmd").is_file());
    assert!(!package.join("cargo-home/credentials.toml").exists());
    assert!(!package.join("cargo-home/registry/cache").exists());
    assert!(!package.join("cargo-home/registry/src").exists());
    assert!(!package.join("steamcmd/config").exists());
    assert!(!package.join("steamcmd/logs").exists());
    assert!(!package.join("steamcmd/steamapps").exists());
}

#[cfg(unix)]
#[test]
fn toolchain_install_applies_existing_package_content() {
    let installation = TestTree::new("toolchain-package-installation");
    installation.write(
        "Vapor.toml",
        "[root]\nname = \"installation\"\norganization = \"example\"\n",
    );
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

    let source = TestTree::new("toolchain-package-source");
    source.write(
        "Vapor.toml",
        "[workspace]\nname = \"source\"\norganization = \"example\"\n",
    );
    source.write("Cargo.toml", "[workspace]\nresolver = \"3\"\n");
    let paths = EnvironmentPaths::from_paths(&vapor_executable, source.root()).unwrap();
    let home = TestTree::new("toolchain-package-home");
    let setup = PathSetup::new(
        home.root().to_path_buf(),
        installation.root().join("bin"),
        Some("/bin/bash".to_owned()),
    );
    toolchain::register_location_with_setup(paths.installation(), &setup).unwrap();

    let report = toolchain::install(paths.installation()).unwrap();
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
}

#[cfg(unix)]
#[test]
fn setup_install_dry_run_does_not_mutate_app_root() {
    let installation = TestTree::new("toolchain-dry-run-installation");
    installation.write(
        "Vapor.toml",
        "[root]\nname = \"installation\"\norganization = \"example\"\n",
    );
    let vapor_executable = write_executable(&installation, "bin/vapor");

    let source = TestTree::new("toolchain-dry-run-source");
    source.write(
        "Vapor.toml",
        "[workspace]\nname = \"source\"\norganization = \"example\"\n",
    );
    source.write("Cargo.toml", "[workspace]\nresolver = \"3\"\n");
    let paths = EnvironmentPaths::from_paths(&vapor_executable, source.root()).unwrap();
    let mut state = ShellState::new(paths).unwrap();

    command::execute(
        ShellCommand::Setup {
            command: SetupCommand::Install { dry_run: true },
        },
        &mut state,
    )
    .unwrap();

    assert!(
        !installation
            .root()
            .join(".vapor/state/vapor-home.toml")
            .exists()
    );
    assert!(!installation.root().join("rustup/bin/rustup").exists());
    assert!(!installation.root().join("tools/git/bin/git").exists());
    assert!(!installation.root().join("tools/steamcmd/steamcmd").exists());
}

#[test]
fn moved_location_requires_explicit_repair() {
    let installation = TestTree::new("moved-installation");
    installation.write(
        "Vapor.toml",
        "[root]\nname = \"installation\"\norganization = \"example\"\n",
    );
    let executable = installation.write("bin/vapor", "binary");
    installation.write(
        ".vapor/state/vapor-home.toml",
        "version = 1\npath = \"/previous/steam/library/Vapor\"\n",
    );
    let source = TestTree::new("moved-source");
    source.write(
        "Vapor.toml",
        "[workspace]\nname = \"source\"\norganization = \"example\"\n",
    );
    let paths = EnvironmentPaths::from_paths(&executable, source.root()).unwrap();

    let status = toolchain::location_status(paths.installation()).unwrap();
    assert!(matches!(status, toolchain::LocationStatus::Moved { .. }));
    let error =
        toolchain::require_registered_location(paths.installation(), "run a workspace build")
            .unwrap_err();
    assert!(
        error.contains("no location or PATH state was changed"),
        "{error}"
    );
    assert!(error.contains("setup repair"), "{error}");
}

#[cfg(unix)]
fn write_executable(tree: &TestTree, relative: &str) -> std::path::PathBuf {
    let path = tree.write(relative, "#!/bin/sh\nexit 0\n");
    let mut permissions = fs::metadata(&path).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&path, permissions).unwrap();
    path
}

#[cfg(unix)]
fn write_host_git_wrapper(tree: &TestTree, relative: &str) -> std::path::PathBuf {
    let path = tree.write(relative, "#!/bin/sh\nexec '/usr/bin/git' \"$@\"\n");
    let mut permissions = fs::metadata(&path).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&path, permissions).unwrap();
    path
}
