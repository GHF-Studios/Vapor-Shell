# Vapor product topology

Status: **canonical design checkpoint; implemented baseline plus planned policy**

This document records the product model Vapor Shell is converging on. It is the
source of truth for vocabulary, containment, authority, and workflow intent. It
does not claim every described operation is implemented today; command reference
documents must say when behavior is implemented, planned, or deliberately
unsettled.

## Design objective

Vapor is a globally available, Steam-installed development and publishing
surface for building Vapor itself and Vapor content. It should make ordinary
work obvious while keeping Cargo, Git, GitHub, SteamCMD, filesystem staging, and
other backend machinery controlled and mostly invisible.

The model must support:

- the complete first-party Steam application;
- independent first-party and third-party Vapor workspaces;
- multiple Rust packages in one workspace;
- multiple independently publishable Workshop artifacts in one workspace;
- nested technical metadata for content, modules, capabilities, and publication;
- an interactive Vapor Shell and reusable Vapor scripts;
- source that always remains outside the Steam installation/app root.

## Core vocabulary

Use these terms consistently:

- **App root**: the root identity of a complete Vapor application. The
  application source stage uses `[root]` in `App-Source.vapor.toml`; the
  installed runtime stage uses `[root]` in `App.vapor.toml`.
- **Steam installation**: the installed-stage materialization of the app root,
  managed by Steam and discovered from the running Vapor executable.
- **Application source root**: the source-stage materialization of the same app
  root identity. It assembles and publishes a complete Steam application/depot.
  Vapor-Root is the first-party instance.
- **Vapor workspace**: one source repository that is also one Cargo workspace.
- **Vapor project**: one Cargo package inside a Vapor workspace.
- **Vapor content artifact**: a project with a content role such as engine,
  game, mod, or pack.
- **Packagepack**: the top-level playable content composition selected by the
  launcher or user.
- **Registry**: infrastructure authority data, not a source workspace and not a
  Cargo package.

Avoid unqualified “root” and “manifest” where they could mean several things.
Use “app root,” “Steam installation,” “application source root,” “workspace,”
`App-Source.vapor.toml`, `App.vapor.toml`, `Workspace.vapor.toml`,
role-specific content manifests, and `Cargo.toml` explicitly.

## The three roots that must not be confused

```text
App root identity
  Declared by [root]
  Same logical identity across source and installed stages

Steam installation
  Installed-stage app root
  Owns global vapor binary, bundled tools, caches, outputs, installed content

Application source root
  Source-stage app root outside the Steam installation
  Owns depot/app assembly policy
  Example: GHF-Studios/Vapor-Root

Vapor workspace
  Source repository outside the Steam installation
  Owns Cargo packages and Workshop/content artifacts
  Example: GHF-Studios/Loo-Cast
```

The Steam installation is the installed-stage app root. It is where the global
`vapor` executable and bundled development tools live. It is never authored
source.

An application source root is the source-stage app root. It lives outside the
Steam installation even though it is the source that eventually produces that
installation. Both stages use the same `[root]` identity; context determines
whether Vapor is looking at installed app state or authored source.

A workspace is the normal authoring unit. It may be first-party or third-party,
but it does not become the installed app directory.

## Structural hierarchy

```text
Application source root repo
│  Pure Vapor-managed Git super-repository
│  Not a Cargo workspace
│  Publishes the complete Steam depot
│
├── Vapor workspace source repo
│   │  Direct Git submodule of the application source root
│   │  Also a Cargo workspace
│   │
│   ├── Vapor project
│   │   Cargo package
│   │
│   ├── Vapor content artifact
│   │   Cargo package plus content role
│   │
│   └── Nested metadata fragments
│       Capability, composition, publication, or module metadata
│
└── Additional direct Vapor workspace source repos
```

Normal standalone work uses only a Vapor workspace source repo. The application
source-root layer exists for complete app/depot assembly, not for every content
author.

## Steam installation stage and global CLI

Owning or installing the app installs `vapor` as a global CLI tool. The PATH
entry points at the Steam installation's own active binary directory, typically
`bin/<target>`; the authoritative binary does not live in a user-data shim
directory such as `~/.local/bin`.

The Steam installation owns:

