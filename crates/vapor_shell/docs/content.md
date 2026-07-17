# Content lifecycle

Vapor content commands manage Workshop-backed runtime content from the Steam
installation/app root. Source stays outside the app root; installed content is
runtime material, not a Git checkout.

## App-root layout

Generated content state uses this app-owned layout:

```text
content/
├── workshop/downloads/          provider-observed Steam downloads
├── cache/packages/              Vapor-managed package cache
├── installed/                   enabled artifact roots by content ID
├── disabled/                    retained disabled artifact roots by content ID
└── quarantine/                  corrupt or incomplete artifact roots

output/content/
├── packages/                    staged deployable artifact roots
└── scripts/                     Workshop provider VDF previews

.vapor/state/content/
├── index.toml                   installed/cache index
├── locks/                       per-artifact resolved install records
├── selection.toml                selected packagepack
└── receipts/                    package, acquire, install, publish, repair receipts
```

`Vapor.toml` is also the content metadata carrier. In a source workspace it
holds authoring intent: content role, version policy, dependencies, conflicts,
composition, AppID, PublishedFileId when one exists, visibility, title, tags,
update intent, and declared runtime outputs such as `binaries` and `libraries`.
Source content is only content when its child path is registered by the
workspace manifest:

```toml
[[workspace.projects]]
path = "spacetime-engine"
```

The child `Vapor.toml` owns the content role and metadata; the workspace
registration owns membership in the workspace. Vapor does not recursively guess
content membership from every nested manifest it finds.

When `content package` stages an artifact, Vapor writes a resolved deployed
`Vapor.toml` into the artifact root and copies declared runtime outputs into
target-specific `bin/<target>/` and `lib/<target>/` directories so the installed
or Workshop-downloaded artifact remains self-describing without becoming a
standalone source workspace. The deployed manifest records the actual staged
files under `[[engine.runtime]]`, `[[game.runtime]]`, or the matching content
section. Fingerprints, installed paths, cache observations, locks, receipts, and
repair diagnostics are generated state and stay under the app root.

`content build`, `content deploy`, and `content package` accept
`--target TARGET` for explicit platform output such as
`x86_64-pc-windows-gnullvm`. When `[workspace.runtime].targets` is declared in
the source `Vapor.toml`, omitting target flags uses that full matrix by
default. `--release-targets` is accepted as an explicit spelling of the same
manifest-matrix behavior.
`content package`, `content create`, and `content publish` may repeat
`--target` to stage a custom subset into one package root. Use `--host-only`
for quick local smoke passes that should read the normal app-local Cargo output
directory. Manifest or explicit targets read from
`output/dev/<workspace>/<target>/debug/` and stage files under the same target
name inside the deployed content root.

This is the release shape for Workshop content. A Workshop item represents one
logical content artifact and may carry every shipped runtime target under the
same artifact root. Vapor does not split Linux and Windows content into
separate Workshop items or separate app roots; platform-specific material is
selected from `bin/<target>/` and `lib/<target>/` after download/install.

## New content workspace

The installed app can create a minimal engine/game/packagepack workspace:

```text
source init basic-content /path/to/my-content --organization my-studio --name my-content
content validate
content deploy my-studio/my-content/my-content-packagepack --select
```

The template is intentionally small. It creates a normal Cargo workspace and
normal Vapor content projects; it does not create proprietary sidecar metadata.

For first Workshop publication, create dependency items first and repair
dependency Workshop IDs between steps:

```text
content create my-studio/my-content/my-content-engine --account ACCOUNT --yes
source repair --write
content create my-studio/my-content/my-content-game --account ACCOUNT --yes
source repair --write
content create my-studio/my-content/my-content-packagepack --account ACCOUNT --yes
```

After all items have PublishedFileIds, updates use normal batch publication:

```text
source repair --write
content publish my-studio/my-content/my-content-engine my-studio/my-content/my-content-game my-studio/my-content/my-content-packagepack --account ACCOUNT --yes
```

## Loo-Cast first-party workspace

Open Loo-Cast as a normal workspace:

```text
source open /path/to/Loo-Cast
content list
content validate
```

The first-party product artifacts are:

```text
ghf-studios/loo-cast/spacetime-engine
ghf-studios/loo-cast/loo-cast-game
ghf-studios/loo-cast/loo-cast-packagepack
```

They are registered in `Loo-Cast/Vapor.toml` under `[[workspace.projects]]`.
Prototype/demo content belongs in `Vapor-Examples`, not in this product
workspace.

Run the safe local roundtrip with:

```text
script run content-roundtrip
```

That script packages, subscribes/acquires, downloads/caches, installs the
packagepack dependency closure, verifies fingerprints, updates, disables,
enables, repairs, and uninstalls without touching Steam authority.
It also selects and clears the packagepack through app-owned selection state.

## Workshop publication boundary

Preview publication with:

```text
script run content-publish-preview
```

or manually:

```text
content publish ghf-studios/loo-cast/spacetime-engine ghf-studios/loo-cast/loo-cast-game ghf-studios/loo-cast/loo-cast-packagepack --dry-run
```

The dry-run writes packages and Workshop provider VDFs but performs no upload.
Real Workshop updates require an existing `published-file-id`, an account, and
manual interactive confirmation:

```text
content publish ghf-studios/loo-cast/spacetime-engine ghf-studios/loo-cast/loo-cast-game ghf-studios/loo-cast/loo-cast-packagepack --account ACCOUNT --yes
```

For platform-specific runtime payloads, the default release path builds and
publishes the workspace target matrix:

```text
content build
content publish ghf-studios/loo-cast/spacetime-engine ghf-studios/loo-cast/loo-cast-game ghf-studios/loo-cast/loo-cast-packagepack --dry-run
```

For quick local Linux iteration, opt out explicitly:

```text
content deploy ghf-studios/loo-cast/loo-cast-packagepack --select --host-only
```

Scripts cannot perform real Workshop create, publish, or delete operations.
Those authority-changing steps must be typed manually in the interactive shell.

## Corruption and repair

`content verify` recomputes fingerprints from installed artifact roots and
compares them with app-owned locks and receipts. `content repair` quarantines
corrupted artifact roots under `content/quarantine/` and reinstalls from source
or cache when available. If neither source nor cache can satisfy the item,
Vapor reports the missing provider instead of silently deleting or replacing
content.
