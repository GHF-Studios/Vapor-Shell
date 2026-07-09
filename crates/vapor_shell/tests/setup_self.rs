mod common;

use common::TestTree;
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use vapor_shell::{
    command::{self, SetupCommand, SetupSelfCommand, SetupSelfPackageCommand, ShellCommand},
    discovery::EnvironmentPaths,
    path_setup::PathSetup,
    setup_self::{self, SetupSelfRequirement},
    state::ShellState,
};

#[cfg(unix)]
#[test]
fn active_app_local_tools_satisfy_setup_self_preflight() {
    let installation = TestTree::new("setup-self-installation");
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

    let source = TestTree::new("setup-self-source");
    source.write(
        "Vapor.toml",
        "[workspace]\nname = \"source\"\norganization = \"example\"\n",
    );
    let paths = EnvironmentPaths::from_paths(&vapor_executable, source.root()).unwrap();

    let home = TestTree::new("setup-self-home");
    let setup = PathSetup::new(
        home.root().to_path_buf(),
        installation.root().join("bin"),
        Some("/bin/bash".to_owned()),
    );
    let registered =
        setup_self::register_location_with_setup(paths.installation(), &setup).unwrap();
    assert!(registered.status().registered());

    let before = setup_self::inspect(paths.installation());
    assert!(before.complete());
    setup_self::require(
        paths.installation(),
        &[
            SetupSelfRequirement::Rust,
            SetupSelfRequirement::Git,
            SetupSelfRequirement::SteamCmd,
        ],
        "test projects",
    )
    .unwrap();
}

#[cfg(unix)]
#[test]
fn delegating_git_script_is_not_app_owned_git() {
    let installation = TestTree::new("setup-self-git-delegating-script");
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
    write_delegating_git_script(&installation, "tools/git/bin/git");

    let executable = installation.root().join("bin/vapor");
    let source = TestTree::new("setup-self-git-delegating-script-source");
    source.write(
        "Vapor.toml",
        "[workspace]\nname = \"source\"\norganization = \"example\"\n",
    );
    let paths = EnvironmentPaths::from_paths(&executable, source.root()).unwrap();
    let status = setup_self::inspect(paths.installation());

    assert!(!status.git().installed());
    assert!(
        status
            .git()
            .missing()
            .iter()
            .any(|entry| entry.contains("app-owned Git executable")),
        "{:?}",
        status.git().missing()
    );
}

#[cfg(unix)]
#[test]
fn app_owned_git_launcher_satisfies_setup_self_preflight() {
    let installation = TestTree::new("setup-self-git-app-owned-launcher");
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
        "tools/git/libexec/git-core/git",
        "tools/steamcmd/steamcmd",
    ] {
        write_executable(&installation, path);
    }
    write_app_owned_git_launcher(&installation, "tools/git/bin/git");
    installation.write("cargo-home/registry/.keep", "");

    let executable = installation.root().join("bin/vapor");
    let source = TestTree::new("setup-self-git-app-owned-launcher-source");
    source.write(
        "Vapor.toml",
        "[workspace]\nname = \"source\"\norganization = \"example\"\n",
    );
    let paths = EnvironmentPaths::from_paths(&executable, source.root()).unwrap();
    let status = setup_self::inspect(paths.installation());

    assert!(status.git().installed(), "{:?}", status.git().missing());
    assert!(status.complete());
}

