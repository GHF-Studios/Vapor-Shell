# Interactive commands

Run `help` for the command list or `help <COMMAND>` for argument details.
`vapor` without arguments opens the interactive shell. The shell owns source
context, setup state, and command authority. Host-level direct facades are
limited to setup/bootstrap and automation entrypoints: `source`, `setup`,
`metadata`, `installation`, `binaries`, `libraries`, read-only
`content status|list|verify`, and `script run`.

Repeatable automation should live in `.vapor/scripts/NAME.vapor` and run through
`vapor script run NAME`, which executes the same command grammar against a
Vapor shell session state. Real Steam uploads and real IDE repair remain manual
interactive-shell actions.

## Source navigation

### `cd [SOURCE_PATH]`

Change the internal source directory. Relative paths start at the current
internal directory; absolute paths are accepted only inside the active source
root. Omitting the argument returns to the source root.

### `up [LEVELS]`

Move toward the source root. `LEVELS` defaults to `1` and must be a positive
integer. Moving above the source root is an error.

### `pwd`

Print the internal source directory.

### `ls [SOURCE_PATH]`

List a source directory after the same canonical containment checks used by
`cd`.

## Installation resources

### `installation`

Print the Steam installation/app root discovered from the running `bin/vapor`
executable.

### `binaries`

Print the app-local `bin` directory.

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

Accept the current app root, register its `bin` directory for PATH setup, and
install missing Rust/Cargo, Git, and SteamCMD into the app root. This command is
explicit; other commands do not install or repair prerequisites automatically.
It does not create `packages/setup`; use `setup self package install` for package
payloads.

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

Report distributable self-setup payload readiness. These payloads are copied
into app/depot staging and are separate from the active tools used in the
current Steam installation.

### `setup self package install [--dry-run]`

Populate missing `packages/setup` payloads from active app-local tools. The
active Rust/Cargo, Git, and SteamCMD tools must already pass `setup self status`.
A script that delegates to system Git must be replaced with a real app-owned
Git installation before payloads can be built.

`--dry-run` previews package writes without changing files.

### `setup self package repair [--dry-run]`

Rebuild `packages/setup` from active app-local tools. Use this after
repairing active setup or before staging a new Steam app/depot build.

`--dry-run` previews the package rebuild without changing files.

## Cargo workflows

### `fmt|check|test|build [--project PROJECT]`

Run the selected Cargo operation through app-local Rust/Cargo.
`PROJECT` is `all` or a Cargo workspace name discovered from the active source
root. `[workspace]` sources expose their root Cargo workspace. `[root]` sources
expose direct submodules that declare `[workspace]` and contain `Cargo.toml`.

Artifacts go to `output/dev/<project>` inside the app root instead of source
trees. Rust and Git must already pass `setup self status`.

### `validate [--project PROJECT]`

For each selected Cargo workspace, run formatting verification, `cargo check`,
tests, strict Clippy, and strict Rustdoc.

## Source session

### `source status`

Report whether a source is open, the active source identity and cursor, and the
number of indexed sources.

### `source open SOURCE`

Open a Vapor source root by registered name or path. A path is resolved,
validated, added to the app-local source registry, and persisted as the active
source for later shell launches.

The active source must be outside the installed app root. Once opened, all
navigation commands are confined to that source root.

### `source close`

Close the active source and return the shell to its app-only state. Setup,
source-registry, metadata, and installation-inspection commands remain
available; source navigation and source-backed Cargo/content workflows wait for
another `source open`.

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

### `source repair`

Inspect source registry state and print repair guidance. The current
implementation does not mutate source repositories; stale app-local source
entries can be removed with `source remove SOURCE` and re-added with
`source add PATH`.

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

### `root build`

Build discovered Cargo workspaces and promote declared application binaries
into the Steam installation/app root.

### `root package`

Build installed documentation, assemble the clean allowlisted app/depot payload,
and smoke-check the staged package without invoking SteamCMD. This requires
complete `packages/setup` content; run `setup self package install` or
`setup self package repair` first when metadata reports missing self-setup payloads.

### `root publish [--account ACCOUNT] [--branch BRANCH] [--dry-run] [--yes]`

Validate, build, promote binaries, build docs, stage the clean app/depot
payload, smoke-check it, generate a SteamPipe VDF, and optionally upload it.

`--dry-run` writes the staged payload and preview VDF without requiring
SteamCMD or performing an upload. A real upload requires `--account ACCOUNT`
and `--yes`, and must be typed manually in the interactive shell. The branch
defaults to `[root.steam].development-branch` and must be non-default.

