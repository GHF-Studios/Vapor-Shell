mod common;

use common::TestTree;
use std::{fs, path::PathBuf, process::Command};

#[test]
fn direct_facade_allows_app_first_setup_and_scripts_with_open_source() {
    let installation = TestTree::new("installation-command");
    installation.write(
        "Vapor.toml",
        "[root]\nname = \"installation\"\norganization = \"example\"\n",
    );
    let executable = installation.root().join("bin/vapor");
    fs::create_dir_all(executable.parent().unwrap()).unwrap();
    fs::copy(vapor_binary(), &executable).unwrap();
    let outside = TestTree::new("outside-source-root");

    let setup = Command::new(&executable)
        .args(["setup", "status"])
        .current_dir(outside.root())
        .env("HOME", outside.root())
        .env("SHELL", "/bin/bash")
        .output()
        .unwrap();
    assert!(
        setup.status.success(),
        "{}",
        String::from_utf8_lossy(&setup.stderr)
    );
    let stdout = String::from_utf8(setup.stdout).unwrap();
    assert!(stdout.contains("app root:"), "{stdout}");

    let source = TestTree::new("external-source-root");
    source.write(
        "Vapor.toml",
        "[workspace]\nname = \"source\"\norganization = \"example\"\n",
    );
    source.write("Cargo.toml", "[workspace]\nresolver = \"3\"\n");
    source.write(".vapor/scripts/status.vapor", "metadata --format json\n");

    let rejected_one_shot = Command::new(&executable)
        .args(["validate"])
        .current_dir(source.root())
        .env("HOME", outside.root())
        .env("SHELL", "/bin/bash")
        .output()
        .unwrap();
    assert!(!rejected_one_shot.status.success());
    let stderr = String::from_utf8(rejected_one_shot.stderr).unwrap();
    assert!(
        stderr.contains("must run inside the interactive Vapor shell"),
        "{stderr}"
    );

    let add = Command::new(&executable)
        .args(["sources", "add", source.root().to_str().unwrap()])
        .current_dir(outside.root())
        .env("HOME", outside.root())
        .env("SHELL", "/bin/bash")
        .output()
        .unwrap();
    assert!(
        add.status.success(),
        "{}",
        String::from_utf8_lossy(&add.stderr)
    );

    let open = Command::new(&executable)
        .args(["open", "source"])
        .current_dir(outside.root())
        .env("HOME", outside.root())
        .env("SHELL", "/bin/bash")
        .output()
        .unwrap();
    assert!(
        open.status.success(),
        "{}",
        String::from_utf8_lossy(&open.stderr)
    );

    let script = Command::new(&executable)
        .args(["script", "run", "status"])
        .current_dir(outside.root())
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