- `bin/<target>/vapor[.exe]` for release-mode app binaries;
- optional bootstrap `bin/vapor[.exe]` during early local setup;
- app-local Rust, Cargo, rustup state, and Cargo home;
- app-local Git/GitHub tooling when bundled;
- app-local SteamCMD;
- libraries, launchers, SDK tools, docs, caches, staging, and output;
- installed or downloaded custom content;
- runtime indexes, receipts, generated metadata, and other app-owned state.

The Steam installation does not own:

- authored source;
- Git working trees for user projects;
- long-lived Steam authentication state outside the Vapor session;
- user-global IDE or operating-system configuration beyond explicit PATH setup.

Vapor discovers the installation from the running executable. A public
`VAPOR_HOME` environment variable is not part of the product model. Internal
state may record the accepted installation path so Vapor can detect a Steam
library move and ask for explicit repair.

## Application source stage

A Vapor application source root is a pure Vapor-managed source
super-repository. It is the source-stage form of the same `[root]` identity
that the Steam installation materializes. It is not the Steam installation, not
a Cargo workspace, and does not require a root `Cargo.toml`.

The application source root:

- lives outside the Steam installation;
- contains application workspaces as direct Git submodules;
- does not allow nested Git submodules in the application membership graph;
- assembles the complete Steam application;
- is the only source-root kind that may publish itself as a Steam depot;
- requires first-party authority for depot publication.

Vapor-Root is special because it is the first-party source-stage app root. It is
not special because the whole product revolves around hard-coded Vapor-Root
paths or names.

## Workspace

A Vapor workspace is one source checkout, one Git repository, and one Cargo
workspace. Its root contains both `Workspace.vapor.toml` with `[workspace]` and
`Cargo.toml` with Cargo workspace metadata.

A workspace:

- may be a direct submodule of a first-party application source root;
- may exist independently as a normal first-party or third-party workspace;
- contains one or more Vapor projects represented by Cargo packages;
- may contain multiple publishable Workshop artifacts;
- may publish those artifacts independently;
- may contain a Packagepack that composes artifacts from the same or other
  workspaces;
- is never the installed Steam application.

Workspace membership in an application source root is a Git/Vapor
relationship. Cargo governs Rust package membership inside the workspace; Cargo
does not govern which workspaces belong to an application source root.

## Project

A Vapor project is a Cargo package inside its containing Vapor workspace.

Cargo terminology remains authoritative:

- a Cargo workspace contains packages;
- a Cargo package is described by `[package]` in `Cargo.toml`;
- a package may produce a library crate and multiple binary, example, test, or
  benchmark crates;
- an individual crate target is not automatically another Vapor project.

Projects are not standalone repositories, Git submodules, or nested Cargo
workspaces. Cargo metadata supplies package and target structure. Vapor adds
identity, content role, composition, publication intent, authority, and other
semantics Cargo does not own.

## Content artifacts

Some Vapor projects are content artifacts. The canonical content roles are:

- engine;
- game;
- engine mod;
- game mod;
- extension mod;
- enginepack;
- gamepack;
- modpack;
- packagepack.

Content artifacts are machine-governed entities, not documentation sections.
Their role-specific manifests should contain technical metadata: identity,
content role, ownership, composition, dependencies, conflicts, compatibility,
publication policy, and capability metadata. Explanations and design rationale
belong in docs.

## Pack and content composition

Vapor content composition is typed by role and constrained by the containing
artifact:

- An **enginepack** contains exactly one engine and zero or more engine mods.
- A **gamepack** contains exactly one game and zero or more game mods.
- A **modpack** contains engine mods, game mods, and extension mods.
- A **packagepack** is the playable top-level composition. It may contain either
  one engine or one enginepack, either one game or one gamepack, zero or more
  modpacks, zero or more engine mods, zero or more game mods, and zero or more
  extension mods.
- An **extension mod** can extend any mod, including another extension mod. This
  makes extension-mod relationships recursive.

Composition edges are real content relationships. Cargo dependencies remain
separate Rust build dependencies and should not be overloaded to mean playable
content composition, Workshop dependency, compatibility, or publication policy.

Source-authored content can be backed by Cargo packages when it contains code or
uses the workspace build pipeline. Pack artifacts are not inherently code,
though. Packagepacks, Enginepacks, Gamepacks, and Modpacks must also be
representable as simpler Vapor metadata assembled by the Launcher or runtime UI
inside installed/content state. The shell may validate and publish source-backed
content; it must not be the only way to compose playable packs.

