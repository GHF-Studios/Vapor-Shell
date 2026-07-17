mod common;

use common::TestTree;
use std::fs;
use vapor_shell::{
    command::{self, ShellCommand, SourceCommand, SourceTemplate},
    discovery::EnvironmentPaths,
    state::ShellState,
    workspace::WorkspaceManifest,
};

#[test]
fn source_init_basic_content_creates_and_opens_workspace() {
    let installation = TestTree::new("source-init-installation");
    installation.write(
        "Vapor.toml",
        "schema = 1\n\n[root]\nname = \"installation\"\norganization = \"example\"\n\n[root.steam]\napp-id = 2122620\ndepot-id = 2122621\ndevelopment-branch = \"vapor-dev\"\n",
    );
    let executable = installation.write("bin/vapor", "binary");
    let bootstrap = TestTree::new("source-init-bootstrap");
    bootstrap.write(
        "Vapor.toml",
        "schema = 1\n\n[workspace]\nname = \"bootstrap\"\norganization = \"example\"\n",
    );
    bootstrap.write("Cargo.toml", "[workspace]\nresolver = \"3\"\n");
    let paths = EnvironmentPaths::from_paths(&executable, bootstrap.root()).unwrap();
    let mut state = ShellState::new(paths).unwrap();
    let target_parent = TestTree::new("source-init-target");
    let target = target_parent.root().join("friend-demo");

    command::execute(
        ShellCommand::Source {
            command: SourceCommand::Init {
                template: SourceTemplate::BasicContent,
                path: target.clone(),
                organization: "friend-studio".to_owned(),
                name: "friend-demo".to_owned(),
                app_id: None,
            },
        },
        &mut state,
    )
    .unwrap();

    assert_eq!(state.source().unwrap().id(), "friend-studio/friend-demo");
    assert!(target.join("Vapor.toml").is_file());
    assert!(target.join("Cargo.lock").is_file());
    assert!(
        target
            .join("crates/friend-demo-engine/src/main.rs")
            .is_file()
    );
    assert!(target.join("crates/friend-demo-game/src/lib.rs").is_file());
    assert!(
        fs::read_to_string(target.join("crates/friend-demo-game/Vapor.toml"))
            .unwrap()
            .contains("app-id = 2122620")
    );

    let manifest = WorkspaceManifest::load(state.active_paths().unwrap()).unwrap();
    assert_eq!(manifest.projects().len(), 3);
}

#[test]
fn source_repair_write_adds_dependency_workshop_ids_from_sibling_artifacts() {
    let installation = TestTree::new("source-repair-installation");
    installation.write(
        "Vapor.toml",
        "schema = 1\n\n[root]\nname = \"installation\"\norganization = \"example\"\n",
    );
    let executable = installation.write("bin/vapor", "binary");
    let source = TestTree::new("source-repair-source");
    source.write(
        "Vapor.toml",
        "schema = 1\n\n[workspace]\nname = \"demo\"\norganization = \"friend\"\nversion = \"0.1.0\"\n\n[[workspace.projects]]\npath = \"engine\"\n\n[[workspace.projects]]\npath = \"game\"\n\n[[workspace.projects]]\npath = \"packagepack\"\n",
    );
    source.write("Cargo.toml", "[workspace]\nresolver = \"3\"\n");
    source.write(
        "engine/Vapor.toml",
        "schema = 1\n\n[engine]\nname = \"engine\"\nversion.workspace = true\n\n[engine.steam]\napp-id = 2122620\npublished-file-id = \"9001\"\nvisibility = \"private\"\ntitle = \"Demo Engine\"\n",
    );
    source.write("engine/src/lib.rs", "pub fn engine() {}\n");
    source.write(
        "game/Vapor.toml",
        "schema = 1\n\n[game]\nname = \"game\"\nversion.workspace = true\n\n[game.engine]\nid = \"friend/demo/engine\"\n\n[game.steam]\napp-id = 2122620\npublished-file-id = \"9002\"\nvisibility = \"private\"\ntitle = \"Demo Game\"\n",
    );
    source.write("game/src/lib.rs", "pub fn game() {}\n");
    source.write(
        "packagepack/Vapor.toml",
        "schema = 1\n\n[packagepack]\nname = \"packagepack\"\nversion.workspace = true\n\n[packagepack.engine]\nid = \"friend/demo/engine\"\n\n[packagepack.game]\nid = \"friend/demo/game\"\n\n[packagepack.steam]\napp-id = 2122620\nvisibility = \"private\"\ntitle = \"Demo Packagepack\"\n",
    );
    source.write("packagepack/src/lib.rs", "pub fn pack() {}\n");
    let paths = EnvironmentPaths::from_paths(&executable, source.root()).unwrap();
    let mut state = ShellState::new(paths).unwrap();

    command::execute(
        ShellCommand::Source {
            command: SourceCommand::Repair { write: true },
        },
        &mut state,
    )
    .unwrap();

    let game = fs::read_to_string(source.root().join("game/Vapor.toml")).unwrap();
    assert!(game.contains("workshop-id = \"9001\""), "{game}");
    let packagepack = fs::read_to_string(source.root().join("packagepack/Vapor.toml")).unwrap();
    assert!(
        packagepack.contains("workshop-id = \"9001\""),
        "{packagepack}"
    );
    assert!(
        packagepack.contains("workshop-id = \"9002\""),
        "{packagepack}"
    );
}
