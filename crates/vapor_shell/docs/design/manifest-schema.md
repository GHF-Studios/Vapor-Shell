# Vapor manifest schema

Status: **implemented baseline; extended by design constraints as workflows harden**

This document describes the current Vapor manifest model. The implementation
validates the baseline identity shape today; later passes will add deeper
semantic validation for composition, traits, slots, publication, and authority.

`Cargo.toml` remains authoritative for Cargo packages, workspace membership,
Rust dependencies, features, crate targets, and build behavior. `Vapor.toml`
records Vapor-owned identity, content role, composition, capability,
publication, installation, and authority metadata.

## Universal rules

- Every manifest starts with `schema = 1`.
- Every manifest declares exactly one primary identity section.
- Declaration names and organization names use lowercase kebab-case.
- Root, workspace, registry, project, content, and trait declarations use local
  `name`; they do not repeat their fully-qualified identifier.
- Declaration-side `id` is invalid.
- `[project].kind` is invalid; the identity section chooses the role.
- A root, workspace, or registry identifier is inferred as
  `organization/name`.
- A project or content identifier is inferred as
  `organization/workspace/project`.
- A trait identifier is inferred as
  `organization/workspace/project/trait`.
- Repository URLs preserve canonical GitHub owner and repository spelling.
- References use fully-qualified identifiers. An `id` field therefore denotes a
  reference, never the declaration containing that field.
- Publishable references use stable identifiers.
- Relative paths are reserved for private, local, or bundled relationships.
- Generated resolution, hashes, receipts, and installed state do not belong in
  source manifests.

## Source-root manifests

### Application source root

The application source root is a pure Vapor-managed Git super-repository, not
the Steam installation/app root and not a Cargo workspace:

```toml
schema = 1

[root]
name = "vapor-root"
organization = "ghf-studios"
version = "0.5.0"
repository = "https://github.com/GHF-Studios/Vapor-Root"

[root.steam]
app-id = 2122620
depot-id = 2122621
development-branch = "vapor-dev"

[root.runtime]
targets = [
    "x86_64-unknown-linux-gnu",
    "x86_64-pc-windows-msvc",
]

[[root.content]]
id = "ghf-studios/loo-cast/loo-cast-packagepack"
kind = "packagepack"
app-id = 2122620
workshop-id = "3762162548"
default-launch = "loo-cast"
```

Direct Git submodules define its application/depot workspace membership. Each
member must be a `[workspace]` repository with a root `Cargo.toml`.
`[root.runtime].targets` declares the app/depot release target matrix consumed
by target-aware root commands by default. Use `--host-only` for local host-only
smoke passes.

`[[root.content]]` records first-party installed-app discovery seeds for public
Workshop content that the app may need before any external source checkout or
registry client is available. It is not installed state; it is source-authored
identity/provider metadata and should later be mirrored by the reviewed
Vapor-Registry records.

Workshop compositions such as Loo-Cast live in separate `[workspace]`
repositories and are not submodules of Vapor-Root merely because they are
first-party content.

### Workspace

A workspace source repository is one Vapor workspace and one Cargo workspace
rooted in the same directory:

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
    "x86_64-pc-windows-msvc",
]
```

Cargo metadata defines package membership. The Vapor manifest does not repeat
Cargo member paths. Vapor project registration is a separate relationship:
`[[workspace.projects]]` declares which child paths are Vapor-managed projects
inside this source workspace.
`[workspace.runtime].targets` declares the content release target matrix
consumed by target-aware content commands by default. Use `--host-only` for
local host-only smoke passes.

```toml
[[workspace.projects]]
path = "spacetime-engine"

[[workspace.projects]]
path = "loo-cast-game"

[[workspace.projects]]
path = "loo-cast-packagepack"
```

Each registered path must contain its own `Vapor.toml` with either `[project]`
or a content identity section such as `[engine]`, `[game]`, or
`[packagepack]`. The child manifest owns the project/content identity and role;
the workspace manifest only owns membership. Unregistered nested manifests are
not source content merely because they exist on disk.

A workspace that contributes installed app commands may declare
`binaries = ["name"]` under `[workspace]`; root workflows promote those Cargo
binary outputs into the app root's `bin/<target>/`. Content workspaces should
not declare promoted app binaries.

### Registry

The registry is infrastructure authority, not a workspace and not a Cargo
package:

```toml
schema = 1

[registry]
name = "vapor-registry"
organization = "ghf-studios"
repository = "https://github.com/GHF-Studios/Vapor-Registry"
authority = "github.com/GHF-Studios/Vapor-Registry"
```

Registry data verifies declared organization, inferred identity, containment,
and first-party authority. Naming an organization in a source manifest is a
namespace claim, not authorization.

## Project and content manifests

Every Vapor project is authored under a registered `[[workspace.projects]]`
path in a Vapor workspace. A non-content package uses `[project]`:

```toml
schema = 1

[project]
name = "cli"
version.workspace = true
```

A content artifact uses its content kind instead:

```toml
schema = 1

[engine]
name = "spacetime-engine"
version.workspace = true

