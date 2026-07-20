# Setup and backend superpass

Status: **superseded checkpoint**

This checkpoint is historical. The current product direction moves
installation, uninstallation, runtime bootstrap, and development-environment
tooling into the `Vapor-Installer` project. Vapor Shell no longer exposes the
`setup` command group or retired setup staging flags.

This checkpoint records the owner decisions behind the setup/tooling rework. It
extends `product-topology.md`; it does not replace the root/source/content
model defined there.

## Recovery note

This document is the durable handoff point for the current superpass. If an
agent session fails, continue from this file before using older chat context.

Original implementation intent, now superseded:

1. keep installed-environment setup under a Shell-owned setup command group;
2. keep backend tools hidden behind Vapor source, content, root, Steam, and setup
   goals;
3. always attempt to provide player SteamCMD readiness, with Rust/Cargo
   development tooling and external Git providers handled explicitly;
4. use OS and package-manager detection to explain repair paths without silently
   performing privileged system changes;
5. keep app/depot root authority separate from operating-system administrator
   privilege.

Current implementation progress:

- installed app setup is owned by `Vapor-Installer`;
- Vapor Shell no longer exposes retired setup staging commands;
- Workshop/content commands do not prepare installer-managed tools;
- Shell preflight still uses setup status terminology internally until that
  reporting layer is renamed.

## Non-goals

- Do not expose Git, Cargo, rustup, SteamCMD, package managers, DLLs, or shared
  libraries as the primary product grammar.
- Do not make users choose formal modes such as player, developer, or publisher.
- Do not keep legacy public compatibility surfaces.
- Do not treat a script that delegates to system Git as app-owned Git.
- Do not silently edit user source, global IDE settings, Steam authentication
  state, shell profiles, or operating-system packages.
- Do not merge Workshop/content source with Vapor-Root app/depot source.

## Product shape

The retired design wanted Shell to expose setup goals directly:

```text
source
  open, index, sync, repair, and validate authored source roots

content
  inspect, validate, build, package, install, and publish Workshop content

root
  build, package, and publish the Vapor application/depot source root

steam
  inspect Steam availability and session-scoped publication state when needed
```

Current lifecycle commands live in Vapor-Installer instead:

```text
vapor-installer install
vapor-installer uninstall
vapor-installer dev-env install
vapor-installer dev-env uninstall
```

Backend tools remain implementation details unless a diagnostic needs to explain
why a capability is unavailable.

## Player-to-creator gradient

Vapor should support a gradual path from installed play to authorship without a
mode switch. The same installed command surface should stay present while deeper
operations reveal more system requirements.

Examples:

- a normal player should not need to know whether Git or SteamCMD exists;
- a user who opens content source should see source and build requirements only
  when those operations need them;
- a user who packages or publishes content should see Steam/session
  requirements at the point of publication;
- a user who opens Vapor-Root should see app/depot authority requirements only
  for root operations.

Setup levels may exist internally as completeness tiers, but they are not
public identities. Status should say what is ready and what is still needed, not
ask the user to declare a persona.

## Authority boundaries

Root/admin means Vapor application source-root authority: the right to package
or publish the app/depot represented by `[root]` and `[root.steam]`.

Operating-system administrator privilege is a separate concern. Vapor may need
elevation or package-manager instructions for system dependencies, but that
privilege never implies Vapor app/depot authority.

Steam publication authority is also separate. SteamCMD or Steam can ultimately
reject a depot or Workshop upload regardless of local Vapor state.

## Backend capability model

Commands should validate capabilities rather than fixed executable groups.

Representative capabilities:

- global Vapor command registration;
- accepted app-root location;
- Rust build execution;
- Cargo workspace projection;
- source repository inspection;
- source repository synchronization;
- app-source submodule membership management;
- Workshop content packaging;
- Workshop content publishing;
- app/depot packaging;
- app/depot publishing;
- Steam publication session.

Representative providers:

- Vapor-managed app-local package;
- installed Steam application metadata;
- system-detected tool;
- user-configured path;
- Steam runtime/common redistributable;
- OS package-manager action;
- unavailable, with suggested next actions.

Providers are internal resolution details. Diagnostics may name them, but command
grammar should remain Vapor-domain grammar.

## Retired setup command direction

This document previously proposed Shell-owned setup command grammar. That
direction is retired. Vapor-Installer owns install, uninstall, and
development-environment upgrade/downgrade. Vapor Shell keeps source, content,
root, diagnostics, docs, and IDE operations, and reports missing
installer-managed capabilities only as command preflight blockers.

