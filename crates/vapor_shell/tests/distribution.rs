mod common;

use common::TestTree;
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use vapor_shell::{
    command::{self, RootCommand, SetupCommand, SetupPackageCommand, ShellCommand},
    discovery::EnvironmentPaths,
    distribution::{self, DistributionManifest},
    manifest,
    path_setup::PathSetup,
    setup,
    state::ShellState,
    steam,
};

fn fixture() -> (TestTree, TestTree, EnvironmentPaths, DistributionManifest) {
    let installation = TestTree::new("dist-installation");
    installation.write(
        manifest::FILE_NAME,
        "[root]\nname = \"installation\"\norganization = \"example\"\n",
    );
    let executable = installation.write("bin/vapor", "binary");
    installation.write("docs/index.html", "docs");
    for path in [
        "packages/setup/rustup/bin/rustup",
        "packages/setup/rustup-home/toolchains/test-host/bin/cargo",
        "packages/setup/rustup-home/toolchains/test-host/bin/rustc",
        "packages/setup/rustup-home/toolchains/test-host/bin/rustfmt",
        "packages/setup/rustup-home/toolchains/test-host/bin/cargo-clippy",
        "packages/setup/rustup-home/toolchains/test-host/bin/rustdoc",
        "packages/setup/git/bin/git",
        "packages/setup/steamcmd/steamcmd",
    ] {
        write_tool(&installation, path);
    }
    installation.write("packages/setup/cargo-home/registry/.keep", "");
    let source = TestTree::new("dist-source");
    source.write(
        distribution::FILE_NAME,
        r#"
[root]
name = "vapor-root"
organization = "example"

[root.steam]
app-id = 123
depot-id = 124
development-branch = "vapor-dev"
"#,
    );

    let paths = EnvironmentPaths::from_paths(&executable, source.root()).unwrap();
    let manifest = DistributionManifest::load(&paths).unwrap();
    (installation, source, paths, manifest)
}

fn write_tool(tree: &TestTree, relative: &str) -> std::path::PathBuf {
    let path = tree.write(relative, "#!/bin/sh\nexit 0\n");
    #[cfg(unix)]
    {
        let mut permissions = fs::metadata(&path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&path, permissions).unwrap();
    }
    path
}

#[cfg(unix)]
fn write_cargo(tree: &TestTree, relative: &str) -> std::path::PathBuf {
    let path = tree.write(
        relative,
        "#!/bin/sh\nif [ -n \"$CARGO_TARGET_DIR\" ]; then\n  mkdir -p \"$CARGO_TARGET_DIR/doc/vapor_shell\"\n  printf '<!doctype html>docs' > \"$CARGO_TARGET_DIR/doc/vapor_shell/index.html\"\nfi\nexit 0\n",
    );
    let mut permissions = fs::metadata(&path).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&path, permissions).unwrap();
    path
}

#[test]
fn staging_is_allowlisted_and_uses_package_content() {
    let (_installation, _source, paths, manifest) = fixture();

    let report = distribution::stage(&paths, &manifest).unwrap();

    assert!(report.root().join("bin/vapor").is_file());
    assert!(report.root().join("docs/index.html").is_file());
    assert!(
        report
            .root()
            .join("packages/setup/rustup/bin/rustup")
            .is_file()
    );
    assert!(
        report
            .root()
            .join("packages/setup/rustup-home/toolchains/test-host/bin/cargo")
            .is_file()
    );
    assert!(report.root().join("packages/setup/git/bin/git").is_file());
    assert!(
        report
            .root()
            .join("packages/setup/steamcmd/steamcmd")
            .is_file()
    );
    assert!(!report.root().join("rustup").exists());
    assert!(!report.root().join("tools/git").exists());
    assert!(
        !fs::read_to_string(report.root().join("Vapor.toml"))
            .unwrap()
            .contains("SECRET")
    );
}

#[test]
fn publish_dry_run_generates_preview_vdf_without_steamcmd_execution() {
    let (_installation, _source, paths, manifest) = fixture();

    let script = steam::publish(
        &paths,
        &manifest,
        "builder",
        None,
        "test build",
        true,
        false,
    )
    .unwrap();
    let vdf = fs::read_to_string(script).unwrap();

    assert!(vdf.contains("\"Preview\" \"1\""));
    assert!(vdf.contains("\"SetLive\" \"vapor-dev\""));
    assert!(vdf.contains("\"123\""));
    assert!(vdf.contains("\"124\""));
}

#[cfg(unix)]
#[test]
fn root_publish_dry_run_requires_active_app_local_setup() {
    let installation = TestTree::new("app-publish-installation");
    installation.write(
        manifest::FILE_NAME,
        "[root]\nname = \"installation\"\norganization = \"example\"\n",
    );
    let executable = write_tool(&installation, "bin/vapor");
    write_tool(&installation, "rustup/bin/rustup");
    write_cargo(&installation, "rustup-home/toolchains/test-host/bin/cargo");
    for path in [
        "rustup-home/toolchains/test-host/bin/rustc",
        "rustup-home/toolchains/test-host/bin/rustfmt",
        "rustup-home/toolchains/test-host/bin/cargo-clippy",
        "rustup-home/toolchains/test-host/bin/rustdoc",
        "tools/git/bin/git",
        "tools/steamcmd/steamcmd",
    ] {
        write_tool(&installation, path);
    }
    installation.write("cargo-home/registry/.keep", "");

    let source = TestTree::new("app-publish-source");
    source.write(
        distribution::FILE_NAME,
        r#"
[root]
name = "vapor-root"
organization = "example"

[root.steam]
app-id = 123
depot-id = 124
development-branch = "vapor-dev"
"#,
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
    let home = TestTree::new("app-publish-home");
    let setup = PathSetup::new(
        home.root().to_path_buf(),
        installation.root().join("bin"),
        Some("/bin/bash".to_owned()),
    );
    setup::register_location_with_setup(paths.installation(), &setup).unwrap();
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

    command::execute(
        ShellCommand::Root {
            command: RootCommand::Publish {
                account: None,
                branch: None,
                description: "dry-run build".to_owned(),
                dry_run: true,
                yes: false,
            },
        },
        &mut state,
    )
    .unwrap();

    let script = installation
        .root()
        .join("output/root/scripts/app_build_123.vdf");
    let vdf = fs::read_to_string(script).unwrap();
    assert!(vdf.contains("\"Preview\" \"1\""), "{vdf}");
    assert!(vdf.contains("\"SetLive\" \"vapor-dev\""), "{vdf}");
}
