mod common;

use common::TestTree;
use vapor_shell::manifest::{self, ContentKind, ProjectKind, VaporEntity};

#[test]
fn accepts_every_project_kind() {
    let cases = [
        ("core", ProjectKind::Core),
        ("sdk", ProjectKind::Sdk),
        ("launcher", ProjectKind::Launcher),
        ("custom-content", ProjectKind::CustomContent),
        ("shell", ProjectKind::Shell),
    ];

    for (value, expected_kind) in cases {
        let tree = TestTree::new(value);
        let marker = tree.write(
            manifest::FILE_NAME,
            &format!("[project]\nkind = \"{value}\"\nid = \"example.{value}\"\n"),
        );

        match manifest::read(&marker, tree.root()).expect("workspace should parse") {
            VaporEntity::Project { kind, .. } => assert_eq!(kind, expected_kind),
            _ => panic!("expected project manifest"),
        }
    }
}

#[test]
fn accepts_every_canonical_content_kind() {
    let cases = [
        ("engine", ContentKind::Engine),
        ("game", ContentKind::Game),
        ("packagepack", ContentKind::Packagepack),
        ("enginepack", ContentKind::Enginepack),
        ("gamepack", ContentKind::Gamepack),
        ("modpack", ContentKind::Modpack),
        ("engine_mod", ContentKind::EngineMod),
        ("game_mod", ContentKind::GameMod),
        ("extension_mod", ContentKind::ExtensionMod),
    ];

    for (section, expected_kind) in cases {
        let tree = TestTree::new(section);
        let marker = tree.write(
            manifest::FILE_NAME,
            &format!("[{section}]\nid = \"example.{section}\"\n"),
        );

        match manifest::read(&marker, tree.root()).expect("manifest should parse") {
            VaporEntity::Content { kind, id } => {
                assert_eq!(kind, expected_kind);
                assert_eq!(id, format!("example.{section}"));
            }
            VaporEntity::Workspace { .. } => panic!("expected content manifest"),
            VaporEntity::Project { .. } => panic!("expected content manifest"),
        }
    }
}

#[test]
fn rejects_multiple_identity_sections() {
    let tree = TestTree::new("multiple-identities");
    let marker = tree.write(
        manifest::FILE_NAME,
        "[engine]\nid = \"example.engine\"\n[game]\nid = \"example.game\"\n",
    );

    let error = manifest::read(&marker, tree.root()).unwrap_err();
    assert!(error.contains("multiple Vapor identities"), "{error}");
}

#[test]
fn rejects_unknown_project_kind_with_allowed_values() {
    let tree = TestTree::new("unknown-project-kind");
    let marker = tree.write(
        manifest::FILE_NAME,
        "[project]\nkind = \"whatever\"\nid = \"example.unknown\"\n",
    );

    let error = manifest::read(&marker, tree.root()).unwrap_err();
    assert!(error.contains("unknown variant `whatever`"), "{error}");
    assert!(error.contains("custom-content"), "{error}");
}
