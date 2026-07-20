# Vapor manifests

Vapor manifest filenames are role-specific where the filesystem role matters.
Application source roots use `App-Source.vapor.toml`; installed application
roots use `App.vapor.toml`; source workspaces use `Workspace.vapor.toml`;
registry checkouts use `Registry.vapor.toml`; content artifacts use their role
filename, such as `Engine.vapor.toml`, `Game.vapor.toml`, or
`Packagepack.vapor.toml`. `Cargo.toml` is the Cargo manifest.
Avoid unqualified “manifest” when several could be meant.

Vapor manifests describe Vapor-owned information: identity, content role,
composition, capabilities, publication policy, authority, and managed
relationships. They do not duplicate Cargo package membership, Rust
dependencies, crate targets, or generated Cargo metadata.

## One file, one declared identity

Every Vapor manifest declares exactly one identity-bearing section.

- `[root]`, `[workspace]`, and `[registry]` require `name` and `organization`.
- Content sections require local `name`.
- Declaration-side `id` is invalid.
- The filename and table name select the role and must match.
- References still use fully-qualified `id`.

In declarations, `name` means “the local segment declared here.” In references,
`id` means “the already-declared thing being referenced.”

## Source roots

### Application source root

Filename: `App-Source.vapor.toml`.

```toml
schema = 1

[root]
name = "vapor-root"
organization = "ghf-studios"
version = "0.5.0"
repository = "https://github.com/GHF-Studios/Vapor-Root"

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

`[root]` declares a Vapor application source root: a pure Vapor-managed Git
super-repository that assembles and publishes a Steam app/depot. It is not a
Cargo workspace. Direct Git submodules that contain `[workspace]` manifests and
`Cargo.toml` become app member workspaces.

This is source, not the Steam installation/app root. The installed app root
carries the runtime marker `App.vapor.toml`.
`[root.runtime].targets` declares the release target matrix for app/depot
builds and staging. Target-aware root commands consume this matrix by default;
use `--host-only` only for local package/dry-run smoke passes.
`[root.steam.depots]` declares the Steamworks depot IDs and explicit include
lists used by split-depot root publication. Each include copies one file or
directory from either the source checkout or installation/app root into that
depot. `target` makes an include apply only when that runtime target is staged.
The native `vapor-entrypoint[.exe]` launch adapter is a promoted application
binary inside `bin/<target>/`, so it is staged through the target-directory
include rather than through a separate source include. The numeric IDs in this
example are Vapor's current Steamworks depot IDs.

### Normal source workspace

Filename: `Workspace.vapor.toml`.

```toml
schema = 1

[workspace]
name = "loo-cast"
organization = "ghf-studios"
version = "0.1.0"
repository = "https://github.com/GHF-Studios/Loo-Cast"

[workspace.runtime]
targets = [
    "x86_64-unknown-linux-gnu",
    "x86_64-pc-windows-gnullvm",
]
```

`[workspace]` declares one source repository that is also one Cargo workspace.
Its root must contain `Cargo.toml`.

A workspace may contain several Cargo packages, several Vapor content artifacts, and
several publishable Workshop artifacts.
`[workspace.runtime].targets` declares the release target matrix for content
build/package/deploy/create/publish operations. Target-aware content commands
consume this matrix by default; use `--host-only` for local host-only smoke
passes.

Application workspaces that contribute installed commands may add
`binaries = ["name"]` under `[workspace]`. `root build` promotes those Cargo
binary outputs from `output/dev/<workspace>/debug/` or
`output/dev/<workspace>/<target>/debug/` into the app root's
`bin/<target>/`. Content-only workspaces should omit it.

## Content packages

Every source-authored Vapor content artifact is normally a Cargo package inside
a Vapor workspace. Ordinary non-content crates are owned by Cargo only; they do
not need their own Vapor manifest. Content packages use their content role and
must use the matching filename:

```toml
schema = 1

[engine]
name = "spacetime-engine"
version.workspace = true

[engine.steam]
app-id = 2122620
visibility = "private"
title = "Spacetime Engine"
tags = ["engine", "first-party", "loo-cast"]
```

Supported content filenames and sections are:

- `Engine.vapor.toml` with `[engine]`
- `Game.vapor.toml` with `[game]`
- `Packagepack.vapor.toml` with `[packagepack]`
- `Enginepack.vapor.toml` with `[enginepack]`
- `Gamepack.vapor.toml` with `[gamepack]`
- `Modpack.vapor.toml` with `[modpack]`
- `Engine-Mod.vapor.toml` with `[engine-mod]`
- `Game-Mod.vapor.toml` with `[game-mod]`
- `Extension-Mod.vapor.toml` with `[extension-mod]`

For source root `ghf-studios/loo-cast`, `[engine] name =
"spacetime-engine"` declares:

```text
ghf-studios/loo-cast/spacetime-engine
```

## Composition

Composition references other artifacts, so it uses full IDs:

```toml
[packagepack.engine]
id = "ghf-studios/loo-cast/spacetime-engine"

