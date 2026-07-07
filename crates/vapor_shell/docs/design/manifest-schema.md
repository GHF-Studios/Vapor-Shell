# Vapor manifest schema

Status: **design checkpoint; represented by the bootstrap manifests, not yet
implemented by Vapor Shell**

This is the minimal schema that the implementation refactor targets. It keeps
Cargo facts in `Cargo.toml` and records only Vapor-owned identity, composition,
capability, publication, and application policy in `Vapor.toml`.

## Universal rules

- Every manifest starts with `schema = 1`.
- Every manifest declares exactly one primary identity section.
- Declaration names and organization names use lowercase kebab-case.
- Root, workspace, registry, project, content, and trait declarations use a
  local `name`; they do not repeat their fully qualified identifier.
- A root, workspace, or registry identifier is inferred as
  `organization/name`.
- A project or content identifier is inferred as
  `organization/workspace/project`.
- A trait identifier is inferred as
  `organization/workspace/project/trait`.
- Repository URLs preserve the canonical GitHub owner and repository spelling.
- Inferred identifiers are globally unique across identity kinds.
- References use fully qualified identifiers. An `id` field therefore denotes
  a reference, never the declaration containing that field.
- A publishable reference uses a stable identifier. Relative paths are reserved for
  private, local, or otherwise non-publishable relationships.
- Generated resolution, hashes, receipts, and installed state do not belong in
  a source manifest.

## Application root

The application root is a pure Vapor-managed Git super-repository, not a Cargo
workspace:

```toml
schema = 1

[root]
name = "vapor-root"
organization = "ghf-studios"
version = "0.5.0"
repository = "https://github.com/GHF-Studios/Vapor-Root"
default-packagepack = "ghf-studios/loo-cast/loo-cast-packagepack"

[root.steam]
app-id = 2122620
depot-id = 2122621
development-branch = "vapor-dev"
```

Direct Git submodules define its workspace membership. The manifest does not
duplicate `.gitmodules`.

## Workspace

A source repository is one Vapor workspace and one Cargo workspace rooted in
the same directory:

```toml
schema = 1

[workspace]
name = "vapor-examples"
organization = "ghf-studios"
version = "0.5.0"
repository = "https://github.com/GHF-Studios/Vapor-Examples"
```

Cargo metadata defines the package membership. The Vapor manifest does not
repeat Cargo member paths.

## Project and content package

Every Cargo package in a Vapor workspace has a colocated `Vapor.toml`. A
non-content package uses `[project]`:

```toml
schema = 1

[project]
name = "cli"
version.workspace = true
```

A content package uses its content kind instead of `[project]`:

```toml
schema = 1

[engine]
name = "spacetime-engine"
version.workspace = true
```

The content identity sections are `[engine]`, `[game]`, `[engine-mod]`,
`[game-mod]`, `[extension-mod]`, `[enginepack]`, `[gamepack]`, `[modpack]`, and
`[packagepack]`. Their containing Cargo manifest remains authoritative for
package names, targets, crate types, Rust dependencies, and features.

Workspace version inheritance is the default. A separately versioned artifact
may own an explicit semantic version when its release lifecycle actually
diverges.

## Composition

Composition is declared on the containing artifact with kind-qualified child
fields:

```toml
[packagepack.engine]
id = "ghf-studios/loo-cast/spacetime-engine"

[packagepack.game]
id = "ghf-studios/loo-cast/loo-cast-game"
```

The field shape and content-kind rules imply structural cardinality. These
relationships are real content edges; no separate `binding` object is created.
Cargo dependencies remain a separate Rust build graph.

## Traits and slots

Projects may define zero-member marker traits. The containing project supplies
their identity and versioning scope:

```toml
schema = 1

[engine]
name = "spacetime-engine"
version.workspace = true

[[traits]]
name = "render-backend"
```

Consumer content may require providers through named slots:

```toml
[[slots]]
name = "renderer"
trait = "ghf-studios/loo-cast/spacetime-engine/render-backend"
cardinality = "one"
```

Traits describe capabilities; content kinds describe structural roles. Slot
resolution operates over the selected content graph and must satisfy the
declared cardinality. A generally shared trait belongs to a dedicated contracts
project rather than floating at workspace scope. Provider-declaration syntax
and trait composition remain unset until a concrete provider example forces
those fields; the schema does not invent them prematurely.

## Registry authority

The registry is infrastructure authority, not a source workspace and not a
Cargo package:

```toml
schema = 1

[registry]
name = "vapor-registry"
organization = "ghf-studios"
repository = "https://github.com/GHF-Studios/Vapor-Registry"
authority = "github.com/GHF-Studios/Vapor-Registry"
```

Registry data verifies the declared organization, inferred identity, and
containment. Naming an organization in a source manifest is a namespace claim,
not authorization; a repository cannot grant itself first-party authority.

## Deliberate omissions

The bootstrap schema does not duplicate Cargo members, package names, targets,
dependencies, documentation paths, promoted binaries, or Git submodules. It
also does not define installation receipts, Steam authentication state, a
public `VAPOR_HOME`, manual toolchain locks, or backend pipeline stages.
