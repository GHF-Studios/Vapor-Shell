# Vapor installation

Normal closed-alpha testers should not run manual installer commands before first
launch. The Steam app starts through the platform entrypoint, the launch script
runs `vapor-installer install`, and Vapor Shell or Play opens after basic
app-local tooling is ready.

Steam install scripts are not used for this path. Vapor requires one
cross-platform first-run model; Valve's installscript support is not available
on Linux/SteamOS, so tying first-run install to `installscript.vdf` would create a
Windows-only product behavior.

## Installer-owned paths

`Vapor-Installer` owns installation and uninstallation mechanics for the Steam
app root. Running `vapor-installer` with no arguments opens the visual installer
for human-driven install, uninstall, and developer-mode upgrade/downgrade
flows. Steam should expose Vapor Installer as its own launch option through the
same platform entrypoint with the `installer` argument. The headless
subcommands exist for launch scripts and automation:

```text
vapor-installer install
vapor-installer uninstall
vapor-installer dev-env install
vapor-installer dev-env uninstall
```

The app root is disposable. Installer-managed state under the app root is
recreateable tooling, caches, logs, receipts, and launch install state.
Authoritative user progress or account data must live in OS-appropriate user
data directories, not primarily in the Steam application directory.

The default install prepares player-mode runtime functionality:

- app-local SteamCMD under `tools/steamcmd`;
- app-local generated directories for logs, diagnostics, content cache,
  installed content state, and Workshop downloads.

It does not install Rust/Cargo, Zig, llvm-mingw, or other general development
tooling.

Developer-mode upgrade/downgrade is explicit and installer-owned:

```text
vapor-installer dev-env install --app-root /path/to/steam/app
vapor-installer dev-env uninstall --app-root /path/to/steam/app
```

Uninstall is intentionally split between installer-owned state and Steam-owned
files:

1. Optional: `vapor-installer dev-env uninstall --app-root /path/to/steam/app`
   downgrades developer mode back to player mode by removing Rust/Cargo and
   cross-build tooling.
2. `vapor-installer uninstall --app-root /path/to/steam/app` removes every
   installer-owned mutable path: Rust/Cargo and cross-build tooling if present,
   obsolete app-local Git/registry state if present, SteamCMD,
   downloads/extracts, generated `.vapor` state, diagnostics/logs, generated
   `content/` state, and
   `output/`. It does not remove depot-owned binaries, docs, examples, launch
   entrypoints, scripts, or `App.vapor.toml`.
3. Steam's uninstall feature removes the depot-owned application files,
   including Vapor Shell, docs, launch entrypoints, scripts, and the installer
   itself.

No uninstall command removes user-authored source checkouts outside the app
root.

General development commands such as build, validate, package, and publish
remain Vapor Shell commands. If those commands need Rust/Cargo or cross-build
tools, they should report the missing development environment and point to the
installer command above.

Git is not a player-mode install dependency. Developer workflows that need Git
use a linked provider resolved by Vapor Shell:

```text
provider git status
provider git link /path/to/git
provider git clear
```

Vapor Shell first honors `VAPOR_GIT`, then the linked provider state, then
PATH/common OS install locations. If no usable Git exists, the developer must
link one explicitly.

## Logging

Installer operations write to:

```text
<app-root>/.vapor/logs/installer.log
```

External helper tools launched by the installer, such as SteamCMD archive
download/extraction tools or Rust toolchain installers, write stdout/stderr to
that log instead of directly to the Steam-visible terminal. The user-facing
terminal should show the launch wrapper, the installer report or Shell status,
and explicit failures.

If launch-time install fails, the first visible Vapor Shell reports what
failed, the installer log path, and the exact installer command. The launch
script records that failure under:

```text
<app-root>/.vapor/state/installer/bootstrap-failure.txt
```

For normal testers, the preferred recovery is reinstalling the Steam app
because the app root should contain no authoritative user state.
