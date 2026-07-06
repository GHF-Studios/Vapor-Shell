# Interactive commands

Run `help` for the command list or `help <COMMAND>` for argument details. Clap
also supplies contextual completion candidates and validation errors.

## Source navigation

### `cd [WORKSPACE_PATH]`

Change the internal source directory. Relative paths start at the current
internal directory; absolute paths are accepted only inside the source root.
Omitting the argument returns to the source root.

### `up [LEVELS]`

Move toward the source root. `LEVELS` defaults to `1` and must be a positive
integer. Reaching above the source root is an error.

### `pwd`

Print the internal source directory. This does not depend on the process working
directory after startup.

### `ls [WORKSPACE_PATH]`

List a source directory after the same canonical containment checks used by
`cd`. The default is the internal current directory.

### `root`

Return to the external source workspace root.

## Installation resources

These commands report replaceable Steam paths and never change the source
cursor.

### `installation`

Print the Steam application root discovered from the shell executable.

### `binaries`

Print the directory containing the running shell executable.

### `libraries`

Print the installation `lib` directory. Absence is reported as an error because
there is no path to display.

## Derived context

### `metadata [--format human|json]`

Resolve the active source workspace, nearest content, VAPOR_HOME, app-local
tools, root workspace policy, optional Steam distribution policy, and Cargo
index into one report. Human-readable output is the default. JSON is the stable
machine interface for scripts and agents.

Metadata reporting is best-effort: missing tools, failed Cargo projection, an
unfinalized VAPOR_HOME, or absent optional distribution policy are reported as
diagnostics instead of hiding the rest of the environment. This does not make a
Cargo manifest optional. Commands use the same resolved model and reject
missing required source structure or unmet command prerequisites before acting.

### `toolchain status|finalize|install [--repair]|unlock`

`status` reports both the executable-derived VAPOR_HOME and its persisted
fixpoint, followed by Rust/Cargo, Git, SteamCMD, and vendored-package health.

`finalize` explicitly accepts the current app location. It writes
`$VAPOR_HOME/state/vapor-home.toml` and updates the marked shell-profile PATH
entry to the app's own `bin` directory. No launch option performs this action.
After a Steam move, the lock moves with the app but retains the previous absolute
path, so status reports both locations until the user finalizes or moves it back.

`install` is allowed only after finalization. It installs app-local Rust/Cargo,
Git, and SteamCMD from `$VAPOR_HOME/packages/toolchain`; `--repair` reapplies all
package files. `unlock` removes the fixpoint and marked PATH entry. Open a new
terminal after finalize or unlock because a child cannot alter its parent shell.

### `fmt|check|test|build [--project PROJECT]`

Run the selected operation through Cargo inside the Steam installation. The
default is every project declared by `[workspace].members`; `--project` accepts
the exhaustive set `all`, `core`, `sdk`, `launcher`, `shell`, and `examples`.
Artifacts go to `$VAPOR_HOME/output/dev/<project>` instead of source trees.
Rust and Git must already pass `toolchain status`.

### `validate [--project PROJECT]`

For each selected project, run formatting verification, `cargo check`, tests,
strict Clippy, and strict Rustdoc. This is the normal local and agent validation
entrypoint.

### `workspace remember|forget`

Persist the current external source root at
`$VAPOR_HOME/state/source-workspace`, or clear it. Steam GUI launches use this
selection because they normally start from the installation rather than a source
terminal.

### `docs build|path|open [TOPIC]`

Build every project Cargo workspace declared by the root Vapor manifest into
the installed `docs/` tree, print a generated document path, or open it
asynchronously.

### `self rebuild|stage|smoke`

`rebuild` builds every root project into Steam-owned output and promotes only
the binaries explicitly selected by the final project/distribution policy into
the installation.
`stage` builds docs and reconstructs the clean depot content tree from the
distribution allowlist. `smoke` verifies its marker, binaries, docs, and pinned
toolchain.

### `script run NAME [--plan]`

Read `.vapor/scripts/NAME.vapor` and execute each non-comment line through this
same command parser. Scripts stop on error and cannot recursively invoke scripts
or terminate the host REPL.

### `steam login --account ACCOUNT`

Temporarily hand the terminal to installation-owned SteamCMD. SteamCMD exits
after authentication and the REPL resumes.

### `steam publish --account ACCOUNT [--branch BRANCH] [--plan] [--yes]`

Build docs, reconstruct and smoke-test staging, generate a SteamPipe VDF, then
publish to a non-default beta. Before staging, publishing validates every
project, rebuilds through installed Cargo, and promotes declared binaries.
`--plan` writes a SteamPipe preview build. A real upload requires `--yes`.

## One-shot facade

No arguments starts the REPL. Supplying a command executes that exact REPL
command once and exits, for example `vapor self smoke`. Humans, scripts,
and agents therefore share one parser and implementation.

## Session control

### `exit`

Exit the shell. `quit` is an alias. Ctrl-D also exits; Ctrl-C cancels the current
input line.
