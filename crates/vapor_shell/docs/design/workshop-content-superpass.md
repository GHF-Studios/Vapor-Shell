# Workshop content superpass

Status: **owner decision checkpoint and implementation handoff**

This checkpoint records the current direction for SteamUGC, Workshop-backed
custom content, source command unification, scripting, and user-facing docs. It
extends `product-topology.md` and `setup-and-backend-superpass.md`; it does not
replace either document.

## Recovery note

This document is a durable handoff point for the next implementation pass. If
chat context is lost, continue from this file before older planning discussion.

The immediate target is a full working Workshop/custom-content stack, not a
thin metadata-only prototype. The first concrete proving workspace is
`GHF-Studios/Loo-Cast`, which should behave as a normal first-party custom
content Vapor workspace. Its current publishable content artifacts are:

- `ghf-studios/loo-cast/spacetime-engine` as an engine item;
- `ghf-studios/loo-cast/loo-cast-game` as a game item;
- `ghf-studios/loo-cast/loo-cast-packagepack` as a packagepack item.

Vapor-Root remains the application/depot source root. Vapor-Registry remains
separate registry authority infrastructure. Neither should be collapsed into
the normal custom-content workspace model.

## Required product outcome

The next SteamUGC pass should design and implement the whole content lifecycle
around Vapor concepts:

- discover publishable content artifacts in a Vapor workspace;
- create and delete Workshop items where authority permits;
- update Workshop item metadata and artifact roots;
- subscribe to or otherwise acquire required Workshop items through SteamUGC
  where the API permits it;
- download, cache if useful, install, update, disable, enable, and uninstall
  Workshop-backed content;
- track dependencies, conflicts, compatibility, fingerprints, installed state,
  receipts, and repair status;
- make Loo-Cast's engine, game, and packagepack work as the first complete
  first-party content target.

A partial implementation may still be staged internally, but the planned
feature is the complete subscribe/download/install/update/uninstall and
publish/update/delete stack.

## Steam identity and manifest ownership

When it is sensible and stable, authored Steam and Workshop intent belongs in
`Vapor.toml`. This includes source-owned publication identity and policy such
as AppID, PublishedFileId, visibility, title, tags, dependency IDs,
compatibility policy, update intent, and declared runtime outputs such as
content-owned binaries or libraries.

`Vapor.toml` is also the deployed artifact metadata carrier. Source content
remains workspace-bound for authoring, but packaged, installed, and
Workshop-downloaded content roots carry a resolved deployed `Vapor.toml` so the
artifact can be understood without a separate proprietary package manifest.
Declared runtime outputs are copied into the deployed artifact root under
target-specific `bin/<target>/` and `lib/<target>/` directories. The deployed
artifact `Vapor.toml` records the actual staged filenames in target-specific
runtime entries.

Workspace source manifests own Vapor project membership through
`[[workspace.projects]]`. Cargo membership remains in `Cargo.toml`, but Vapor
content commands only operate on registered workspace projects whose child
`Vapor.toml` declares a content identity. This prevents arbitrary nested
manifest scans from deciding what counts as publishable source content.

Generated or observed runtime state belongs in generated app-owned files, not
in source manifests. Examples include local download state, installed receipts,
fingerprint observations, cache records, last-seen Steam metadata, and
operation logs.

The schema pass must draw this line explicitly before implementing broad
SteamUGC writes. `Vapor.toml` should describe what the source intends to be;
locks, receipts, and indexes should describe what was resolved, downloaded,
installed, verified, or changed.

## Installed content layout

The implemented installed-content layout is app-root owned:

```text
content/
├── workshop/downloads/
├── cache/packages/
├── installed/
├── disabled/
└── quarantine/

output/content/
├── packages/
└── scripts/

.vapor/state/content/
├── index.toml
├── locks/
├── selection.toml
└── receipts/
```

`content/workshop/downloads/` is reserved for provider-observed Steam downloads
when Vapor can see them. `content/cache/packages/` holds Vapor-managed package
cache entries. `content/installed/` contains enabled artifact roots,
`content/disabled/` contains retained disabled artifact roots, and
`content/quarantine/` contains corrupt or incomplete artifact roots moved aside
during repair. `output/content/packages/` holds staged deployable artifact roots
with resolved `Vapor.toml` files, and `output/content/scripts/` holds Workshop
provider VDF previews.
`.vapor/state/content/index.toml`, `locks/`, and `receipts/` hold generated
dependency/conflict indexes, fingerprints, install locks, packagepack
selection, and operation receipts.

