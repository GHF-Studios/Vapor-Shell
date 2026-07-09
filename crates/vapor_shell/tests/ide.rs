mod common;

use common::TestTree;
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use vapor_shell::{
    command::{self, IdeCommand, ShellCommand},
    discovery::EnvironmentPaths,
    path_setup::PathSetup,
    state::ShellState,
    toolchain,
};

#[cfg(unix)]
#[test]
fn ide_repair_dry_run_does_not_write_project_settings() {
    let (_installation, source, mut state) = fixture();

    command::execute(
        ShellCommand::Ide {
            command: IdeCommand::Repair { dry_run: true },
        },
        &mut state,
    )
    .unwrap();

    assert!(!source.root().join(".idea/cargoProjects.xml").exists());
    assert!(!source.root().join(".idea/rust.xml").exists());
    assert!(!source.root().join(".idea/vapor.xml").exists());
}

#[cfg(unix)]
#[test]
fn ide_repair_writes_project_local_rustrover_settings() {
    let (installation, source, mut state) = fixture();

    command::execute(
        ShellCommand::Ide {
            command: IdeCommand::Repair { dry_run: false },
        },
        &mut state,
    )
    .unwrap();

    let cargo_projects = fs::read_to_string(source.root().join(".idea/cargoProjects.xml")).unwrap();
    assert!(
        cargo_projects.contains("FILE=\"$PROJECT_DIR$/Cargo.toml\""),
        "{cargo_projects}"
    );

    let rust = fs::read_to_string(source.root().join(".idea/rust.xml")).unwrap();
    assert!(
        rust.contains(&format!(
            "toolchainHomeDirectory&quot; value=&quot;{}",
            installation
                .root()
                .join("rustup-home/toolchains/nightly-host/bin")
                .display()
        )) || rust.contains(&format!(
            "toolchainHomeDirectory\" value=\"{}",
            installation
                .root()
                .join("rustup-home/toolchains/nightly-host/bin")
                .display()
        )),
        "{rust}"
    );
    assert!(
        rust.contains("explicitPathToStdlib"),
        "stdlib source should be linked when packaged: {rust}"
    );

    let vapor = fs::read_to_string(source.root().join(".idea/vapor.xml")).unwrap();
    assert!(vapor.contains("VaporProjectSettings"), "{vapor}");
    assert!(
        vapor.contains(&installation.root().display().to_string()),
        "{vapor}"
    );
    assert!(vapor.contains("cargoHome"), "{vapor}");
    assert!(vapor.contains("rustupHome"), "{vapor}");
}

#[cfg(unix)]
#[test]
fn scripts_cannot_apply_ide_repair() {
    let (_installation, source, mut state) = fixture();
    source.write(".vapor/scripts/ide-repair.vapor", "ide repair\n");

    let error = command::execute(
        ShellCommand::Script {
            command: vapor_shell::command::ScriptCommand::Run {
                name: "ide-repair".to_owned(),
                dry_run: false,
            },
        },
        &mut state,
    )
    .unwrap_err();

    assert!(error.contains("IDE repairs"), "{error}");
    assert!(!source.root().join(".idea/vapor.xml").exists());
}

#[cfg(unix)]
fn fixture() -> (TestTree, TestTree, ShellState) {
    let installation = TestTree::new("ide-installation");
    installation.write(
        "Vapor.toml",
        "[root]\nname = \"installation\"\norganization = \"example\"\n",
    );
    let vapor_executable = write_executable(&installation, "bin/vapor");
    write_executable(&installation, "rustup/bin/rustup");
    for path in [
        "rustup-home/toolchains/nightly-host/bin/cargo",
        "rustup-home/toolchains/nightly-host/bin/rustc",
        "rustup-home/toolchains/nightly-host/bin/rustfmt",
        "rustup-home/toolchains/nightly-host/bin/cargo-clippy",
        "rustup-home/toolchains/nightly-host/bin/rustdoc",
        "tools/git/bin/git",
    ] {
        write_executable(&installation, path);
    }
    installation.write(
        "rustup-home/toolchains/nightly-host/lib/rustlib/src/rust/library/.keep",
        "",
    );

    let source = TestTree::new("ide-source");
    source.write(
        "Vapor.toml",
        "[workspace]\nname = \"source\"\norganization = \"example\"\n",
    );
    source.write("Cargo.toml", "[workspace]\nresolver = \"3\"\n");
    let paths = EnvironmentPaths::from_paths(&vapor_executable, source.root()).unwrap();
    let home = TestTree::new("ide-home");
    let setup = PathSetup::new(
        home.root().to_path_buf(),
        installation.root().join("bin"),
        Some("/bin/bash".to_owned()),
    );
    toolchain::register_location_with_setup(paths.installation(), &setup).unwrap();

    let state = ShellState::new(paths).unwrap();
    (installation, source, state)
}

#[cfg(unix)]
fn write_executable(tree: &TestTree, relative: &str) -> std::path::PathBuf {
    let path = tree.write(relative, "#!/bin/sh\nexit 0\n");
    let mut permissions = fs::metadata(&path).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&path, permissions).unwrap();
    path
}
