mod common;

use common::TestTree;
use vapor_shell::manifest::{self, ContentKind, VaporEntity};

#[test]
fn infers_source_root_identities_from_name_and_organization() {
    let cases = [
        ("root", "vapor-root"),
        ("workspace", "loo-cast"),
        ("registry", "vapor-registry"),
    ];

    for (section, name) in cases {
        let tree = TestTree::new(section);
        let marker = tree.write(
            manifest::FILE_NAME,
            &format!("[{section}]\nname = \"{name}\"\norganization = \"ghf-studios\"\n"),
        );

        let entity = manifest::read(&marker, tree.root()).expect("source root should parse");
        assert_eq!(entity.id(), format!("ghf-studios/{name}"));
    }
}

#[test]
fn infers_project_identity_from_source_root() {
    let tree = TestTree::new("project");
    tree.write(
        manifest::FILE_NAME,
        "[workspace]\nname = \"loo-cast\"\norganization = \"ghf-studios\"\n",
    );
    let marker = tree.write("crates/cli/Vapor.toml", "[project]\nname = \"cli\"\n");

    match manifest::read(&marker, tree.root()).expect("project should parse") {
        VaporEntity::Project { id, name } => {
            assert_eq!(id, "ghf-studios/loo-cast/cli");
            assert_eq!(name, "cli");
        }
        other => panic!("expected project manifest, got {other:?}"),
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
        ("engine-mod", ContentKind::EngineMod),
        ("game-mod", ContentKind::GameMod),
        ("extension-mod", ContentKind::ExtensionMod),
    ];

    for (section, expected_kind) in cases {
        let tree = TestTree::new(section);
        tree.write(
            manifest::FILE_NAME,
            "[workspace]\nname = \"examples\"\norganization = \"ghf-studios\"\n",
        );
        let relative = format!("content/{section}/Vapor.toml");
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
        manifest::FILE_NAME,
        "[workspace]\nname = \"examples\"\norganization = \"ghf-studios\"\n",
    );
    let marker = tree.write(
        "content/broken/Vapor.toml",
        "[engine]\nname = \"example-engine\"\n[game]\nname = \"example-game\"\n",
    );

    let error = manifest::read(&marker, tree.root()).unwrap_err();
    assert!(error.contains("multiple Vapor identities"), "{error}");
}

#[test]
fn rejects_declaration_side_ids() {
    let tree = TestTree::new("legacy-id");
    let marker = tree.write(
        manifest::FILE_NAME,
        "[workspace]\nname = \"source\"\norganization = \"example\"\nid = \"example.source\"\n",
    );

    let error = manifest::read(&marker, tree.root()).unwrap_err();
    assert!(error.contains("removed field `id`"), "{error}");
}

#[test]
fn rejects_project_kind_field() {
    let tree = TestTree::new("legacy-kind");
    tree.write(
        manifest::FILE_NAME,
        "[workspace]\nname = \"source\"\norganization = \"example\"\n",
    );
    let marker = tree.write(
        "crates/shell/Vapor.toml",
        "[project]\nname = \"shell\"\nkind = \"shell\"\n",
    );

    let error = manifest::read(&marker, tree.root()).unwrap_err();
    assert!(error.contains("removed field `kind`"), "{error}");
}
