# Steam development workflow

## Launch and install

Vapor does not use `installscript.vdf` for first-run app install. Steam install scripts
are not available on Linux/SteamOS, and Vapor needs one cross-platform first-run
model for Windows and Linux testers.

## Steamworks app, packages, and depots

Vapor should publish one app build made from three root depots:

- **common depot**: OS-neutral `App.vapor.toml`, docs, app scripts, and
  examples.
  Steamworks OS rule: all operating systems.
- **linux depot**: Linux launch wrapper and Linux `bin/<target>/` runtime
  binaries. Steamworks OS rule: Linux.
- **windows depot**: Windows launch wrapper, Windows `bin/<target>/` runtime
  binaries, and required runtime DLLs. Steamworks OS rule: Windows.

The root `App-Source.vapor.toml` records those IDs and each depot's include
list under `[root.steam.depots.*]`. Do not publish a split build until the real
Steamworks depot IDs and file mappings are configured there:

```toml
[root.steam]
app-id = 2122620
development-branch = "vapor-dev"

[root.steam.depots.common]
id = 2122621

[[root.steam.depots.common.include]]
root = "source"
from = "App.vapor.toml"
to = "App.vapor.toml"
required = true

[root.steam.depots.linux]
id = 2122622

[[root.steam.depots.linux.include]]
root = "source"
from = "resources/vapor/shell-scripts/linux/vapor-launch.sh"
to = "bin/vapor-launch.sh"
required = true

[[root.steam.depots.linux.include]]
root = "installation"
from = "bin/x86_64-unknown-linux-gnu"
to = "bin/x86_64-unknown-linux-gnu"
target = "x86_64-unknown-linux-gnu"
required = true

[root.steam.depots.windows]
id = 2122623

[[root.steam.depots.windows.include]]
root = "source"
from = "resources/vapor/shell-scripts/windows/vapor-launch.cmd"
to = "bin/vapor-launch.cmd"
required = true

[[root.steam.depots.windows.include]]
root = "installation"
from = "bin/x86_64-pc-windows-gnullvm"
to = "bin/x86_64-pc-windows-gnullvm"
target = "x86_64-pc-windows-gnullvm"
required = true

[[root.steam.depots.windows.include]]
root = "installation"
from = "bin/x86_64-pc-windows-gnullvm/libunwind.dll"
to = "bin/x86_64-pc-windows-gnullvm/libunwind.dll"
target = "x86_64-pc-windows-gnullvm"
required = true
```

The project root has the full include list, including docs, launch wrappers,
and examples. Windows runtime DLLs belong in the promoted/imported
`bin/<windows-target>/` directory before depot staging.

Use packages as access/license containers, not as source branches:

- **developer package**: private/internal access to the app and every app depot;
- **closed-alpha/key package**: tester access to the same runtime depots;
- **future public/release package**: customer access once the app is ready.

Branches remain build channels. Keep `vapor-dev` as the development beta branch
for uploaded test builds. Add more branches only when there is a concrete build
promotion need, such as a stable alpha branch distinct from internal dev.

After a depot build that includes `bin/vapor-launch.*`, Steam launch options
should target the small platform wrapper. The wrapper runs
`vapor-installer install --app-root <app-root>` first, then hands off to the
installed Vapor binary for Play/Shell modes:

- **Linux Play Loo-Cast**: executable `bin/vapor-launch.sh`,
  arguments `play`.
- **Linux Vapor Shell**: executable `bin/vapor-launch.sh`,
  arguments `shell`.
- **Linux Vapor Installer**: executable `bin/vapor-launch.sh`,
  arguments `installer`.
- **Windows Play Loo-Cast**: executable `bin\vapor-launch.cmd`,
  arguments `play`.
- **Windows Vapor Shell**: executable `bin\vapor-launch.cmd`,
  arguments `shell`.
- **Windows Vapor Installer**: executable `bin\vapor-launch.cmd`,
  arguments `installer`.

If Steam refuses to execute a `.cmd` entry directly on Windows, use `cmd.exe`
as the executable and `/c "bin\vapor-launch.cmd" play`,
`/c "bin\vapor-launch.cmd" shell`, or
`/c "bin\vapor-launch.cmd" installer` as the arguments.

The Linux wrapper opens Konsole when Steam starts it without a terminal. The
Windows wrapper opens a persistent `cmd` window. Both wrappers are intentionally
thin; installation mechanics belong to Vapor Installer and product interaction
belongs to Vapor Shell. Running `vapor-installer` without arguments opens the
visual installer for human-driven lifecycle work; wrappers use only the
headless install command for Play/Shell. The `installer` wrapper mode skips
headless install and opens `vapor-installer` directly, so users can manage
install/uninstall/developer-mode state even when player-mode install is broken
or intentionally removed.

