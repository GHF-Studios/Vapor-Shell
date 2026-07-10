mod common;

use common::TestTree;
use std::fs;
use vapor_shell::{content, discovery::EnvironmentPaths};

#[test]
fn content_lifecycle_packages_caches_installs_verifies_repairs_and_uninstalls() {
    let installation = TestTree::new("content-installation");
    installation.write(
        "Vapor.toml",
        "[root]\nname = \"installation\"\norganization = \"example\"\n",
    );
    let executable = installation.write("bin/vapor", "binary");

    let source = TestTree::new("content-source");
    write_loo_cast_source(&source);
    let paths = EnvironmentPaths::from_paths(&executable, source.root()).unwrap();

    let catalog = content::discover(&paths).unwrap();
    assert_eq!(catalog.artifacts().len(), 3);
    assert!(
        catalog
            .find("example/loo-cast/loo-cast-packagepack")
            .unwrap()
            .dependencies()
            .iter()
            .any(|reference| reference.relationship() == "engine")
    );

    let validation = content::validate(&paths, None).unwrap();
    assert_eq!(validation.checked().len(), 3);

    let package = content::package(&paths, "loo-cast-packagepack", false).unwrap();
    assert!(package.root().join("Vapor-package.toml").is_file());
    assert!(package.payload().join("Vapor.toml").is_file());

    let acquired = content::acquire(
        paths.installation(),
        Some(&paths),
        "example/loo-cast/loo-cast-packagepack",
    )
    .unwrap();
    assert!(acquired.cache_root().join("Vapor-package.toml").is_file());

    let installed =
        content::install(paths.installation(), Some(&paths), "loo-cast-packagepack").unwrap();
    let installed_ids = installed
        .iter()
        .map(|report| report.artifact_id())
        .collect::<Vec<_>>();
    assert_eq!(
        installed_ids,
        vec![
            "example/loo-cast/spacetime-engine",
            "example/loo-cast/loo-cast-game",
            "example/loo-cast/loo-cast-packagepack",
        ]
    );

    let verified = content::verify(paths.installation(), None).unwrap();
    assert_eq!(verified.len(), 3);
    assert!(verified.iter().all(|report| report.ok()));

    let selection = content::select_packagepack(
        paths.installation(),
        "example/loo-cast/loo-cast-packagepack",
    )
    .unwrap();
    assert_eq!(
        selection.artifact_id(),
        "example/loo-cast/loo-cast-packagepack"
    );
    assert_eq!(
        content::current_selection(paths.installation())
            .unwrap()
            .unwrap()
            .artifact_id(),
        "example/loo-cast/loo-cast-packagepack"
    );

    let corrupt_file = installation
        .root()
        .join("content/installed/example/loo-cast/loo-cast-packagepack/src/lib.rs");
    fs::write(&corrupt_file, "corrupt").unwrap();
    let verified = content::verify(
        paths.installation(),
        Some("example/loo-cast/loo-cast-packagepack"),
    )
    .unwrap();
    assert!(!verified[0].ok());

    let repaired = content::repair(
        paths.installation(),
        Some(&paths),
        Some("example/loo-cast/loo-cast-packagepack"),
    )
    .unwrap();
    assert!(
        repaired
            .iter()
            .any(|report| report.artifact_id() == "example/loo-cast/loo-cast-packagepack")
    );
    assert!(
        content::verify(
            paths.installation(),
            Some("example/loo-cast/loo-cast-packagepack"),
        )
        .unwrap()[0]
            .ok()
    );

    let disabled = content::disable(paths.installation(), "loo-cast-packagepack");
    assert!(
        disabled.is_err(),
        "local names are source selectors, not installed selectors"
    );
    let disabled = content::disable(
        paths.installation(),
        "example/loo-cast/loo-cast-packagepack",
    )
    .unwrap();
    assert!(
        disabled
            .installed_root()
            .starts_with(installation.root().join("content/disabled"))
    );
    let enabled = content::enable(
        paths.installation(),
        "example/loo-cast/loo-cast-packagepack",
    )
    .unwrap();
    assert!(
        enabled
            .installed_root()
            .starts_with(installation.root().join("content/installed"))
    );

    let uninstalled = content::uninstall(
        paths.installation(),
        "example/loo-cast/loo-cast-packagepack",
    )
    .unwrap();
    assert!(uninstalled.removed());
    assert!(
        content::current_selection(paths.installation())
            .unwrap()
            .is_none()
    );
}

#[test]
fn workshop_publish_dry_run_writes_provider_script_without_uploading() {
    let installation = TestTree::new("content-publish-installation");
    installation.write(
        "Vapor.toml",
        "[root]\nname = \"installation\"\norganization = \"example\"\n",
    );
    let executable = installation.write("bin/vapor", "binary");

    let source = TestTree::new("content-publish-source");
    write_loo_cast_source(&source);
    let paths = EnvironmentPaths::from_paths(&executable, source.root()).unwrap();

    let report = content::publish_workshop_item(
        &paths,
        "spacetime-engine",
        None,
        Some("test update"),
        true,
        false,
    )
    .unwrap();
    assert!(!report.uploaded());
    let script = report.script().unwrap();
    let script_text = fs::read_to_string(script).unwrap();
    assert!(script_text.contains("\"preview\" \"1\""), "{script_text}");
    assert!(
        script_text.contains("\"publishedfileid\" \"1001\""),
        "{script_text}"
    );
}

fn write_loo_cast_source(source: &TestTree) {
    source.write(
        "Vapor.toml",
        "schema = 1\n\n[workspace]\nname = \"loo-cast\"\norganization = \"example\"\nversion = \"1.2.3\"\n",
    );
    source.write("Cargo.toml", "[workspace]\nresolver = \"3\"\n");
    source.write(
        "spacetime-engine/Vapor.toml",
        "schema = 1\n\n[engine]\nname = \"spacetime-engine\"\nversion.workspace = true\n\n[engine.steam]\napp-id = 2122620\npublished-file-id = \"1001\"\nvisibility = \"private\"\ntitle = \"Spacetime Engine\"\ntags = [\"first-party\"]\n",
    );
    source.write("spacetime-engine/src/lib.rs", "pub fn engine() {}\n");
    source.write(
        "loo-cast-game/Vapor.toml",
        "schema = 1\n\n[game]\nname = \"loo-cast-game\"\nversion.workspace = true\n\n[game.steam]\napp-id = 2122620\npublished-file-id = \"1002\"\nvisibility = \"private\"\ntitle = \"Loo-Cast Game\"\n",
    );
    source.write("loo-cast-game/src/lib.rs", "pub fn game() {}\n");
    source.write(
        "loo-cast-packagepack/Vapor.toml",
        "schema = 1\n\n[packagepack]\nname = \"loo-cast-packagepack\"\nversion.workspace = true\n\n[packagepack.steam]\napp-id = 2122620\npublished-file-id = \"1003\"\nvisibility = \"private\"\ntitle = \"Loo-Cast Packagepack\"\n\n[packagepack.engine]\nid = \"example/loo-cast/spacetime-engine\"\n\n[packagepack.game]\nid = \"example/loo-cast/loo-cast-game\"\n",
    );
    source.write("loo-cast-packagepack/src/lib.rs", "pub fn pack() {}\n");
}
