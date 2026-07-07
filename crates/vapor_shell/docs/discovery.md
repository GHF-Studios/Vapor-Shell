# Discovery and filesystem boundaries

## Steam installation discovery

Vapor canonicalizes `current_exe()` and walks every ancestor directory. The
highest `Vapor.toml` becomes the installation manifest and must declare
`[root]`. The executable must be laid out as:

```text
<app-root>/bin/vapor[.exe]
```

The app root may contain replaceable resources including:

```text
<app-root>/
├── Vapor.toml
├── bin/vapor[.exe]
├── packages/toolchain/
├── cargo-home/
├── lib/
├── rustup-home/
├── tools/git/
├── tools/steamcmd/
└── installed-content/
```

Only the `[root]` manifest and running executable are required for installation
discovery. The installed app is not a Cargo workspace.

## Source root discovery

Vapor chooses the source location in this order:

1. `VAPOR_WORKSPACE`, when set;
2. the path stored in `<app-root>/state/source-workspace` by
   `workspace remember`;
3. the process invocation directory.

The selected directory is canonicalized independently. Vapor walks its
ancestors, chooses the highest `Vapor.toml`, and accepts `[root]` or
`[workspace]`.

- `[root]` is the Vapor application/depot source root. It may contain direct
  Vapor workspace submodules such as Vapor-Shell.
- `[workspace]` is a normal Vapor/Cargo source workspace rooted in the same
  directory as its `Cargo.toml`.

Starting inside a nested game, engine, mod, pack, or the Vapor-Shell checkout
still selects the highest containing source root. A standalone `[project]` or
content manifest is rejected as a source root.

Invoking the global shell outside a Vapor source root fails with a direct
diagnostic. Vapor does not fall back to a home directory or the Steam app root.

## Steam and desktop launches

Starting the interactive shell without attached standard-input and
standard-output terminals causes a guarded, one-time relaunch in a supported
terminal emulator. On Linux the shell tries `x-terminal-emulator`, Konsole,
GNOME Terminal, and XTerm in that order. The launcher process exits immediately;
the REPL runs in the terminal child.

Run `workspace remember` once from an installed shell already opened in the
desired source root. A later Steam launch can then resolve that external source
before the REPL starts. If the saved path is absent or invalid, discovery fails
visibly in the newly opened terminal.

Ad-hoc one-shot commands are disabled. `vapor script run NAME` remains available
as the script facade and does not trigger terminal relaunch.

## Disjoint-root invariant

The source root may not be equal to, inside, or contain the installed app root.
Rejecting overlap prevents accidental authoring in replaceable Steam state and
prevents installation machinery from becoming source-visible through `cd`.

The Steam app is a replaceable installed realization of Vapor-Root. The external
Vapor-Root checkout is the authoritative source. They can declare related
identities, but they must be separate filesystem roots.

## Navigation confinement

Every user path is resolved relative to the internal source cursor, then
canonicalized. Containment is checked after canonicalization, so `..` traversal
and symlinks cannot escape the source root. `up` fails at the root instead of
silently moving above it.

Installation paths are exposed by reporting commands. They are not navigation
targets and do not mutate the current source directory.
