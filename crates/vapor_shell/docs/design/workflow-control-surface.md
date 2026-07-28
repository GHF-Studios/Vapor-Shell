# Workflow and control surface doctrine

Status: **owner discussion checkpoint and immediate migration guide**

This document captures the current direction for Vapor's operational UX/DX.
It is not a rigid workflow-catalog schema. It records the functional properties
the implementation should move toward while the exact command names and internal
data structures are still allowed to change.

The priority is unification and fat removal. Vapor is pre-alpha; do not preserve
bad public seams merely for compatibility.

## Core stance

Vapor should feel like one installed app-root-owned control plane with several
entry styles, not like unrelated products named Shell, Installer, RustRover
configs, app-root scripts, launch wrappers, Vapor scripts, and Steam options.

The app root is the authority and resolution center. That does not mean users
or developers should see app-root mechanics everywhere. User-facing surfaces
may expose different nouns for their context, but build, stage, publish, setup,
maintenance, source membership, play launch, and diagnostics should ultimately
route through one coherent precompiled Vapor control layer.

## Entry styles, not ownership boundaries

Steam launch options, the interactive Shell, RustRover configurations, direct
terminal commands, and app-root helper binaries are entry styles. They should
not define separate product models.

Current target shape:

- **Steam Play** is an opinionated app-mode route into the default playable
  composition.
- **Steam Shell** opens the versatile interactive Vapor control face.
- **Vapor Shell** can inspect, initiate, and route setup, maintenance,
  development, publish, source, content, and app operations.
- **RustRover** is an editor/controller for a SuperWorkspace. Its generated
  configurations should mostly open Vapor-owned external terminals rather than
  run substantial work inside the IDE process.
- **Direct terminal invocation** is valid when a developer or support person
  deliberately enters the app-root control surface.

Steam should expose only app entry styles such as Play and Shell. Maintenance,
setup, and developer work are initiated/configured from Shell, RustRover, or
terminal surfaces; they are not separate Steam launch-option matrix entries.

## Precompiled control-plane boundary

Core Vapor control-plane functionality must not depend on the providers it
manages. The installed app must include enough precompiled code to:

- inspect and explain app-root state;
- launch Play and Shell;
- run setup and maintenance;
- install, remove, downgrade, and repair development providers;
- clean app-owned generated state;
- prepare uninstall cleanup before Steam removes depot-owned files;
- orchestrate source, build, stage, publish, and diagnostics operations.

Rust, Cargo, Git, SteamCMD, Steamworks, Caddy, systemd, and similar tools are
providers. They may be installed, checked, invoked, or repaired by Vapor, but
they should not be prerequisites for the core Vapor control plane itself.

Rust scripts are therefore not normal app-root UX dependencies. They remain
useful as source-visible prototypes, development-only helpers, or safety-focused
implementation scripts when typed path handling, manifest parsing, validation,
or cross-platform behavior makes Rust materially safer than shell. They require
a Rust/Cargo provider and therefore belong only behind developer/source
readiness, not player/bootstrap paths.

OS shell scripts remain acceptable for platform glue and launch/bootstrap
hand-off where the work is simple and does not need structured state mutation.

## Player path

Steam Play should be the cleanest and most opinionated path:

1. start from precompiled installed app pieces;
2. resolve the default packagepack by stable Vapor content ID;
3. acquire and install required public content when possible;
4. launch the resolved engine/game composition;
5. show only concise player-appropriate failure state if something blocks play.

Normal Play must not require Rust, Cargo, Git, developer setup, source
checkouts, or publishing credentials.

Normal player Workshop acquisition should eventually use Steam client /
Steamworks-backed content APIs so the Steam client owns authentication and
subscriptions. SteamCMD-backed acquisition is a temporary provider compromise
for current implementation, private testing, and publishing-oriented workflows;
it is not product doctrine for player Play.

Diagnostics during Play should be mostly invisible. Player-visible errors
should report rough severity, subsystem/category, and a run/log ID. Detailed
diagnostics and provider logs are support/developer material.

## Developer readiness

"Developer mode" is a capability/readiness state, not a persona or permanent
product mode.

Developer setup should install or reconcile providers such as Rust, Cargo, Git,
SteamCMD, cross-target toolchains, and IDE support into the app-root-managed
environment. Developer downgrade should remove those providers and return the
app root to the player-capable baseline without requiring Steam reinstall.

Developer setup and downgrade are operation experiences owned by Vapor. They
may be initiated from Shell, terminal, or RustRover, but should open a scoped
Vapor-owned terminal/session when the operation is long-running, noisy, or
mutates app-root capabilities.

