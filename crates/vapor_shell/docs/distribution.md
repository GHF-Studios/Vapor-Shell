# Distribution and self-hosting staging

Vapor-Root is the app/depot source root. Its `Vapor.toml` declares `[root]` and
`[root.steam]`; it does not declare workshop packagepacks and does not duplicate
Cargo or Git submodule membership.

```toml
[root.steam]
app-id = 2122620
depot-id = 2122621
development-branch = "vapor-dev"
```

`self stage` reconstructs a clean Steam depot payload under
`output/root/content` inside the app root. The current baseline payload is
conventional and app-owned:

- `Vapor.toml`
- `bin/`
- `docs/`
- `packages/toolchain/`

Source repositories, Cargo build targets, Cargo registries, Git checkouts,
Steam authentication, logs, and SteamPipe cache state are not staged.

`self smoke` requires the app marker, `bin/vapor`, `docs`, and the immutable
`packages/toolchain` input containing Rust/Cargo, Git, and SteamCMD.

Workshop content is separate from this depot flow. A workspace such as
Loo-Cast can publish workshop items and packagepacks, but it is not part of
Vapor-Root's app payload.