There is no separate “binding” object in the design model. Parent-child
relationships and slot/provider relationships are resolved relationships between
content artifacts and traits.

## Packagepack workspaces

A packagepack that bundles an engine and game should live in a workspace whose
job is that composition. It should not be nested under one of the constituents
when it conceptually bundles both.

For example, Loo-Cast as a first-party Workshop/content workspace may contain:

```text
Loo-Cast/
├── Workspace.vapor.toml       [workspace]
├── Cargo.toml                 Cargo workspace
├── spacetime-engine/          [engine]
├── loo-cast-game/             [game]
└── loo-cast-packagepack/      [packagepack]
```

That workspace is separate from Vapor-Root. Vapor-Root is app/depot source;
Loo-Cast is Workshop/content source.

## Traits, slots, and providers

Traits are zero-member marker capabilities owned by projects or content
artifacts. They are closer to namespaced capability tags than to Rust traits
with methods.

A trait:

- has a local name;
- receives its full identity from the containing workspace/project path;
- may define cardinality such as `zero-or-one`;
- may later compose or imply other traits when real examples require it.

A slot is a named extension point that requires providers implementing a
fully-qualified trait. Slots do not own cardinality; the referenced trait owns
cardinality because cardinality is part of the capability contract.

Provider-declaration syntax is intentionally not finalized yet. The model only
requires that consumers declare required or optional slots and that providers
can be matched through stable trait identities.

Shared traits belong in a dedicated contracts project when they are genuinely
shared. Fundamental content roles such as engine or game are content kinds, not
traits.

## Identity and naming

Declaration-side identities use local names. Full identifiers are inferred from
the containing source root and project path.

```text
workspace: ghf-studios/loo-cast
project:   ghf-studios/loo-cast/spacetime-engine
trait:     ghf-studios/loo-cast/spacetime-engine/replacement-render-backend
```

Rules:

- declaration sections use `name`, not `id`;
- references use fully-qualified `id`;
- organization and inferred ID segments use lowercase kebab-case;
- repository URLs preserve canonical GitHub owner/repository spelling;
- public/publishable references use stable IDs;
- relative paths are allowed only for local, private, or bundled relationships.

This keeps source manifests concise while preserving stable public references.

## Versioning

Versioning is artifact-owned and inheritance-friendly.

The default is for project and content versions to inherit from their containing
workspace. An artifact owns an explicit semantic version only when its release
lifecycle truly diverges.

Any published artifact that changes compatibility must bump semver according to
the compatibility impact. Vapor dependencies and compatibility rules are
separate from Cargo dependency versions, even when Cargo packages and Vapor
artifacts share source.

## Publishable artifacts and Workshop

A workspace may produce multiple publishable Vapor artifacts. In the bootstrap
model, publishable artifacts are content projects backed by Cargo packages.
Ordinary Cargo packages are not independently publishable Vapor content merely
because they are Cargo packages.

Normal workspace publishing:

- discovers publishable artifacts;
- validates each artifact, dependency, slot, provider, conflict, and packaging
  rule that is implemented;
- previews the exact intended changes with `--dry-run`;
- publishes selected or applicable artifacts individually;
- hides SteamCMD plumbing from ordinary user workflows.

First-party content follows the same artifact model. Several first-party
Workshop items may be composed by a first-party Packagepack. That Packagepack is
downloaded, installed, and selected by default for the shipped application.

Application publishing is separate: the application source root assembles and
publishes the Steam app depot rather than pretending to be one Workshop item.
The implemented first pass exposes this as `root build`, `root package`, and
`root publish [--dry-run]`. Staging is runtime-only. Dry-run publication
validates, builds, stages, smoke-checks, and writes a preview VDF without
requiring active SteamCMD. When `[root.runtime].targets` is declared,
target-aware root commands use that matrix by default and stage only the
matching `bin/<target>/` app binaries and launch wrappers. Host-only local
staging is an explicit `--host-only` opt-out for package/dry-run work. Real
publication is manual, requires an account plus explicit confirmation, and
always stages the complete declared Linux+Windows runtime matrix.

## First-party authority

