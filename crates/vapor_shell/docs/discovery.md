# Discovery and filesystem boundaries

## Steam installation discovery

Vapor canonicalizes `current_exe()`, requires the executable to be laid out as
`<app-root>/bin/vapor[.exe]` for bootstrap compatibility or
`<app-root>/bin/<target>/vapor[.exe]` for release-mode launch wrappers, and
then validates exactly `<app-root>/App.vapor.toml`. That manifest must declare
`[root]`.

```text
<app-root>/bin/vapor[.exe]
<app-root>/bin/x86_64-unknown-linux-gnu/vapor
<app-root>/bin/x86_64-pc-windows-gnullvm/vapor.exe
```

The app root may contain app-owned resources including:

```text
<app-root>/
├── App.vapor.toml
├── bin/
│   ├── vapor-launch.sh
│   ├── vapor-launch.cmd
│   ├── vapor[.exe]                         bootstrap compatibility
│   ├── x86_64-unknown-linux-gnu/vapor
│   └── x86_64-pc-windows-gnullvm/vapor.exe
├── rustup/
├── cargo-home/
├── lib/
├── rustup-home/
├── tools/steamcmd/
├── content/
└── output/
```

Only `App.vapor.toml` and the running executable are required for installation
discovery. The installed app is not a Cargo workspace.

## Source root selection

Starting Vapor discovers the installed app first. The app can then start with an
active source or in a closed, app-only shell state. Source selection is explicit:

1. `VAPOR_WORKSPACE`, when set;
2. the path stored in `<app-root>/.vapor/state/source-workspace` by
   `source open`;
3. no source, leaving the shell closed until `source open SOURCE` succeeds.

`source open PATH` and `source add PATH` canonicalize the selected directory
independently. Vapor walks its ancestors, chooses the highest source marker, and
accepts `[root]` or `[workspace]`. Vapor-Root uses
`App-Source.vapor.toml`; ordinary workspaces use `Workspace.vapor.toml`.

- `[root]` is the Vapor application source/depot root. It may contain direct
  Vapor workspace submodules such as Vapor-Shell.
- `[workspace]` is a normal Vapor/Cargo source workspace rooted in the same
  directory as its `Cargo.toml`.

Starting inside a nested game, engine, mod, pack, or the Vapor-Shell checkout
and then opening that path still selects the highest containing source root. A
standalone content manifest is rejected as a source root.

Invoking the app-owned `vapor` command from any terminal directory is valid as
long as the executable itself is still under the app root's `bin/` layout. The
shell starts from `VAPOR_WORKSPACE`, the remembered source, or a closed app-only
state. Vapor does not infer source context from arbitrary host-shell cwd for
source-bound commands; use `source open PATH` or a Vapor script when source
context matters.

## Steam and desktop launches

Starting the interactive shell without attached standard-input and
standard-output terminals causes a guarded, one-time relaunch in a supported
terminal emulator. On Linux this path is currently Konsole-only. The
Steam-started parent waits for the terminal process; the REPL runs in the
terminal child.

The product launch command `launch loo-cast` uses the same no-terminal relaunch
path so the current terminal-based runtime handoff is visible from Steam's Play
button.

Run `source open /path/to/source` or `source add /path/to/source` from the
installed shell to register external source roots under the app root. A later
Steam launch can reopen the last active external source. If the saved path is
absent or invalid, Vapor reports that problem and continues in the closed
app-only shell.

Most host-level direct facades do not trigger terminal relaunch. They are
limited to bootstrap, source selection, app inspection, metadata, explicit
launch, content, root, and `script run` entrypoints. Source-bound workflows such
as `validate` and `build` run inside the interactive shell or a Vapor script.

## Disjoint-root invariant

The source root may not be equal to, inside, or contain the installed app root.
Rejecting overlap prevents accidental authoring inside the Steam installation and
prevents installation machinery from becoming visible to source-backed
workflows.

The Steam installation/app root is the installed realization produced from an
application source root such as Vapor-Root. The external Vapor-Root checkout is
the authoritative source. They can declare related identities, but they must be
separate filesystem roots.

## Source boundary confinement

Source-backed workflows start from the active source root selected through
`source open`. Paths accepted by those workflows must resolve inside that source
root after canonicalization, so `..` traversal and symlinks cannot escape into
the installed app root or unrelated checkouts.

Installation paths are exposed by reporting commands. They are not source
targets and do not mutate the active source selection.
