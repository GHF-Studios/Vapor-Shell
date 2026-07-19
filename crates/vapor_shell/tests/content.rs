mod common;

use common::TestTree;
use std::fs;
use vapor_shell::{
    command::{self, LaunchCommand, ShellCommand},
    content,
    discovery::{EnvironmentPaths, InstallationPaths},
    state::ShellState,
};

fn host_runtime_target() -> String {
    let arch = std::env::consts::ARCH;
    match (arch, std::env::consts::OS, std::env::consts::FAMILY) {
        ("x86_64", "linux", _) => "x86_64-unknown-linux-gnu".to_owned(),
        ("aarch64", "linux", _) => "aarch64-unknown-linux-gnu".to_owned(),
        ("x86_64", "windows", _) => "x86_64-pc-windows-gnullvm".to_owned(),
        ("aarch64", "windows", _) => "aarch64-pc-windows-msvc".to_owned(),
        ("x86_64", "macos", _) => "x86_64-apple-darwin".to_owned(),
        ("aarch64", "macos", _) => "aarch64-apple-darwin".to_owned(),
        _ => format!(
            "{arch}-{}-{}",
            std::env::consts::OS,
            std::env::consts::FAMILY
        ),
    }
}

fn runtime_library_file_name(stem: &str, target: &str) -> String {
    if target.contains("windows") {
        format!("{stem}.dll")
    } else if target.contains("darwin") || target.contains("apple") {
        format!("lib{stem}.dylib")
    } else {
        format!("lib{stem}.so")
    }
}

#[test]
fn content_lifecycle_packages_caches_installs_verifies_repairs_and_uninstalls() {
    let installation = TestTree::new("content-installation");
    installation.write(
        "App.vapor.toml",
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

    let preview_package = content::package(&paths, "loo-cast-packagepack", true).unwrap();
    assert!(preview_package.dry_run());
    assert!(!preview_package.root().exists());

    let package = content::package(&paths, "loo-cast-packagepack", false).unwrap();
    assert_eq!(preview_package.fingerprint(), package.fingerprint());
    assert!(package.root().join("Packagepack.vapor.toml").is_file());
    assert!(package.root().join("src/lib.rs").is_file());
    assert!(!package.root().join("Vapor-package.toml").exists());
    let deployed_manifest =
        fs::read_to_string(package.root().join("Packagepack.vapor.toml")).unwrap();
    assert!(
        deployed_manifest.contains("id = \"example/loo-cast/loo-cast-packagepack\""),
        "{deployed_manifest}"
    );
    assert!(
        deployed_manifest.contains("version = \"1.2.3\""),
        "{deployed_manifest}"
    );
    assert!(!deployed_manifest.contains("version.workspace"));

    let acquired = content::acquire(
        paths.installation(),
        Some(&paths),
        "example/loo-cast/loo-cast-packagepack",
        None,
    )
    .unwrap();
    assert!(
        acquired
            .cache_root()
            .join("Packagepack.vapor.toml")
            .is_file()
    );

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
        "App.vapor.toml",
        "[root]\nname = \"installation\"\norganization = \"example\"\n",
    );
    let executable = installation.write("bin/vapor", "binary");

    let source = TestTree::new("content-publish-source");
    write_loo_cast_source(&source);
    let paths = EnvironmentPaths::from_paths(&executable, source.root()).unwrap();

    let report = content::publish_workshop_item(
        &paths,
        "loo-cast-packagepack",
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
        script_text.contains("\"publishedfileid\" \"1003\""),
        "{script_text}"
    );
    assert!(
        script_text.contains("output/content/packages/example_loo-cast_loo-cast-packagepack"),
        "{script_text}"
    );
    assert_eq!(script_text.matches("\"packagepack\"").count(), 1);
    assert!(!script_text.contains("/payload"), "{script_text}");
}