[engine.steam]
app-id = 2122620
visibility = "private"
title = "Spacetime Engine"
description = "First-party Vapor engine content for Loo-Cast."
tags = ["engine", "first-party", "loo-cast"]
change-note = "Vapor content update."
```

Supported content identity sections:

- `[engine]`
- `[game]`
- `[engine-mod]`
- `[game-mod]`
- `[extension-mod]`
- `[enginepack]`
- `[gamepack]`
- `[modpack]`
- `[packagepack]`

Their containing Cargo manifest remains authoritative for package names,
targets, crate types, Rust dependencies, and features.

Workspace version inheritance is the default. A separately versioned artifact
may own an explicit semantic version when its release lifecycle actually
diverges.

Content artifacts may declare built runtime outputs:

```toml
[engine]
name = "spacetime-engine"
version.workspace = true
binaries = ["spacetime-engine"]
libraries = ["spacetime_engine"]
```

`binaries` and `libraries` are file names resolved from the app-local Cargo
build output for the source workspace. Packaging copies them into the deployed
artifact root under `bin/<target>/` and `lib/<target>/`, then records the
actual staged filenames in target-specific runtime entries:

```toml
[[engine.runtime]]
target = "x86_64-unknown-linux-gnu"
binaries = ["spacetime-engine"]
libraries = ["libspacetime_engine.so"]
```

This supports tools, helper processes, native libraries, and high-performance
side executables without treating them as root application binaries. Supported
platforms are expressed by shipping runtime outputs once per target.

## Workshop publication schema

Authored Steam/Workshop intent lives under the artifact's content table:

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

`app-id`, `visibility`, `title`, `description`, `tags`, dependency IDs,
conflict IDs, and update intent are source-authored policy. `published-file-id`
is also source-authored once Steam has assigned a stable Workshop item for that
artifact. It should be absent before item creation rather than replaced with a
generated local placeholder.

Generated or observed state does not belong in source manifests. Vapor writes
resolved deployed `Vapor.toml` files into staged artifact roots, and writes
fingerprints, cache records, installed indexes, locks, operation receipts, last
local verification results, and quarantine diagnostics under the Steam
installation/app root.

## Deployed content manifests

Custom content is authored inside a Vapor workspace, including first-party
content such as Loo-Cast's engine, game, and packagepack. A deployed or
installed content artifact is not a standalone authoring workspace, but it does
carry its own resolved `Vapor.toml` at the artifact root:

```toml
schema = 1

[engine]
id = "ghf-studios/loo-cast/spacetime-engine"
name = "spacetime-engine"
version = "0.5.0"
binaries = ["spacetime-engine"]

[engine.steam]
app-id = 2122620
published-file-id = "1234567890"
visibility = "private"
title = "Spacetime Engine"
```

The deployed manifest uses the same content section names, but it resolves
workspace-dependent authoring fields so the artifact can be installed, verified,
and repaired outside the source workspace.

## Composition schema

Composition is declared on the containing artifact with kind-qualified child
fields:

```toml
[packagepack.engine]
id = "ghf-studios/loo-cast/spacetime-engine"

[packagepack.game]
id = "ghf-studios/loo-cast/loo-cast-game"
```

The design model constrains composition by artifact role:

- an enginepack contains exactly one engine and zero or more engine mods;
- a gamepack contains exactly one game and zero or more game mods;
- a modpack contains engine mods, game mods, and extension mods;
- a packagepack contains either one engine or one enginepack, either one game
  or one gamepack, zero or more modpacks, and optional direct mods;
- extension mods may extend any mod, including another extension mod.

These relationships are real content edges. Cargo dependencies remain a
separate Rust build graph. No separate `binding` object is created.

## Traits and slots

Projects may define zero-member marker traits. The containing project supplies
their identity and versioning scope:

```toml
schema = 1

[engine]
name = "spacetime-engine"
version.workspace = true

[[traits]]
name = "replacement-render-backend"
cardinality = "zero-or-one"
```

Consumer content may require providers through named slots:

```toml
[[slots]]
name = "replacement-render-backend"
trait = "ghf-studios/loo-cast/spacetime-engine/replacement-render-backend"
```

Traits describe capabilities. Content kinds describe structural roles.
Cardinality belongs to the trait because it is part of the capability contract:
a `replacement-render-backend` permits zero or one selected provider wherever
that trait is accepted.

Slots name extension points and reference traits. They do not redefine trait
cardinality.

A generally shared trait belongs to a dedicated contracts project rather than
floating at workspace scope. Provider-declaration syntax and trait composition
remain unset until concrete provider examples force those fields.

## Loo-Cast-style composition workspace

A packagepack that bundles a game and engine should not live under either
constituent. It belongs in a workspace that owns the composition:

```text
Loo-Cast/
├── Vapor.toml                 [workspace]
├── Cargo.toml                 Cargo workspace
├── spacetime-engine/          [engine]
├── loo-cast-game/             [game]
└── loo-cast-packagepack/      [packagepack]
```

The workspace may be first-party content without being part of Vapor-Root.
Vapor-Root is app/depot source; Loo-Cast is Workshop/content source.

## Deliberate omissions

The bootstrap schema does not duplicate Cargo members, package names, targets,
Rust dependencies, documentation paths, or Git submodules. It may name
promoted application binary outputs, but Cargo still defines those targets and
build behavior. It also does not define Steam authentication state, a public `VAPOR_HOME`,
manual setup/provider locks, raw backend pipeline stages, or provider syntax
that has not been forced by a real example.
