# Cargo integration

## Required structural relationship

Vapor source roots are Rust-governed, but `[root]` and `[workspace]` have
different shapes.

```text
normal workspace/
├── Vapor.toml    # [workspace]
└── Cargo.toml    # Cargo workspace

app root/
├── Vapor.toml    # [root]
├── .gitmodules
└── Vapor-Shell/
    ├── Vapor.toml
    └── Cargo.toml
```

`[workspace]` roots must contain a root `Cargo.toml`. `[root]` is not itself a
Cargo workspace; its direct submodules that contain `Cargo.toml` are routed as
Cargo workspaces.

Inside a Cargo workspace, `[project]` and content `Vapor.toml` files describe
Cargo packages/content packages. They are not standalone source roots.

## Separate authorities

The word *manifest* must always be qualified in documentation and diagnostics:

- `Vapor.toml` is the Vapor manifest. It owns Vapor identity, content
  composition, Steam app policy, and workflow intent.
- `Cargo.toml` is the Cargo manifest. It owns Rust packages, crates, features,
  dependencies, targets, profiles, and Cargo workspace policy.

Cargo does not infer Vapor identity. Vapor does not duplicate Cargo's Rust build
graph. Vapor workflows use Cargo facts where Cargo is authoritative.

## Derived metadata

`cargo metadata` is the canonical machine-readable projection of a Cargo
manifest. It reports workspace members, package manifests, targets, the Cargo
workspace root, and target directory. The command output is replaceable; the
`Cargo.toml` that produces it is not.

Vapor invokes its bundled Cargo for a discovered Cargo workspace:

```text
cargo metadata --format-version 1 --no-deps --manifest-path <workspace>/Cargo.toml
```

`--format-version 1` pins Cargo's JSON contract. `--no-deps` keeps the
projection focused on workspace packages without resolving the full dependency
graph.

## Validation and recovery

For Cargo-backed workflows:

- a `[workspace]` source root without `Cargo.toml` is invalid Vapor structure;
- a `[root]` child workspace with malformed Vapor or Cargo policy is invalid
  source structure;
- malformed Cargo policy or incompatible metadata blocks Cargo-backed
  workflows;
- a missing app-local Cargo executable is an unmet toolchain prerequisite, not
  permission to fall back to host Cargo.

`vapor metadata` reports these states without hiding otherwise recoverable
context. Commands that require Rust tooling fail their targeted preflight before
performing work.

The official Cargo documentation defines the metadata JSON contract:
<https://doc.rust-lang.org/cargo/commands/cargo-metadata.html>.
