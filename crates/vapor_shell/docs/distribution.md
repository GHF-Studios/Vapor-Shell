# Distribution and app/depot staging

Vapor-Root is the app/depot source root. Its `App-Source.vapor.toml` declares
`[root]`, `[root.steam]`, and the split-depot file membership. The shipped app
root carries `App.vapor.toml` as its runtime app marker. Vapor-Root does not
package Workshop content into the app root and does not duplicate Cargo or Git
submodule membership.

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

[root.runtime]
targets = [
    "x86_64-unknown-linux-gnu",
    "x86_64-pc-windows-gnullvm",
]
```

The depot IDs are the Steamworks DepotIDs. The include list is the source of
truth for file membership. `root = "source"` copies from the Vapor-Root source
checkout; `root = "installation"` copies from the Steam app root. `target`
limits an include to package/publish commands that stage that runtime target.

`root publish --dry-run` reconstructs clean Steam depot payloads under
`output/root/content` inside the app root and writes a preview SteamPipe app
build VDF plus one depot build VDF per staged depot without uploading. The
payload is split by the manifest-declared Steam depot includes:

- `common/` contains OS-neutral files: `App.vapor.toml`, `docs/`,
  `resources/vapor/vapor-scripts/` when runtime startup scripts are present, and
  `examples/vapor-examples/` when the root source includes official examples.
- `linux/` contains Linux runtime files: selected Linux `bin/<target>/`
  application binaries including `vapor-entrypoint`, plus
  `bin/vapor-launch.sh`.
- `windows/` contains Windows runtime files: selected Windows `bin/<target>/`
  application binaries including `vapor-entrypoint.exe`, required Windows
  runtime DLLs, plus `bin/vapor-launch.cmd`.

Source repositories, Cargo build targets, Cargo registries, Git checkouts,
Steam authentication, installer logs, generated app-root state, and SteamPipe
cache state are not staged.
The examples payload is source material for learning and templating; it is
copied without its `.git/` or `target/` directories and is not Workshop content.
SteamPipe VDF file bodies live as checked-in source templates under
`resources/steam/steampipe-templates/`; Vapor Shell loads them from the active
source checkout and fills IDs, paths, branch, and description values at publish
time. The templates are distribution policy, not Shell crate assets and not
runtime depot content unless `App-Source.vapor.toml` explicitly includes them.

Source-controlled support assets live under visible `resources/` paths. The
installed app-root `.vapor/` directory is reserved for disposable generated
state such as logs, registry checkout, content cache, diagnostics cache, and
receipts.

When `[root.runtime].targets` is declared, no target flag means release-matrix
staging. Repeat `--target` to stage a deliberate custom subset of
already-promoted target directories. Use `--host-only` for quick local Linux
packages that should not require or advertise Windows launch scripts. Staging
always includes the common depot plus the platform depots implied by the
selected targets, and fails when a selected `bin/<target>/` directory is
missing.

Real `root publish` does not accept custom target subsets, `--host-only`, or
`--skip-build`. It validates, builds, promotes, stages, and uploads the complete
declared Linux+Windows runtime matrix. Those escape hatches exist only for
`root package` and `root publish --dry-run` staging previews.

Release staging is one app build with split depot roots. Steamworks should
mount the common depot for every OS, the Linux depot only for Linux, and the
Windows depot only for Windows. This prevents Linux Steam clients from selecting
the Windows/Proton launch path because of a mixed mono-depot payload.

For a release assembled from artifacts built on multiple machines, import the
target-specific app binaries into `bin/<target>/`, run
`root package`, then use `root publish --skip-build --dry-run` only as a
preview that confirms the imported files stage correctly.

Steam-facing native entrypoints are the minimal terminal adapter split. They
open the platform terminal, forward arguments unchanged to the matching launch
script, and wait for the terminal to close:

```text
bin/x86_64-unknown-linux-gnu/vapor-entrypoint play
  -> Konsole -> bin/vapor-launch.sh play
  -> bin/x86_64-unknown-linux-gnu/vapor

bin/x86_64-unknown-linux-gnu/vapor-entrypoint installer
  -> Konsole -> bin/vapor-launch.sh installer
  -> bin/x86_64-unknown-linux-gnu/vapor-installer

bin\x86_64-pc-windows-gnullvm\vapor-entrypoint.exe play
  -> cmd /K -> bin\vapor-launch.cmd play
  -> bin\x86_64-pc-windows-gnullvm\vapor.exe

bin\x86_64-pc-windows-gnullvm\vapor-entrypoint.exe installer
  -> cmd /K -> bin\vapor-launch.cmd installer
  -> bin\x86_64-pc-windows-gnullvm\vapor-installer.exe
```

The launch scripts run the shipped `vapor-installer install` first for
Play/Shell modes, then select the installed platform binary and command mode.
Installer mode opens `vapor-installer` directly and skips headless install. The
real implementation remains Vapor Shell. Entry points and scripts are not a
substitute for target-specific app or content payloads; Vapor itself, engines,
games, tools, and dynamic libraries all need runtime outputs staged per
supported target.

The publish preflight requires app-local Rust/Cargo and cross-build tooling for
the release matrix, a linked developer Git provider for explicit Git-backed
workflows, plus SteamCMD for real upload. Player-mode tooling belongs to
`vapor-installer install`; explicit development toolchain upgrade/downgrade
belongs to `vapor-installer dev-env`. The root depots contain release runtime
files only; installer-managed tools and generated app-local state stay outside
SteamPipe staging.

Workshop content is separate from this depot flow. A workspace such as
Loo-Cast can publish Workshop items and packagepacks, but the content itself is
not part of Vapor-Root's app payload.
