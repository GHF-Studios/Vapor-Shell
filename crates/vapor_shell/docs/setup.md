# Vapor installation and local setup

Normal closed-alpha testers should not run manual setup commands before first
launch. The Steam app starts through the platform entrypoint, the launch script
calls the shipped `vapor-installer` shim, and that shim delegates to the
app-root tool layer.

Canonical tools live in the installed app root:

```text
<app-root>/resources/vapor/tools/
```

The source copy under `Vapor-Root/resources/vapor/tools/` is payload source for
the next app/root local publish or global publish. Runtime workflows should call
the app-root copy.

## App-root operations

```text
rust-script --force <app-root>/resources/vapor/tools/production/app_setup/status.rs
rust-script --force <app-root>/resources/vapor/tools/production/app_setup/setup_player.rs
rust-script --force <app-root>/resources/vapor/tools/production/app_setup/setup_development.rs
rust-script --force <app-root>/resources/vapor/tools/production/app_setup/teardown_development.rs
rust-script --force <app-root>/resources/vapor/tools/production/app_setup/teardown_player.rs
```

When run from `<app-root>/resources/vapor/tools`, setup scripts infer the app
root from their own deployed path. The resolver also checks `LOO_CAST_APP_ROOT`,
the app-root anchor file, the installed binary layout, common Steam library
locations, `libraryfolders.vdf`, and `appmanifest_2122620.acf`.

## SuperWorkspace bootstrap

Create and patch a SuperWorkspace before opening RustRover:

```text
rust-script --force <app-root>/resources/vapor/tools/development/superworkspace/create.rs --path /path/to/SuperWorkspace
rust-script --force <app-root>/resources/vapor/tools/development/source_setup/clone.rs --super-workspace /path/to/SuperWorkspace --all
rust-script --force <app-root>/resources/vapor/tools/development/ide_setup/patch_rustrover.rs --super-workspace /path/to/SuperWorkspace
```

`clone.rs` clones sources into `SuperWorkspace/sources/`.

## Installer shim

`vapor-installer` remains an app binary wrapper for launch scripts and old
command habits. It does not make the SuperWorkspace own tools.

The app root is disposable. Script-managed state under the app root is
recreateable tooling, caches, logs, receipts, and launch install state.
Authoritative user progress or account data must live in OS-appropriate user
data directories, not primarily in the Steam application directory.
