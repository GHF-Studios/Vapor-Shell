# Vapor manifests

`Vapor.toml` is the Vapor manifest. `Cargo.toml` is the Cargo manifest. Avoid
unqualified “manifest” when both could be meant.

## Identity rule

Every `Vapor.toml` declares exactly one identity-bearing section.
Declarations use local `name`; full IDs are inferred.

- `[root]`, `[workspace]`, and `[registry]` require `name` and `organization`.
- `[project]` and content sections require only `name`.
- Declaration-side `id` is invalid.
- `[project].kind` is invalid; the table name selects the role.

References still use fully-qualified IDs. In a reference, `id` means “the thing
being referenced,” not “the thing being declared here.”

## Source roots

### App/depot root

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
```

`[root]` is the pure Vapor super-repository for the Steam app/depot. It is not a
Cargo workspace. Direct Git submodules that contain `Cargo.toml` become the
Cargo workspaces routed by root workflows.

### Normal workspace

```toml
schema = 1

[workspace]
name = "loo-cast"
organization = "ghf-studios"
version = "0.1.0"
repository = "https://github.com/GHF-Studios/Loo-Cast"
```

`[workspace]` is a normal Vapor/Cargo source workspace. Its root must also
contain `Cargo.toml`.

## Projects and content

Non-content Cargo packages use `[project]`:

```toml
schema = 1

[project]
name = "cli"
version.workspace = true
```

Content Cargo packages use a content section:

```toml
schema = 1

[engine]
name = "spacetime-engine"
version.workspace = true
```

Supported content sections are:

- `[engine]`
- `[game]`
- `[packagepack]`
- `[enginepack]`
- `[gamepack]`
- `[modpack]`
- `[engine-mod]`
- `[game-mod]`
- `[extension-mod]`

For source root `ghf-studios/loo-cast`, `[engine] name = "spacetime-engine"`
declares `ghf-studios/loo-cast/spacetime-engine`.

## Composition references

Composition uses full IDs because it references other artifacts:

```toml
[packagepack.engine]
id = "ghf-studios/loo-cast/spacetime-engine"

[packagepack.game]
id = "ghf-studios/loo-cast/loo-cast-game"
```

Cargo dependencies remain separate Rust build dependencies. Vapor dependencies
describe content composition, packaging, compatibility, and publication
relationships.

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

Shared traits belong to a dedicated contracts project, not to workspace scope.
Slots do not own cardinality. The referenced trait owns it as part of the
capability contract.

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

Nested entities each get their own directory and `Vapor.toml`.

See `docs/design/manifest-schema.md` for the fuller schema checkpoint.
