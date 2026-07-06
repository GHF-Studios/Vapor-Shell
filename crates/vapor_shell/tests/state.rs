mod common;

use common::TestTree;
use std::{fs, path::Path};
use vapor_shell::{
    cargo_metadata::CargoIndex,
    discovery::EnvironmentPaths,
    manifest::{self, ContentKind},
    state::ShellState,
};

fn fixture() -> (TestTree, TestTree, ShellState) {
    let installation = TestTree::new("state-installation");
    installation.write(
        manifest::FILE_NAME,
        "[workspace]\nid = \"example.installation\"\n",
    );
    let executable = installation.write("bin/vapor", "binary");

    let source = TestTree::new("state-source");
    source.write(
        manifest::FILE_NAME,
        "[workspace]\nid = \"example.source\"\n",
    );
    source.write(
        "games/example/Vapor.toml",
        "[game]\nid = \"example.game\"\n",
    );
    fs::create_dir_all(source.root().join("games/example/src")).unwrap();

    let paths = EnvironmentPaths::from_paths(&executable, source.root()).unwrap();
    let state = ShellState::new(paths).unwrap();
    (installation, source, state)
}

#[test]
fn navigation_activates_the_nearest_content_marker() {
    let (_installation, source, mut state) = fixture();

    let warnings = state
        .change_directory(&source.root().join("games/example/src"))
        .expect("content directory should be reachable");

    assert!(warnings.is_empty());
    let content = state.content().expect("game context should be active");
    assert_eq!(content.id(), "example.game");
    assert_eq!(content.kind(), ContentKind::Game);
    assert!(matches!(state.cargo_index(), CargoIndex::NotPresent));
}

#[test]
fn navigation_cannot_enter_the_installation_or_cross_source_root() {
    let (installation, _source, mut state) = fixture();

    let error = state.change_directory(installation.root()).unwrap_err();
    assert!(error.contains("boundary violation"), "{error}");

    let error = state.change_directory(Path::new("..")).unwrap_err();
    assert!(error.contains("boundary violation"), "{error}");

    let error = state.move_up(1).unwrap_err();
    assert!(error.contains("source workspace boundary"), "{error}");
}
