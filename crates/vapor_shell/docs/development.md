# Development guide

## Repository layout

```text
crates/vapor_shell/
├── README.md
├── docs/
├── src/
│   ├── lib.rs
│   ├── main.rs
│   ├── app.rs
│   ├── command.rs
│   ├── cargo_metadata.rs
│   ├── distribution.rs
│   ├── documentation.rs
│   ├── discovery.rs
│   ├── manifest.rs
│   ├── metadata/
│   │   ├── mod.rs
│   │   ├── report/
│   │   │   ├── mod.rs
│   │   │   └── render.rs
│   │   └── validation.rs
│   ├── path_setup.rs
│   ├── prompt.rs
│   ├── steam.rs
│   ├── terminal.rs
│   ├── toolchain.rs
│   ├── workflow.rs
│   ├── workspace.rs
│   └── state.rs
└── tests/
    ├── common/
    ├── cargo_metadata.rs
    ├── command.rs
    ├── discovery.rs
    ├── installation_commands.rs
    ├── manifest.rs
    ├── metadata.rs
    ├── path_setup.rs
    ├── state.rs
    ├── toolchain.rs
    ├── workflow.rs
    └── workspace.rs
```

`lib.rs` is the documented implementation surface. `main.rs` remains a thin
process adapter. Behavioral tests live outside `src` and exercise public
contracts as downstream code would.

## Running locally

A direct `cargo run` places the executable under the same repository that acts
as authored source, intentionally violating the disjoint-root invariant. For a
manual session, build the binary, copy it beneath a staging installation with a
`[workspace]` `Vapor.toml` and bundled Cargo layout, then invoke that staged
binary while the process directory is a separate Vapor source workspace. The
integration fixtures automate this topology for tests.

After the staged or installed binary works from Vapor-Root, run `workspace
remember`. This persists the external source selection for future Steam GUI
launches without moving source into the app installation.

## Documentation policy

The crate denies missing public documentation and forbids unsafe code. Public
items should include whichever sections clarify their contract:

- purpose and authority;
- examples for non-obvious construction or parsing;
- `# Errors` for fallible functions;
- `# Panics` only when unavoidable;
- security or boundary invariants;
- relationship to replaceable versus critical state.

Long-form concepts belong in `docs/`; API-specific contracts stay beside code.
README links provide the entry path instead of duplicating every detail.

## Adding a command

1. Add a documented `ShellCommand` variant.
2. Use a Clap `ValueEnum` for every finite argument domain.
3. Describe unrestricted paths or numeric domains with semantic value names.
4. Implement the effect in `command::execute`. Reuse `ResolvedMetadata` and a
   targeted `ValidationPlan` when the command depends on environment state.
5. Decide explicitly whether it reads source, reads installation state, or
   mutates source state. Installation navigation is not allowed implicitly.
6. Add integration coverage in `tests/command.rs` or a focused new file.
7. Update `docs/commands.md`.

## Adding a manifest identity

1. Extend `ProjectKind` or `ContentKind`; `[workspace]` itself has no kind.
2. Add the deserialization field and mapping in `manifest.rs`.
3. Add exhaustive integration coverage in `tests/manifest.rs`.
4. Document syntax, semantics, and composition role in `docs/manifests.md`.
5. Update shared Vapor vocabulary rather than introducing a shell-only spelling.

## Adding a workspace project

1. Give the project root a `[project]` Vapor manifest.
2. Give the same root a Cargo manifest declaring a Cargo workspace.
3. Add the project path to `[workspace].members` in Vapor-Root's Vapor manifest.
4. Keep the project out of the root Cargo workspace; it owns its own packages.
5. Declare only binaries that belong in the Steam installation once the final
   project/distribution policy schema is settled.
6. Extend workspace, Cargo-metadata, and workflow integration tests.

## Changing discovery

Discovery changes require tests for both roots, overlap rejection, canonical
containment, behavior below nested content, and escalation from the shell
component to its containing umbrella workspace. Do not introduce a fallback that
places authored source inside installation state or permits self-targeting.

## Validation

Run tests, strict Clippy, rustdoc with warnings denied, and formatting before
handoff. Cargo metadata tests use a fake bundled Cargo executable and do not
depend on the developer's global Cargo installation. Tests must distinguish an
invalid missing Cargo manifest from a repairable missing bundled Cargo tool.
