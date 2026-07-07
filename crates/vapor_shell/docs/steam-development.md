# Steam development workflow

## Linux installation and PATH

Steam does not provide Linux/SteamOS install-script execution. Vapor therefore
does not attach location mutation to any Steam launch option. The Shell, SDK,
Launcher, and future game launch entries only launch their configured programs.

After installation or movement, open Vapor from an external source root, review
`toolchain status`, and explicitly choose `toolchain install` or
`toolchain repair`. No executable is copied into a user-data directory.

The bootstrap sequence is:

1. build the initial app and complete `packages/toolchain` payload with the host
   environment;
2. place that build in Steam's app directory;
3. from external Vapor-Root, run `/path/to/app/bin/vapor` to enter the shell;
4. run `toolchain status`;
5. run `toolchain install`;
6. open a new terminal so PATH changes are visible;
7. run `vapor`, then `workspace remember` from Vapor-Root if needed;
8. run `validate` using the installed toolchain;
9. run `self rebuild`, `self stage`, `self smoke`, and a dry-run publish;
10. upload the rebuilt app with `steam publish ... --yes` from the interactive
    shell.

From step 8 onward, Cargo, Git, SteamCMD, and build outputs come from the Steam
application. Publishing never installs missing tools; it reports the failed
precondition and leaves that decision to the operator.

## Authentication

`steam login --account NAME` starts the installation-owned SteamCMD with
inherited stdin/stdout. It waits while SteamCMD owns authentication prompts,
then returns to the REPL. Vapor never accepts a password argument and never
copies SteamCMD's `config/` into staging.

Steam authentication is session-scoped by policy. Commands that publish must be
typed manually in the interactive shell; scripts may stage and dry-run but may
not authenticate or perform real uploads.

## Preview and publish

Use `steam publish --account NAME --dry-run` first. It builds docs, stages and
smoke-tests the complete app, and writes an app-build VDF with `Preview = 1` and
`SetLive = vapor-dev`; it performs no upload.

A real upload requires both a non-default branch and explicit confirmation:

```text
steam publish --account NAME --branch vapor-dev --yes
```

SteamCMD runs in the foreground so progress, prompts, exit status, and failure
remain attached to the operation. `output/root/steam-build` is not cleared by
staging; it contains SteamPipe manifests and chunk cache that improve subsequent
uploads.

The VDF maps only the already-clean staging root. Inclusion and credential
exclusion are therefore decided before SteamPipe sees any files.
