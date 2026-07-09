# Discovery and filesystem boundaries

## Steam installation discovery

Vapor canonicalizes `current_exe()`, requires the executable to be laid out as
`<app-root>/bin/vapor[.exe]`, and then validates exactly
`<app-root>/Vapor.toml`. That manifest must declare `[root]`.

```text
<app-root>/bin/vapor[.exe]
```

The app root may contain app-owned resources including:

```text
<app-root>/
├── Vapor.toml
├── bin/vapor[.exe]
├── rustup/
├── cargo-home/
├── lib/
├── rustup-home/
├── tools/git/
├── tools/steamcmd/
└── installed-content/
```

Only the `[root]` manifest and running executable are required for installation
discovery. The installed app is not a Cargo workspace.

## Source root selection

Starting Vapor discovers the installed app first. The app can then start with an
active source or in a closed, app-only shell state. Source selection is explicit:

1. `VAPOR_WORKSPACE`, when set;
2. the path stored in `<app-root>/.vapor/state/source-workspace` by `open`;
3. no source, leaving the shell closed until `open SOURCE` succeeds.

`open PATH` and `sources add PATH` canonicalize the selected directory
independently. Vapor walks its ancestors, chooses the highest `Vapor.toml`, and
accepts `[root]` or `[workspace]`.

- `[root]` is the Vapor application source/depot root. It may contain direct
  Vapor workspace submodules such as Vapor-Shell.
- `[workspace]` is a normal Vapor/Cargo source workspace rooted in the same
  directory as its `Cargo.toml`.

Starting inside a nested game, engine, mod, pack, or the Vapor-Shell checkout
and then opening that path still selects the highest containing source root. A
standalone `[project]` or content manifest is rejected as a source root.

Invoking the app-owned `vapor` command from any terminal directory is valid as
long as the executable itself is still `<app-root>/bin/vapor`: the shell starts
closed, reports the app root, and waits for `open NAME`, `open PATH`, or
`sources add PATH`. Vapor does not fall back to a home directory or treat the
Steam installation as source.

## Steam and desktop launches

Starting the interactive shell without attached standard-input and
standard-output terminals causes a guarded, one-time relaunch in a supported
terminal emulator. On Linux the shell tries `x-terminal-emulator`, Konsole,
GNOME Terminal, and XTerm in that order. The launcher process exits immediately;
the REPL runs in the terminal child.

Run `open /path/to/source` or `sources add /path/to/source` from the installed
shell to register external source roots under the app root. A later Steam launch
can reopen the last active external source. If the saved path is absent or
invalid, Vapor reports that problem and continues in the closed app-only shell.

Ad-hoc one-shot commands are disabled. Direct facades are limited to bootstrap
and automation-safe commands such as `open`, `close`, `sources`, `setup`,
`metadata`, app inspection, and `script run`. They do not trigger terminal
relaunch.

## Disjoint-root invariant

The source root may not be equal to, inside, or contain the installed app root.
Rejecting overlap prevents accidental authoring inside the Steam installation and
prevents installation machinery from becoming source-visible through `cd`.

The Steam installation/app root is the installed realization produced from an
application source root such as Vapor-Root. The external Vapor-Root checkout is
the authoritative source. They can declare related identities, but they must be
separate filesystem roots.

## Navigation confinement

Every user path is resolved relative to the internal source cursor, then
canonicalized. Containment is checked after canonicalization, so `..` traversal
and symlinks cannot escape the source root. `up` fails at the root instead of
silently moving above it.

Installation paths are exposed by reporting commands. They are not navigation
targets and do not mutate the current source directory.