#[test]
fn workshop_publish_dry_run_can_stage_explicit_windows_runtime_payload() {
    let installation = TestTree::new("content-publish-windows-runtime-installation");
    installation.write(
        "App.vapor.toml",
        "[root]\nname = \"installation\"\norganization = \"example\"\n",
    );
    let executable = installation.write("bin/vapor", "binary");
    let linux_target = "x86_64-unknown-linux-gnu";
    let windows_target = "x86_64-pc-windows-gnullvm";
    installation.write(
        &format!("output/dev/loo-cast/{linux_target}/debug/spacetime-engine"),
        "engine executable",
    );
    installation.write(
        &format!("output/dev/loo-cast/{linux_target}/debug/libspacetime_engine.so"),
        "engine library",
    );
    installation.write(
        &format!("output/dev/loo-cast/{windows_target}/debug/spacetime-engine.exe"),
        "engine executable",
    );
    installation.write(
        &format!("output/dev/loo-cast/{windows_target}/debug/spacetime_engine.dll"),
        "engine library",
    );
    installation.write(
        "tools/llvm-mingw/x86_64-w64-mingw32/bin/libunwind.dll",
        "runtime dll",
    );

    let source = TestTree::new("content-publish-windows-runtime-source");
    source.write(
        "Workspace.vapor.toml",
        "schema = 1\n\n[workspace]\nname = \"loo-cast\"\norganization = \"example\"\nversion = \"1.2.3\"\n\n[[workspace.projects]]\npath = \"spacetime-engine\"\n",
    );
    source.write("Cargo.toml", "[workspace]\nresolver = \"3\"\n");
    source.write(
        "spacetime-engine/Engine.vapor.toml",
        "schema = 1\n\n[engine]\nname = \"spacetime-engine\"\nversion.workspace = true\nbinaries = [\"spacetime-engine\"]\nlibraries = [\"spacetime_engine\"]\n\n[engine.steam]\napp-id = 2122620\npublished-file-id = \"1001\"\nvisibility = \"private\"\ntitle = \"Spacetime Engine\"\n",
    );
    source.write("spacetime-engine/src/main.rs", "fn main() {}\n");
    let paths = EnvironmentPaths::from_paths(&executable, source.root()).unwrap();

    let targets = vec![linux_target.to_owned(), windows_target.to_owned()];
    let report = content::publish_workshop_item_for_targets(
        &paths,
        "spacetime-engine",
        None,
        &targets,
        None,
        true,
        false,
    )
    .unwrap();

    assert!(!report.uploaded());
    let package_root = installation
        .root()
        .join("output/content/packages/example_loo-cast_spacetime-engine");
    assert!(
        package_root
            .join("bin/x86_64-pc-windows-gnullvm/spacetime-engine.exe")
            .is_file()
    );
    assert!(
        package_root
            .join("bin/x86_64-pc-windows-gnullvm/libunwind.dll")
            .is_file()
    );
    assert!(
        package_root
            .join("bin/x86_64-unknown-linux-gnu/spacetime-engine")
            .is_file()
    );
    assert!(
        package_root
            .join("lib/x86_64-pc-windows-gnullvm/spacetime_engine.dll")
            .is_file()
    );
    assert!(
        package_root
            .join("lib/x86_64-pc-windows-gnullvm/libunwind.dll")
            .is_file()
    );
    assert!(
        package_root
            .join("lib/x86_64-unknown-linux-gnu/libspacetime_engine.so")
            .is_file()
    );
    let deployed = fs::read_to_string(package_root.join("Engine.vapor.toml")).unwrap();
    assert!(
        deployed.contains("target = \"x86_64-pc-windows-gnullvm\""),
        "{deployed}"
    );
    assert!(
        deployed.contains("target = \"x86_64-unknown-linux-gnu\""),
        "{deployed}"
    );
    let script_text = fs::read_to_string(report.script().unwrap()).unwrap();
    assert!(
        script_text.contains("output/content/packages/example_loo-cast_spacetime-engine"),
        "{script_text}"
    );
}