First-party status is a real policy distinction, not a hard-coded exception and
not a boolean a repository can grant itself.

Effective first-party authority requires all applicable evidence:

```text
declared identity
    + trusted registry or application/workspace authority
    + valid containment and membership
    + Steam-side publish authorization
    = authorized first-party publication
```

Consequences:

- first-party names remain data-driven;
- first-party workspaces care which application source root contains them when
  publishing app-owned material;
- first-party projects care which workspace contains them;
- first-party publication is rejected outside authorized containment;
- ordinary third-party workspaces use the same structure without being allowed
  to publish first-party identities;
- SteamCMD/Steam ultimately controls whether an account may upload a depot or
  Workshop item.

The exact trust proof may evolve without changing these semantics.

## Setup experience

Installation is owned by `Vapor-Installer`, not by Vapor Shell commands. Normal
closed-alpha testers launch the Steam app; the platform launch wrapper invokes
`vapor-installer install` before opening Vapor Shell or Play.

Player-mode install prepares only basic runtime capability: app-local Git,
SteamCMD, the public Vapor-Registry checkout, and generated disposable app-root
directories. Development tooling is explicit and separate:

```text
vapor-installer dev-env install --app-root /path/to/steam/app
vapor-installer dev-env uninstall --app-root /path/to/steam/app
```

The Steam app root is disposable by design. Authoritative user progress, account
state, and authored source work must live in OS-appropriate user data or source
directories outside the app root. If the app-root tooling is badly damaged, the
preferred recovery is reinstalling the app or rerunning the installer-owned
bootstrap/dev-env command, not maintaining a growing set of Shell repair shims.

Root depot staging is runtime-only. Installer-managed tools and generated
app-local state stay outside SteamPipe staging.

## IDE integration

Vapor discovers, validates, repairs, and manages project-local IDE settings
where that can be done safely. RustRover and other JetBrains IDEs are the first
target because they are part of the intended authoring loop.

Opening a Vapor application source root or workspace should be able to
configure that project to use app-local Rust/Cargo, Cargo home, rustup paths,
relevant environment variables, and project mappings.

The implemented first pass is `ide status` and `ide repair [--dry-run]`.
It manages only selected-source-root `.idea` files for routed Cargo projects,
Rust/Cargo discovery, stdlib source discovery when packaged, and Vapor-owned
setup metadata.

IDE setup is not automatic startup behavior. It is a manual, repo-by-repo or
workspace-by-workspace operation because changing project files can be annoying
or destructive. Vapor provides status and dry-run previews before repairing IDE
state, then applies only to the selected source root. It may normalize
project-local configuration, but it must not silently mutate global IDE
configuration or hide what it changed. Scripts may preview IDE repair but must
not apply it.

## Steam session and publication

Steam authentication is session-scoped by Vapor policy. Vapor should not leave
SteamCMD authentication state dangling after the Vapor session ends. This is
stricter than SteamCMD's default caching behavior, but it keeps publishing
authority temporally and spatially tied to the Vapor session.

Scripts may validate and preview publication through `root publish --dry-run`.
They must not perform interactive Steam authentication or a real publish. Final
upload remains a manual interactive-shell action because Steam auth, account
choice, branch choice, and irreversible upload confirmation must remain visible
to the human.

## Opening and targeting source

The global CLI is usable regardless of the terminal's current directory because
the executable discovers the Steam installation/app root from itself first.
Invoking `vapor` opens the interactive Vapor Shell even when no source is
active. A closed shell may inspect the app, manage the source index, report
metadata, and manage app-local setup.

Source work starts only after explicitly opening an external source:

- `source open PATH` validates, indexes, and activates a source root;
- `source open NAME` activates a previously indexed source;
- `source close` returns to the app-only shell state;
- `source list|add|remove` manages the app-local source index.

The source index belongs to the Steam installation/app root because it is tool
state. Source working trees remain authored Git repositories outside the app
root. Opening a path inside a nested project, content item, or shell checkout
escalates to the highest containing valid Vapor source root.

Once a source is open, Vapor owns the session cursor. Navigation commands may
move inside the selected source root, but they cannot leave it or target the
app root. Selecting a project or content node occurs within its containing
source. Vapor owns this session without imitating an operating-system shell.

