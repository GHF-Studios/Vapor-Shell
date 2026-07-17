mod common;

use common::TestTree;
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use vapor_shell::{
    discovery::EnvironmentPaths,
    workflow::{self, CargoWorkflow, ProjectSelection},
    workspace::WorkspaceManifest,
};

fn host_runtime_target() -> String {
    let arch = std::env::consts::ARCH;
    match (arch, std::env::consts::OS, std::env::consts::FAMILY) {
        ("x86_64", "linux", _) => "x86_64-unknown-linux-gnu".to_owned(),
        ("aarch64", "linux", _) => "aarch64-unknown-linux-gnu".to_owned(),
        ("x86_64", "windows", _) => "x86_64-pc-windows-gnullvm".to_owned(),
        ("aarch64", "windows", _) => "aarch64-pc-windows-msvc".to_owned(),
        ("x86_64", "macos", _) => "x86_64-apple-darwin".to_owned(),
        ("aarch64", "macos", _) => "aarch64-apple-darwin".to_owned(),
        _ => format!(
            "{arch}-{}-{}",
            std::env::consts::OS,
            std::env::consts::FAMILY
        ),
    }
}

#[cfg(unix)]
#[test]
fn test_workflow_uses_installed_cargo_and_app_owned_output() {
    let installation = TestTree::new("workflow-installation");
    installation.write(
        "Vapor.toml",
        "[root]\nname = \"installation\"\norganization = \"example\"\n",
    );
    let executable = installation.write("bin/vapor", "binary");
    let cargo = installation.write(
        "rustup-home/toolchains/test-host/bin/cargo",
        "#!/bin/sh\nprintf '%s\\n' \"$@\" > \"$VAPOR_HOME/workflow-args\"\nprintf '%s\\n' \"$CARGO_HOME\" > \"$VAPOR_HOME/workflow-cargo-home\"\nprintf '%s\\n' \"$CARGO_TARGET_DIR\" > \"$VAPOR_HOME/workflow-target\"\n",
    );
    let mut permissions = fs::metadata(&cargo).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&cargo, permissions).unwrap();

    let source = TestTree::new("workflow-source");
    source.write(
        "Vapor.toml",
        "[workspace]\nname = \"workflow-source\"\norganization = \"example\"\n",
    );
    let cargo_manifest = source.write("Cargo.toml", "[workspace]\nresolver = \"3\"\n");

    let paths = EnvironmentPaths::from_paths(&executable, source.root()).unwrap();
    let manifest = WorkspaceManifest::load(&paths).unwrap();
    workflow::run(
        &paths,
        &manifest,
        ProjectSelection::One("workflow-source".to_owned()),
        CargoWorkflow::Test,
    )
    .unwrap();

    let args = fs::read_to_string(installation.root().join("workflow-args")).unwrap();
    assert!(args.contains("test\n--workspace\n--all-targets"), "{args}");
    assert!(
        args.contains(&cargo_manifest.display().to_string()),
        "{args}"
    );
    assert_eq!(
        fs::read_to_string(installation.root().join("workflow-cargo-home")).unwrap(),
        format!("{}\n", installation.root().join("cargo-home").display())
    );
    assert_eq!(
        fs::read_to_string(installation.root().join("workflow-target")).unwrap(),
        format!(
            "{}\n",
            installation
                .root()
                .join("output/dev/workflow-source")
                .display()
        )
    );
}