#[test]
fn content_package_copies_declared_runtime_outputs() {
    let installation = TestTree::new("content-runtime-installation");
    installation.write(
        "App.vapor.toml",
        "[root]\nname = \"installation\"\norganization = \"example\"\n",
    );
    let executable = installation.write("bin/vapor", "binary");
    let runtime_target = host_runtime_target();
    let binary_name = format!("spacetime-engine{}", std::env::consts::EXE_SUFFIX);
    let library_name = runtime_library_file_name("spacetime_engine", &runtime_target);
    installation.write(
        &format!("output/dev/loo-cast/debug/{binary_name}"),
        "engine executable",
    );
    installation.write(
        &format!("output/dev/loo-cast/debug/{library_name}"),
        "engine library",
    );

    let source = TestTree::new("content-runtime-source");
    source.write(
        "Workspace.vapor.toml",
        "schema = 1\n\n[workspace]\nname = \"loo-cast\"\norganization = \"example\"\nversion = \"1.2.3\"\n\n[[workspace.projects]]\npath = \"spacetime-engine\"\n",
    );
    source.write("Cargo.toml", "[workspace]\nresolver = \"3\"\n");
    source.write(
        "spacetime-engine/Engine.vapor.toml",
        "schema = 1\n\n[engine]\nname = \"spacetime-engine\"\nversion.workspace = true\nbinaries = [\"spacetime-engine\"]\nlibraries = [\"spacetime_engine\"]\n\n[engine.steam]\napp-id = 2122620\npublished-file-id = \"1001\"\nvisibility = \"private\"\ntitle = \"Spacetime Engine\"\n",
    );
    source.write("spacetime-engine/src/main.rs", "fn main() {}\n");
    let paths = EnvironmentPaths::from_paths(&executable, source.root()).unwrap();

    let package = content::package(&paths, "spacetime-engine", false).unwrap();

    assert!(
        package
            .root()
            .join("bin")
            .join(&runtime_target)
            .join(&binary_name)
            .is_file()
    );
    assert!(
        package
            .root()
            .join("lib")
            .join(&runtime_target)
            .join(&library_name)
            .is_file()
    );
    let deployed = fs::read_to_string(package.root().join("Engine.vapor.toml")).unwrap();
    assert!(
        deployed.contains("binaries = [\"spacetime-engine\"]"),
        "{deployed}"
    );
    assert!(
        deployed.contains("libraries = [\"spacetime_engine\"]"),
        "{deployed}"
    );
    assert!(deployed.contains("[[engine.runtime]]"), "{deployed}");
    assert!(
        deployed.contains(&format!("target = \"{runtime_target}\"")),
        "{deployed}"
    );
    assert!(
        deployed.contains(&format!("binaries = [\"{binary_name}\"]")),
        "{deployed}"
    );
    assert!(
        deployed.contains(&format!("libraries = [\"{library_name}\"]")),
        "{deployed}"
    );
}

#[test]
fn content_package_can_stage_explicit_windows_gnu_runtime_outputs() {
    let installation = TestTree::new("content-windows-runtime-installation");
    installation.write(
        "App.vapor.toml",
        "[root]\nname = \"installation\"\norganization = \"example\"\n",
    );
    let executable = installation.write("bin/vapor", "binary");
    let target = "x86_64-pc-windows-gnullvm";
    installation.write(
        &format!("output/dev/loo-cast/{target}/debug/spacetime-engine.exe"),
        "engine executable",
    );
    installation.write(
        &format!("output/dev/loo-cast/{target}/debug/spacetime_engine.dll"),
        "engine library",
    );
    installation.write(
        "tools/llvm-mingw/x86_64-w64-mingw32/bin/libunwind.dll",
        "runtime dll",
    );

    let source = TestTree::new("content-windows-runtime-source");
    source.write(
        "Workspace.vapor.toml",
        "schema = 1\n\n[workspace]\nname = \"loo-cast\"\norganization = \"example\"\nversion = \"1.2.3\"\n\n[[workspace.projects]]\npath = \"spacetime-engine\"\n",
    );
    source.write("Cargo.toml", "[workspace]\nresolver = \"3\"\n");
    source.write(
        "spacetime-engine/Engine.vapor.toml",
        "schema = 1\n\n[engine]\nname = \"spacetime-engine\"\nversion.workspace = true\nbinaries = [\"spacetime-engine\"]\nlibraries = [\"spacetime_engine\"]\n\n[engine.steam]\napp-id = 2122620\npublished-file-id = \"1001\"\nvisibility = \"private\"\ntitle = \"Spacetime Engine\"\n",
    );
    source.write("spacetime-engine/src/main.rs", "fn main() {}\n");
    let paths = EnvironmentPaths::from_paths(&executable, source.root()).unwrap();

    let package =
        content::package_for_target(&paths, "spacetime-engine", false, Some(target)).unwrap();

    assert!(
        package
            .root()
            .join("bin/x86_64-pc-windows-gnullvm/spacetime-engine.exe")
            .is_file()
    );
    assert!(
        package
            .root()
            .join("bin/x86_64-pc-windows-gnullvm/libunwind.dll")
            .is_file()
    );
    assert!(
        package
            .root()
            .join("lib/x86_64-pc-windows-gnullvm/spacetime_engine.dll")
            .is_file()
    );
    assert!(
        package
            .root()
            .join("lib/x86_64-pc-windows-gnullvm/libunwind.dll")
            .is_file()
    );
    let deployed = fs::read_to_string(package.root().join("Engine.vapor.toml")).unwrap();
    assert!(deployed.contains("[[engine.runtime]]"), "{deployed}");
    assert!(
        deployed.contains("target = \"x86_64-pc-windows-gnullvm\""),
        "{deployed}"
    );
    assert!(
        deployed.contains("binaries = [\"spacetime-engine.exe\"]"),
        "{deployed}"
    );
    assert!(
        deployed.contains("libraries = [\"spacetime_engine.dll\"]"),
        "{deployed}"
    );
}

