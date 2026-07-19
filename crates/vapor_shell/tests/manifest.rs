mod common;

use common::TestTree;
use vapor_shell::manifest::{self, ContentKind, VaporEntity};

#[test]
fn infers_source_root_identities_from_name_and_organization() {
    let cases = [
        (manifest::APP_SOURCE_FILE_NAME, "root", "vapor-root"),
        (manifest::WORKSPACE_FILE_NAME, "workspace", "loo-cast"),
        (manifest::REGISTRY_FILE_NAME, "registry", "vapor-registry"),
    ];

    for (file_name, section, name) in cases {
        let tree = TestTree::new(section);
        let marker = tree.write(
            file_name,
            &format!("[{section}]\nname = \"{name}\"\norganization = \"ghf-studios\"\n"),
        );

        let entity = manifest::read(&marker, tree.root()).expect("source root should parse");
        assert_eq!(entity.id(), format!("ghf-studios/{name}"));
    }
}

#[test]
fn accepts_every_canonical_content_kind() {
    let cases = [
        ("engine", manifest::ENGINE_FILE_NAME, ContentKind::Engine),
        ("game", manifest::GAME_FILE_NAME, ContentKind::Game),
        (
            "packagepack",
            manifest::PACKAGEPACK_FILE_NAME,
            ContentKind::Packagepack,
        ),
        (
            "enginepack",
            manifest::ENGINEPACK_FILE_NAME,
            ContentKind::Enginepack,
        ),
        (
            "gamepack",
            manifest::GAMEPACK_FILE_NAME,
            ContentKind::Gamepack,
        ),
        ("modpack", manifest::MODPACK_FILE_NAME, ContentKind::Modpack),
        (
            "engine-mod",
            manifest::ENGINE_MOD_FILE_NAME,
            ContentKind::EngineMod,
        ),
        (
            "game-mod",
            manifest::GAME_MOD_FILE_NAME,
            ContentKind::GameMod,
        ),
        (
            "extension-mod",
            manifest::EXTENSION_MOD_FILE_NAME,
            ContentKind::ExtensionMod,
        ),
    ];

    for (section, file_name, expected_kind) in cases {
        let tree = TestTree::new(section);
        tree.write(
            manifest::WORKSPACE_FILE_NAME,
            "[workspace]\nname = \"examples\"\norganization = \"ghf-studios\"\n",
        );
        let relative = format!("content/{section}/{file_name}");
        let marker = tree.write(
            &relative,
            &format!("[{section}]\nname = \"example-{section}\"\n"),
        );

        match manifest::read(&marker, tree.root()).expect("content manifest should parse") {
            VaporEntity::Content { kind, id, name } => {
                assert_eq!(kind, expected_kind);
                assert_eq!(id, format!("ghf-studios/examples/example-{section}"));
                assert_eq!(name, format!("example-{section}"));
            }
            other => panic!("expected content manifest, got {other:?}"),
        }
    }
}

#[test]
fn rejects_multiple_identity_sections() {
    let tree = TestTree::new("multiple-identities");
    tree.write(
        manifest::WORKSPACE_FILE_NAME,
        "[workspace]\nname = \"examples\"\norganization = \"ghf-studios\"\n",
    );
    let marker = tree.write(
        &format!("content/broken/{}", manifest::ENGINE_FILE_NAME),
        "[engine]\nname = \"example-engine\"\n[game]\nname = \"example-game\"\n",
    );

    let error = manifest::read(&marker, tree.root()).unwrap_err();
    assert!(
        error.contains("multiple Vapor identity sections"),
        "{error}"
    );
}

#[test]
fn rejects_declaration_side_ids() {
    let tree = TestTree::new("declaration-side-id");
    let marker = tree.write(
        manifest::WORKSPACE_FILE_NAME,
        "[workspace]\nname = \"source\"\norganization = \"example\"\nid = \"example.source\"\n",
    );

    let error = manifest::read(&marker, tree.root()).unwrap_err();
    assert!(error.contains("removed field `id`"), "{error}");
}

#[test]
fn rejects_filename_section_mismatch() {
    let tree = TestTree::new("role-mismatch");
    tree.write(
        manifest::WORKSPACE_FILE_NAME,
        "[workspace]\nname = \"source\"\norganization = \"example\"\n",
    );
    let marker = tree.write(
        &format!("crates/shell/{}", manifest::ENGINE_FILE_NAME),
        "[game]\nname = \"shell\"\n",
    );

    let error = manifest::read(&marker, tree.root()).unwrap_err();
    assert!(error.contains("requires [engine]"), "{error}");
}
