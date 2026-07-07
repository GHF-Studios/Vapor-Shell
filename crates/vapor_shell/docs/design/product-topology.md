# Vapor product topology

Status: **implemented baseline; first-party authority and publication policy still evolving**

This document records the current product model after the first command,
manifest, and implementation alignment pass. It supersedes older conflicting
topology assumptions and separates implemented baseline from remaining design
work.

## Design objective

Vapor is a globally available, Steam-installed development and publishing
surface. It should make ordinary work obvious while keeping Cargo, Git, GitHub,
SteamCMD, filesystem staging, and other backend machinery controlled and mostly
invisible.

The model must support:

- the complete first-party Steam application;
- independent third-party Vapor workspaces;
- multiple Rust projects in one workspace;
- multiple independently published Workshop artifacts in one workspace;
- nested technical metadata for content and Rust modules;
- an interactive Vapor Shell and reusable non-interactive Vapor scripts;
- source that always remains outside the replaceable Steam installation.

## Structural hierarchy

```text
Vapor application root
│  Pure Vapor-managed Git super-repository
│  Not a Cargo workspace
│  Publishes the complete Steam application
│
├── Vapor workspace
│   │  Direct Git submodule repository
│   │  Cargo workspace
│   │
│   ├── Vapor project
│   │   Cargo package
│   │
│   ├── Vapor project
│   │   Cargo package
│   │
│   └── Publishable Vapor artifacts
│       Zero or more Workshop items and packs
│
└── Additional Vapor workspaces
```

The application, workspace, project, and content layers have different jobs.
They must not be collapsed merely because their roots may all contain a
`Vapor.toml`.

## Application root

The application root is a pure Vapor-managed Git super-repository. Vapor-Root
is the first-party instance, but the model is defined by its role rather than by
hard-coded repository or project names.

The application root:

- is source code and therefore lives outside the Steam installation;
- is not a Cargo workspace and does not require a root Cargo manifest;
- contains Vapor workspace repositories as direct Git submodules;
- does not permit nested Git submodules;
- combines those workspaces into the complete Steam application;
- is the only root kind that may publish itself as a Steam depot;
- requires verified first-party authority for application publishing.

The application root is structurally exceptional without making ordinary Vapor
commands revolve around Vapor-Root.

## Workspace

A Vapor workspace is one source repository and one Cargo workspace. It is the
normal authoring, validation, and publishing scope.

A workspace:

- may be a direct member of the first-party application root;
- may exist independently as a third-party workspace;
- contains one or more Vapor projects represented by Cargo packages;
- may contain zero or more publishable Workshop artifacts;
- may publish several artifacts independently, including artifacts with
  dependencies on one another;
- is never the installed Steam application.

Application membership is a Git and Vapor relationship. Cargo governs the Rust
packages inside each workspace, not the relationship between application and
workspace repositories.

## Project

A Vapor project corresponds to a Cargo package inside its containing Cargo
workspace.

Cargo terminology remains authoritative:

- a Cargo workspace contains packages;
- a Cargo package is described by `[package]` in `Cargo.toml`;
- a package may produce a library crate and multiple binary, example, test, or
  benchmark crates;
- an individual crate target is not automatically another Vapor project.

Projects are therefore not Git submodules, standalone repositories, or nested
Cargo workspaces. Cargo metadata supplies their package and target structure.
Vapor adds identity, roles, content relationships, publication intent, and
other semantics Cargo does not own.

## Content and nested metadata

Games, engines, mods, extension mods, and packs are Vapor content concepts.
They are not documentation sections and do not duplicate ordinary Cargo
package data.

Nested `Vapor.toml` files are scoped technical metadata fragments. A fragment
may identify or configure content, a Rust module, a capability area, or another
managed subtree. Vapor resolves a workspace by ingesting the applicable
metadata hierarchy rather than treating every file as an unrelated root.

Content metadata should contain only machine-relevant Vapor information, such
as:

- Vapor identity and content role;
- ownership and containment;
- composition and dependencies not represented adequately by Cargo;
- conflicts and compatibility constraints;
- publication and installation policy;
- module- or capability-specific metadata.

