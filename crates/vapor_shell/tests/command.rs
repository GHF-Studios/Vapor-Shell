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
    assert!(help.contains("script"), "{help}");
    assert!(help.contains("open"), "{help}");
    assert!(help.contains("close"), "{help}");
    assert!(help.contains("sources"), "{help}");
    assert!(help.contains("validate"), "{help}");
    assert!(help.contains("setup"), "{help}");
    assert!(!help.contains("toolchain"), "{help}");
    assert!(!help.contains("\n  workspace"), "{help}");
    assert!(!help.contains("self"), "{help}");
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
    for command in ["status", "install", "uninstall", "repair", "package"] {
        assert!(
            setup_help.contains(command),
            "missing {command}: {setup_help}"
        );
    }
    for command in ["install", "uninstall", "repair"] {
        let help = ShellCommand::try_parse_from(["", "setup", command, "--help"])
            .expect_err("setup mutation help should exit through Clap")
            .to_string();
        assert!(help.contains("--dry-run"), "{help}");
    }
    let package_help = ShellCommand::try_parse_from(["", "setup", "package", "--help"])
        .expect_err("setup package help should exit through Clap")
        .to_string();
    for command in ["status", "install", "repair"] {
        assert!(
            package_help.contains(command),
            "missing {command}: {package_help}"
        );
    }
    for command in ["install", "repair"] {
        let help = ShellCommand::try_parse_from(["", "setup", "package", command, "--help"])
            .expect_err("setup package mutation help should exit through Clap")
            .to_string();
        assert!(help.contains("--dry-run"), "{help}");
    }
    let removed_toolchain = ShellCommand::try_parse_from(["", "toolchain", "status"])
        .expect_err("toolchain command must be removed")
        .to_string();
    assert!(
        removed_toolchain.contains("unrecognized"),
        "{removed_toolchain}"
    );

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
    for command in ["build", "package", "publish"] {
        assert!(
            root_help.contains(command),
            "missing {command}: {root_help}"
        );
    }
    let root_publish_help = ShellCommand::try_parse_from(["", "root", "publish", "--help"])
        .expect_err("root publish help should exit through Clap")
        .to_string();
    for argument in ["--dry-run", "--account", "--branch", "--yes"] {
        assert!(
            root_publish_help.contains(argument),
            "missing {argument}: {root_publish_help}"
        );
    }

    let content_help = ShellCommand::try_parse_from(["", "content", "--help"])
        .expect_err("content help should exit through Clap")
        .to_string();
    assert!(content_help.contains("status"), "{content_help}");
    assert!(!content_help.contains("install"), "{content_help}");
    assert!(!content_help.contains("repair"), "{content_help}");
    let removed_content_install = ShellCommand::try_parse_from(["", "content", "install"])
        .expect_err("content install command must be removed")
        .to_string();
    assert!(
        removed_content_install.contains("unrecognized"),
        "{removed_content_install}"
    );

    let sources_help = ShellCommand::try_parse_from(["", "sources", "--help"])
        .expect_err("sources help should exit through Clap")
        .to_string();
    for command in ["list", "add", "remove"] {
        assert!(
            sources_help.contains(command),
            "missing {command}: {sources_help}"
        );
    }

    let test_help = ShellCommand::try_parse_from(["", "test", "--help"])
        .expect_err("test help should exit through Clap")
        .to_string();
    assert!(test_help.contains("PROJECT"), "{test_help}");
    assert!(test_help.contains("Cargo workspace name"), "{test_help}");

    let cd_help = ShellCommand::try_parse_from(["", "cd", "--help"])
        .expect_err("cd help should exit through Clap")
        .to_string();
    assert!(cd_help.contains("SOURCE_PATH"), "{cd_help}");

    let error = ShellCommand::try_parse_from(["", "up", "0"])
        .expect_err("zero is not a valid number of levels")
        .to_string();
    assert!(error.contains("non-zero"), "{error}");
}