#[cfg(unix)]
#[test]
fn setup_self_package_install_populates_payload_without_auth_state() {
    let installation = TestTree::new("setup-self-package-installation");
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

    let source = TestTree::new("setup-self-package-source");
    source.write(
        "Vapor.toml",
        "[workspace]\nname = \"source\"\norganization = \"example\"\n",
    );
    source.write("Cargo.toml", "[workspace]\nresolver = \"3\"\n");
    let paths = EnvironmentPaths::from_paths(&vapor_executable, source.root()).unwrap();
    let home = TestTree::new("setup-self-package-home");
    let setup = PathSetup::new(
        home.root().to_path_buf(),
        installation.root().join("bin"),
        Some("/bin/bash".to_owned()),
    );
    setup_self::register_location_with_setup(paths.installation(), &setup).unwrap();
    let mut state = ShellState::new(paths).unwrap();

    command::execute(
        ShellCommand::Setup {
            command: SetupCommand::Self_ {
                command: SetupSelfCommand::Package {
                    command: SetupSelfPackageCommand::Install { dry_run: false },
                },
            },
        },
        &mut state,
    )
    .unwrap();

    let package = installation.root().join("packages/setup");
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
fn setup_self_install_applies_existing_payload() {
    let installation = TestTree::new("setup-self-package-installation");
    installation.write(
        "Vapor.toml",
        "[root]\nname = \"installation\"\norganization = \"example\"\n",
    );
    let vapor_executable = write_executable(&installation, "bin/vapor");
    for path in [
        "packages/setup/rustup/bin/rustup",
        "packages/setup/rustup-home/toolchains/nightly-host/bin/cargo",
        "packages/setup/rustup-home/toolchains/nightly-host/bin/rustc",
        "packages/setup/rustup-home/toolchains/nightly-host/bin/rustfmt",
        "packages/setup/rustup-home/toolchains/nightly-host/bin/cargo-clippy",
        "packages/setup/rustup-home/toolchains/nightly-host/bin/rustdoc",
        "packages/setup/git/bin/git",
        "packages/setup/steamcmd/steamcmd",
    ] {
        write_executable(&installation, path);
    }
    installation.write("packages/setup/cargo-home/registry/.keep", "");

    let source = TestTree::new("setup-self-package-source");
    source.write(
        "Vapor.toml",
        "[workspace]\nname = \"source\"\norganization = \"example\"\n",
    );
    source.write("Cargo.toml", "[workspace]\nresolver = \"3\"\n");
    let paths = EnvironmentPaths::from_paths(&vapor_executable, source.root()).unwrap();
    let home = TestTree::new("setup-self-package-home");
    let setup = PathSetup::new(
        home.root().to_path_buf(),
        installation.root().join("bin"),
        Some("/bin/bash".to_owned()),
    );
    setup_self::register_location_with_setup(paths.installation(), &setup).unwrap();

    let report = setup_self::install(paths.installation()).unwrap();
    assert_eq!(report.installed_groups(), ["Rust/Cargo", "Git", "SteamCMD"]);
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
fn setup_self_install_dry_run_does_not_mutate_app_root() {
    let installation = TestTree::new("setup-self-dry-run-installation");
    installation.write(
        "Vapor.toml",
        "[root]\nname = \"installation\"\norganization = \"example\"\n",
    );
    let vapor_executable = write_executable(&installation, "bin/vapor");

    let source = TestTree::new("setup-self-dry-run-source");
    source.write(
        "Vapor.toml",
        "[workspace]\nname = \"source\"\norganization = \"example\"\n",
    );
    source.write("Cargo.toml", "[workspace]\nresolver = \"3\"\n");
    let paths = EnvironmentPaths::from_paths(&vapor_executable, source.root()).unwrap();
    let mut state = ShellState::new(paths).unwrap();

    command::execute(
        ShellCommand::Setup {
            command: SetupCommand::Self_ {
                command: SetupSelfCommand::Install { dry_run: true },
            },
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

    let status = setup_self::location_status(paths.installation()).unwrap();
    assert!(matches!(status, setup_self::LocationStatus::Moved { .. }));
    let error =
        setup_self::require_registered_location(paths.installation(), "run a workspace build")
            .unwrap_err();
    assert!(
        error.contains("no location or PATH state was changed"),
        "{error}"
    );
    assert!(error.contains("setup self repair"), "{error}");
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
fn write_delegating_git_script(tree: &TestTree, relative: &str) -> std::path::PathBuf {
    let path = tree.write(relative, "#!/bin/sh\nexec '/usr/bin/git' \"$@\"\n");
    let mut permissions = fs::metadata(&path).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&path, permissions).unwrap();
    path
}

#[cfg(unix)]
fn write_app_owned_git_launcher(tree: &TestTree, relative: &str) -> std::path::PathBuf {
    let path = tree.write(
        relative,
        "#!/bin/sh\nself_dir=$(CDPATH= cd -- \"$(dirname -- \"$0\")\" && pwd)\ngit_root=$(CDPATH= cd -- \"$self_dir/..\" && pwd)\nexec \"$git_root/libexec/git-core/git\" \"$@\"\n",
    );
    let mut permissions = fs::metadata(&path).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&path, permissions).unwrap();
    path
}