Explanations, tutorials, and design rationale belong in documentation, not in
technical manifests.

## Publishable artifacts and Workshop

A workspace may produce multiple publishable Vapor artifacts. In the bootstrap
model, a publishable artifact is a content project backed by a Cargo package;
that artifact is the unit mapped to a Workshop item. Generic `[project]`
packages are not independently publishable content merely because they are
Cargo packages.

Normal workspace publishing:

- discovers the workspace's publishable artifacts;
- validates each artifact and its dependencies;
- previews the exact intended changes with `--dry-run`;
- publishes selected or applicable artifacts individually;
- does not expose SteamCMD as the user's workflow model.

First-party content follows the same artifact model. Several first-party
Workshop items are composed by a first-party Packagepack. That Packagepack is
downloaded, installed, and selected by default for the shipped application.

The application root has a separate publication capability: it assembles and
publishes the complete Steam depot rather than masquerading as one Workshop
artifact.

## First-party authority

First-party status is a real policy distinction, not a hard-coded list and not
a boolean that any repository can grant itself.

Effective first-party authority requires all applicable evidence:

```text
declared identity
    + trusted first-party application/workspace authority
    + valid containment and membership
    = authorized first-party publication
```

Consequences:

- first-party workspace and project names remain data-driven;
- first-party projects care which workspace contains them;
- first-party workspaces care which application root contains them;
- first-party publication is rejected outside authorized containment;
- ordinary third-party workspaces use the same structural model without being
  allowed to publish first-party identities;
- the exact trust proof may evolve without changing these semantics.

## Steam installation and the global CLI

Owning or installing the game installs `vapor` as a global CLI tool. The binary,
Rust toolchain, Cargo state, Git/GitHub tooling, SteamCMD, libraries, caches, and
managed outputs live inside the Steam application.

The Steam installation:

- is replaceable installed state, not a source workspace;
- exposes its own `bin` directory through the user's `PATH`;
- is discovered from the running `vapor` executable;
- does not require a public `VAPOR_HOME` environment variable;
- never becomes the location of authored workspace source.

An internal location lock records the canonical installation path. Vapor
compares it with the executable-derived path and can repair the recorded
location and PATH registration after Steam moves the application. The lock is
implementation state, not a normal user-facing lifecycle.

## Toolchain experience

The toolchain is explicit only when the user intentionally manages it. Its
minimal conceptual lifecycle is:

- status;
- install with preview support;
- uninstall with preview support;
- repair.

Repair means restoring a known-good installation, conceptually equivalent to
uninstalling and reinstalling it while preserving only state that is explicitly
supposed to survive. Normal build, validation, and publication commands never
silently install or repair prerequisites.

Toolchain operations follow the same status, preview, and explicit repair model
as other consequential workflows. `toolchain status` explains current state and
next actions. Mutating commands expose a dry-run style preview before changing
installation-owned Rust, Cargo, Git, SteamCMD, PATH registration, or app-root
location state. Scripts may call these operations and bubble up flags, but Vapor
must still keep the consequential action explicit and visible.

## IDE integration

Vapor should discover, validate, repair, and manage IDE project settings where
that can be done safely. RustRover and other JetBrains IDEs are the first
target because they are part of the intended authoring loop.

Opening a Vapor application root or workspace should be able to configure the
project to use the Steam-installed Vapor toolchain: bundled Rust and Cargo,
Cargo home, rustup and toolchain paths, relevant environment variables, and
project mappings. This must be explicit, inspectable, and repairable.

IDE setup is not automatic startup behavior. It is a manual, repo-by-repo or
workspace-by-workspace operation because changing project files can be annoying
or destructive. Vapor should provide status and dry-run style previews before
repairing IDE state, then apply only to the selected source root. It should
normalize project-local configuration, but it must not silently mutate global
IDE configuration or hide what it changed.

## Opening and targeting source

