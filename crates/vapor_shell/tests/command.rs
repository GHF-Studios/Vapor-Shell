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
    assert!(help.contains("self"), "{help}");
    assert!(help.contains("script"), "{help}");
    assert!(help.contains("steam"), "{help}");
    assert!(help.contains("workspace"), "{help}");
    assert!(help.contains("validate"), "{help}");
    assert!(help.contains("toolchain"), "{help}");

    let metadata_help = ShellCommand::try_parse_from(["", "metadata", "--help"])
        .expect_err("metadata help should exit through Clap")
        .to_string();
    for format in ["human", "json"] {
        assert!(
            metadata_help.contains(format),
            "missing {format}: {metadata_help}"
        );
    }

    let toolchain_help = ShellCommand::try_parse_from(["", "toolchain", "--help"])
        .expect_err("toolchain help should exit through Clap")
        .to_string();
    for command in ["status", "install", "uninstall", "repair"] {
        assert!(
            toolchain_help.contains(command),
            "missing {command}: {toolchain_help}"
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