Non-interactive automation runs a declared Vapor script or one of the narrow
direct facades needed for bootstrap and source selection. It does not expose
every interactive operation as an ad-hoc one-shot command. The script entrypoint
still executes through the Vapor shell command model, so source context and
authority remain session-scoped.

Commands should remain discoverable even when unavailable. Help and interactive
surfaces show the operation and explain which target or prerequisite is missing
rather than silently hiding commands.

## Command experience

Commands express recognizable user goals. Backend programs and pipeline stages
are implementation details unless an expert diagnostic explicitly exposes them.

Preferred vocabulary includes:

- status;
- install, uninstall, and repair;
- create and initialize;
- open and close;
- validate;
- publish and publish with `--dry-run`.

`--dry-run` means externally or persistently consequential changes are not
applied. It replaces the ambiguous `--plan` spelling and requires behavioral,
not merely cosmetic, alignment.

Meaningful operations should resolve context, validate, preview when relevant,
apply, verify, and report what changed and what remains. The user should not
need to understand Cargo, Git, SteamCMD, depot staging, or internal pipeline
steps to perform ordinary work.

## Creating and adopting repositories

Creating a new empty application source root or workspace can follow a
deterministic Vapor layout.

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

Schema design happens in passes.

The implemented baseline defines only invariants required by the current
product model:

- application, workspace, registry, content, and fragment roles;
- Cargo and Git correspondence;
- containment and nesting rules;
- local names versus inferred IDs;
- identity and authority boundaries;
- which information Vapor owns instead of Cargo.

Later schema passes define detailed fields after workflows, artifact ownership,
slot/provider examples, and publication semantics are settled. This prevents
speculative TOML structure from forcing the rest of the product into an
accidental model.

## Confirmed constraints

- The Steam-installed `vapor` command is globally usable.
- Source never lives inside the Steam installation.
- The app root uses one `[root]` identity across source and installed stages.
- The Steam installation is the installed-stage app root directory.
- The application source root is the source-stage app root and is a non-Cargo
  Git super-repository.
- A workspace is a repository and Cargo workspace.
- A project is a Cargo package, not another repository or workspace.
- Application workspaces are direct submodules; nested application submodules
  are not allowed.
- A workspace can produce multiple Workshop items.
- Loo-Cast-style content composition is separate from Vapor-Root app/depot
  source.
- Application publishing and Workshop publishing are distinct operations.
- First-party status is verified policy, not hard-coded names or
  self-assertion.
- First-party content can be composed into a default Packagepack.
- Cargo dependencies and Vapor content dependencies are separate graphs.
- Traits are marker capabilities; content roles are not traits.
- Trait cardinality belongs to the trait, not to each slot.
- Ordinary commands hide controlled Cargo, Git, GitHub, and SteamCMD backends.
- Contextual unavailability must be visible and explained.
- Consequential previews use `--dry-run`.
- Application publishing uses `root build`, `root package`, and
  `root publish [--dry-run]`; it
  does not expose SteamCMD as a top-level user workflow.
- Self-setup repair and IDE repair are manual and previewable.
- Scripts may automate safe preparation and previews, but not interactive auth
  or real publication.

## Unresolved decisions

The topology does not depend on settling these immediately:

- the exact provider-declaration syntax for trait implementations;
- whether and how traits compose or imply other traits;
- the precise final definition of a publishable artifact;
- whether one Workshop artifact may combine output from several Cargo packages;
- how Vapor dependency constraints reuse, reference, or intentionally ignore
  Cargo dependency information;
- the cryptographic, registry, installation, or Steam evidence proving
  first-party authority;
- the final open, close, create, initialize, adopt, and Workshop publish command
  grammar;
- which operation metadata drives help, documentation, previews, scripts, and
  future GUI surfaces.

## Documentation consequence

Documentation should be rebuilt in this order:

1. keep this checkpoint as the canonical design model;
2. keep the implemented manifest/schema docs synchronized with code;
3. write beginner-facing principal workflows from the model;
4. settle command vocabulary before documenting detailed CLI syntax as final;
5. document artifact, dependency, trait, slot, and publication schemas only
   where examples have forced concrete fields;
6. derive task guides from implemented behavior;
7. keep implementation architecture synchronized after each workflow pass.

User-facing documentation is authoritative only where it matches implemented
and verified behavior.
