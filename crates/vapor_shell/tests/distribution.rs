mod common;

use common::TestTree;
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use vapor_shell::{
    discovery::EnvironmentPaths,
    distribution::{self, DistributionManifest},
    manifest, steam,
};

fn fixture() -> (TestTree, TestTree, EnvironmentPaths, DistributionManifest) {
    let installation = TestTree::new("dist-installation");
    installation.write(
        manifest::FILE_NAME,
        "[root]\nname = \"installation\"\norganization = \"example\"\n",
    );
    let executable = installation.write("bin/vapor", "binary");
    installation.write("docs/index.html", "docs");
    installation.write("rustup-home/toolchains/test/bin/cargo", "cargo");
    for path in [
        "packages/toolchain/rustup/bin/rustup",
        "packages/toolchain/rustup-home/toolchains/test-host/bin/cargo",
        "packages/toolchain/rustup-home/toolchains/test-host/bin/rustc",
        "packages/toolchain/rustup-home/toolchains/test-host/bin/rustfmt",
        "packages/toolchain/rustup-home/toolchains/test-host/bin/cargo-clippy",
        "packages/toolchain/rustup-home/toolchains/test-host/bin/rustdoc",
        "packages/toolchain/git/bin/git",
        "packages/toolchain/steamcmd/steamcmd",
    ] {
        write_tool(&installation, path);
    }
    installation.write("packages/toolchain/cargo-home/registry/.keep", "");
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

fn write_tool(tree: &TestTree, relative: &str) {
    let path = tree.write(relative, "tool");
    #[cfg(unix)]
    {
        let mut permissions = fs::metadata(&path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).unwrap();
    }
}

#[test]
fn staging_is_allowlisted_and_excludes_auth_state() {
    let (_installation, _source, paths, manifest) = fixture();

    let report = distribution::stage(&paths, &manifest).unwrap();

    assert!(report.root().join("bin/vapor").is_file());
    assert!(report.root().join("docs/index.html").is_file());
    assert!(
        !report
            .root()
            .join("tools/steamcmd/config/config.vdf")
            .exists()
    );
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
