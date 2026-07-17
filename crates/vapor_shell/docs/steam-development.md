# Steam development workflow

## Linux installation and PATH

Steam does not provide Linux/SteamOS install-script execution. Vapor therefore
does not attach location mutation to any Steam launch option. The Shell, SDK,
Launcher, and future game launch entries only launch their configured programs.

After a depot build that includes `.vapor/launch/`, launch the small platform
wrapper and let it hand off to the installed platform binary:

- **Linux Play Loo-Cast**: executable `.vapor/launch/linux/vapor.sh`,
  arguments `play`.
- **Linux Vapor Shell**: executable `.vapor/launch/linux/vapor.sh`,
  arguments `shell`.
- **Windows Play Loo-Cast**: executable `.vapor\launch\windows\vapor.cmd`,
  arguments `play`.
- **Windows Vapor Shell**: executable `.vapor\launch\windows\vapor.cmd`,
  arguments `shell`.

These paths are not valid for already-published depots that predate the
`.vapor/launch/` payload. For those depots, use only files that actually exist
in the installed app root. Windows launch options also require a shipped Windows
`bin\x86_64-pc-windows-msvc\vapor.exe`; a Linux
`bin/x86_64-unknown-linux-gnu/vapor` cannot satisfy a Windows Steam launch
entry.

If Steam refuses to execute a `.cmd` entry directly on Windows, use `cmd.exe`
as the executable and `/c ".vapor\launch\windows\vapor.cmd" play` or
`/c ".vapor\launch\windows\vapor.cmd" shell` as the arguments.

The Linux wrapper relies on Vapor's terminal detection to spawn Konsole when
Steam starts it without a terminal. The Windows wrapper opens a persistent
`cmd` window. Both wrappers are intentionally thin; Vapor Shell remains the
central implementation surface.

On Windows, the Shell launch option is expected to be single-click-to-shell.
First-run Vapor tool preparation happens inside that visible shell with
`setup self install`; it downloads app-local portable Git and SteamCMD payloads
instead of requiring system Git to be installed before launch. The MSVC
compiler/linker prerequisite is supplied separately by the Steam-configured
Visual Studio 2022 Build Tools redistributable with Desktop development with
C++.

The `play` wrapper mode opens the normal interactive Vapor Shell, runs the
installed `.vapor/scripts/loo-cast.vapor` script, and leaves the shell open.
That script currently calls `launch loo-cast --account ghf_vapor_build` so
SteamCMD authentication, Steam Guard prompts, Workshop download output, and
runtime handoff output stay visible in the Konsole-owned session.

`launch loo-cast` verifies the selected installed Loo-Cast Packagepack,
resolves that packagepack's Spacetime Engine dependency, and hands off to the
installed engine binary for the host runtime target. The current first-party
Loo-Cast engine is a product placeholder; the dynamic terminal game-library
proof lives in `Vapor-Examples`. If the packagepack is missing, Vapor can
download/cache/install/select the public first-party Workshop packagepack and
dependencies from the app-root `[[root.content]]` seed. It does not silently
install toolchains, mutate PATH state, or perform Steam authority-changing
publish/delete operations.

The `--account` argument is currently needed while the app and Workshop items
are not publicly accessible to anonymous SteamCMD sessions. Once the app/content
is public, the same command can be tested without `--account`.

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

   This writes only `Vapor.toml` and a bootstrap `bin/vapor`.
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

Release-mode depot builds should target every shipped app platform:

```text
root build --release-targets
root publish --release-targets --dry-run
```

The Windows/MSVC target normally needs to be built on Windows after Steam has
installed the Visual Studio 2022 Build Tools redistributable, then staged
alongside the Linux payload before publication.
For quick local Linux smoke, omit target flags and Vapor stages only the host
`bin/<target>/` directory plus the matching launch wrapper.
When Windows artifacts were imported from another machine, use
`root publish --release-targets --skip-build --dry-run` so the publishing
machine stages and smoke-checks the imported `bin/<target>/` payloads without
trying to rebuild Windows/MSVC locally.

The concrete Windows build and Linux handoff checklist is documented in
[`windows-msvc-release-proof.md`](windows-msvc-release-proof.md).

From step 5 onward, Cargo, Git, SteamCMD, and build outputs come from the Steam
application. `setup self install` is the explicit bootstrap operation that installs
active tools into the app root. Default depot staging is runtime-only. The
separate `packages/setup` payload is used only with
`--include-setup-payload`, with credential/cache exclusions. Publishing never
installs missing tools; it reports the failed precondition and leaves that
decision to the operator.

The bootstrap script is intentionally not a full depot installer. It does not
copy source repos, Cargo workspaces, staged package trees, or generated outputs.
Its only job is to place the first runnable shell inside the Steam app root so
that every serious operation happens through the installed Vapor shell. Release
launches should go through `.vapor/launch/...` wrappers and
`bin/<target>/vapor[.exe]`.

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
builds docs, stages and smoke-tests the runtime app, and writes an app-build
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

Use `--include-setup-payload` only when deliberately shipping the large
self-setup/toolchain payload in the app depot.

Workshop content publication is separate from app/depot publication. Use
`content publish ARTIFACT --dry-run` from a content workspace for package and
Workshop VDF previews, then perform any real content upload manually with
`content publish ARTIFACT --account ACCOUNT --yes`. Use `--release-targets`
when one Workshop item should receive every platform-specific runtime payload
declared in `[workspace.runtime].targets`; use repeated `--target` only for an
ad hoc subset.
