# Interactive commands

Run `help` for the command list or `help <COMMAND>` for argument details.
`vapor` without arguments opens the interactive shell. Ad-hoc one-shot commands
are intentionally disabled; the direct CLI facade is reserved for
`vapor script run NAME`.

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

### `root`

Return to the active source root.

## Installation resources

### `installation`

Print the Steam application root discovered from the running `bin/vapor`
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

## Toolchain

### `toolchain status`

Report the executable-derived app root, persisted app-root registration, active
Rust/Cargo, Git, SteamCMD, and vendored package health.

### `toolchain install`

Accept the current app root, register its `bin` directory for PATH setup, and
install missing Rust/Cargo, Git, and SteamCMD from
`packages/toolchain`. This command is explicit; other commands do not install
or repair prerequisites automatically.

### `toolchain repair`

Accept the current app root and reapply all vendored Rust/Cargo, Git, and
SteamCMD packages. Use this after an intentional Steam app move or suspected
toolchain damage.

### `toolchain uninstall`

Remove app-local Rust/Cargo, Git, SteamCMD, PATH registration, and app-root
location state.

Planned alignment: mutating toolchain commands should gain `--dry-run` preview
support before this command surface is considered final. The current baseline is
still explicit and manual, but does not yet preview the exact filesystem and
PATH changes.

## Cargo workflows

### `fmt|check|test|build [--project PROJECT]`

Run the selected Cargo operation through the Steam-installed toolchain.
`PROJECT` is `all` or a Cargo workspace name discovered from the active source
root. `[workspace]` sources expose their root Cargo workspace. `[root]` sources
expose direct submodules that contain `Cargo.toml`.

Artifacts go to `output/dev/<project>` inside the app root instead of source
trees. Rust and Git must already pass `toolchain status`.

### `validate [--project PROJECT]`

For each selected Cargo workspace, run formatting verification, `cargo check`,
tests, strict Clippy, and strict Rustdoc.

## Source selection

### `workspace remember|forget`

Persist or clear the current external source root under app-local state. Steam
GUI launches use this remembered path because they normally start from the app
directory rather than a source terminal.

## Documentation

### `docs build|path|open [TOPIC]`

Build Rustdoc for discovered Cargo workspaces into the installed `docs/` tree,
print a generated document path, or open it asynchronously.

## Self-hosting app workflows

### `self rebuild|stage|smoke`

`rebuild` builds discovered Cargo workspaces and promotes selected app binaries.
`stage` builds docs and reconstructs the clean depot content tree from the
current app payload policy. `smoke` verifies the staged marker, binaries, docs,
and pinned toolchain package.

## Scripts

### `script run NAME [--dry-run]`

Read `.vapor/scripts/NAME.vapor` and execute each non-comment line through this
same command parser. `--dry-run` prints the commands without executing them.
Scripts stop on error and cannot recursively invoke scripts, exit the host REPL,
authenticate Steam, or perform real publishes.

## Steam

### `steam login --account ACCOUNT`

Temporarily hand the terminal to installation-owned SteamCMD. SteamCMD exits
after authentication and the REPL resumes.

### `steam publish --account ACCOUNT [--branch BRANCH] [--dry-run] [--yes]`

Validate, build, stage, smoke-test, generate a SteamPipe VDF, then publish to a
non-default beta branch. `--dry-run` writes a SteamPipe preview build. A real
upload requires `--yes` and must be typed manually in the interactive shell.

## Session control

### `exit`

Exit the shell. `quit` is an alias. Ctrl-D also exits; Ctrl-C cancels the
current input line.