## SuperWorkspace and sources

A SuperWorkspace is a durable checkout container and registry, not a Git repo,
not a submodule umbrella, and not the source authority itself. It can be empty,
partially populated, or fully populated.

Sources are explicit optional members. Current source kinds are:

- **App Root**: singleton when present.
- **Server Root**: singleton when present.
- **Content**: many content workspaces may be present.

Clone, add, remove, init, and discover operations manage SuperWorkspace
membership. They are not app install/setup side effects and must not run as
hidden bulk operations.

There is no product-level "active source" concept in the target model. The
current SuperWorkspace is the durable developer context; operations should
target a source kind or explicit member inside it. Any temporary selection or
cursor is implementation/UI state, not the conceptual model.

## RustRover projection

RustRover does not define the SuperWorkspace. It projects an existing
SuperWorkspace and app-root control layer into IDE project metadata:

- Cargo import units;
- app-local Rust/Cargo toolchain shims;
- source membership visibility;
- operation launchers;
- status and remediation hints.

Generated RustRover configurations should mostly launch scoped external Vapor
terminals. This keeps noisy, interactive, authority-heavy, or long-running work
inside a Vapor-owned lifecycle instead of inside RustRover's run window.

RustRover patching is a real boundary exception: it mutates the invoking IDE
surface, so it must not be offered as a RustRover-run configuration. Patch
RustRover from outside RustRover before opening or after closing/reloading the
IDE project.

## Operation behavior doctrine

Do not prematurely turn operation behavior into a rigid catalog schema. For
each major operation, user and implementation docs should state the expected
functional properties:

- what durable thing the operation acts on;
- which providers or capabilities it requires;
- what may be auto-resolved for that operation;
- what must stay explicit or authority-confirmed;
- whether substantial work should run inline or in a scoped terminal/session;
- what concise output the user sees by default;
- where operation-specific logs and provider transcripts are written;
- what remediation is shown when a prerequisite is missing.

These properties should guide command, tool, script, Shell, RustRover, and Steam
entry implementation. Add code abstractions only when repeated concrete cases
prove the abstraction useful.

## Publish and external authority

Preview/dry-run is a modifier on a real operation, not its own conceptual
workflow. Prefer "Publish app" with dry-run mode over a separate "Publish
preview" action.

Any real external mutation must stay visible and explicit. App publish, Workshop
create/publish/delete, server deploy, registry mutation, and similar authority
boundaries should run in a scoped visible terminal/session with:

- exact target summary;
- account or authority summary without secrets;
- dry-run/real-mode status;
- final human confirmation immediately before the external mutation;
- operation-scoped logs and receipts.

Do not add compatibility shims for retired public surfaces unless there is a
specific owner-approved reason.

## Vapor scripts are parked

Vapor command scripts exist today, but their final role is unresolved. They may
remain useful as app-domain command recipes or proof fixtures. They should not
be treated as the primary model for Play, setup, build, stage, publish, IDE
actions, or maintenance while this UX/DX pass is active.

For now, design new UX around named operations and the precompiled app-root
control layer. Keep Vapor scripts as provisional/internal convenience unless a
specific use case proves they deserve first-class product status.

## Output and logs

Default output should be concise, intent-level, and actionable:

```text
Status
  <short readiness/result>

Next
  <one or two concrete next actions>

Logs
  <operation run/log path when relevant>
```

Provider transcripts should not flood normal user surfaces. Cargo, Git,
SteamCMD, Steamworks, installer downloads, server deployment, and RustRover
patch details belong in operation-specific logs by default, with an aggregate
app log linking or referencing those runs.

Logs should therefore be both:

- aggregate, so support/developers can reconstruct app-level history; and
- operation-scoped, so a build, publish, play launch, setup, repair, or source
  operation has a clear run directory or log bundle.

## Immediate migration consequences

Near-term implementation should cut toward this model even when that breaks old
surfaces:

- move normal setup/maintenance UX out of Rust-script entrypoints and into
  precompiled Vapor control-plane binaries;
- keep Rust scripts only where they are dev-only or materially safer than shell;
- generate RustRover actions from the app-root/SuperWorkspace model and launch
  external Vapor terminals for most substantial operations;
- keep Steam launch options minimal: Play and Shell;
- treat SteamCMD player acquisition as temporary provider plumbing, not product
  language;
- remove "publish preview" as a named workflow in favor of `publish --dry-run`;
- align build/stage/publish naming across Shell, RustRover, terminal, and docs;
- replace "active source" product language with SuperWorkspace membership and
  explicit source/content/app/server targeting;
- keep compatibility cleanup aggressive while Vapor is pre-alpha.

