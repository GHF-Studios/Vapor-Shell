# Interactive commands

Run `help` for the command list or `help <COMMAND>` for argument details.
`vapor` without arguments opens the interactive shell. The shell owns source
context, setup state, and command authority. Host-level direct facades are
limited to setup/bootstrap and explicit automation entrypoints: `source`,
`setup`, `metadata`, `installation`, `binaries`, `libraries`, `launch`,
`content`, `root`, and `script run`.

Repeatable automation should live in `.vapor/scripts/NAME.vapor` and run through
`vapor script run NAME`, which executes the same command grammar against a
Vapor shell session state. Use `vapor --startup-script NAME` to enter the
interactive shell, run a source or app-root script, and keep the shell open.
Real Steam uploads and real IDE repair remain manual interactive-shell actions.

## Launch

### `launch loo-cast [--account ACCOUNT]`

Launch Play Loo-Cast through the selected installed packagepack composition.
When no packagepack is selected, Vapor tries the first-party Loo-Cast
Packagepack, `ghf-studios/loo-cast/loo-cast-packagepack`, if it is already
installed.

The command verifies installed content, resolves the packagepack's Spacetime
Engine dependency, and hands off to the installed engine binary declared by
that engine artifact's deployed `Vapor.toml`. The current first-party
Spacetime Engine is a product placeholder; the dynamic terminal/game-library
proof lives in `Vapor-Examples`. On Linux/Steam desktop starts without a
terminal, this command opens the same Konsole-owned terminal path used by the
Shell so terminal-based runtime output remains visible.

If content is not installed yet, the command uses the app-root first-party
content seed to download/cache/install/select the public Loo-Cast Packagepack
and required first-party engine/game dependencies. It still does not silently
install the toolchain; missing setup reports `setup self install`.

Use `--account ACCOUNT` when the Workshop item is not downloadable by anonymous
SteamCMD, such as unreleased/private app testing.

## Installation resources

### `installation`

Print the Steam installation/app root discovered from the running Vapor
executable.

### `binaries`

Print the app-local binary directory that contains the running Vapor executable.
In release-mode wrapper launches this is usually `bin/<target>/`.

### `libraries`

Print the app-local `lib` directory when it exists.

## Derived context

### `metadata [--format human|json]`

Resolve the active source root, nearest content, app-root registration state,
app-local tools, root/workspace policy, optional `[root.steam]` policy, and
Cargo metadata into one report. Human-readable output is the default. JSON is
the stable machine interface for scripts and agents.

Metadata reporting is best-effort: missing tools, failed Cargo projection, an
unregistered app root, or absent optional Steam policy are reported as
diagnostics instead of hiding the rest of the environment. Commands use the same
resolved model and reject unmet prerequisites before acting.

## Setup

### `setup self status`

Report the executable-derived app root, persisted app-root registration, active
Rust/Cargo, Git, SteamCMD health, and distributable self-setup payload status.

### `setup self install [--dry-run]`

Accept the current app root, register the active app-owned Vapor binary
directory for PATH setup, and install missing Rust/Cargo, Git, and SteamCMD into
the app root. This command is explicit; other commands do not install or repair
prerequisites automatically. It does not create `packages/setup`; use
`setup self package install` for package payloads.

`--dry-run` prints the app-root registration, PATH profiles, acquisition paths,
package status, and tool group actions without changing files or shell profile
state.

When complete `packages/setup` payloads exist, Git is copied from the app-owned
payload. Otherwise Linux setup imports a usable host Git binary into
`tools/git`, copies its Git exec-path support files, and replaces any script
that delegates to system Git.

### `setup self repair [--dry-run]`

Accept the current app root and reapply/reacquire Rust/Cargo, Git, and SteamCMD.
Use this after an intentional Steam app move or suspected setup damage.
Repairing active setup still does not refresh self-setup payloads.

`--dry-run` previews the reinstall/repair actions without changing files or
registration state.

### `setup self uninstall [--dry-run]`

Remove app-local Rust/Cargo, Git, SteamCMD, PATH registration, and app-root
location state.

`--dry-run` previews removals and registration cleanup without deleting active
tools or changing PATH setup.

### `setup self package status`

Report distributable self-setup payload readiness. These payloads are available
to explicit stacked app/depot staging and are separate from the active tools
used in the current Steam installation.

### `setup self package install [--dry-run]`

Populate missing `packages/setup` payloads from active app-local tools. The
active Rust/Cargo, Git, and SteamCMD tools must already pass `setup self status`.
A script that delegates to system Git must be replaced with a real app-owned
Git installation before payloads can be built.

`--dry-run` previews package writes without changing files.

### `setup self package repair [--dry-run]`

Rebuild `packages/setup` from active app-local tools. Use this after
repairing active setup or before an explicit
`--include-setup-payload` app/depot build.

