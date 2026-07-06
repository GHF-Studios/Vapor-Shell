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
        "[workspace]\nid = \"example.installation\"\n",
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
    installation.write("steam/steamcmd/steamcmd", "steamcmd");
    installation.write("steam/steamcmd/config/config.vdf", "SECRET");

    let source = TestTree::new("dist-source");
    source.write(
        distribution::FILE_NAME,
        r#"
[workspace]
id = "example.source"

[distribution.application]
app_id = 123
depot_id = 124
development_branch = "vapor-dev"

[[distribution.payload]]
root = "source"
from = "Vapor.toml"
to = "Vapor.toml"
required = true

[[distribution.payload]]
root = "installation"
from = "bin"
to = "bin"
required = true

[[distribution.payload]]
root = "installation"
from = "docs"
to = "docs"
required = true

[[distribution.payload]]
root = "installation"
from = "packages/toolchain"
to = "packages/toolchain"
required = true

[[distribution.payload]]
root = "installation"
from = "steam/steamcmd"
to = "tools/steamcmd"
exclude = ["config"]
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
fn publish_plan_generates_preview_vdf_without_steamcmd_execution() {
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
