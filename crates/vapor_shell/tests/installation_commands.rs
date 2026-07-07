mod common;

use common::TestTree;
use std::{fs, path::PathBuf, process::Command};

#[test]
fn one_shot_facade_allows_scripts_only() {
    let installation = TestTree::new("installation-command");
    installation.write(
        "Vapor.toml",
        "[root]\nname = \"installation\"\norganization = \"example\"\n",
    );
    let executable = installation.root().join("bin/vapor");
    fs::create_dir_all(executable.parent().unwrap()).unwrap();
    fs::copy(vapor_binary(), &executable).unwrap();
    let outside = TestTree::new("outside-source-root");

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
        stderr.contains("not inside an external Vapor source root"),
        "{stderr}"
    );

    let source = TestTree::new("external-source-root");
    source.write(
        "Vapor.toml",
        "[workspace]\nname = \"source\"\norganization = \"example\"\n",
    );
    source.write("Cargo.toml", "[workspace]\nresolver = \"3\"\n");
    source.write(".vapor/scripts/status.vapor", "metadata --format json\n");

    let rejected_one_shot = Command::new(&executable)
        .args(["toolchain", "status"])
        .current_dir(source.root())
        .env("HOME", outside.root())
        .env("SHELL", "/bin/bash")
        .output()
        .unwrap();
    assert!(!rejected_one_shot.status.success());
    let stderr = String::from_utf8(rejected_one_shot.stderr).unwrap();
    assert!(
        stderr.contains("one-shot commands are disabled"),
        "{stderr}"
    );

    let script = Command::new(&executable)
        .args(["script", "run", "status"])
        .current_dir(source.root())
        .env("HOME", outside.root())
        .env("SHELL", "/bin/bash")
        .output()
        .unwrap();
    assert!(
        script.status.success(),
        "{}",
        String::from_utf8_lossy(&script.stderr)
    );
    let stdout = String::from_utf8(script.stdout).unwrap();
    assert!(
        stdout.contains("\"source_id\": \"example/source\""),
        "{stdout}"
    );
}

fn vapor_binary() -> PathBuf {
    std::env::current_exe()
        .expect("test executable path should be available")
        .parent()
        .expect("test executable should live in target/debug/deps")
        .parent()
        .expect("deps should live under target/debug")
        .join(format!("vapor{}", std::env::consts::EXE_SUFFIX))
}
