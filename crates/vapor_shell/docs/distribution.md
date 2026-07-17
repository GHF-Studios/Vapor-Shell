# Distribution and app/depot staging

Vapor-Root is the app/depot source root. Its `Vapor.toml` declares `[root]` and
`[root.steam]`; it does not package Workshop content into the app root and does
not duplicate Cargo or Git submodule membership.

```toml
[root.steam]
app-id = 2122620
depot-id = 2122621
development-branch = "vapor-dev"

[root.runtime]
targets = [
    "x86_64-unknown-linux-gnu",
    "x86_64-pc-windows-msvc",
]
```

`root publish --dry-run` reconstructs a clean Steam depot payload under
`output/root/content` inside the app root and writes a preview SteamPipe VDF
without uploading. The baseline payload is the runtime app only:

- `Vapor.toml`
- selected target-specific `bin/<target>/` app binaries
- `docs/`
- target-matching `.vapor/launch/<platform>/` wrappers when present in the root source
- `.vapor/scripts/` when runtime startup scripts are present in the root source
- `examples/vapor-examples/` when the root source includes official examples

Source repositories, Cargo build targets, Cargo registries, Git checkouts,
Steam authentication, logs, and SteamPipe cache state are not staged.
The examples payload is source material for learning and templating; it is
copied without its `.git/` or `target/` directories and is not Workshop content.

No target flag means host-only staging, so quick local Linux packages do not
require or advertise Windows launch wrappers. Repeat `--target` to stage
specific already-promoted target directories, or use `--release-targets` to
stage every target declared in `[root.runtime].targets`. Staging fails when a
selected `bin/<target>/` directory is missing.

Release staging is one unified app/depot content root. Linux and Windows
runtime binaries live side by side under `bin/<target>/`; Vapor does not create
separate app roots or require separate depot publication shapes just to ship
multiple operating systems.

For a release assembled from artifacts built on multiple machines, import the
target-specific app binaries into `bin/<target>/`, run
`root package --release-targets`, then use
`root publish --release-targets --skip-build --dry-run` so publish does not
rebuild imported targets.

Launch wrappers are the minimal OS-facing entrypoint split. They select the
installed platform binary and command mode:

```text
.vapor/launch/linux/vapor.sh    -> bin/x86_64-unknown-linux-gnu/vapor
.vapor/launch/windows/vapor.cmd -> bin/x86_64-pc-windows-msvc/vapor.exe
```

The real implementation remains Vapor Shell. Wrappers are not a substitute for
target-specific app or content payloads; Vapor itself, engines, games, tools,
and dynamic libraries all need runtime outputs staged per supported target.

The publish preflight requires installed app-local Rust/Cargo, Git, and
SteamCMD setup for the build and upload workflow. The large distributable
self-setup/toolchain payload is not part of the default root depot. Use
`root package --include-setup-payload` or
`root publish --include-setup-payload ...` only for an intentional stacked
bootstrap/depot package. That mode copies `packages/setup`; populate it
explicitly with `setup self package install` or refresh it with
`setup self package repair`. Active tool directories such as `rustup-home/`,
`cargo-home/`, `tools/git/`, and `tools/steamcmd/` are never staged directly.

Workshop content is separate from this depot flow. A workspace such as
Loo-Cast can publish Workshop items and packagepacks, but the content itself is
not part of Vapor-Root's app payload.
