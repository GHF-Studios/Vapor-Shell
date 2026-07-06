# Steam development workflow

## Linux installation and PATH

Steam does not provide Linux/SteamOS install-script execution. Vapor therefore
does not attach location mutation to any Steam launch option. The Shell, SDK,
Launcher, and future game launch entries only launch their configured programs.
After installation or movement, invoke Vapor from an external source workspace,
review `vapor toolchain status`, and explicitly run `vapor toolchain finalize`.
No executable is copied into a user-data directory.

The one permitted bootstrap sequence is:

1. build the initial app and its complete `packages/toolchain` payload with the
   host environment;
2. place that build in Steam's app directory and, from external Vapor-Root, run
   `/path/to/app/bin/vapor toolchain status`;
3. explicitly run `/path/to/app/bin/vapor toolchain finalize`;
4. open a new terminal and run `vapor toolchain install`;
5. run `vapor workspace remember` from Vapor-Root if it is not remembered;
6. run `vapor validate` using the installed toolchain;
7. run `vapor self rebuild`, `vapor self stage`, `vapor self smoke`, and a
   preview publish;
8. upload the rebuilt app with `vapor steam publish ... --yes`.

From step 6 onward, Cargo, Git, SteamCMD, and build outputs come from the Steam
application. Publishing never installs missing tools; it reports the failed
precondition and leaves that decision to the operator.
`steam publish` repeats validation, rebuilding, binary promotion, documentation,
staging, and smoke checks before invoking SteamCMD, preventing stale installed
binaries from entering a depot.

## Authentication

`steam login --account NAME` starts the installation-owned SteamCMD with inherited
stdin/stdout. It waits while SteamCMD owns authentication prompts, then returns
to the REPL. Vapor never accepts a password argument and never copies SteamCMD's
`config/` into staging.

## Preview and publish

Use `steam publish --account NAME --plan` first. It builds docs, stages and
smoke-tests the complete app, and writes an app-build VDF with `Preview = 1` and
`SetLive = vapor-dev`; it performs no upload.

A real upload requires both a non-default branch and explicit confirmation:

```text
steam publish --account NAME --branch vapor-dev --yes
```

SteamCMD runs in the foreground so progress, prompts, exit status, and failure
remain attached to the operation. `$VAPOR_HOME/output/root/steam-build` is never
cleared by staging; it contains SteamPipe manifests and chunk cache that improve
subsequent uploads.

The VDF maps only the already-clean staging root. Inclusion and credential
exclusion are therefore decided before SteamPipe sees any files.
