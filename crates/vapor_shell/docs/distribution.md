# Distribution and app/depot staging

Vapor-Root is the app/depot source root. Its `Vapor.toml` declares `[root]` and
`[root.steam]`; it does not declare workshop packagepacks and does not duplicate
Cargo or Git submodule membership.

```toml
[root.steam]
app-id = 2122620
depot-id = 2122621
development-branch = "vapor-dev"
```

`root publish --dry-run` reconstructs a clean Steam depot payload under
`output/root/content` inside the app root and writes a preview SteamPipe VDF
without uploading. The current baseline payload is conventional and app-owned:

- `Vapor.toml`
- `bin/`
- `docs/`
- `packages/setup/`

Source repositories, Cargo build targets, Cargo registries, Git checkouts,
Steam authentication, logs, and SteamPipe cache state are not staged.

The publish preflight requires installed app-local Rust/Cargo, Git, and
SteamCMD setup plus complete distributable package payloads. Staging copies
`packages/setup`; populate it explicitly with `setup package install` or
refresh it with `setup package repair`. Active tool directories such as
`rustup-home/`, `cargo-home/`, `tools/git/`, and `tools/steamcmd/` are not
staged directly.

Workshop content is separate from this depot flow. A workspace such as
Loo-Cast can publish workshop items and packagepacks, but it is not part of
Vapor-Root's app payload.