Authored source repositories stay outside the Steam installation. Installed
Workshop artifacts are runtime/content material, not Git working trees and not
source checkouts.

## Command model

Source operations should move toward a unified `source` domain:

```text
source status
source open SOURCE
source close
source list
source add PATH
source remove SOURCE
source sync
source repair
```

This is a breaking command-model cleanup. Do not preserve legacy public shims
solely for compatibility. If the rename is accepted during implementation,
remove the old public surface and update docs, help, scripts, tests, and
diagnostics together.

Root/application operations remain special because they package and publish the
Steam app/depot. Registry operations remain special because they manage
registry authority. Normal custom content, including first-party Loo-Cast
content, should use the content/Workshop model.

Content commands express Vapor goals rather than raw SteamUGC calls. The current
implemented grammar is:

```text
content status
content list
content validate
content build [--target TARGET]... [--release-targets] [--host-only]
content deploy ARTIFACT [--select] [--target TARGET]... [--release-targets] [--host-only]
content package ARTIFACT [--target TARGET]... [--release-targets] [--host-only]
content acquire ARTIFACT_OR_WORKSHOP_ID
content subscribe ARTIFACT_OR_WORKSHOP_ID
content download ARTIFACT_OR_WORKSHOP_ID...
content install ARTIFACT_OR_WORKSHOP_ID
content update [ARTIFACT_OR_WORKSHOP_ID]
content verify [ARTIFACT_OR_WORKSHOP_ID]
content selected
content select ARTIFACT_OR_WORKSHOP_ID
content deselect
content disable ARTIFACT_OR_WORKSHOP_ID
content enable ARTIFACT_OR_WORKSHOP_ID
content uninstall ARTIFACT_OR_WORKSHOP_ID
content repair [ARTIFACT_OR_WORKSHOP_ID]
content create ARTIFACT [--target TARGET]... [--release-targets] [--host-only] --dry-run
content publish ARTIFACT... [--target TARGET]... [--release-targets] [--host-only] [--dry-run]
content delete ARTIFACT_OR_WORKSHOP_ID --dry-run
```

SteamUGC, SteamCMD, filesystem staging, Git, and Cargo remain backend
providers. Real Workshop create/publish/delete stays manual and authority-bound;
scripts may run the local lifecycle and dry-runs only.

## Scripting and authority

Scripts are the sanctioned replacement for broad one-shot host commands. They
may be used in two conventions:

- host-invoked one-shot automation through `vapor script run NAME`;
- in-shell automation that runs inside an already established Vapor context.

Pipelines should split cleanly into scriptable pre-process work, the
human-owned final authority boundary, and scriptable post-process work where
that is safe.

Scripts may automate validation, metadata resolution, packaging, dry-runs,
local install/update/uninstall work, and preparation for publish. They may
collect context and pass it through the shell session. They must not bypass the
absolute final human confirmation for irreversible upload/publication actions.

The same rule applies to the three publication domains:

- application/depot publication from the app/root source;
- registry publication or mutation;
- custom-content Workshop publication.

Everything up to the final total confirmation can be scriptable if the command
itself is safe to automate and its effects are explicit. Final upload or
authority-changing confirmation must be manual and interactive.

## Documentation direction

User-facing docs and planning/agent docs have different jobs.

`README.md` files should be usable by someone who knows only the basic premise:
Vapor is the Steam-installed shell, SDK, and content workflow for Loo Cast and
Vapor content. They should also scale to power users by progressive disclosure
inside the same document or linked reference sections. Avoid splitting docs into
many tiny README files just to separate entry-level and advanced usage.

`AGENTS.md` should remain the agent/session protocol. It may point to product
docs and planning gates, but it should not be the human product entry point and
should not contradict the shell command model.

Design docs preserve vocabulary, authority, and rationale. User docs should
explain what to do, what state means, how to recover, and which operations are
implemented versus planned.

## Proof target

SteamUGC integration is not proven by a unit test alone. The target proof is a
working Workshop lifecycle:

```text
create or resolve item
publish or update artifact root
subscribe/acquire
download
install
verify
update
uninstall
repair or report corrupted state
```

Where real Steam credentials or live Workshop state are unavailable, implement
the strongest safe lower-level proof, but keep that clearly labeled as less
than full integration. A final pass should exercise a private or otherwise safe
real Workshop roundtrip.
