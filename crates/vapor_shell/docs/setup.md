# Vapor setup

Vapor setup lives inside the Steam installation/app root. Vapor does not
use system Rust, system SteamCMD, or user-data shim directories as authoritative
tooling.

There is one mandatory app-local setup:

- Rust/Cargo through app-local `rustup`, `rustup-home`, and `cargo-home`;
- Git through `tools/git`;
- SteamCMD through `tools/steamcmd`.

Git must be an app-owned distribution. A script that delegates to host `git`,
for example `/usr/bin/git`, is rejected by setup health checks and cannot be
used as a distributable self-setup payload.

The self-setup lifecycle is intentionally small:

- `setup self status` reports app-root registration and installed Rust/Cargo,
  Git, and SteamCMD health.
- `setup self install` accepts the current app root, registers its `bin`
  directory for PATH setup, and installs missing Rust/Cargo, Git, and SteamCMD.
- `setup self repair` accepts the current app root and reinstalls setup components.
  Use it after an intentional Steam app move or suspected tool damage.
- `setup self uninstall` removes app-local tools, PATH registration, and the
  app-root location record.
- `setup self package install` and `setup self package repair` populate
  `packages/setup`, the distributable self-setup payload used by app/depot
  staging. They are separate from active self-setup installation and are never
  run implicitly after bootstrap.

No workflow command installs or repairs prerequisites implicitly. Premature
commands stop with an actionable diagnostic and point to `setup self status`,
`setup self install`, or `setup self repair`.

Mutating self-setup commands follow Vapor's status-preview-repair model. Use
`--dry-run` with `setup self install`, `setup self repair`, or `setup
self uninstall` to preview active tool directories, PATH registration, and
app-root location changes before applying them.

## App-root registration

Vapor persists the accepted app-root path at:

```text
.vapor/state/vapor-home.toml
```

The file lives inside the app root's `.vapor` metadata area. If Steam moves the
app, the file moves with it while still recording the previous absolute path.
`setup self status` reports that mismatch. `setup self repair` is the explicit
“yes, this move is intended” operation.

Launching the Shell, SDK GUI, Launcher GUI, or a game never updates this state
implicitly.

## Self-Setup Installation

Explicit `setup self install` performs app-local installation into the app root:

- Rust is installed through `rustup-init` with `RUSTUP_HOME` and `CARGO_HOME`
  pointing inside the app root.
- Git is applied from a complete app-owned `packages/setup/git` payload when
  one exists. Otherwise Linux setup imports a usable host Git binary into
  `tools/git`, copies its Git exec-path support files, and replaces delegating
  scripts with an app-owned launcher.
- SteamCMD is downloaded and extracted under `tools/steamcmd`.

This is still app-local operation: active tools and build outputs live under the
Steam app root, and no workflow command performs this installation implicitly.

## Installed layout

The installed setup uses these app-owned paths:

```text
rustup/
rustup-home/
cargo-home/
tools/git/
tools/steamcmd/
```

Distributable self-setup payloads live separately:

```text
packages/setup/
```

Commands that need Cargo, Git, or SteamCMD validate these installed paths
directly. If anything is missing, the command stops and tells the operator to run
`setup self status`, `setup self install`, or `setup self repair`.

Final app/depot staging copies `packages/setup`, not the live active
tool directories. Populate or refresh those payloads explicitly with
`setup self package install` or `setup self package repair`.
