# Discovery and filesystem boundaries

## Steam installation discovery

Vapor canonicalizes `current_exe()` and walks every ancestor directory. The
highest `Vapor.toml` becomes the installation Vapor manifest. This remains
stable whether the shell is invoked through an absolute path, a launcher, or
`PATH`.

VAPOR_HOME is replaceable installed state, not an authored source workspace.
The current implementation requires `[workspace]` in this manifest, but that
identity conflicts with the new rule that every workspace is authored Rust
source with a companion Cargo manifest. The replacement installation identity
is intentionally left for the next documentation iteration rather than silently
cementing the wrong section name.

The installation may contain replaceable resources including:

```text
<installation>/
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

Only the installation Vapor manifest and running executable are needed for
installation discovery. Optional directories are reported when present; the
installation is not required to contain a source Cargo manifest.

## Source workspace discovery

Vapor chooses the source location in this order:

1. `VAPOR_WORKSPACE`, when set;
2. the path stored in `<installation>/state/source-workspace` by
   `workspace remember`;
3. the process invocation directory.

The selected directory is canonicalized independently. Vapor walks its
ancestors, chooses the highest `Vapor.toml`, and requires `[workspace]`. This root
owns critical source. Starting inside a nested game, engine, mod, or pack still
selects the containing source workspace.

The selected root must also contain `Cargo.toml` declaring a Cargo workspace.
The Vapor and Cargo manifests must resolve to the same canonical root. Missing
Cargo structure invalidates the source workspace; missing bundled Cargo remains
a separately repairable toolchain prerequisite.

The same rule applies to the shell repository itself. In this repository layout,
invocation below `Vapor-Root/Vapor-Shell` sees both the component marker and the
higher `Vapor-Root/Vapor.toml`; the umbrella workspace wins. A standalone
`[project] kind = "shell"` marker is not a workspace and is rejected.

Invoking the global shell outside a Vapor source workspace fails with a direct
error. It does not fall back to a home directory or the Steam app root.

## Steam and desktop launches

Starting the interactive shell without attached standard-input and
standard-output terminals causes a guarded, one-time relaunch in a supported
terminal emulator. On Linux the shell tries `x-terminal-emulator`, Konsole,
GNOME Terminal, and XTerm in that order. The launcher process exits immediately;
the REPL runs in the terminal child. If startup fails, the Linux terminal waits
for Enter so the diagnostic and its `help:` suggestions remain visible.

Run `workspace remember` once from an installed shell already opened in
Vapor-Root. A later Steam launch can then resolve the external source before the
REPL starts. If the saved path is absent or invalid, discovery fails visibly in
the newly opened terminal. One-shot commands never open a terminal, which keeps
the CLI facade usable by scripts and agents.

## Disjoint-root invariant

The source root may not be equal to, inside, or contain the installation root.
Rejecting overlap prevents accidental authoring in replaceable Steam state and
prevents installation machinery from becoming source-visible through `cd`.
The Steam app is a replaceable installed realization of Vapor-Root, while
external Vapor-Root is the authoritative source checkout. Their identities are
related but not the same kind: VAPOR_HOME must not masquerade as an authored
`[workspace]` merely to make discovery convenient.

## Navigation confinement

Every user path is resolved relative to the internal source cursor, then
canonicalized. Containment is checked after canonicalization, so `..` traversal
and symlinks cannot escape the source workspace. `up` fails at the root instead
of silently moving above it.

Installation paths are exposed by reporting commands. They are not navigation
targets and do not mutate the current source directory.
