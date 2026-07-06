# Cargo integration

## Required structural relationship

Vapor does not merely support Cargo alongside some workspaces. Every source
workspace and every project is a Rust workspace:

```text
source workspace/
├── Vapor.toml    # Vapor workspace manifest
└── Cargo.toml    # Cargo workspace manifest

project/
├── Vapor.toml    # Vapor project manifest
└── Cargo.toml    # Cargo workspace manifest
```

Both files are authored, critical source. Neither is optional, generated, or a
cache. Their canonical parent directory must be identical.

A project maps to a Cargo workspace, not necessarily to one Cargo package. Its
`Cargo.toml` may be a virtual workspace containing several crates, or may combine
`[workspace]` and `[package]` when the root is itself a package. A Vapor
workspace may likewise be virtual.

## Separate authorities

The word *manifest* must always be qualified in documentation and diagnostics:

- `Vapor.toml` is the **Vapor manifest**. It owns Vapor identity, membership,
  roles, composition, distribution policy, and workflow intent.
- `Cargo.toml` is the **Cargo manifest**. It owns Rust packages, crates,
  features, dependencies, targets, profiles, and Cargo workspace policy.

Cargo does not infer Vapor identity. Vapor does not duplicate Cargo's Rust build
graph. A valid Vapor source workspace or project requires both authorities to
agree on their root.

Content such as games, engines, mods, and packs may live below a project and
does not thereby become an independent Cargo workspace. Whether every content
root must also map to a Rust package or workspace remains a separate design
decision; this contract currently applies to `[workspace]` and `[project]`.

## Nested workspace consequence

Vapor-Root is the one umbrella Vapor workspace. Its member projects are
independent Cargo workspaces. Cargo does not support treating those nested Cargo
workspaces as ordinary members of one outer Cargo workspace while preserving
their independent workspace boundaries.

Therefore the two membership graphs have different jobs:

- the Vapor workspace manifest inventories Vapor projects;
- each project Cargo manifest inventories that project's Rust packages;
- the root Cargo workspace contains only root-owned packages, and may be empty;
- root workflows iterate Vapor projects and invoke Cargo once per project.

This is deliberate orchestration, not an attempt to make nested Cargo
workspaces behave as one Cargo workspace.

## Derived metadata

`cargo metadata` is the canonical machine-readable projection of a required
Cargo manifest. It reports workspace members, package manifests, targets, the
Cargo workspace root, and target directory. The command output is replaceable;
the `Cargo.toml` that produces it is not.

Vapor invokes its bundled Cargo for the selected source workspace or project:

```text
cargo metadata --format-version 1 --no-deps --manifest-path <root>/Cargo.toml
```

`--format-version 1` pins Cargo's JSON contract. `--no-deps` keeps the projection
focused on workspace packages without resolving the full dependency graph.

## Validation and recovery

For a source workspace or project:

- missing `Cargo.toml` is an invalid Vapor structure;
- a Cargo workspace root different from the Vapor root is an invalid boundary;
- malformed Cargo policy or incompatible metadata blocks Cargo-backed workflows;
- a missing app-local Cargo executable is an unmet toolchain prerequisite, not
  permission to fall back to host Cargo;
- toolchain installation and diagnostic commands must remain reachable so the
  developer can repair that prerequisite explicitly.

`vapor metadata` reports these states without hiding otherwise recoverable
context. Commands that require a valid Rust workspace fail their targeted
preflight before performing work.

The official Cargo documentation defines the metadata JSON contract:
<https://doc.rust-lang.org/cargo/commands/cargo-metadata.html>.