## Content workflows

### `content status`

Report the nearest typed content node under the source cursor and print the
app-root content layout used for installed payloads, cache, and generated
state. Packagepacks, Enginepacks, Gamepacks, and Modpacks are content
artifacts, not application depot roots.

### `content list`

List source-discovered content artifacts when a source is open and list
installed content recorded in the app-owned content index.

### `content validate [ARTIFACT]`

Validate source content metadata, required composition/dependency references,
conflicts, and Workshop publication intent. Omit `ARTIFACT` to validate every
source-discovered artifact.

### `content build`

Build the active content workspace through app-local Cargo. This uses the same
setup preflight as other Cargo workflows and writes build output under the app
root.

### `content package ARTIFACT [--dry-run]`

Stage one artifact under `output/content/packages/`, copy its payload, write a
package manifest, fingerprint it, and record a receipt. `--dry-run` computes
the intended package path and source fingerprint without writing files.

### `content acquire ARTIFACT_OR_WORKSHOP_ID`

Acquire content into the app-owned cache. Source artifacts are packaged and
cached locally. Cached Workshop IDs can be reused. A live uncached Workshop
download requires a SteamUGC-enabled provider session; this build reports that
provider boundary when no cache exists.

### `content subscribe ARTIFACT_OR_WORKSHOP_ID`

Subscribe to or otherwise acquire content through controlled providers. The
current implementation shares the safe acquire/cache path and records the
provider boundary when live SteamUGC subscription is unavailable.

### `content download ARTIFACT_OR_WORKSHOP_ID`

Download content into the app-owned cache. Source artifacts use the local
package/cache path; uncached Workshop IDs require a SteamUGC-enabled provider.

### `content install ARTIFACT_OR_WORKSHOP_ID`

Install source or cached content into `content/installed/`, resolving required
local dependency/composition edges first. Installation writes a content index,
per-artifact lock, fingerprint, and receipt.

### `content update [ARTIFACT_OR_WORKSHOP_ID]`

Reinstall one installed item, or every installed item when omitted, from source
or cache.

### `content verify [ARTIFACT_OR_WORKSHOP_ID]`

Compare installed payloads against app-owned fingerprints and receipts. Omit
the target to verify everything in the installed-content index.

### `content selected`

Print the currently selected packagepack, if one is recorded in app-owned
content state.

### `content select ARTIFACT_OR_WORKSHOP_ID`

Select an installed, enabled packagepack for play. Selection writes
`.vapor/state/content/selection.toml` and an operation receipt.

### `content deselect`

Clear the selected packagepack.

### `content repair [ARTIFACT_OR_WORKSHOP_ID]`

Verify installed content, quarantine corrupted payloads under
`content/quarantine/`, and reinstall from source or cache when available.

### `content disable ARTIFACT_OR_WORKSHOP_ID`

Move installed content to `content/disabled/` and update the content index
without deleting its payload.

### `content enable ARTIFACT_OR_WORKSHOP_ID`

Move disabled content back to `content/installed/` and update the content
index.

### `content uninstall ARTIFACT_OR_WORKSHOP_ID`

Remove installed or disabled payloads and delete the app-owned installed-state
record. Dependency payloads are not removed implicitly; uninstall them
explicitly when desired.

### `content create ARTIFACT --dry-run`

Record a safe preview of creating a new Workshop item. Real item creation is a
SteamUGC authority-changing action and must be performed manually through a
SteamUGC-enabled provider.

### `content publish ARTIFACT [--dry-run] [--account ACCOUNT] [--change-note TEXT] [--yes]`

Package an artifact and write a Workshop provider VDF. `--dry-run` performs no
upload. A real upload requires `--account ACCOUNT`, `--yes`, an existing
PublishedFileId in the artifact's `Vapor.toml`, and must be typed manually in
the interactive shell.

### `content delete ARTIFACT_OR_WORKSHOP_ID --dry-run`

Record a safe preview of deleting or retiring a Workshop item. Real deletion is
a SteamUGC authority-changing action and is refused unless a controlled
SteamUGC provider implements it.

## Scripts

### `script run NAME [--dry-run]`

Read `.vapor/scripts/NAME.vapor` and execute each non-comment line through this
same command parser. `--dry-run` prints the commands without executing them.
Scripts stop on error and cannot recursively invoke scripts, exit the host REPL,
authenticate Steam, perform real publishes, or apply IDE repairs.

## Session control

### `exit`

Exit the shell. `quit` is an alias. Ctrl-D also exits; Ctrl-C cancels the
current input line.
