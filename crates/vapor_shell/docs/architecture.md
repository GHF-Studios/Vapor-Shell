# Architecture

## Purpose

Vapor Shell gives authored source a stable command environment without making
that source part of the Steam installation/app root. The executable may be
globally reachable through PATH; process invocation does not determine source
ownership or widen filesystem access.

## Authority hierarchy

1. `Vapor.toml` is authoritative for Vapor identity, composition, Steam app
   policy, and workflow intent.
2. `Cargo.toml` is authoritative for Rust workspaces, packages, targets,
   dependencies, features, and profiles.
3. Canonical filesystem paths are authoritative for containment.
4. `cargo metadata` output is a rebuildable projection of Cargo authority.
5. Prompt context and command output are derived views.

Cargo metadata never creates, renames, or redefines a Vapor entity. Declaration
IDs are inferred from `name` and source-root context; references use fully
qualified IDs.

## Source roots

Vapor accepts two source-root kinds:

- `[root]`: the pure Vapor application source/depot super-repository. It is not a
  Cargo workspace. Direct submodules that declare `[workspace]` and contain
  `Cargo.toml` become routed Cargo workspaces.
- `[workspace]`: a normal Vapor/Cargo source workspace rooted in the same
  directory as its `Cargo.toml`.

`[project]` and content manifests cannot be standalone source roots. Starting
inside Vapor-Shell, a game, an engine, or another nested artifact escalates to
the highest containing `[root]` or `[workspace]`.

The Steam installation is the app root, not a source root. It is discovered from
the running executable.

## Session startup

```text
current_exe() ── ancestors ──> installed app [root]
                                  │
current_dir()/remembered path ──> external source [root] or [workspace]
                                  │
                    reject if either root overlaps
                                  │
                  compare app-root registration
                                  │
                 validate source Vapor identity
                                  │
                  discover routed Cargo workspaces
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

- **invalid structure**: a `[workspace]` source root has no root `Cargo.toml`, or
  a `[root]` child workspace is malformed;
- **unmet prerequisite**: Cargo structure exists, but app-local Cargo is not
  installed or healthy;
- **loaded**: bundled Cargo returned compatible metadata.

The shell retains enough diagnostic and setup functionality to recover from
unmet prerequisites. Cargo-backed workflows do not proceed until required source
structure and tools validate.

## Resolution and validation

`metadata::ResolvedMetadata` is the shared snapshot used by reporting and
action commands. It resolves source-root policy, optional `[root.steam]` policy,
app-root registration state, tool health, and Cargo-derived state once.

Before an action mutates state or launches a child process, its handler supplies
a targeted `ValidationPlan`. A Cargo build requires an accepted app root, Rust,
Git, and valid source-root policy. A real root publish additionally requires
SteamCMD; `root publish --dry-run` does not. Commands never install or repair
failed prerequisites implicitly.

## Command surface

The interactive shell is the primary interface. Ad-hoc one-shot commands are
disabled. Direct CLI facades are reserved for bootstrap, app inspection,
source selection, setup lifecycle, metadata reporting, and repeatable
non-auth scripts.

Scripts may dry-run publish staging and IDE repair, but they may not
authenticate Steam, perform real uploads, or apply project-local IDE changes.

## Module map

- `app`: startup and REPL control flow.
- `command`: Clap grammar and command effects.
- `cargo_metadata`: invocation and typed projection of Cargo's JSON output.
- `discovery`: installation/source discovery and disjoint-root validation.
- `ide`: explicit project-local RustRover/JetBrains status and repair.
- `manifest`: strict Vapor identity vocabulary.
- `metadata`: shared environment resolution, reporting, and targeted preflight.
- `source_registry`: app-local index and active selection for external sources.
- `workspace`: source-root Cargo workspace discovery.
- `workflow`: app-local Rust/Cargo formatting, checking, testing, and validation.
- `path_setup`: marked registration of the app-owned `bin` directory in PATH.
- `setup`: explicit app-local Rust/Cargo, Git, and SteamCMD lifecycle.
- `setup_packages`: distributable setup package payload inspection and copying.
- `state`: source navigation and current content context.
- `prompt`: Reedline presentation adapter.

The executable `main.rs` only reports a fatal startup error and exit status.

## Failure behavior

Fatal workflow failures protect an authority or boundary: missing Vapor or Cargo
manifests, invalid identities, self-targeting, path escape, or overlapping
roots. Missing tools remain inspectable and explicitly repairable; they never
authorize host-tool fallback.
