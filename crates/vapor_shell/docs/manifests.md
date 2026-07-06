# Vapor manifests

`Vapor.toml` is always called the **Vapor manifest**. `Cargo.toml` is always
called the **Cargo manifest**. Unqualified “manifest” should be avoided whenever
both could be meant.

## Identity rule

Every `Vapor.toml` declares exactly one identity-bearing section. Supporting
tables such as `[toolchain]` and `[[content]]` may coexist, but a marker cannot be
both a workspace and content root.

Identifiers are trimmed, non-empty strings. They should be stable, globally
namespaced values such as `ghf-studios.example-game`; display names belong in
separate presentation metadata.

## Workspace

```toml
[workspace]
id = "ghf-studios.vapor-root"
members = [
    "Vapor",
    "Vapor-SDK",
    "Vapor-Launcher",
    "Vapor-Shell",
    "Vapor-Examples",
]
```

A workspace groups project repositories. Vapor-Root is the single umbrella
workspace and also owns `[distribution]` policy.

Every member path resolves relative to the workspace root and must contain both
a `[project]` Vapor manifest and a Cargo workspace manifest. Membership is a
Vapor relationship; member projects remain independent Cargo workspaces.

The workspace root itself also has a required Cargo manifest:

```toml
# Cargo.toml
[workspace]
resolver = "3"
members = []
```

The root Cargo workspace contains root-owned crates only and may be empty. It
does not list the independent project workspaces as Cargo members. Root Vapor
workflows iterate `[workspace].members` and invoke Cargo separately for each
project.

The existing `[[workspace.cargo]]` draft is therefore not the target model: it
indexes Cargo manifests directly and duplicates project ownership. Code remains
to be aligned after this documentation contract stabilizes.

## Project

```toml
[project]
kind = "shell"
id = "ghf-studios.vapor-shell"
```

Every component repository is a project. Supported `kind` values are exhaustive:

- `core`: foundational Vapor runtime and contracts.
- `sdk`: authoring and workspace-management tooling.
- `launcher`: player-facing installation and launch tooling.
- `custom-content`: external authored engines, games, mods, and packs.
- `shell`: Vapor Shell installation or development workspace.

Component source repositories retain their Vapor project manifests when
assembled or distributed. VAPOR_HOME needs a separate installed identity rather
than a copied source-workspace identity; its exact section is still open below.

Every project root must also contain a Cargo workspace manifest. For example:

```toml
# Cargo.toml
[workspace]
resolver = "3"
members = ["crates/*"]
```

A Vapor project corresponds to this entire Cargo workspace, not to an
individual Cargo package. Its crates remain ordinary Cargo packages governed by
that project workspace.

## Engine

```toml
[engine]
id = "ghf-studios.examples.basic-engine"
```

An engine is a runtime foundation consumed by games and engine-oriented
composition. It is a leaf identity: composition relationships belong in pack or
workspace metadata rather than additional identity sections.

## Game

```toml
[game]
id = "ghf-studios.examples.basic-game"
```

A game is authored playable content targeting an engine contract. Its marker
identifies the game source root; resolved engine selection and installed state do
not belong in this identity file.

## Engine mod

```toml
[engine_mod]
id = "ghf-studios.examples.rendering-overhaul"
```

An engine mod extends or changes engine behavior. The underscore spelling is
canonical and matches Vapor's stable content vocabulary. Engine mods may be
composed by engine packs, mod packs, and package packs.

## Game mod

```toml
[game_mod]
id = "ghf-studios.examples.campaign-expansion"
```

A game mod targets game behavior or content. Compatibility constraints and
dependency resolution belong to composition metadata, not the identity section.

## Extension mod

```toml
[extension_mod]
id = "ghf-studios.examples.shared-extension"
```

An extension mod represents an extension that can participate across supported
targets. It can be selected by mod packs and package packs. It is distinct from
engine-specific and game-specific mods.

## Engine pack

```toml
[enginepack]
id = "ghf-studios.examples.engine-suite"
```

An engine pack composes engines, engine mods, and nested engine packs. It is a
composition boundary, not itself an engine implementation.

## Game pack

```toml
[gamepack]
id = "ghf-studios.examples.game-suite"
```

A game pack composes games, game mods, and nested game packs. It can capture a
curated game-side configuration while remaining reusable inside a package pack.

## Mod pack

```toml
[modpack]
id = "ghf-studios.examples.mod-collection"
```

A mod pack composes engine mods, game mods, extension mods, and nested mod packs.
It does not replace a package pack as the root playable selection.

## Package pack

```toml
[packagepack]
id = "ghf-studios.examples.complete-experience"
```

A package pack is the root playable composition boundary. It may combine package
packs, engine packs, game packs, mod packs, engines, games, and all supported mod
kinds. Launcher selection and lock workflows operate around this root concept.

## Invalid combinations

This marker is rejected because it declares two identities:

```toml
[game]
id = "ghf-studios.examples.game"

[game_mod]
id = "ghf-studios.examples.mod"
```

Nested entities should each receive their own directory and `Vapor.toml`.

## Open contract questions

Before implementation alignment, the next documentation iteration must settle:

- the Vapor manifest identity used by installed VAPOR_HOME, which is not
  authored source and therefore should not masquerade as `[workspace]`;
- whether every content root must map to a Cargo package/workspace, or only live
  somewhere inside its owning project's required Cargo workspace;
- where project documentation and promoted-binary policy belongs after removing
  the root `[[workspace.cargo]]` inventory.
