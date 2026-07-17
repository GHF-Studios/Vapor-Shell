# Windows/MSVC release proof

This checklist proves the Windows side of the release target matrix without
publishing Steam depot or Workshop changes. It starts from the product path:
Steam opens a visible Vapor Shell, then Vapor prepares app-local tools.

The target is:

```text
x86_64-pc-windows-msvc
```

## Steam launch

The Windows Steam launch option should target the wrapper shipped in the depot:

```text
executable: .vapor\launch\windows\vapor.cmd
arguments:  shell
```

If Steam refuses to execute `.cmd` directly, use:

```text
executable: cmd.exe
arguments:  /c ".vapor\launch\windows\vapor.cmd" shell
```

Clicking that launch option should open a persistent `cmd` window running Vapor
Shell. The user should not need to install Git before clicking Play.

## First-run setup

Run this from the visible Vapor Shell:

```text
setup self status
setup self install
setup self status
```

Expected setup behavior:

- Rustup is downloaded and run with `RUSTUP_HOME` and `CARGO_HOME` inside the
  Steam app root.
- Git is downloaded as the portable MinGit zip and extracted under
  `tools\git`.
- SteamCMD is downloaded as the Windows zip and extracted under
  `tools\steamcmd`.

No downloaded setup component should run a system installer, require global
Git, write to a global Git location, or mutate machine-wide PATH state.

The MSVC compiler/linker prerequisite is expected to come from Steam's
configured Visual Studio 2022 Build Tools redistributable with Desktop
development with C++. Vapor does not install that redistributable itself; the
Steam app install/update flow owns it.

## Source handoff

The source import/template command is still a product gap. Until that exists,
the source root must be present by one of these explicit handoff methods:

- a prepared source checkout or archive;
- a future Vapor workspace/template import command;
- a manual clone using app-local Git after `setup self install`.

If a manual clone is needed for this proof, use the app-local Git binary:

```cmd
set "APP_ROOT=C:\Program Files (x86)\Steam\steamapps\common\Loo Cast"
"%APP_ROOT%\tools\git\cmd\git.exe" clone --recurse-submodules REPO_URL "%USERPROFILE%\Documents\Loo Cast Repos\Vapor-Root"
```

Do not ask the Windows machine to install system Git.

## Windows build

Set paths for the local machine:

```cmd
set "APP_ROOT=C:\Program Files (x86)\Steam\steamapps\common\Loo Cast"
set "REPOS=%USERPROFILE%\Documents\Loo Cast Repos"
set "VAPOR=%APP_ROOT%\bin\x86_64-pc-windows-msvc\vapor.exe"
```

Build and promote the Windows Vapor Shell app binary:

```cmd
"%VAPOR%" source open "%REPOS%\Vapor-Root"
"%VAPOR%" root build --target x86_64-pc-windows-msvc
```

Build the first-party Loo-Cast content runtime outputs:

```cmd
"%VAPOR%" source open "%REPOS%\Loo-Cast"
"%VAPOR%" content build --target x86_64-pc-windows-msvc
```

Build the example runtime outputs:

```cmd
"%VAPOR%" source open "%REPOS%\Vapor-Root\Vapor-Examples"
"%VAPOR%" content build --target x86_64-pc-windows-msvc
```

This proof expects Steam to have installed the Visual Studio 2022 Build Tools
redistributable before the Windows build commands run. Git, Rustup state, Cargo
state, and SteamCMD are app-local Vapor setup; the Microsoft compiler/linker
toolchain is a Steam-managed redistributable prerequisite.

## Windows artifact checks

Confirm these files exist:

```text
%APP_ROOT%\bin\x86_64-pc-windows-msvc\vapor.exe
%APP_ROOT%\output\dev\loo-cast\x86_64-pc-windows-msvc\debug\spacetime-engine.exe
%APP_ROOT%\output\dev\loo-cast\x86_64-pc-windows-msvc\debug\loo_cast_game.dll
%APP_ROOT%\output\dev\vapor-examples\x86_64-pc-windows-msvc\debug\terminal-engine.exe
%APP_ROOT%\output\dev\vapor-examples\x86_64-pc-windows-msvc\debug\hello_world_on_steroids_game.dll
```

The exact `.dll` names come from Cargo crate names. If a content artifact adds
or removes declared `libraries` later, update this checklist with the new
declared runtime outputs from that artifact's `Vapor.toml`.

## Handoff back to Linux

Copy these Windows app-root paths back to the Linux publishing app root,
preserving the same relative paths:

```text
bin\x86_64-pc-windows-msvc\
output\dev\loo-cast\x86_64-pc-windows-msvc\debug\
output\dev\vapor-examples\x86_64-pc-windows-msvc\debug\
```

The root app package only needs `bin\x86_64-pc-windows-msvc\vapor.exe`.
Content dry-runs and Workshop package dry-runs need the `output\dev\...`
directories because `content package` stages runtime outputs from app-local
Cargo output.

## Linux staging proof

After copying the Windows artifacts into the Linux Steam app root, prove the
release matrix without rebuilding Windows on Linux:

```text
source open /home/leslieghf/Documents/GitHub/Loo Cast Repos/Vapor-Root
root package
root publish --skip-build --dry-run
```

Expected app staging shape:

```text
output/root/content/bin/x86_64-unknown-linux-gnu/vapor
output/root/content/bin/x86_64-pc-windows-msvc/vapor.exe
output/root/content/.vapor/launch/linux/vapor.sh
output/root/content/.vapor/launch/windows/vapor.cmd
```

Then prove the Loo-Cast Workshop package preview:

```text
source open /home/leslieghf/Documents/GitHub/Loo Cast Repos/Loo-Cast
content publish ghf-studios/loo-cast/spacetime-engine ghf-studios/loo-cast/loo-cast-game ghf-studios/loo-cast/loo-cast-packagepack --dry-run
```

No command in this checklist performs a real Steam upload. Real publication
still requires manually typed `--account ACCOUNT --yes` after reviewing the
staged payloads and provider scripts.