Player-mode install prepares only app-local basic runtime tooling:

- Git under `tools/git`;
- SteamCMD under `tools/steamcmd`;
- Vapor-Registry checkout under `.vapor/registry`;
- disposable app-local state, log, diagnostics, and content-cache directories.

It does not install Rust/Cargo or cross-build toolchains. Development tooling is
explicit:

```text
vapor-installer dev-env install --app-root /path/to/steam/app
vapor-installer dev-env uninstall --app-root /path/to/steam/app
```

If launch-time install fails, the first visible Vapor Shell reports the
failure, the log at `<app-root>/.vapor/logs/installer.log`, and the exact
installer command. For ordinary testers, reinstalling the Steam app is the
preferred recovery because the app root is disposable and should not hold
authoritative user data.

The `play` wrapper mode opens the normal interactive Vapor Shell, runs the
installed `resources/vapor/vapor-scripts/loo-cast.vapor` script, and leaves the
shell open.
That script currently calls `launch loo-cast --account ghf_vapor_build` so
SteamCMD authentication, Steam Guard prompts, Workshop download output, and
runtime handoff output stay visible in the terminal-owned session.

`launch loo-cast` verifies the selected installed Loo-Cast Packagepack,
resolves that packagepack's Spacetime Engine dependency, and hands off to the
installed engine binary for the host runtime target. If the packagepack is
missing, Vapor can download/cache/install/select the first-party Workshop
packagepack and dependencies from the app-root `[[root.content]]` seed once
SteamCMD is available.

The `--account` argument is currently needed while the app and Workshop items
are not publicly accessible to anonymous SteamCMD sessions. Once the app/content
is public, the same command can be tested without `--account`.

## Local development deploy bridge

The local bootstrap script is still only a developer bridge for placing the
first runnable binaries into a Steam app directory. It is not a product
installer and does not copy source repos, Cargo workspaces, staged package
trees, generated outputs, or user state.

A development loop now has two explicit phases:

1. Use the source-controlled bootstrap/deploy path to place current Vapor
   binaries and launch wrappers into the Steam app directory.
2. Run `vapor-installer dev-env install --app-root <app-root>` only when that
   app root needs to build, validate, package, or publish Vapor projects.

Release-mode depot builds should target every shipped app platform:

```text
root build
root publish --dry-run
```

The Windows GNU/LLVM target can be built from Linux with app-local llvm-mingw,
and the Linux GNU target can be built from Windows with the app-local Zig
wrapper model. If a target is built on another machine, preserve the same
`bin/<target>/` and `output/dev/<workspace>/<target>/debug/` relative paths
when copying artifacts back to the publishing app root.

For quick local Linux smoke, pass `--host-only`; Vapor then stages only the
host `bin/<target>/` directory plus the matching launch wrapper. When Windows
artifacts were imported from another machine, use
`root publish --skip-build --dry-run` only to preview staging. A real
`root publish` always validates, builds, promotes, stages, and uploads the full
declared Linux+Windows matrix.

The concrete Windows build and Linux handoff checklist is documented in
[`windows-gnullvm-release-proof.md`](windows-gnullvm-release-proof.md).

## Authentication

Vapor does not expose a standalone SteamCMD login workflow. Real publication is
the authentication boundary: `root publish --account NAME --yes` starts the
installation-owned SteamCMD with inherited stdin/stdout, lets Steam own any
password or mobile-authenticator prompts, and returns to the REPL after SteamCMD
exits. Vapor never accepts a password argument and never copies SteamCMD's
`config/` into staging.

Steam authentication is session-scoped by policy. Commands that publish for real
must be typed manually in the interactive shell; scripts may dry-run but may not
authenticate, perform real uploads, create Workshop items, or delete Workshop
items.

## Preview and publish

Use `root publish --dry-run` first. It validates, builds, promotes app binaries,
builds docs, stages and smoke-tests the split runtime depots, and writes an
app-build VDF with `Preview = 1` and `SetLive = vapor-dev` plus one depot-build
VDF per staged depot; it performs no upload and does not require active
SteamCMD.

A real upload requires both a non-default branch and explicit confirmation:

```text
root publish --account NAME --branch vapor-dev --yes
```

SteamCMD runs in the foreground so progress, prompts, exit status, and failure
remain attached to the operation. `output/root/steam-build` is not cleared by
staging; it contains SteamPipe manifests and chunk cache that improve subsequent
uploads.

The VDFs map only the already-clean split staging roots. Inclusion and
credential exclusion are therefore decided before SteamPipe sees any files.

Workshop content publication is separate from app/depot publication. Use
`content publish ARTIFACT --dry-run` from a content workspace for package and
Workshop VDF previews, then perform any real content upload manually with
`content publish ARTIFACT --account ACCOUNT --yes`.
