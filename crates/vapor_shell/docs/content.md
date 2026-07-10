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
├── installed/                   enabled payloads by content ID
├── disabled/                    retained disabled payloads by content ID
└── quarantine/                  corrupt or incomplete payloads

output/content/
├── packages/                    staged package payloads
└── scripts/                     Workshop provider VDF previews

.vapor/state/content/
├── index.toml                   installed/cache index
├── locks/                       per-artifact resolved install records
├── selection.toml                selected packagepack
└── receipts/                    package, acquire, install, publish, repair receipts
```

`Vapor.toml` holds source-authored intent: identity, content role, version
policy, dependencies, conflicts, composition, AppID, PublishedFileId when one
exists, visibility, title, tags, and update intent. Fingerprints, installed
paths, cache observations, locks, receipts, and repair diagnostics are generated
state and stay under the app root.

## Loo-Cast proving workspace

Open Loo-Cast as a normal workspace:

```text
source open /path/to/Loo-Cast
content list
content validate
```

The first-party proving artifacts are:

```text
ghf-studios/loo-cast/spacetime-engine
ghf-studios/loo-cast/loo-cast-game
ghf-studios/loo-cast/loo-cast-packagepack
```

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
content publish ghf-studios/loo-cast/loo-cast-packagepack --dry-run
```

The dry-run writes a package and Workshop provider VDF but performs no upload.
Real Workshop updates require an existing `published-file-id`, an account, and
manual interactive confirmation:

```text
content publish ghf-studios/loo-cast/loo-cast-packagepack --account ACCOUNT --yes
```

Scripts cannot perform real Workshop create, publish, or delete operations.
Those authority-changing steps must be typed manually in the interactive shell.

## Corruption and repair

`content verify` recomputes fingerprints from installed payloads and compares
them with app-owned locks and receipts. `content repair` quarantines corrupted
payloads under `content/quarantine/` and reinstalls from source or cache when
available. If neither source nor cache can satisfy the item, Vapor reports the
missing provider instead of silently deleting or replacing content.