`--dry-run` previews the package rebuild without changing files.

## Cargo workflows

### `fmt|check|test|build [--project PROJECT]`

Run the selected Cargo operation through app-local Rust/Cargo.
`PROJECT` is `all` or a Cargo workspace name discovered from the active source
root. `[workspace]` sources expose their root Cargo workspace. `[root]` sources
expose direct submodules that declare `[workspace]` and contain `Cargo.toml`.

Artifacts go to `output/dev/<project>` inside the app root instead of source
trees. Rust and Git must already pass `setup self status`.
Workspaces that declare `[workspace].binaries` in `Vapor.toml` can promote
those outputs into `bin/<target>/` through `root build`.

### `validate [--project PROJECT]`

For each selected Cargo workspace, run formatting verification, `cargo check`,
tests, strict Clippy, and strict Rustdoc.

## Source session

### `source init basic-content PATH --organization ORG --name NAME [--app-id APPID]`

Create a new source workspace with a basic engine, game, and packagepack. The
target path must be empty or absent. The generated workspace is ordinary source:
`Vapor.toml` declares `[workspace]` and `[[workspace.projects]]`, child
`Vapor.toml` files own content metadata, and Cargo owns Rust compilation.

If `--app-id` is omitted, Vapor uses the installed app's `[root.steam].app-id`.
After creation, Vapor opens the new source root.

First local proof:

```text
content validate
content deploy ORG/NAME/NAME-packagepack --select
```

First Workshop publication should create dependencies before the packagepack:

```text
content create ORG/NAME/NAME-engine --account ACCOUNT --yes
source repair --write
content create ORG/NAME/NAME-game --account ACCOUNT --yes
source repair --write
content create ORG/NAME/NAME-packagepack --account ACCOUNT --yes
```

### `source status`

Report whether a source is open, the active source identity and cursor, and the
number of indexed sources.

### `source open SOURCE`

Open a Vapor source root by registered name or path. A path is resolved,
validated, added to the app-local source registry, and persisted as the active
source for later shell launches.

The active source must be outside the installed app root. Once opened,
source-backed workflows are confined to that source root.

### `source close`

Close the active source and return the shell to its app-only state. Setup,
source-registry, metadata, and installation-inspection commands remain
available; source-backed Cargo/content workflows wait for another
`source open`.

### `source list`

List source roots registered under the current app root.

### `source add [PATH]`

Validate and register a source root. `PATH` defaults to the process directory
used to start Vapor. Registration does not open the source; use
`source open NAME` or `source open PATH` for that.

### `source remove SOURCE`

Remove a registered source by local name or fully qualified identity. If the
removed source is active, run `source close` or `source open` another source
before source workflows.

### `source sync`

Reserved for controlled source-provider synchronization. The current
implementation reports the active source and explains that no synchronization
is applied yet.

### `source repair [--write]`

Inspect source registry state and safe source metadata repairs. Without
`--write`, this is read-only. With `--write`, Vapor updates dependency
`workshop-id` fields it can derive from sibling content artifacts'
`published-file-id` values.

This is meant for the first Workshop publication loop: after creating an engine
or game item, run `source repair --write` so packagepack/game dependency
metadata carries the newly assigned Workshop IDs before dependent items are
created or published.

## Documentation

### `docs build|path|open [TOPIC]`

Build Rustdoc for discovered Cargo workspaces into the installed `docs/` tree,
print a generated document path, or open it asynchronously.

## IDE setup

### `ide status`

Inspect project-local RustRover/JetBrains settings for the active source root.
This is read-only. It reports the selected source root, `.idea` directory,
app-local Rust/Cargo bin directory, Rust standard-library source
status, and the state of the files Vapor manages.

### `ide repair [--dry-run]`

Write project-local IDE settings for the active source root so RustRover can
see app-local Rust/Cargo and routed Cargo workspaces.

The current first pass manages only files under the selected source root's
`.idea` directory:

- `.idea/cargoProjects.xml` for routed Cargo workspace manifests;
- `.idea/rust.xml` for the Rust/Cargo path and stdlib source path when
  packaged;
- `.idea/vapor.xml` for Vapor-owned app-root, Cargo home, rustup home, Cargo,
  rustc, rustup, Git, and source identity metadata.

`--dry-run` previews which project-local files would be written without
changing them. Real IDE repair must be typed manually in the interactive shell;
scripts may run `ide status` and `ide repair --dry-run`, but not real
`ide repair`.

## Root application/depot workflows

### `root build [--target TARGET]... [--release-targets]`

Build discovered Cargo workspaces and promote declared application binaries
from `[workspace].binaries` into the Steam installation/app root under
`bin/<target>/`. Omitting `--target` uses Cargo's host-default output and
promotes into `bin/<host-target>/`. Repeat `--target` to promote specific
platform payloads, such as Linux and Windows/MSVC.

