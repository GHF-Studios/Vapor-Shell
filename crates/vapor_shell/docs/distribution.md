# Distribution and self-hosting staging

`Vapor-Root/Vapor.toml` is the workspace identity and distribution allowlist. Its
`[distribution]` section owns the
Steam AppID, DepotID, development branch, staged payload mappings, and
exclusions. `[workspace]` owns the project inventory used by both
validation and documentation through its Vapor project members, so packaging
code contains no hard-coded component list. Each member is resolved from its
Vapor manifest and required companion Cargo workspace manifest.

Payload entries choose either the replaceable installation or critical source as
their root, then map a safe relative `from` path to a relative depot `to` path.
Required inputs fail staging; optional inputs are skipped. Every source is
canonicalized and symlinks escaping its declared root are rejected.

The distributable payload includes the app marker, Vapor-owned binaries,
libraries, generated docs, source-controlled workflows, and the immutable
`packages/toolchain` input containing Rust/Cargo, Git, and SteamCMD. Activated
tool directories, Cargo credentials, Git checkouts, Steam authentication, logs,
and app cache are not mapped into the depot.

`self stage` deletes only `$VAPOR_HOME/output/root/content`, rebuilds docs, and
copies allowlisted inputs. It never stages source repositories, build targets,
Cargo registries, Steam credentials, or SteamPipe cache state.

`self smoke` requires `Vapor.toml`, `bin/vapor`, `docs`, and
`packages/toolchain`. This is intentionally structural; executable-level
component health checks can be added without changing distribution ownership.
