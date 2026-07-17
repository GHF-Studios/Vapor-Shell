mod common;

use common::TestTree;
use vapor_shell::{
    cargo_metadata::CargoIndex, discovery::EnvironmentPaths, manifest, state::ShellState,
};

fn fixture() -> (TestTree, TestTree, ShellState) {
    let installation = TestTree::new("state-installation");
    installation.write(
        manifest::FILE_NAME,
        "[root]\nname = \"installation\"\norganization = \"example\"\n",
    );
    let executable = installation.write("bin/vapor", "binary");

    let source = TestTree::new("state-source");
    source.write(
        manifest::FILE_NAME,
        "[workspace]\nname = \"source\"\norganization = \"example\"\n",
    );

    let paths = EnvironmentPaths::from_paths(&executable, source.root()).unwrap();
    let state = ShellState::new(paths).unwrap();
    (installation, source, state)
}

#[test]
fn source_open_anchors_context_at_the_source_root() {
    let (_installation, source, state) = fixture();

    assert_eq!(state.current_dir().unwrap(), source.root());
    assert_eq!(state.source().unwrap().id(), "example/source");
    assert!(state.content().is_none());
    assert!(matches!(state.cargo_index(), CargoIndex::NotPresent));
}

#[test]
fn source_close_removes_source_backed_context() {
    let (_installation, _source, mut state) = fixture();

    state.close_source();

    assert!(state.source().is_none());
    assert!(state.content().is_none());
    assert!(matches!(state.cargo_index(), CargoIndex::NotPresent));
    let error = state.current_dir().unwrap_err();
    assert!(error.contains("no Vapor source is open"), "{error}");
}