Use `--release-targets` to build the target matrix declared in
`[root.runtime].targets`. Do not combine it with explicit `--target` values.

### `root deploy [--skip-docs] [--target TARGET]... [--release-targets]`

Build discovered Cargo workspaces, promote declared application binaries into
the Steam installation/app root under `bin/<target>/`, and rebuild installed
docs. This is local-only: it does not stage a SteamPipe VDF, upload a depot, or
touch Workshop authority.

Use `--skip-docs` for a faster local binary-only deploy. Omitting target flags
deploys only the host target. `--release-targets` deploys every target declared
in `[root.runtime].targets` and copies only the matching platform launch
wrappers.

### `root package [--include-setup-payload] [--target TARGET]... [--release-targets]`

Build installed documentation, assemble the clean allowlisted app/depot payload,
and smoke-check the staged package without invoking SteamCMD. The default root
payload is runtime-only: `Vapor.toml`, selected `bin/<target>/` application
binaries, `docs/`, app scripts, target-matching launch wrappers, and packaged
examples.

Omitting target flags stages the host target only. Repeat `--target` to stage
specific platform binaries that already exist in the app root. Use
`--release-targets` to stage the `[root.runtime].targets` matrix.

Use `--include-setup-payload` only for an intentional stacked bootstrap/depot
package. That mode additionally validates and stages `packages/setup`; run
`setup self package install` or `setup self package repair` first when metadata
reports missing self-setup payloads.

### `root publish [--include-setup-payload] [--account ACCOUNT] [--branch BRANCH] [--target TARGET]... [--release-targets] [--dry-run] [--yes]`

Validate, build, promote binaries, build docs, stage the clean app/depot
payload, smoke-check it, generate a SteamPipe VDF, and optionally upload it.
The default upload payload is runtime-only; Loo-Cast and other content
artifacts are published through `content publish`, not through the app depot.

Repeat `--target` for release-mode app builds that must ship more than one
platform binary, or use `--release-targets` for the `[root.runtime].targets`
matrix. The depot smoke check rejects platform launch wrappers when their
matching `bin/<target>/vapor[.exe]` payload is missing.

`--dry-run` writes the staged payload and preview VDF without requiring
SteamCMD or performing an upload. A real upload requires `--account ACCOUNT`
and `--yes`, and must be typed manually in the interactive shell. The branch
defaults to `[root.steam].development-branch` and must be non-default.
`--include-setup-payload` is the explicit large setup/toolchain payload mode.

## Content workflows

### `content status`

Report the nearest typed content node under the source cursor and print the
app-root content layout used for installed artifact roots, cache, and generated
state. Packagepacks, Enginepacks, Gamepacks, and Modpacks are content
artifacts, not application depot roots.

### `content list`

List registered source content artifacts when a source is open and list
installed content recorded in the app-owned content index. A child directory is
source content only when the active workspace registers it under
`[[workspace.projects]]` and the child `Vapor.toml` declares a content identity.

### `content validate [ARTIFACT]`

Validate source content metadata, required composition/dependency references,
conflicts, and Workshop publication intent. Omit `ARTIFACT` to validate every
registered source artifact.

### `content build [--target TARGET]... [--release-targets]`

Build the active content workspace through app-local Cargo. This uses the same
setup preflight as other Cargo workflows and writes build output under the app
root. Use `--target` to build explicit runtime output such as
`x86_64-pc-windows-msvc`; otherwise Vapor builds the host target. Use
`--release-targets` to build every target declared in
`[workspace.runtime].targets`.

### `content deploy ARTIFACT [--select] [--target TARGET]... [--release-targets]`

Build the active content workspace and install the selected source artifact into
the app-owned installed-content tree. Dependencies present in the same source
workspace are installed first. Declared `binaries` and `libraries` are copied
from app-local Cargo output into each deployed artifact root under
`bin/<target>/` and `lib/<target>/`. This is local-only: it does not create,
publish, delete, subscribe to, or upload Workshop items.

Use `--select` when deploying a packagepack that should become the active
playable packagepack. Omitting target flags deploys only the host target;
`--release-targets` deploys the `[workspace.runtime].targets` matrix.

### `content package ARTIFACT [--target TARGET]... [--release-targets] [--dry-run]`

Stage one deployable artifact root under `output/content/packages/`, write a
resolved deployed `Vapor.toml`, fingerprint the staged root, and record a
receipt. Declared `binaries` and `libraries` are copied from app-local Cargo
output into `bin/<target>/` and `lib/<target>/`, and the deployed manifest
records the staged filenames in target-specific `runtime` entries. Repeat
`--target` to stage several platform payloads into one package root, or use
`--release-targets` for the workspace runtime matrix.
`--dry-run` computes the intended artifact path and deployed-artifact
fingerprint without writing package files.