#[test]
fn source_content_uses_workspace_project_registration() {
    let installation = TestTree::new("content-registration-installation");
    installation.write(
        "App.vapor.toml",
        "[root]\nname = \"installation\"\norganization = \"example\"\n",
    );
    let executable = installation.write("bin/vapor", "binary");

    let source = TestTree::new("content-registration-source");
    source.write(
        "Workspace.vapor.toml",
        "schema = 1\n\n[workspace]\nname = \"loo-cast\"\norganization = \"example\"\nversion = \"1.2.3\"\n\n[[workspace.projects]]\npath = \"registered-engine\"\n",
    );
    source.write("Cargo.toml", "[workspace]\nresolver = \"3\"\n");
    source.write(
        "registered-engine/Engine.vapor.toml",
        "schema = 1\n\n[engine]\nname = \"registered-engine\"\nversion.workspace = true\n",
    );
    source.write("registered-engine/src/lib.rs", "pub fn engine() {}\n");
    source.write(
        "unregistered-game/Game.vapor.toml",
        "schema = 1\n\n[game]\nname = \"unregistered-game\"\nversion.workspace = true\n",
    );
    source.write("unregistered-game/src/lib.rs", "pub fn game() {}\n");

    let paths = EnvironmentPaths::from_paths(&executable, source.root()).unwrap();
    let catalog = content::discover(&paths).unwrap();

    assert_eq!(catalog.artifacts().len(), 1);
    assert!(catalog.find("registered-engine").is_some());
    assert!(catalog.find("unregistered-game").is_none());
}

#[test]
fn root_content_seed_resolves_public_workshop_selector() {
    let installation = TestTree::new("content-root-seed-installation");
    installation.write(
        "App.vapor.toml",
        "schema = 1\n\n[root]\nname = \"installation\"\norganization = \"example\"\n\n[[root.content]]\nid = \"example/loo-cast/loo-cast-packagepack\"\nkind = \"packagepack\"\napp-id = 2122620\nworkshop-id = \"1003\"\ndefault-launch = \"loo-cast\"\n",
    );
    let executable = installation.write("bin/vapor", "binary");
    let installation_paths = InstallationPaths::from_executable(&executable).unwrap();

    let error = content::acquire(&installation_paths, None, "loo-cast-packagepack", None)
        .expect_err("missing SteamCMD should be the first unresolved provider boundary");

    assert!(error.contains("SteamCMD is not installed"), "{error}");
    assert!(!error.contains("no matching source artifact"), "{error}");
}

#[test]
fn launch_loo_cast_uses_root_seed_not_active_source_package() {
    let installation = TestTree::new("launch-root-seed-installation");
    installation.write(
        "App.vapor.toml",
        "schema = 1\n\n[root]\nname = \"installation\"\norganization = \"example\"\n\n[[root.content]]\nid = \"ghf-studios/loo-cast/spacetime-engine\"\nkind = \"engine\"\napp-id = 2122620\nworkshop-id = \"1001\"\n\n[[root.content]]\nid = \"ghf-studios/loo-cast/loo-cast-game\"\nkind = \"game\"\napp-id = 2122620\nworkshop-id = \"1002\"\n\n[[root.content]]\nid = \"ghf-studios/loo-cast/loo-cast-packagepack\"\nkind = \"packagepack\"\napp-id = 2122620\nworkshop-id = \"1003\"\ndefault-launch = \"loo-cast\"\n",
    );
    let executable = installation.write("bin/vapor", "binary");
    let source = TestTree::new("launch-active-source");
    write_ghf_loo_cast_source_with_unbuilt_binary(&source);
    let paths = EnvironmentPaths::from_paths(&executable, source.root()).unwrap();
    let mut state = ShellState::new(paths).unwrap();

    let error = command::execute(
        ShellCommand::Launch {
            command: LaunchCommand::LooCast { account: None },
        },
        &mut state,
    )
    .expect_err("missing SteamCMD should be the first unresolved Play bootstrap boundary");

    assert!(error.contains("SteamCMD is not installed"), "{error}");
    assert!(
        !error.contains("declared content binary 'spacetime-engine' was not built"),
        "{error}"
    );
}

