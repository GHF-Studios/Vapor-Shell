# Windows GNU/LLVM release proof

This checklist proves the Windows side of the release target matrix without
publishing Steam depot or Workshop changes. It starts from the product path:
Steam opens a visible Vapor Shell through the native entrypoint, and the launch
script runs the installer-owned player-mode install before Shell starts.

The target is:

```text
x86_64-pc-windows-gnullvm
```

## Steam launch

The Windows Steam launch option should target the native entrypoint shipped in
the target-specific depot:

```text
executable: bin\x86_64-pc-windows-gnullvm\vapor-entrypoint.exe
arguments:  shell
```

Clicking that launch option should open a persistent `cmd` window running Vapor
Shell. The user should not need to install Git before clicking Play.

The installer should also be exposed as its own launch option:

```text
executable: bin\x86_64-pc-windows-gnullvm\vapor-entrypoint.exe
arguments:  installer
```

Clicking that launch option should open a persistent `cmd` window running
Vapor Installer directly. It should not run the quiet player-mode install
before showing the installer surface.

## First-run installer

Player-mode install is automatic through the launch path. For development
build/proof work, run the installer visual surface or this explicit headless
command:

```text
vapor-installer dev-env install --app-root "C:\Program Files (x86)\Steam\steamapps\common\Loo Cast"
```

Expected installer behavior:

- Rustup is downloaded and run with `RUSTUP_HOME` and `CARGO_HOME` inside the
  Steam app root.
- SteamCMD is downloaded as the Windows zip and extracted under
  `tools\steamcmd`.
- Zig is downloaded as the portable Windows zip and extracted under
  `tools\zig`; Vapor writes linker wrappers under `tools\cross`.
- llvm-mingw is downloaded as a portable archive and extracted under
  `tools\llvm-mingw`.

No downloaded setup component should run a system installer, write to a global
Git/tool location, or mutate machine-wide PATH state.

The Windows GNU/LLVM and Linux GNU cross-linker path is app-local and portable.
Vapor uses llvm-mingw from `tools\llvm-mingw` for Windows GNU/LLVM and Zig
from `tools\zig` for Linux GNU cross-links. This proof must not require Visual
Studio, MSVC, system MinGW, or a machine-wide compiler install.

## Source handoff

The source import/template command is still a product gap. Until that exists,
the source root must be present by one of these explicit handoff methods:

- a prepared source checkout or archive;
- a future Vapor workspace/template import command;
- a manual clone using a developer-installed Git provider.

If Git-backed source work is needed for this proof, install Git as a developer
tool on the host and link it inside Vapor Shell:

```cmd
provider git link "C:\Program Files\Git\cmd\git.exe"
```

Normal Steam play does not require Git.

## Windows build

Set paths for the local machine:

```cmd
set "APP_ROOT=C:\Program Files (x86)\Steam\steamapps\common\Loo Cast"
set "REPOS=%USERPROFILE%\Documents\Loo Cast Repos"
set "VAPOR=%APP_ROOT%\bin\x86_64-pc-windows-gnullvm\vapor.exe"
```

Build and promote the Windows Vapor Shell app binary:

```cmd
"%VAPOR%" source open "%REPOS%\Vapor-Root"
"%VAPOR%" root build --target x86_64-pc-windows-gnullvm
```

Build the first-party Loo-Cast content runtime outputs:

```cmd
"%VAPOR%" source open "%REPOS%\Loo-Cast"
"%VAPOR%" content build --target x86_64-pc-windows-gnullvm
```

Build the example runtime outputs:

```cmd
"%VAPOR%" source open "%REPOS%\Vapor-Root\Vapor-Examples"
"%VAPOR%" content build --target x86_64-pc-windows-gnullvm
```

This proof expects `vapor-installer dev-env install` to have prepared app-local
Git, Rustup state, Cargo state, SteamCMD, Zig, and linker wrappers. No Microsoft
compiler/linker toolchain is part of this proof path.

## Windows artifact checks

Confirm these files exist:

```text
%APP_ROOT%\bin\x86_64-pc-windows-gnullvm\vapor.exe
%APP_ROOT%\output\dev\loo-cast\x86_64-pc-windows-gnullvm\debug\spacetime-engine.exe
%APP_ROOT%\output\dev\loo-cast\x86_64-pc-windows-gnullvm\debug\loo_cast_game.dll
%APP_ROOT%\output\dev\vapor-examples\x86_64-pc-windows-gnullvm\debug\terminal-engine.exe
%APP_ROOT%\output\dev\vapor-examples\x86_64-pc-windows-gnullvm\debug\hello_world_on_steroids_game.dll
```

The exact `.dll` names come from Cargo crate names. If a content artifact adds
or removes declared `libraries` later, update this checklist with the new
declared runtime outputs from that artifact's role manifest.

## Handoff back to Linux

Copy these Windows app-root paths back to the Linux publishing app root,
preserving the same relative paths:

```text
bin\x86_64-pc-windows-gnullvm\
output\dev\loo-cast\x86_64-pc-windows-gnullvm\debug\
output\dev\vapor-examples\x86_64-pc-windows-gnullvm\debug\
```

The root app package only needs `bin\x86_64-pc-windows-gnullvm\vapor.exe`.
Content dry-runs and Workshop package dry-runs need the `output\dev\...`
directories because `content package` stages runtime outputs from app-local
Cargo output.

## Linux staging proof

After copying the Windows artifacts into the Linux Steam app root, prove the
release matrix without rebuilding Windows on Linux:

```text
source open /home/leslieghf/Documents/GitHub/Loo Cast Repos/Vapor-Root
root publish --skip-build --dry-run
```

Expected app staging shape:

```text
output/root/content/linux/bin/vapor-launch.sh
output/root/content/linux/bin/x86_64-unknown-linux-gnu/vapor
output/root/content/linux/bin/x86_64-unknown-linux-gnu/vapor-entrypoint
output/root/content/windows/bin/vapor-launch.cmd
output/root/content/windows/bin/x86_64-pc-windows-gnullvm/vapor.exe
output/root/content/windows/bin/x86_64-pc-windows-gnullvm/vapor-entrypoint.exe
```

Then prove the Loo-Cast Workshop package preview:

```text
source open /home/leslieghf/Documents/GitHub/Loo Cast Repos/Loo-Cast
content publish ghf-studios/loo-cast/spacetime-engine ghf-studios/loo-cast/loo-cast-game ghf-studios/loo-cast/loo-cast-packagepack --dry-run
```

No command in this checklist performs a real Steam upload. Real publication
still requires manually typed `--account ACCOUNT --yes` after reviewing the
staged payloads and provider scripts.
