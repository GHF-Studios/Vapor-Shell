# Vapor setup

Vapor setup lives inside the Steam installation/app root. Vapor does not
use system Rust, system SteamCMD, or user-data shim directories as authoritative
tooling.

There is one mandatory installed toolchain bundle:

- Rust/Cargo through app-local `rustup`, `rustup-home`, and `cargo-home`;
- Git through `tools/git`;
- SteamCMD through `tools/steamcmd`.

Git must be an app-owned distribution. A script that delegates to host `git`,
for example `/usr/bin/git`, is rejected by toolchain health checks and cannot be
used as distributable package content.

The public lifecycle is intentionally small and intentionally breaking from the
older `toolchain` command surface:

- `setup status` reports app-root registration and installed Rust/Cargo,
  Git, and SteamCMD health.
- `setup install` accepts the current app root, registers its `bin`
  directory for PATH setup, and installs missing Rust/Cargo, Git, and SteamCMD.
- `setup repair` accepts the current app root and reinstalls setup components.
  Use it after an intentional Steam app move or suspected tool damage.
- `setup uninstall` removes app-local tools, PATH registration, and the
  app-root location record.
- `setup package install` and `setup package repair` populate
  `packages/toolchain`, the
  distributable package content used by app/depot staging. They are separate
  from active setup installation and are never run implicitly after
  bootstrap.

No workflow command installs or repairs prerequisites implicitly. Premature
commands stop with an actionable diagnostic and point to `setup status`,
`setup install`, or `setup repair`.

Mutating setup commands follow Vapor's status-preview-repair model. Use
`--dry-run` with `setup install`, `setup repair`, or `setup
uninstall` to preview active tool directories, PATH registration, and app-root
location changes before applying them.

## App-root registration

Vapor persists the accepted app-root path at:

```text
.vapor/state/vapor-home.toml
```

The file lives inside the app root's `.vapor` metadata area. If Steam moves the
app, the file moves with it while still recording the previous absolute path.
`setup status` reports that mismatch. `setup repair` is the explicit
“yes, this move is intended” operation.

Launching the Shell, SDK GUI, Launcher GUI, or a game never updates this state
implicitly.

## Setup installation

Explicit `setup install` performs app-local installation into the app root:

- Rust is installed through `rustup-init` with `RUSTUP_HOME` and `CARGO_HOME`
  pointing inside the app root.
- Git is applied from a complete app-owned `packages/toolchain/git` package.
  Host Git wrappers are not accepted.
- SteamCMD is downloaded and extracted under `tools/steamcmd`.

This is still app-local operation: active tools and build outputs live under the
Steam app root, and no workflow command performs this installation implicitly.

## Installed layout

The installed toolchain uses these app-owned paths:

```text
rustup/
rustup-home/
cargo-home/
tools/git/
tools/steamcmd/
```

Distributable package content lives separately:

```text
packages/toolchain/
```

Commands that need Cargo, Git, or SteamCMD validate these installed paths
directly. If anything is missing, the command stops and tells the operator to run
`setup status`, `setup install`, or `setup repair`.

Final app/depot staging copies `packages/toolchain`, not the live active
tool directories. Populate or refresh that package content explicitly with
`setup package install` or `setup package repair`.
