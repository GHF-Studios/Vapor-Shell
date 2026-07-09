# Vapor Shell design checkpoints

These documents preserve owner-reviewed product direction. Some sections are an
implemented baseline; others remain design constraints for upcoming workflow
alignment.

- [Product topology](product-topology.md) — canonical vocabulary, installation
  versus source roots, Cargo and Git correspondence, content composition,
  traits and slots, first-party authority, publishing, IDE/toolchain policy,
  and open design questions.
- [Vapor manifest schema](manifest-schema.md) — the bootstrap schema now used
  by the first-party application, workspaces, Cargo packages, content, and
  registry authority.
- [Setup and backend superpass](setup-and-backend-superpass.md) — owner-aligned
  direction for setup, backend capability resolution, hidden Cargo/Git/SteamCMD
  plumbing, and the migration away from overloaded public toolchain/content
  commands.

User guides and reference documentation should cite a checkpoint only when the
corresponding behavior is either implemented or explicitly labeled as planned.