### `content acquire ARTIFACT_OR_WORKSHOP_ID`

Acquire content into the app-owned cache. Source artifacts are packaged and
cached locally. Cached Workshop IDs can be reused. A live uncached Workshop
download requires a SteamUGC-enabled provider session; this build reports that
provider boundary when no cache exists.

### `content subscribe ARTIFACT_OR_WORKSHOP_ID`

Subscribe to or otherwise acquire content through controlled providers. The
current implementation shares the safe acquire/cache path and records the
provider boundary when live SteamUGC subscription is unavailable.

### `content download ARTIFACT_OR_WORKSHOP_ID...`

Download one or more content items into the app-owned cache. Source artifacts
use the local package/cache path. Numeric PublishedFileIds with
`--account ACCOUNT` are downloaded through one SteamCMD provider session.

### `content install ARTIFACT_OR_WORKSHOP_ID [--account ACCOUNT]`

Install source or cached content into `content/installed/`, resolving required
local dependency/composition edges first. Installation writes a content index,
per-artifact lock, fingerprint, and receipt.

If the selector matches a root content seed or numeric PublishedFileId and no
cache exists, Vapor downloads it through SteamCMD before installing. Use
`--account ACCOUNT` for private/unreleased Workshop access.

### `content update [ARTIFACT_OR_WORKSHOP_ID]`

Reinstall one installed item, or every installed item when omitted, from source
or cache.

### `content verify [ARTIFACT_OR_WORKSHOP_ID]`

Compare installed artifact roots against app-owned fingerprints and receipts.
Omit the target to verify everything in the installed-content index.

### `content selected`

Print the currently selected packagepack, if one is recorded in app-owned
content state.

### `content select ARTIFACT_OR_WORKSHOP_ID`

Select an installed, enabled packagepack for play. Selection writes
`.vapor/state/content/selection.toml` and an operation receipt.

### `content deselect`

Clear the selected packagepack.

### `content repair [ARTIFACT_OR_WORKSHOP_ID]`

Verify installed content, quarantine corrupted artifact roots under
`content/quarantine/`, and reinstall from source or cache when available.

### `content disable ARTIFACT_OR_WORKSHOP_ID`

Move installed content to `content/disabled/` and update the content index
without deleting its artifact root.

### `content enable ARTIFACT_OR_WORKSHOP_ID`

Move disabled content back to `content/installed/` and update the content
index.

### `content uninstall ARTIFACT_OR_WORKSHOP_ID`

Remove installed or disabled artifact roots and delete the app-owned
installed-state record. Dependency artifact roots are not removed implicitly;
uninstall them explicitly when desired.

### `content create ARTIFACT [--target TARGET]... [--release-targets] --dry-run`

Record a safe preview of creating a new Workshop item. Real item creation is a
SteamUGC authority-changing action and must be performed manually through a
SteamUGC-enabled provider. Repeat `--target` to stage the package payload for
each runtime target before the Workshop create/upload boundary, or use
`--release-targets` for the workspace runtime matrix.

### `content publish ARTIFACT... [--target TARGET]... [--release-targets] [--dry-run] [--account ACCOUNT] [--change-note TEXT] [--yes]`

Package one or more artifacts and write Workshop provider VDFs. `--dry-run`
performs no upload. A real upload requires `--account ACCOUNT`, `--yes`,
existing PublishedFileIds in the artifacts' `Vapor.toml` files, and must be
typed manually in the interactive shell. Multiple artifacts are sent through one
SteamCMD provider session. Repeat `--target` when publishing a package that
must contain more than one platform payload, or use `--release-targets` for the
workspace runtime matrix.

### `content delete ARTIFACT_OR_WORKSHOP_ID --dry-run`

Record a safe preview of deleting or retiring a Workshop item. Real deletion is
a SteamUGC authority-changing action and is refused unless a controlled
SteamUGC provider implements it.

## Scripts

### `script run NAME [--dry-run]`

Read `.vapor/scripts/NAME.vapor` and execute each non-comment line through this
same command parser. Source scripts are preferred when a source is open; app-root
scripts under the installed app's `.vapor/scripts/` are used as a fallback.
`--dry-run` prints the commands without executing them.

Scripts stop on error and cannot recursively invoke scripts, exit the host REPL,
perform real publishes, delete Workshop items, or apply IDE repairs. Scripts may
run setup inspection and Workshop download/install operations, including
account-backed SteamCMD acquisition when a private or unreleased item requires
visible Steam authentication.

## Session control

### `exit`

Exit the shell. `quit` is an alias. Ctrl-D also exits; Ctrl-C cancels the
current input line.
