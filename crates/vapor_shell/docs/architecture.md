# Architecture

## Purpose

Vapor Shell gives authored source a stable command environment without making
that source part of the replaceable Steam installation. The executable may be
globally reachable through `PATH`; process invocation does not determine source
ownership or widen filesystem access.

## Authority hierarchy

1. The Vapor manifest, `Vapor.toml`, is authoritative for Vapor identity,
   project membership, composition, distribution, and workflow intent.
2. The Cargo manifest, `Cargo.toml`, is authoritative for the required Rust
   workspace, packages, targets, dependencies, features, and profiles.
3. Canonical filesystem paths are authoritative for containment and for proving
   that the two manifests describe the same root.
4. `cargo metadata` output is a rebuildable projection of the Cargo manifest.
5. Prompt context and command output are derived views.

Cargo metadata must never silently create, rename, or redefine a Vapor entity.
Every source workspace and project must have both manifests at the same root.
The workspace may contain non-Rust assets and content, but its governing
structure is always a Rust workspace.

A `[project] kind = "shell"` repository cannot become authored workspace state.
Discovery must select its higher containing `[workspace]` or stop.

VAPOR_HOME is an installed, replaceable realization, not authored source and not
a Cargo workspace merely because Vapor Shell runs from it. The current draft
reuses a copied `[workspace]` marker for installation discovery; that conflicts
with the stricter source-workspace definition above. The installation manifest
identity and discovery contract must be settled in the next documentation
iteration before code alignment.

## Session startup

```text
current_exe() ── ancestors ──> VAPOR_HOME installation root
                                  │
current_dir() ── ancestors ──> external source workspace
                                  │
                    reject if either root overlaps
                                  │
                  compare finalized VAPOR_HOME lock
                                  │
                 validate source Vapor manifest
                                  │
                 require matching Cargo manifest
                                  │
                    project Cargo metadata with
                  bundled Cargo when it is installed
                                  │
                         enter interactive loop
```

## State ownership

`ShellState` owns one immutable pair of discovered roots and mutable source-only
navigation state. The current directory can never become an installation path.
Installation commands print or execute explicit resources without changing the
source cursor.

Cargo integration has three materially different states:

- **invalid structure**: a source workspace or project has no root Cargo
  manifest, or Cargo reports a different workspace root;
- **unmet prerequisite**: the required Cargo manifest exists, but the app-local
  Cargo executable is not installed or healthy;
- **loaded**: bundled Cargo returned compatible metadata for the same root.

The shell must retain enough diagnostic and toolchain functionality to recover
from an unmet prerequisite. Cargo-backed workflows must not proceed until the
required Cargo workspace is successfully validated.

## Resolution and validation

`metadata::ResolvedMetadata` is the shared snapshot used by reporting and
action commands. It resolves workspace policy, optional distribution policy,
VAPOR_HOME status, tool health, and Cargo-derived state once. `vapor metadata`
renders that complete snapshot without confusing a required Cargo manifest with
its replaceable metadata projection.

Before an action mutates state or launches a child process, its handler supplies
a targeted `ValidationPlan`. A Cargo build requires a finalized app location,
Rust, Git, and valid workspace policy; Steam login requires only a finalized
location and SteamCMD. Commands do not invoke other Vapor commands as
subprocesses and never install or repair failed prerequisites implicitly.

## Module map

- `app`: startup and REPL control flow.
- `command`: Clap grammar and command effects.
- `cargo_metadata`: invocation and typed projection of Cargo's JSON output.
- `discovery`: installation/source discovery and disjoint-root validation.
- `manifest`: strict Vapor identity vocabulary.
- `metadata`: shared environment resolution, reporting, and targeted preflight.
- `workspace`: root Cargo-project and documentation policy.
- `workflow`: Steam-toolchain formatting, checking, testing, and validation.
- `path_setup`: marked registration of the app-owned `bin` directory in PATH.
- `toolchain`: explicit app-local Rust, Git, and SteamCMD installation and health.
- `state`: source navigation and current content context.
- `prompt`: Reedline presentation adapter.

The executable `main.rs` only reports a fatal startup error and exit status.

## Failure behavior

Fatal workflow failures protect an authority or boundary: missing Vapor or Cargo
manifests, invalid identities, mismatched roots, self-targeting, path escape, or
overlapping roots. Missing tools remain inspectable and explicitly repairable;
they never authorize host-tool fallback.
