# Vapor Shell

Vapor Shell is the interactive boundary between the Steam-installed app root
and an external source root containing critical authored source.

At runtime, the shell works with two active filesystem roots:

```text
Steam installation / app root         Active source root (critical)
├── bin/vapor                         ├── Vapor.toml
├── .vapor/state/                     ├── Cargo.toml      (workspace roots)
├── rustup-home / cargo-home
├── tools/git / tools/steamcmd
├── lib / state / output
└── installed custom content
```

The roots must not overlap. Vapor discovers the installation from the running
executable. Source roots are opened explicitly by path or app-local registered
name, and the last active source is remembered under the app root. Interactive
filesystem navigation is confined to the opened source root.

The product model also distinguishes a third concept: a Vapor application
source root such as Vapor-Root. That is source for building the Steam app/depot,
not the installed Steam directory itself.

## Documentation

- [Architecture](docs/architecture.md): authority, state ownership, module map,
  and failure behavior.
- [Discovery and boundaries](docs/discovery.md): both discovery algorithms,
  expected installation layout, canonicalization, and overlap rejection.
- [Vapor manifests](docs/manifests.md): root, workspace, project, content,
  composition, trait, and slot syntax.
- [Commands](docs/commands.md): command behavior, arguments, and which root each
  command can affect.
- [Cargo integration](docs/cargo-metadata.md): required Rust workspaces,
  authority boundaries, nested-workspace consequences, and derived metadata.
- [Setup](docs/setup.md): explicit app-local installation of Rust, Git,
  and SteamCMD with prerequisite diagnostics.
- [Distribution](docs/distribution.md): allowlisted staging, exclusions, docs,
  self-setup payloads, and smoke validation.
- [Steam development](docs/steam-development.md): root publish previews, manual
  upload confirmation, beta publishing, and persistent cache state.
- [Command scripts](docs/scripts.md): reusable REPL command sequences exposed
  through the script CLI facade.
- [Development](docs/development.md): source layout, extension checklists, tests,
  and documentation policy.
- [Design checkpoints](docs/design/README.md): owner-reviewed direction that is
  authoritative only where it matches implemented and verified behavior.

## Core guarantees

- Authored source never needs to live in the Steam application directory.
- Every source workspace has a Vapor manifest and Cargo manifest at
  the same root; every Vapor project is represented by a Cargo package.
- Vapor application source roots are source super-repositories; they are not
  the same thing as the Steam installation/app root.
- Deleting or rebuilding `cargo metadata` output does not lose authored source;
  deleting either source manifest invalidates the workspace or project.
- Missing app-local Cargo remains diagnosable and explicitly repairable, but
  Cargo-backed workflows cannot proceed or fall back to host Cargo.
- `vapor metadata` reports partial state in human-readable or JSON form.
- Commands validate only their own prerequisites and never repair them implicitly.
- User paths are canonicalized before source-boundary checks, including symlinks.
- Nested content markers update context; nested workspace markers are rejected.
- Vapor Shell can start closed with only an app root. Source work begins only
  after `open SOURCE`, and invocation inside a nested shell checkout opens the
  highest containing Vapor source root.

## Bootstrap and validate

The only host-built artifact allowed in the initial local bootstrap is the
Vapor shell executable. Deploy it into the Steam app directory first:

```text
crates/vapor_shell/scripts/bootstrap-local-app-deploy.sh \
  --binary /path/to/built/vapor \
  --target "$HOME/.local/share/Steam/steamapps/common/Loo Cast" \
  --yes
```

Then run the installed app-local command:

```text
/home/.../steamapps/common/Loo Cast/bin/vapor open /path/to/source
/home/.../steamapps/common/Loo Cast/bin/vapor setup self status
/home/.../steamapps/common/Loo Cast/bin/vapor setup self install
fmt
test
validate
```

After that shell is installed, all normal builds and checks are routed through
the Steam app's own `bin/vapor`. `setup self install` explicitly installs active
tools into the app root. Final app packaging stages `packages/setup`; there is
no second source tree for self-setup payloads.
