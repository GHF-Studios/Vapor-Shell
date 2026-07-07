# Vapor manifest schema

Status: **design checkpoint; represented by the bootstrap manifests, not yet
implemented by Vapor Shell**

This is the minimal schema that the implementation refactor targets. It keeps
Cargo facts in `Cargo.toml` and records only Vapor-owned identity, composition,
capability, publication, and application policy in `Vapor.toml`.

## Universal rules

- Every manifest starts with `schema = 1`.
- Every manifest declares exactly one primary identity section.
- IDs use lowercase, slash-separated, kebab-case components, normally rooted
  at the canonical GitHub owner and repository: `ghf-studios/vapor-sdk/cli`.
- Repository URLs preserve the canonical GitHub owner and repository spelling.
- IDs are globally unique across identity kinds. A project ID extends its
  workspace ID with the project slug, even when that repeats the repository
  name: `ghf-studios/loo-cast/loo-cast-game`.
- A publishable reference uses a stable ID. Relative paths are reserved for
  private, local, or otherwise non-publishable relationships.
- Generated resolution, hashes, receipts, and installed state do not belong in
  a source manifest.

## Application root

The application root is a pure Vapor-managed Git super-repository, not a Cargo
workspace:

```toml
schema = 1

[root]
id = "ghf-studios/vapor-root"
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
id = "ghf-studios/vapor-examples"
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
id = "ghf-studios/vapor-sdk/cli"
version.workspace = true
```

A content package uses its content kind instead of `[project]`:

```toml
schema = 1

[engine]
id = "ghf-studios/loo-cast/spacetime-engine"
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

Workspaces may define globally identified, zero-member marker traits:

```toml
[[traits]]
id = "ghf-studios/loo-cast/render-backend"
```

Consumer content may require providers through named slots:

```toml
[[slots]]
name = "renderer"
trait = "ghf-studios/loo-cast/render-backend"
cardinality = "one"
```

Traits describe capabilities; content kinds describe structural roles. Slot
resolution operates over the selected content graph and must satisfy the
declared cardinality. Provider-declaration syntax and trait composition remain
unset until a concrete provider example forces those fields; the schema does
not invent them prematurely.

## Registry authority

The registry is infrastructure authority, not a source workspace and not a
Cargo package:

```toml
schema = 1

[registry]
id = "ghf-studios/vapor-registry"
repository = "https://github.com/GHF-Studios/Vapor-Registry"
authority = "github.com/GHF-Studios/Vapor-Registry"
```

Registry data verifies declared identity and containment. A repository cannot
grant itself first-party authority merely by adding a manifest field.

## Deliberate omissions

The bootstrap schema does not duplicate Cargo members, package names, targets,
dependencies, documentation paths, promoted binaries, or Git submodules. It
also does not define installation receipts, Steam authentication state, a
public `VAPOR_HOME`, manual toolchain locks, or backend pipeline stages.
