use clap::Parser;
use vapor_shell::command::ShellCommand;

#[test]
fn help_uses_defined_argument_domains() {
    let help = ShellCommand::try_parse_from(["", "--help"])
        .expect_err("help should exit through Clap")
        .to_string();
    assert!(!help.contains("NAME"), "{help}");
    assert!(help.contains("metadata"), "{help}");
    assert!(help.contains("installation"), "{help}");
    assert!(help.contains("docs"), "{help}");
    assert!(help.contains("ide"), "{help}");
    assert!(help.contains("root"), "{help}");
    assert!(help.contains("content"), "{help}");
    assert!(help.contains("launch"), "{help}");
    assert!(help.contains("script"), "{help}");
    assert!(help.contains("source"), "{help}");
    assert!(!help.contains("\n  open"), "{help}");
    assert!(!help.contains("\n  close"), "{help}");
    assert!(!help.contains("\n  sources"), "{help}");
    assert!(help.contains("validate"), "{help}");
    assert!(help.contains("setup"), "{help}");
    assert!(!help.contains("\n  workspace"), "{help}");
    assert!(!help.contains("steam"), "{help}");

    let metadata_help = ShellCommand::try_parse_from(["", "metadata", "--help"])
        .expect_err("metadata help should exit through Clap")
        .to_string();
    for format in ["human", "json"] {
        assert!(
            metadata_help.contains(format),
            "missing {format}: {metadata_help}"
        );
    }

    let setup_help = ShellCommand::try_parse_from(["", "setup", "--help"])
        .expect_err("setup help should exit through Clap")
        .to_string();
    assert!(setup_help.contains("self"), "{setup_help}");
    let setup_self_help = ShellCommand::try_parse_from(["", "setup", "self", "--help"])
        .expect_err("setup self help should exit through Clap")
        .to_string();
    for command in ["status", "install", "uninstall", "repair", "package"] {
        assert!(
            setup_self_help.contains(command),
            "missing {command}: {setup_self_help}"
        );
    }
    for command in ["install", "uninstall", "repair"] {
        let help = ShellCommand::try_parse_from(["", "setup", "self", command, "--help"])
            .expect_err("setup self mutation help should exit through Clap")
            .to_string();
        assert!(help.contains("--dry-run"), "{help}");
    }
    let package_help = ShellCommand::try_parse_from(["", "setup", "self", "package", "--help"])
        .expect_err("setup self package help should exit through Clap")
        .to_string();
    for command in ["status", "install", "repair"] {
        assert!(
            package_help.contains(command),
            "missing {command}: {package_help}"
        );
    }
    for command in ["install", "repair"] {
        let help =
            ShellCommand::try_parse_from(["", "setup", "self", "package", command, "--help"])
                .expect_err("setup self package mutation help should exit through Clap")
                .to_string();
        assert!(help.contains("--dry-run"), "{help}");
    }

    let ide_help = ShellCommand::try_parse_from(["", "ide", "--help"])
        .expect_err("ide help should exit through Clap")
        .to_string();
    for command in ["status", "repair"] {
        assert!(ide_help.contains(command), "missing {command}: {ide_help}");
    }
    let ide_repair_help = ShellCommand::try_parse_from(["", "ide", "repair", "--help"])
        .expect_err("ide repair help should exit through Clap")
        .to_string();
    assert!(ide_repair_help.contains("--dry-run"), "{ide_repair_help}");

    let root_help = ShellCommand::try_parse_from(["", "root", "--help"])
        .expect_err("root help should exit through Clap")
        .to_string();
    for command in ["build", "deploy", "package", "publish"] {
        assert!(
            root_help.contains(command),
            "missing {command}: {root_help}"
        );
    }
    let root_build_help = ShellCommand::try_parse_from(["", "root", "build", "--help"])
        .expect_err("root build help should exit through Clap")
        .to_string();
    assert!(root_build_help.contains("--target"), "{root_build_help}");
    assert!(
        root_build_help.contains("--release-targets"),
        "{root_build_help}"
    );
    assert!(root_build_help.contains("--host-only"), "{root_build_help}");
    ShellCommand::try_parse_from([
        "",
        "root",
        "build",
        "--target",
        "x86_64-unknown-linux-gnu",
        "--target",
        "x86_64-pc-windows-gnullvm",
    ])
    .expect("root build should accept repeated runtime targets");
    let root_deploy_help = ShellCommand::try_parse_from(["", "root", "deploy", "--help"])
        .expect_err("root deploy help should exit through Clap")
        .to_string();
    assert!(
        root_deploy_help.contains("--skip-docs"),
        "{root_deploy_help}"
    );
    assert!(root_deploy_help.contains("--target"), "{root_deploy_help}");
    assert!(
        root_deploy_help.contains("--release-targets"),
        "{root_deploy_help}"
    );
    assert!(
        root_deploy_help.contains("--host-only"),
        "{root_deploy_help}"
    );
    let root_package_help = ShellCommand::try_parse_from(["", "root", "package", "--help"])
        .expect_err("root package help should exit through Clap")
        .to_string();
    for argument in [
        "--include-setup-payload",
        "--target",
        "--release-targets",
        "--host-only",
    ] {
        assert!(
            root_package_help.contains(argument),
            "missing {argument}: {root_package_help}"
        );
    }
    let root_publish_help = ShellCommand::try_parse_from(["", "root", "publish", "--help"])
        .expect_err("root publish help should exit through Clap")
        .to_string();
    for argument in [
        "--dry-run",
        "--account",
        "--branch",
        "--target",
        "--release-targets",
        "--host-only",
        "--skip-build",
        "--yes",
    ] {
        assert!(
            root_publish_help.contains(argument),
            "missing {argument}: {root_publish_help}"
        );
    }

    let content_help = ShellCommand::try_parse_from(["", "content", "--help"])
        .expect_err("content help should exit through Clap")
        .to_string();
    for command in [
        "status",
        "list",
        "validate",
        "build",
        "deploy",
        "package",
        "acquire",
        "subscribe",
        "download",
        "install",
        "update",
        "verify",
        "selected",
        "select",
        "deselect",
        "repair",
        "disable",
        "enable",
        "uninstall",
        "create",
        "publish",
        "delete",
    ] {
        assert!(
            content_help.contains(command),
            "missing {command}: {content_help}"
        );
    }
    let content_deploy_help = ShellCommand::try_parse_from(["", "content", "deploy", "--help"])
        .expect_err("content deploy help should exit through Clap")
        .to_string();
    assert!(
        content_deploy_help.contains("--select"),
        "{content_deploy_help}"
    );
    assert!(
        content_deploy_help.contains("--target"),
        "{content_deploy_help}"
    );
    assert!(
        content_deploy_help.contains("--release-targets"),
        "{content_deploy_help}"
    );
    assert!(
        content_deploy_help.contains("--host-only"),
        "{content_deploy_help}"
    );
    let content_build_help = ShellCommand::try_parse_from(["", "content", "build", "--help"])
        .expect_err("content build help should exit through Clap")
        .to_string();
    assert!(
        content_build_help.contains("--target"),
        "{content_build_help}"
    );
    assert!(
        content_build_help.contains("--release-targets"),
        "{content_build_help}"
    );
    assert!(
        content_build_help.contains("--host-only"),
        "{content_build_help}"
    );
    let content_package_help = ShellCommand::try_parse_from(["", "content", "package", "--help"])
        .expect_err("content package help should exit through Clap")
        .to_string();
    assert!(
        content_package_help.contains("--target"),
        "{content_package_help}"
    );
    assert!(
        content_package_help.contains("--release-targets"),
        "{content_package_help}"
    );
    assert!(
        content_package_help.contains("--host-only"),
        "{content_package_help}"
    );
    ShellCommand::try_parse_from([
        "",
        "content",
        "package",
        "spacetime-engine",
        "--target",
        "x86_64-unknown-linux-gnu",
        "--target",
        "x86_64-pc-windows-gnullvm",
    ])
    .expect("content package should accept repeated runtime targets");
    for command in ["package", "create", "publish", "delete"] {
        let help = ShellCommand::try_parse_from(["", "content", command, "--help"])
            .expect_err("content mutation help should exit through Clap")
            .to_string();
        assert!(help.contains("--dry-run"), "{help}");
    }
    for command in ["create", "publish"] {
        let help = ShellCommand::try_parse_from(["", "content", command, "--help"])
            .expect_err("content Workshop help should exit through Clap")
            .to_string();
        assert!(help.contains("--target"), "{help}");
        assert!(help.contains("--release-targets"), "{help}");
        assert!(help.contains("--host-only"), "{help}");
    }

    let launch_help = ShellCommand::try_parse_from(["", "launch", "--help"])
        .expect_err("launch help should exit through Clap")
        .to_string();
    assert!(launch_help.contains("loo-cast"), "{launch_help}");
    ShellCommand::try_parse_from(["", "launch", "loo-cast"]).expect("launch loo-cast should parse");
    let launch_loo_cast_help = ShellCommand::try_parse_from(["", "launch", "loo-cast", "--help"])
        .expect_err("launch loo-cast help should exit through Clap")
        .to_string();
    assert!(
        launch_loo_cast_help.contains("--account"),
        "{launch_loo_cast_help}"
    );

    let source_help = ShellCommand::try_parse_from(["", "source", "--help"])
        .expect_err("source help should exit through Clap")
        .to_string();
    for command in [
        "init", "status", "open", "close", "list", "add", "remove", "sync", "repair",
    ] {
        assert!(
            source_help.contains(command),
            "missing {command}: {source_help}"
        );
    }
    let source_init_help = ShellCommand::try_parse_from(["", "source", "init", "--help"])
        .expect_err("source init help should exit through Clap")
        .to_string();
    for item in ["basic-content", "--organization", "--name", "--app-id"] {
        assert!(source_init_help.contains(item), "{source_init_help}");
    }
    let source_repair_help = ShellCommand::try_parse_from(["", "source", "repair", "--help"])
        .expect_err("source repair help should exit through Clap")
        .to_string();
    assert!(
        source_repair_help.contains("--write"),
        "{source_repair_help}"
    );

    let test_help = ShellCommand::try_parse_from(["", "test", "--help"])
        .expect_err("test help should exit through Clap")
        .to_string();
    assert!(test_help.contains("PROJECT"), "{test_help}");
    assert!(test_help.contains("Cargo workspace name"), "{test_help}");

    for removed in ["cd", "up", "pwd", "ls"] {
        let error = ShellCommand::try_parse_from(["", removed])
            .expect_err("removed navigation command should not parse")
            .to_string();
        assert!(error.contains("unrecognized subcommand"), "{error}");
    }
}
