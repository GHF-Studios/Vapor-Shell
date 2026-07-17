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

    let host_help = Command::new(&executable)
        .arg("--help")
        .current_dir(outside.root())
        .env("HOME", outside.root())
        .env("SHELL", "/bin/bash")
        .output()
        .unwrap();
    assert!(
        host_help.status.success(),
        "{}",
        String::from_utf8_lossy(&host_help.stderr)
    );
    let stdout = String::from_utf8(host_help.stdout).unwrap();
    assert!(stdout.contains("--startup-script"), "{stdout}");
    for command in ["setup", "source", "script"] {
        assert!(stdout.contains(command), "missing {command}: {stdout}");
    }
    for legacy in ["\n  sources", "\n  open", "\n  close"] {
        assert!(
            !stdout.contains(legacy),
            "host help should not list legacy command {legacy}: {stdout}"
        );
    }
    for shell_only in ["validate", "build"] {
        assert!(
            !stdout.contains(&format!("\n  {shell_only}")),
            "host help should not list shell-only command {shell_only}: {stdout}"
        );
    }
    for removed in ["cd", "up", "pwd", "ls"] {
        assert!(
            !stdout.contains(&format!("\n  {removed}")),
            "host help should not list removed command {removed}: {stdout}"
        );
    }

    let setup = Command::new(&executable)
        .args(["setup", "self", "status"])
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
    assert!(stdout.contains("Setup Status"), "{stdout}");
    assert!(
        stdout.contains("Install location: not confirmed yet"),
        "{stdout}"
    );
    assert!(stdout.contains("Local tools: not installed"), "{stdout}");
    assert!(stdout.contains("Next\n  setup self install"), "{stdout}");

    installation.write(".vapor/scripts/app-status.vapor", "installation\n");
    let app_script = Command::new(&executable)
        .args(["script", "run", "app-status"])
        .current_dir(outside.root())
        .env("HOME", outside.root())
        .env("SHELL", "/bin/bash")
        .output()
        .unwrap();
    assert!(
        app_script.status.success(),
        "{}",
        String::from_utf8_lossy(&app_script.stderr)
    );
    let stdout = String::from_utf8(app_script.stdout).unwrap();
    assert!(
        stdout.contains(&installation.root().display().to_string()),
        "{stdout}"
    );

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
    assert!(
        stderr.contains("put repeatable commands in `.vapor/scripts/NAME.vapor`"),
        "{stderr}"
    );

    let add = Command::new(&executable)
        .args(["source", "add", source.root().to_str().unwrap()])
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
        .args(["source", "open", "source"])
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