#[cfg(unix)]
#[test]
fn explicit_windows_gnullvm_build_uses_app_local_llvm_mingw_linker() {
    let installation = TestTree::new("workflow-cross-linker-installation");
    installation.write(
        "Vapor.toml",
        "[root]\nname = \"installation\"\norganization = \"example\"\n",
    );
    let executable = installation.write("bin/vapor", "binary");
    let cargo = installation.write(
        "rustup-home/toolchains/test-host/bin/cargo",
        "#!/bin/sh\nprintf '%s\\n' \"$CARGO_TARGET_X86_64_PC_WINDOWS_GNULLVM_LINKER\" > \"$VAPOR_HOME/workflow-linker\"\nprintf '%s\\n' \"$@\" > \"$VAPOR_HOME/workflow-args\"\n",
    );
    make_executable(&cargo);
    make_executable(&installation.write("tools/zig/zig", "#!/bin/sh\nexit 0\n"));
    let linker = installation.write(
        "tools/llvm-mingw/bin/x86_64-w64-mingw32-clang",
        "#!/bin/sh\nexit 0\n",
    );
    make_executable(&linker);
    make_executable(&installation.write(
        "tools/llvm-mingw/bin/x86_64-w64-mingw32-dlltool",
        "#!/bin/sh\nexit 0\n",
    ));
    make_executable(
        &installation.write("tools/llvm-mingw/bin/llvm-dlltool", "#!/bin/sh\nexit 0\n"),
    );
    make_executable(&installation.write(
        "tools/cross/bin/x86_64-unknown-linux-gnu-zig-cc",
        "#!/bin/sh\nexit 0\n",
    ));

    let source = TestTree::new("workflow-cross-linker-source");
    source.write(
        "Vapor.toml",
        "[workspace]\nname = \"workflow-source\"\norganization = \"example\"\n",
    );
    source.write("Cargo.toml", "[workspace]\nresolver = \"3\"\n");

    let paths = EnvironmentPaths::from_paths(&executable, source.root()).unwrap();
    let manifest = WorkspaceManifest::load(&paths).unwrap();
    workflow::run_with_target(
        &paths,
        &manifest,
        ProjectSelection::One("workflow-source".to_owned()),
        CargoWorkflow::Build,
        Some("x86_64-pc-windows-gnullvm"),
    )
    .unwrap();

    assert_eq!(
        fs::read_to_string(installation.root().join("workflow-linker")).unwrap(),
        format!("{}\n", linker.display())
    );
    let args = fs::read_to_string(installation.root().join("workflow-args")).unwrap();
    assert!(
        args.contains("--target\nx86_64-pc-windows-gnullvm"),
        "{args}"
    );
}

#[test]
fn promote_places_root_binaries_under_host_target_directory() {
    let installation = TestTree::new("workflow-promote-installation");
    installation.write(
        "Vapor.toml",
        "[root]\nname = \"installation\"\norganization = \"example\"\n",
    );
    let executable = installation.write("bin/vapor", "binary");
    let binary_name = format!("vapor{}", std::env::consts::EXE_SUFFIX);
    installation.write(
        &format!("output/dev/workflow-source/debug/{binary_name}"),
        "promoted binary",
    );

    let source = TestTree::new("workflow-promote-source");
    source.write(
        "Vapor.toml",
        "[workspace]\nname = \"workflow-source\"\norganization = \"example\"\nbinaries = [\"vapor\"]\n",
    );
    source.write("Cargo.toml", "[workspace]\nresolver = \"3\"\n");

    let paths = EnvironmentPaths::from_paths(&executable, source.root()).unwrap();
    let manifest = WorkspaceManifest::load(&paths).unwrap();

    let promoted = workflow::promote(&paths, &manifest).unwrap();

    assert_eq!(promoted, 1);
    assert!(
        installation
            .root()
            .join("bin")
            .join(host_runtime_target())
            .join(binary_name)
            .is_file()
    );
}

#[cfg(unix)]
fn make_executable(path: &std::path::Path) {
    let mut permissions = fs::metadata(path).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).unwrap();
}

#[test]
fn promote_places_explicit_windows_root_binaries_under_target_directory() {
    let installation = TestTree::new("workflow-promote-windows-installation");
    installation.write(
        "Vapor.toml",
        "[root]\nname = \"installation\"\norganization = \"example\"\n",
    );
    let executable = installation.write("bin/vapor", "binary");
    installation.write(
        "output/dev/workflow-source/x86_64-pc-windows-gnullvm/debug/vapor.exe",
        "promoted binary",
    );
    installation.write(
        "tools/llvm-mingw/x86_64-w64-mingw32/bin/libunwind.dll",
        "runtime dll",
    );

    let source = TestTree::new("workflow-promote-windows-source");
    source.write(
        "Vapor.toml",
        "[workspace]\nname = \"workflow-source\"\norganization = \"example\"\nbinaries = [\"vapor\"]\n",
    );
    source.write("Cargo.toml", "[workspace]\nresolver = \"3\"\n");

    let paths = EnvironmentPaths::from_paths(&executable, source.root()).unwrap();
    let manifest = WorkspaceManifest::load(&paths).unwrap();
    let targets = vec!["x86_64-pc-windows-gnullvm".to_owned()];

    let promoted = workflow::promote_for_targets(&paths, &manifest, &targets).unwrap();

    assert_eq!(promoted, 1);
    assert!(
        installation
            .root()
            .join("bin/x86_64-pc-windows-gnullvm/vapor.exe")
            .is_file()
    );
    assert!(
        installation
            .root()
            .join("bin/x86_64-pc-windows-gnullvm/libunwind.dll")
            .is_file()
    );
}
