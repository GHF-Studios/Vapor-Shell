# Setup and backend superpass

Status: **owner-aligned checkpoint for implementation**

This checkpoint records the owner decisions behind the setup/tooling rework. It
extends `product-topology.md`; it does not replace the root/source/content
model defined there.

## Recovery note

This document is the durable handoff point for the current superpass. If an
agent session fails, continue from this file before using older chat context.

Current implementation intent:

1. replace public `toolchain` lifecycle with a broader `setup` command surface;
2. keep backend tools hidden behind Vapor source, content, root, Steam, and setup
   goals;
3. always attempt to provide Rust/Cargo, Git, and SteamCMD availability rather
   than asking users to choose player/developer/publisher modes;
4. use OS and package-manager detection to explain repair paths without silently
   performing privileged system changes;
5. keep app/depot root authority separate from operating-system administrator
   privilege.

## Non-goals

- Do not expose Git, Cargo, rustup, SteamCMD, package managers, DLLs, or shared
  libraries as the primary product grammar.
- Do not make users choose formal modes such as player, developer, or publisher.
- Do not keep legacy public compatibility surfaces after command grammar changes.
- Do not pretend a wrapper around host Git is a portable bundled Git.
- Do not silently edit user source, global IDE settings, Steam authentication
  state, shell profiles, or operating-system packages.
- Do not merge Workshop/content source with Vapor-Root app/depot source.

## Product shape

Vapor should expose user goals and Vapor domains:

```text
setup
  inspect, install, repair, extend, downgrade, and uninstall the app-local
  command environment and backend availability

source
  open, index, sync, repair, and validate authored source roots

content
  inspect, validate, build, package, install, and publish Workshop content

root
  build, package, and publish the Vapor application/depot source root

steam
  inspect Steam availability and session-scoped publication state when needed
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

## Setup command direction

`toolchain` is too narrow for the work it currently performs. It mixes:

- app-root acceptance;
- PATH registration;
- Rust/Cargo/rustup installation;
- Git availability;
- SteamCMD availability;
- distributable package content.

The public lifecycle should move under `setup`:

```text
setup status
setup install [--dry-run]
setup repair [--dry-run]
setup extend [--dry-run]
setup downgrade [--dry-run]
setup uninstall [--dry-run]
```

This is an intentionally breaking migration. Public `toolchain` commands should
be removed instead of kept as aliases. Internal module names may lag temporarily
only where a mechanical rename would obscure the behavioral change.

The default setup target is "make the installed Vapor environment as complete as
it reasonably can be on this machine." It should try to resolve Rust/Cargo, Git,
and SteamCMD readiness. It should not require a user to opt into a developer or
publisher role.

## Source command direction

The existing `open`, `close`, and `sources` commands are source-domain
operations. A future source surface can absorb Git-backed source operations
without exposing raw Git as the primary interface:

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

The previous content-package commands that populated `packages/toolchain` were
misnamed. They are setup/package operations, not Workshop content operations,
and must live under setup or package/depot preparation.

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

Avoid generic "toolchain incomplete" errors when the actual blocker is narrower.

## Implementation phases

### Phase 1: durable design and inventory

- Add this checkpoint.
- Link it from design docs.
- Identify current docs and modules that overload `toolchain` and `content`.
- Record removed public commands so tests and diagnostics reject them clearly.

### Phase 2: command grammar migration

- Add `setup status/install/repair/uninstall` as the primary lifecycle.
- Remove the public `toolchain` command surface.
- Move old content package-payload behavior to setup package
  preparation and remove those public content subcommands.

### Phase 3: status model split

- Replace hard-coded tool requirements with named capabilities.
- Report app-root registration, PATH registration, Rust/Cargo, Git, SteamCMD,
  package payloads, source state, and Steam session as separate rows.
- Preserve machine-readable metadata output for scripts and agents.

### Phase 4: provider resolution

- Add an internal backend resolver.
- Detect app-local, system, and configured Git/SteamCMD providers.
- Reject fake host wrappers as bundled providers.
- Allow controlled use of valid system providers when policy permits.
- Add OS/distro/package-manager detection as diagnostic data.

### Phase 5: setup repair actions

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
- Add regression tests for command help, setup status, removed command rejection,
  diagnostics, and preflight capability failures.
- Verify deployed Steam install behavior from a fresh shell.

## Open decisions

- Final exact `setup extend` and `setup downgrade` semantics.
- Final internal rename schedule for modules that still use `toolchain` as an
  implementation term after public commands move to `setup`.
- Exact location and schema for shipped provider/dependency metadata.
- First distro/package-manager actions to support beyond detection.
