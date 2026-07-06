mod common;

use common::TestTree;
use std::fs;
use vapor_shell::path_setup::PathSetup;

#[test]
fn registration_is_idempotent_repairs_moves_and_keeps_binary_in_app() {
    let home = TestTree::new("path-home");
    let first_bin = home.root().join("steam/first/bin");
    home.write("steam/first/bin/vapor", "binary");
    home.write(".bashrc", "# user configuration\n");
    let setup = PathSetup::new(
        home.root().to_path_buf(),
        first_bin.clone(),
        Some("/bin/bash".to_owned()),
    );

    let first = setup.install().unwrap();
    assert!(first.registered());
    assert!(first.changed());
    assert_eq!(first.command(), first_bin.join("vapor"));
    assert!(!home.root().join(".local/bin/vapor").exists());
    let bashrc = fs::read_to_string(home.root().join(".bashrc")).unwrap();
    assert!(bashrc.contains("# user configuration"));
    assert!(bashrc.contains(&first_bin.display().to_string()));
    assert_eq!(bashrc.matches("Vapor managed PATH >>>").count(), 1);

    assert!(!setup.install().unwrap().changed());

    let moved_bin = home.root().join("steam/moved/bin");
    home.write("steam/moved/bin/vapor", "binary");
    let moved = PathSetup::new(
        home.root().to_path_buf(),
        moved_bin.clone(),
        Some("/bin/bash".to_owned()),
    );
    assert!(moved.install().unwrap().changed());
    let bashrc = fs::read_to_string(home.root().join(".bashrc")).unwrap();
    assert!(bashrc.contains(&moved_bin.display().to_string()));
    assert!(!bashrc.contains(&first_bin.display().to_string()));

    let removed = moved.uninstall().unwrap();
    assert!(!removed.registered());
    assert!(removed.command().is_file());
    assert_eq!(
        fs::read_to_string(home.root().join(".bashrc")).unwrap(),
        "# user configuration\n"
    );
}

#[test]
fn registration_requires_the_app_owned_vapor_binary() {
    let home = TestTree::new("path-missing-command");
    let setup = PathSetup::new(
        home.root().to_path_buf(),
        home.root().join("steam/app/bin"),
        Some("/bin/bash".to_owned()),
    );

    let error = setup.install().unwrap_err();
    assert!(
        error.contains("app-owned Vapor command is missing"),
        "{error}"
    );
}
