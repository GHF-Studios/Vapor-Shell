# App-local toolchain

The Vapor toolchain lives inside the Steam application root. Vapor does not use
system Rust, system Git, system SteamCMD, or user-data shim directories as
authoritative tooling.

The public lifecycle is intentionally small:

- `toolchain status` reports app-root registration, active tools, and vendored
  package health.
- `toolchain install` accepts the current app root, registers its `bin`
  directory for PATH setup, and installs missing Rust/Cargo, Git, and SteamCMD.
- `toolchain repair` accepts the current app root and reapplies every vendored
  package. Use it after an intentional Steam app move or suspected tool damage.
- `toolchain uninstall` removes app-local tools, PATH registration, and the
  app-root location record.

No workflow command installs or repairs prerequisites implicitly. Premature
commands stop with an actionable diagnostic and point to `toolchain status`,
`toolchain install`, or `toolchain repair`.

Mutating toolchain commands are intended to follow Vapor's broader
status-preview-repair model. The implemented baseline is explicit and manual;
the next command-surface pass should add dry-run previews for install, repair,
and uninstall before they mutate active tool directories, PATH registration, or
app-root location state.

## App-root registration

Vapor persists the accepted app-root path at:

```text
state/vapor-home.toml
```

The file lives inside the app root. If Steam moves the app, the file moves with
it while still recording the previous absolute path. `toolchain status` reports
that mismatch. `toolchain repair` is the explicit “yes, this move is intended”
operation.

Launching the Shell, SDK GUI, Launcher GUI, or a game never updates this state
implicitly.

## Vendored package layout

Steam delivers immutable install inputs beneath:

```text
packages/toolchain/
├── rustup/bin/rustup
├── rustup-home/toolchains/<toolchain>-<host>/bin/
├── cargo-home/
├── git/bin/git
└── steamcmd/steamcmd
```

`toolchain install` and `toolchain repair` copy them to active app-local paths:

```text
rustup/
rustup-home/
cargo-home/
tools/git/
tools/steamcmd/
```

Steam verification repairs the immutable package inputs. Vapor owns activation
from those packages.

The first bootstrap build must populate all package directories before it is
placed in Steam. Subsequent depots carry `packages/toolchain`, not one
developer machine's activated tool state or credentials.
