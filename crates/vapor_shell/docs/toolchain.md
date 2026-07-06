# App-local toolchain

`vapor toolchain finalize` is the only command that accepts or changes the
persisted VAPOR_HOME location. The lock lives inside the app at
`state/vapor-home.toml`; moving the app therefore preserves the old absolute
fixpoint for comparison with the executable-derived current root. Launching the
Shell, SDK GUI, Launcher GUI, or any game never updates it implicitly.

`vapor toolchain install` is the only command that installs development tools,
and it refuses to run until the current VAPOR_HOME is finalized.
It installs all three governed groups inside the Steam application root:

- Rustup plus Cargo, Rustc, Rustfmt, Clippy, and Rustdoc;
- a portable Git distribution;
- SteamCMD.

Normal commands never call it implicitly. If `vapor test` or `vapor steam
publish` is premature, the command stops, names missing executables, and points
to the specific status, finalize, or install decision required.

## Vendored package layout

Steam delivers immutable install inputs beneath:

```text
$VAPOR_HOME/packages/toolchain/
├── rustup/bin/rustup
├── rustup-home/toolchains/<toolchain>-<host>/bin/
├── cargo-home/
├── git/bin/git
└── steamcmd/steamcmd
```

The explicit install command copies them to active app-local paths:

```text
$VAPOR_HOME/rustup/
$VAPOR_HOME/rustup-home/
$VAPOR_HOME/cargo-home/
$VAPOR_HOME/tools/git/
$VAPOR_HOME/tools/steamcmd/
```

No system package manager, system Git, system Rustup, or PATH SteamCMD is used.
`--repair` reapplies package files but does not delete additional mutable files,
so SteamCMD authentication state is preserved. Steam verification repairs the
immutable package inputs; Vapor owns activation from those packages.

The first bootstrap build must populate all package directories before it is
placed in Steam. Subsequent depots carry `packages/toolchain` rather than one
developer machine's activated tool state or credentials.

Every command still requires an external source workspace. VAPOR_HOME is the
replaceable Steam application root; the source workspace is a separate repository
root such as Vapor-Root containing project repositories or Git submodules.