The global CLI is usable regardless of the terminal's current directory.
Invoking `vapor` opens the interactive Vapor Shell. Non-interactive automation
runs a declared Vapor script rather than exposing every interactive operation
as an ad-hoc one-shot command. Vapor may use an invocation path as one discovery
input, but the selected target is explicit session state rather than the
identity of the executable.

Vapor should support IDE-like opening and closing of an application or
workspace inside the interactive session. Selecting a project or content node
occurs within its containing workspace. Vapor owns this session without
imitating an operating-system shell.

Commands should remain discoverable even when unavailable. Help and interactive
surfaces show the operation and explain which target or prerequisite is missing
rather than silently hiding commands.

## Command experience

Commands express recognizable user goals. Backend programs and pipeline stages
are implementation details unless an expert diagnostic explicitly exposes
them.

Examples of intended vocabulary include:

- create;
- initialize;
- open and close;
- validate;
- install, uninstall, status, and repair;
- publish and publish with `--dry-run`.

`--dry-run` means that externally or persistently consequential changes are not
applied. It replaces the ambiguous `--plan` spelling and requires behavioral,
not merely cosmetic, alignment.

Meaningful operations should resolve context, validate, preview when relevant,
apply, verify, and report what changed and what remains. The user should not
need to understand Cargo, Git, SteamCMD, depot staging, or internal pipeline
steps to perform ordinary work.

## Creating and adopting repositories

Creating a new empty application or workspace can follow a deterministic Vapor
layout.

Existing Rust repositories require a non-destructive adoption workflow. Vapor
may inspect Cargo metadata, Git state, filesystem structure, and existing Vapor
fragments to propose an unambiguous conversion. It must:

- preserve existing source;
- preview intended structural changes;
- distinguish safe automatic edits from ambiguous decisions;
- refuse destructive or uncertain conversion;
- explain required manual edits clearly;
- validate the result after changes are accepted.

The final command name and conversion policy remain unresolved.

## Schema sequencing

Schema design occurs in two passes. The early pass is now represented by the
implemented baseline schema; the later pass remains open.

The early pass defines only invariants required by the product model:

- application, workspace, project, and fragment roles;
- Cargo and Git correspondence;
- containment and nesting rules;
- identity and authority boundaries;
- which information Vapor owns instead of Cargo.

The later pass defines detailed fields after workflows, artifact ownership, and
publication semantics are settled. This prevents speculative TOML structure
from forcing the rest of the product into an accidental model.

## Confirmed constraints

- The Steam-installed `vapor` command is globally usable.
- Source never lives inside the Steam installation.
- An application root is a non-Cargo Git super-repository.
- A workspace is a repository and Cargo workspace.
- A project is a Cargo package, not another repository or workspace.
- Application workspaces are direct submodules; nested submodules are not
  allowed.
- A workspace can produce multiple Workshop items.
- Application publishing and Workshop publishing are distinct operations.
- First-party status is verified policy, not hard-coded names or self-assertion.
- First-party content is composed into a default Packagepack.
- Ordinary commands hide controlled Cargo, Git, GitHub, and SteamCMD backends.
- Contextual unavailability must be visible and explained.
- Consequential previews use `--dry-run`.
- Toolchain management is limited initially to install, uninstall, status, and
  repair.

## Unresolved decisions

The topology does not depend on settling these immediately:

- the precise definition of a publishable artifact;
- whether one Workshop artifact may combine output from several projects;
- how content dependencies reuse or extend Cargo dependency information;
- the cryptographic, registry, or installation evidence proving first-party
  authority;
- the final open, close, create, initialize, adopt, and publish command grammar;
- which operation metadata drives help, documentation, previews, and future GUI
  surfaces.

## Documentation consequence

The remaining documentation rebuild should use this order:

1. keep this checkpoint as design material rather than command reference;
2. keep the implemented baseline manifest invariants synchronized with code;
3. finish the plain-language principal user workflows;
4. settle open/create/initialize/adopt/publish vocabulary and ownership;
5. finish the detailed artifact and publication schema;
6. derive task guides from the accepted operation model;
7. keep implementation architecture synchronized after each workflow pass.

User-facing documentation is authoritative only where it matches implemented
and verified behavior.
