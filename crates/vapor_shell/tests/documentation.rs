#![cfg(unix)]

mod common;

use common::TestTree;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use vapor_shell::{discovery::EnvironmentPaths, documentation, workspace::WorkspaceManifest};

#[test]
fn docs_build_copies_linked_markdown_docs_next_to_crate_pages() {
    let installation = TestTree::new("docs-installation");
    installation.write(
        "App.vapor.toml",
        "[root]\nname = \"installation\"\norganization = \"example\"\n",
    );
    let executable = installation.write("bin/vapor", "binary");
    write_fake_cargo(&installation, "rustup-home/toolchains/test-host/bin/cargo");

    let source = TestTree::new("docs-source");
    source.write(
        "Workspace.vapor.toml",
        "[workspace]\nname = \"vapor-shell\"\norganization = \"example\"\n",
    );
    source.write("Cargo.toml", "[workspace]\nresolver = \"3\"\n");
    source.write("docs/content.md", "# Content lifecycle\n");

    let paths = EnvironmentPaths::from_paths(&executable, source.root()).unwrap();
    let manifest = WorkspaceManifest::load(&paths).unwrap();

    let docs_root = documentation::build(&paths, &manifest).unwrap();

    assert!(
        docs_root
            .join("vapor-shell/vapor_shell/docs/content.md")
            .is_file()
    );
}

fn write_fake_cargo(tree: &TestTree, relative: &str) {
    let path = tree.write(
        relative,
        "#!/usr/bin/env sh\nmkdir -p \"$CARGO_TARGET_DIR/doc/vapor_shell\"\nprintf '<!doctype html><a href=\"docs/content.md\">content</a>' > \"$CARGO_TARGET_DIR/doc/vapor_shell/index.html\"\nexit 0\n",
    );
    let mut permissions = fs::metadata(&path).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).unwrap();
}
