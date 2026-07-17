# Windows/MSVC release proof

This checklist proves the Windows side of the release target matrix without
publishing Steam depot or Workshop changes.

The target is:

```text
x86_64-pc-windows-msvc
```

Use a normal Windows Steam install of Loo-Cast, a Git checkout of the pushed
source repositories, and the installed Vapor Shell. Run commands from a visible
terminal, not a silent Steam process.

## Windows build

Set paths for the local machine:

```cmd
set "APP_ROOT=C:\Program Files (x86)\Steam\steamapps\common\Loo Cast"
set "REPOS=C:\Users\YOU\Documents\GitHub\Loo Cast Repos"
set "VAPOR=%APP_ROOT%\bin\x86_64-pc-windows-msvc\vapor.exe"
```

Refresh source checkouts:

```cmd
cd /d "%REPOS%\Vapor-Root"
git pull --recurse-submodules
git submodule update --init --recursive

cd /d "%REPOS%\Loo-Cast"
git pull
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
root package --release-targets
root publish --release-targets --skip-build --dry-run
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
content publish ghf-studios/loo-cast/spacetime-engine ghf-studios/loo-cast/loo-cast-game ghf-studios/loo-cast/loo-cast-packagepack --release-targets --dry-run
```

No command in this checklist performs a real Steam upload. Real publication
still requires manually typed `--account ACCOUNT --yes` after reviewing the
staged payloads and provider scripts.