[packagepack.game]
id = "ghf-studios/loo-cast/loo-cast-game"
```

Cargo dependencies remain Rust build dependencies. Vapor dependencies describe
content composition, packaging, compatibility, publication, and installation
relationships.

A game may also declare the engine content it is authored against:

```toml
[game.engine]
id = "ghf-studios/loo-cast/spacetime-engine"
```

That Vapor relationship is metadata authority. The concrete Rust API dependency
belongs in `Cargo.toml` and should eventually be generated or repaired from the
Vapor relationship instead of hand-maintained.

Packagepacks, Enginepacks, Gamepacks, and Modpacks can also exist as simpler
Vapor metadata assembled by the Launcher or runtime UI when no source-backed
Rust package is needed. Those dynamic pack manifests are installed/content
state, not Cargo workspace membership.

Current design rules:

- an enginepack contains exactly one engine and zero or more engine mods;
- a gamepack contains exactly one game and zero or more game mods;
- a modpack contains engine mods, game mods, and extension mods;
- a packagepack contains either one engine or one enginepack, either one game
  or one gamepack, zero or more modpacks, and optional direct mods;
- extension mods can extend any mod, including another extension mod.

There is no separate `binding` declaration in the design model. Composition
edges and slot/provider relationships are resolved relationships between
declared artifacts and capabilities.

## Packagepack workspace shape

A packagepack that bundles an engine and a game should live beside those
constituents in a composition workspace instead of being nested under either
constituent:

```text
Loo-Cast/
├── Workspace.vapor.toml       [workspace]
├── Cargo.toml                 Cargo workspace
├── spacetime-engine/          Engine.vapor.toml
├── loo-cast-game/             Game.vapor.toml
└── loo-cast-packagepack/      Packagepack.vapor.toml
```

That workspace is Workshop/content source. It is separate from Vapor-Root,
which is app/depot source.

## Traits and slots

Traits are project/content-owned marker capabilities:

```toml
[[traits]]
name = "replacement-render-backend"
cardinality = "zero-or-one"
```

Slots require providers through fully-qualified trait IDs:

```toml
[[slots]]
name = "replacement-render-backend"
trait = "ghf-studios/loo-cast/spacetime-engine/replacement-render-backend"
```

Rules:

- traits are marker capabilities, not content roles;
- content kinds such as engine and game are not traits;
- shared traits belong in a contracts project when they are genuinely shared;
- slots do not own cardinality;
- the referenced trait owns cardinality as part of the capability contract;
- provider declaration syntax is intentionally not finalized yet.

## Names, IDs, and versions

Declarations use local names. Full IDs are inferred:

```text
workspace: ghf-studios/loo-cast
content:   ghf-studios/loo-cast/spacetime-engine
trait:     ghf-studios/loo-cast/spacetime-engine/replacement-render-backend
```

Public references use stable fully-qualified IDs. Relative paths are reserved
for private, local, or bundled relationships.

Versions are artifact-owned and inheritance-friendly. `version.workspace = true`
is the normal default. A project or content artifact should own an explicit
semantic version only when its release lifecycle diverges from the workspace.

Content artifacts may declare runtime outputs built by Cargo:

```toml
[engine]
name = "spacetime-engine"
version.workspace = true
binaries = ["spacetime-engine"]
libraries = ["spacetime_engine"]
```

Packaging copies declared binaries into `bin/<target>/` and declared libraries
into `lib/<target>/` inside the deployed artifact root. The deployed
role-specific content manifest keeps the authored logical names and adds
target-specific runtime entries with the actual staged filenames:

```toml
[[engine.runtime]]
target = "x86_64-pc-windows-gnullvm"
binaries = ["spacetime-engine.exe"]
libraries = ["spacetime_engine.dll"]
```

Use this for content-owned tools, helper executables, native runtime libraries,
or side processes needed by the artifact. Root launch options decide which root
entrypoint Steam exposes; content runtime outputs remain content payload and
must be shipped once per supported target.

## Workshop fields

Content artifacts declare stable Workshop intent in their own table:

```toml
[packagepack.steam]
app-id = 2122620
published-file-id = "1234567890"
visibility = "private"
title = "Loo-Cast Packagepack"
description = "First-party playable packagepack for Loo-Cast."
tags = ["packagepack", "first-party", "loo-cast"]
change-note = "Vapor content update."
```

Use `published-file-id` only after the artifact has a real stable Workshop
item. Local package fingerprints, cache locations, installed receipts, repair
state, and last-seen provider observations are generated app-owned state and
must stay out of source role-specific content manifests.

## Invalid combinations

This is rejected because a declaration uses the removed `id` field:

```toml
[game]
name = "loo-cast-game"
id = "ghf-studios/loo-cast/loo-cast-game"
```

This is rejected because a file declares two identities:

```toml
[game]
name = "loo-cast-game"

[game-mod]
name = "campaign-expansion"
```

This is rejected at a source root because content belongs in a Cargo package
directory, not as the source-root identity:

```toml
[engine]
name = "spacetime-engine"
```

See `docs/design/manifest-schema.md` for the fuller schema checkpoint and
`docs/design/product-topology.md` for the product model behind it.
