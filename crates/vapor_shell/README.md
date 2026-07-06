# Vapor Shell

Vapor Shell is the interactive boundary between a replaceable Steam-installed
toolchain and an external workspace containing critical authored source.

It intentionally has two roots:

```text
Steam app root (replaceable)          Source workspace (critical)
├── bin/vapor                         ├── Vapor.toml
├── packages/toolchain/               ├── Cargo.toml      (required)
├── rustup-home / cargo-home          └── project repositories/
├── tools/git / tools/steamcmd
├── lib / state / output
└── installed custom content
```

The roots must not overlap. Vapor discovers the installation from the running
executable and the source workspace from the invocation directory. Interactive
filesystem navigation is confined to the source workspace.

## Documentation

- [Architecture](docs/architecture.md): authority, state ownership, module map,
  and failure behavior.
- [Discovery and boundaries](docs/discovery.md): both discovery algorithms,
  expected installation layout, canonicalization, and overlap rejection.
- [Vapor manifests](docs/manifests.md): every supported workspace and content
  identity, required Cargo companions, examples, and intended use.
- [Commands](docs/commands.md): command behavior, arguments, and which root each
  command can affect.
- [Cargo integration](docs/cargo-metadata.md): required Rust workspaces,
  authority boundaries, nested-workspace consequences, and derived metadata.
- [Toolchain](docs/toolchain.md): explicit app-local installation of Rust, Git,
  and SteamCMD with prerequisite diagnostics.
- [Distribution](docs/distribution.md): allowlisted staging, exclusions, docs,
  toolchain payload, and smoke validation.
- [Steam development](docs/steam-development.md): authentication handoff,
  preview builds, confirmation, beta publishing, and persistent cache state.
- [Command scripts](docs/scripts.md): reusable REPL command sequences exposed
  through the same one-shot CLI facade.
- [Development](docs/development.md): source layout, extension checklists, tests,
  and documentation policy.

## Core guarantees

- Authored source never needs to live in the Steam application directory.
- Every source workspace and project has both a Vapor manifest and Cargo
  workspace manifest at the same root.
- Deleting or rebuilding `cargo metadata` output does not lose authored source;
  deleting either source manifest invalidates the workspace or project.
- Missing app-local Cargo remains diagnosable and explicitly repairable, but
  Cargo-backed workflows cannot proceed or fall back to host Cargo.
- `vapor metadata` reports partial state in human-readable or JSON form.
- Commands validate only their own prerequisites and never repair them implicitly.
- User paths are canonicalized before source-boundary checks, including symlinks.
- Nested content markers update context; nested workspace markers are rejected.
- Vapor Shell refuses to target a standalone shell workspace; invocation inside
  the shell repository escalates to a containing Vapor workspace or fails.

## Bootstrap and validate

```text
vapor toolchain status
vapor toolchain finalize
vapor toolchain install
vapor fmt
vapor test
vapor validate
```

The initial bootstrap may use host Cargo once to construct the first Steam app.
After that app is installed, all normal builds and checks are routed through its
bundled toolchain and write outputs beneath the replaceable app root.
