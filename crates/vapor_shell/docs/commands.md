# Interactive commands

Run `help` for the command list or `help <COMMAND>` for argument details.
`vapor` without arguments opens the interactive shell. Ad-hoc one-shot commands
are intentionally disabled; the direct CLI facade is reserved for
setup/bootstrap commands: `open`, `close`, `sources`, `setup`,
`metadata`, `installation`, `binaries`, `libraries`, and `script run`.

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

### `setup status`

Report the executable-derived app root, persisted app-root registration, active
Rust/Cargo, Git, SteamCMD health, and distributable package-content status.

### `setup install [--dry-run]`

Accept the current app root, register its `bin` directory for PATH setup, and
install missing Rust/Cargo, Git, and SteamCMD into the app root. This command is
explicit; other commands do not install or repair prerequisites automatically.
It does not create `packages/setup`; use `setup package install` for package
payloads.

`--dry-run` prints the app-root registration, PATH profiles, acquisition paths,
package status, and tool group actions without changing files or shell profile
state.

### `setup repair [--dry-run]`

Accept the current app root and reapply/reacquire Rust/Cargo, Git, and SteamCMD.
Use this after an intentional Steam app move or suspected setup damage.
Repairing active setup still does not refresh package payloads.

`--dry-run` previews the reinstall/repair actions without changing files or
registration state.

### `setup uninstall [--dry-run]`

Remove app-local Rust/Cargo, Git, SteamCMD, PATH registration, and app-root
location state.

`--dry-run` previews removals and registration cleanup without deleting active
tools or changing PATH setup.

### `setup package status`

Report distributable setup package payload readiness. These payloads are copied
into app/depot staging and are separate from the active tools used in the
current Steam installation.

### `setup package install [--dry-run]`

Populate missing `packages/setup` payloads from active app-local tools. The
active Rust/Cargo, Git, and SteamCMD tools must already pass `setup status`.
Host Git wrappers are rejected; the package must contain an app-owned Git
distribution.

`--dry-run` previews package writes without changing files.

### `setup package repair [--dry-run]`

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
trees. Rust and Git must already pass `setup status`.

### `validate [--project PROJECT]`

For each selected Cargo workspace, run formatting verification, `cargo check`,
tests, strict Clippy, and strict Rustdoc.

## Source session

### `open SOURCE`

Open a Vapor source root by registered name or path. A path is resolved,
validated, added to the app-local source registry, and persisted as the active
source for later shell launches.

The active source must be outside the installed app root. Once opened, all
navigation commands are confined to that source root.

### `close`

Close the active source and return the shell to its app-only state. Setup,
source-registry, metadata, and installation-inspection commands remain
available; source navigation and Cargo/content workflows wait for another
`open`.

### `sources list`

List source roots registered under the current app root.

### `sources add [PATH]`

Validate and register a source root. `PATH` defaults to the process directory
used to start Vapor. Registration does not open the source; use `open NAME` or
`open PATH` for that.

### `sources remove SOURCE`

Remove a registered source by local name or fully qualified identity. If the
removed source is active, run `close` or `open` another source before source
workflows.

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
complete `packages/setup` content; run `setup package install` or
`setup package repair` first when metadata reports missing package payloads.

### `root publish [--account ACCOUNT] [--branch BRANCH] [--dry-run] [--yes]`

Validate, build, promote binaries, build docs, stage the clean app/depot
payload, smoke-check it, generate a SteamPipe VDF, and optionally upload it.

`--dry-run` writes the staged payload and preview VDF without requiring
SteamCMD or performing an upload. A real upload requires `--account ACCOUNT`
and `--yes`, and must be typed manually in the interactive shell. The branch
defaults to `[root.steam].development-branch` and must be non-default.

## Content workflows

### `content status`

Report the nearest typed content node under the source cursor. Packagepacks,
Enginepacks, Gamepacks, and Modpacks are content artifacts, not application
depot roots.

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