## Source command direction

Source-domain operations now live under `source *`. This surface can absorb
Git-backed source operations without exposing raw Git as the primary interface:

```text
source status
source open SOURCE
source close
source list
source add PATH
source remove SOURCE
source sync
source repair
```

Git concepts may still appear where the user goal is explicitly Git-shaped, such
as cloning a source. Even there, Vapor should express the operation as adopting
or opening a Vapor source, not as a general Git shell.

## Content command correction

Content means Workshop-installable Vapor artifacts. It includes first-party
default engine/game/packagepack content. It excludes Vapor-Root app/depot source.

Retired setup commands do not define the future package-onboarding grammar.

## Package metadata and dependency catalogs

Machine-readable provider and dependency metadata should ship with the
application, likely under app-owned metadata. Vapor-Registry remains separate
authority/catalog infrastructure and should not be required for basic local setup
repair.

The shipped metadata can include:

- known backend tool packages;
- expected checksums and versions;
- Linux distro/package-manager mappings;
- Windows redistributable checks;
- Steam runtime expectations;
- smoke-test definitions;
- fallback and repair suggestions.

## Linux dependency resolution

Linux setup should combine:

- `/etc/os-release` detection;
- package-manager detection;
- binary dependency inspection with `readelf`, `objdump`, or equivalent logic;
- library resolution through loader paths and `ldconfig` where available;
- package database checks when a known package manager exists;
- command-specific smoke tests.

Static scans are not enough. Git and SteamCMD can load helpers, certificates,
NSS modules, SSH tooling, shell/perl helpers, or plugins at runtime. Vapor should
therefore report both dependency-scan results and smoke-test results.

Privileged system package installation must be explicit. The first pass can
print distro-specific commands and explain why they are needed. Later passes may
offer guided elevation.

## Windows dependency resolution

Windows setup should later support:

- Git for Windows or Portable Git detection;
- Visual C++ redistributable detection;
- Steam Common Redistributables where applicable;
- DLL import-table inspection;
- side-by-side app-local DLLs where licensing and stability allow;
- command-specific smoke tests.

SteamCMD remains a controlled publishing/session backend, not ordinary player
surface area.

## Diagnostics policy

Diagnostics should answer:

1. what command the user attempted;
2. which Vapor capability is missing;
3. whether that capability is needed for this command;
4. what providers were detected;
5. what Vapor can repair itself;
6. what requires user confirmation, OS privilege, Steam auth, or source edits;
7. the smallest next command or action.

Avoid generic "setup incomplete" errors when the actual blocker is narrower.

## Implementation phases

### Phase 1: durable design and inventory

- Add this checkpoint.
- Link it from design docs.
- Identify docs and modules that overload setup and content terms.
- Keep parser tests focused on supported grammar, not historical command names.

### Phase 2: command grammar migration

- Move install/uninstall/developer-environment lifecycle into Vapor-Installer.
- Keep installer-owned tooling preparation separate from Workshop content
  commands.

### Phase 3: status model split

- Replace hard-coded tool requirements with named capabilities.
- Report app-root registration, PATH registration, Rust/Cargo, Git, SteamCMD,
  source state, and Steam session as separate rows.
- Preserve machine-readable metadata output for scripts and agents.

### Phase 4: provider resolution

- Add an internal backend resolver.
- Detect app-local, imported host, system, and configured Git/SteamCMD
  providers.
- Reject delegating Git scripts as app-owned providers.
- Do not make Git part of player-mode setup. Git-backed developer workflows use
  an explicitly linked or discovered developer Git provider.
- Add OS/distro/package-manager detection as diagnostic data.

### Phase 5: Self-Setup Repair Actions

- Keep app-local Rust/Cargo/rustup installation explicit and previewable.
- Provide Git and SteamCMD repair suggestions first.
- Add managed downloads only when package metadata can describe them honestly.
- Keep privileged OS package installation explicit and non-silent.

### Phase 6: source/content/root alignment

- Move Git-backed repository operations under source/root workflows.
- Keep Workshop content operations under content.
- Keep app/depot publication under root.
- Keep Steam auth/session behavior session-scoped.

### Phase 7: diagnostics, docs, and tests

- Update command docs and topology references.
- Add regression tests for command help, app-local tooling status, diagnostics, and
  preflight capability failures.
- Verify deployed Steam install behavior from a fresh shell.

## Open decisions

- Final installer UX around optional tooling beyond player and developer mode.
- Exact location and schema for shipped provider/dependency metadata.
- First distro/package-manager actions to support beyond detection.
