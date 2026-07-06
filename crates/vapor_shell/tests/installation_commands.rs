mod common;

use common::TestTree;
use std::{fs, process::Command};

#[test]
fn installation_commands_require_an_external_source_workspace() {
    let installation = TestTree::new("installation-command");
    installation.write("Vapor.toml", "[workspace]\nid = \"example.installation\"\n");
    let executable = installation.root().join("bin/vapor");
    fs::create_dir_all(executable.parent().unwrap()).unwrap();
    fs::copy(env!("CARGO_BIN_EXE_vapor"), &executable).unwrap();
    let outside = TestTree::new("outside-workspace");

    let rejected = Command::new(&executable)
        .args(["toolchain", "status"])
        .current_dir(outside.root())
        .env("HOME", outside.root())
        .env("SHELL", "/bin/bash")
        .output()
        .unwrap();
    assert!(!rejected.status.success());
    let stderr = String::from_utf8(rejected.stderr).unwrap();
    assert!(
        stderr.contains("not inside an external Vapor source workspace"),
        "{stderr}"
    );

    let source = TestTree::new("external-source-workspace");
    source.write("Vapor.toml", "[workspace]\nid = \"example.source\"\n");
    let accepted = Command::new(&executable)
        .args(["toolchain", "status"])
        .current_dir(source.root())
        .env("HOME", outside.root())
        .env("SHELL", "/bin/bash")
        .output()
        .unwrap();
    assert!(
        accepted.status.success(),
        "{}",
        String::from_utf8_lossy(&accepted.stderr)
    );
    let stdout = String::from_utf8(accepted.stdout).unwrap();
    assert!(stdout.contains("VAPOR_HOME: unfinalized"), "{stdout}");
    assert!(stdout.contains("Rust toolchain: missing"), "{stdout}");

    let metadata = Command::new(&executable)
        .args(["metadata", "--format", "json"])
        .current_dir(source.root())
        .env("HOME", outside.root())
        .env("SHELL", "/bin/bash")
        .output()
        .unwrap();
    assert!(
        metadata.status.success(),
        "{}",
        String::from_utf8_lossy(&metadata.stderr)
    );
    let metadata: serde_json::Value = serde_json::from_slice(&metadata.stdout).unwrap();
    assert_eq!(metadata["source"]["workspace_id"], "example.source");
    assert_eq!(
        metadata["installation"]["location"]["status"],
        "unfinalized"
    );
}
