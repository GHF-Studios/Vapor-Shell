# Steam development workflow

## Linux installation and PATH

Steam does not provide Linux/SteamOS install-script execution. Vapor therefore
does not attach location mutation to any Steam launch option. The Shell, SDK,
Launcher, and future game launch entries only launch their configured programs.

After installation or movement, run Vapor from the Steam app directory, review
`setup self status`, and explicitly choose `setup self install` or
`setup self repair`. No executable is copied into a user-data directory, and the
source checkout must stay outside the Steam app directory.

The bootstrap sequence is:

1. build only the initial `vapor` shell binary with the host environment;
2. copy the minimal shell bootstrap into Steam's app directory:

   ```text
   crates/vapor_shell/scripts/bootstrap-local-app-deploy.sh \
     --binary /path/to/built/vapor \
     --target "$HOME/.local/share/Steam/steamapps/common/Loo Cast" \
     --yes
   ```

   This writes only `Vapor.toml` and `bin/vapor`.
3. run `/path/to/app/bin/vapor source open /path/to/Vapor-Root` to register
   and open the external application source without moving that source into the
   app dir;
4. run `setup self status`;
5. run `setup self install`;
6. open a new terminal so PATH changes are visible;
7. run `vapor`; it should discover the app from its own executable and reopen
   the last active source;
8. run `validate` using app-local Rust/Cargo;
9. run `root build`, `root package`, and `root publish --dry-run`;
10. upload the rebuilt app with `root publish --account NAME --yes` from the
    interactive shell.

From step 5 onward, Cargo, Git, SteamCMD, and build outputs come from the Steam
application. `setup self install` is the explicit bootstrap operation that installs
active tools into the app root. Final depot staging uses the separate
`packages/setup` payload with credential/cache exclusions. Publishing never
installs missing tools; it reports the failed precondition and leaves that
decision to the operator.

The bootstrap script is intentionally not a full depot installer. It does not
copy source repos, Cargo workspaces, staged package trees, or generated outputs.
Its only job is to place the first runnable shell inside the Steam app root so
that every serious operation happens through the installed `bin/vapor`.

For a later local self-deploy loop, after `root package` exists and is trusted,
use a separate package/depot deployment path rather than this shell-bootstrap
script.

## Authentication

Vapor does not expose a standalone SteamCMD login workflow. Real publication is
the authentication boundary: `root publish --account NAME --yes` starts the
installation-owned SteamCMD with inherited stdin/stdout, lets Steam own any
password or mobile-authenticator prompts, and returns to the REPL after SteamCMD
exits. Vapor never accepts a password argument and never copies SteamCMD's
`config/` into staging.

Steam authentication is session-scoped by policy. Commands that publish for real
must be typed manually in the interactive shell; scripts may dry-run but may not
authenticate, perform real uploads, create Workshop items, or delete Workshop
items.

## Preview and publish

Use `root publish --dry-run` first. It validates, builds, promotes app binaries,
builds docs, stages and smoke-tests the complete app, and writes an app-build
VDF with `Preview = 1` and `SetLive = vapor-dev`; it performs no upload and
does not require active SteamCMD.

A real upload requires both a non-default branch and explicit confirmation:

```text
root publish --account NAME --branch vapor-dev --yes
```

SteamCMD runs in the foreground so progress, prompts, exit status, and failure
remain attached to the operation. `output/root/steam-build` is not cleared by
staging; it contains SteamPipe manifests and chunk cache that improve subsequent
uploads.

The VDF maps only the already-clean staging root. Inclusion and credential
exclusion are therefore decided before SteamPipe sees any files.

Workshop content publication is separate from app/depot publication. Use
`content publish ARTIFACT --dry-run` from a content workspace for package and
Workshop VDF previews, then perform any real content upload manually with
`content publish ARTIFACT --account ACCOUNT --yes`.