fn write_loo_cast_source(source: &TestTree) {
    source.write(
        "Workspace.vapor.toml",
        "schema = 1\n\n[workspace]\nname = \"loo-cast\"\norganization = \"example\"\nversion = \"1.2.3\"\n\n[[workspace.projects]]\npath = \"spacetime-engine\"\n\n[[workspace.projects]]\npath = \"loo-cast-game\"\n\n[[workspace.projects]]\npath = \"loo-cast-packagepack\"\n",
    );
    source.write("Cargo.toml", "[workspace]\nresolver = \"3\"\n");
    source.write(
        "spacetime-engine/Engine.vapor.toml",
        "schema = 1\n\n[engine]\nname = \"spacetime-engine\"\nversion.workspace = true\n\n[engine.steam]\napp-id = 2122620\npublished-file-id = \"1001\"\nvisibility = \"private\"\ntitle = \"Spacetime Engine\"\ntags = [\"first-party\"]\n",
    );
    source.write("spacetime-engine/src/lib.rs", "pub fn engine() {}\n");
    source.write(
        "loo-cast-game/Game.vapor.toml",
        "schema = 1\n\n[game]\nname = \"loo-cast-game\"\nversion.workspace = true\n\n[game.engine]\nid = \"example/loo-cast/spacetime-engine\"\n\n[game.steam]\napp-id = 2122620\npublished-file-id = \"1002\"\nvisibility = \"private\"\ntitle = \"Loo-Cast Game\"\n",
    );
    source.write("loo-cast-game/src/lib.rs", "pub fn game() {}\n");
    source.write(
        "loo-cast-packagepack/Packagepack.vapor.toml",
        "schema = 1\n\n[packagepack]\nname = \"loo-cast-packagepack\"\nversion.workspace = true\n\n[packagepack.steam]\napp-id = 2122620\npublished-file-id = \"1003\"\nvisibility = \"private\"\ntitle = \"Loo-Cast Packagepack\"\n\n[packagepack.engine]\nid = \"example/loo-cast/spacetime-engine\"\n\n[packagepack.game]\nid = \"example/loo-cast/loo-cast-game\"\n",
    );
    source.write("loo-cast-packagepack/src/lib.rs", "pub fn pack() {}\n");
}

fn write_ghf_loo_cast_source_with_unbuilt_binary(source: &TestTree) {
    source.write(
        "Workspace.vapor.toml",
        "schema = 1\n\n[workspace]\nname = \"loo-cast\"\norganization = \"ghf-studios\"\nversion = \"1.2.3\"\n\n[[workspace.projects]]\npath = \"spacetime-engine\"\n\n[[workspace.projects]]\npath = \"loo-cast-game\"\n\n[[workspace.projects]]\npath = \"loo-cast-packagepack\"\n",
    );
    source.write("Cargo.toml", "[workspace]\nresolver = \"3\"\n");
    source.write(
        "spacetime-engine/Engine.vapor.toml",
        "schema = 1\n\n[engine]\nname = \"spacetime-engine\"\nversion.workspace = true\nbinaries = [\"spacetime-engine\"]\n\n[engine.steam]\napp-id = 2122620\npublished-file-id = \"1001\"\nvisibility = \"private\"\ntitle = \"Spacetime Engine\"\n",
    );
    source.write("spacetime-engine/src/main.rs", "fn main() {}\n");
    source.write(
        "loo-cast-game/Game.vapor.toml",
        "schema = 1\n\n[game]\nname = \"loo-cast-game\"\nversion.workspace = true\n\n[game.engine]\nid = \"ghf-studios/loo-cast/spacetime-engine\"\n\n[game.steam]\napp-id = 2122620\npublished-file-id = \"1002\"\nvisibility = \"private\"\ntitle = \"Loo-Cast Game\"\n",
    );
    source.write("loo-cast-game/src/lib.rs", "pub fn game() {}\n");
    source.write(
        "loo-cast-packagepack/Packagepack.vapor.toml",
        "schema = 1\n\n[packagepack]\nname = \"loo-cast-packagepack\"\nversion.workspace = true\n\n[packagepack.steam]\napp-id = 2122620\npublished-file-id = \"1003\"\nvisibility = \"private\"\ntitle = \"Loo-Cast Packagepack\"\n\n[packagepack.engine]\nid = \"ghf-studios/loo-cast/spacetime-engine\"\nworkshop-id = \"1001\"\n\n[packagepack.game]\nid = \"ghf-studios/loo-cast/loo-cast-game\"\nworkshop-id = \"1002\"\n",
    );
    source.write("loo-cast-packagepack/src/lib.rs", "pub fn pack() {}\n");
}
