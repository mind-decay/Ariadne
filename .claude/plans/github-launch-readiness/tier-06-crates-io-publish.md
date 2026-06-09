---
tier_id: tier-06
title: Publish the workspace to crates.io under an available alt name (optional, last)
deps: [tier-01]
exit_criteria:
  - every required ariadne-* crate name confirmed available (or reserved) on crates.io
  - intra-workspace deps carry both path and version; `cargo publish --dry-run` green for the whole tree
  - the CLI published-name decision is recorded (publish-as-ariadne-cli vs rename)
  - real `cargo publish` performed only under explicit owner authorization
status: pending
---

<context>
Optional and highest-cost tier; run last. The bare name `ariadne` is taken (8.1M
downloads) [src: https://crates.io/api/v1/crates/ariadne], so the CLI publishes
under an alt name. crates.io forbids dependencies on code outside the registry,
so publishing the CLI forces publishing the entire `ariadne-*` dependency tree
first, in topological order [src:
https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html]. Real
publishes are permanent (yank-only) and outward-facing — gate them on owner
authorization [src: plan.md D6, R2, R5].
</context>

<files>
- `Cargo.toml` + each `crates/*/Cargo.toml` — modify; flip `publish`, add
  `version` to intra-workspace deps, ensure `description` + `license` present.
- `docs/releasing.md` — modify; document the topological publish order + name.
</files>

<steps>
1. Name availability: for each workspace member that the CLI transitively depends
   on (core, storage, salsa, graph, git, parser, scip, mcp, watcher, and the CLI
   itself; plus daemon/e2e if published), query
   `https://crates.io/api/v1/crates/<name>` — HTTP 404 means free. If any
   required name is taken, stop and surface it as an owner decision (rename or
   use an `ariadne-*`-prefixed alias) [src: crates.io API].
2. CLI published name — record the decision: publish the package as-is
   (`cargo install ariadne-cli`, binary still `ariadne`) — recommended, no
   rename; OR rename the package to `ariadne-code`, which also touches
   `cog.toml [monorepo.packages.cli]`, the `pr-title` scope list, and
   `tests/architecture.rs` references [src: cog.toml; ci.yml:171-187]. Mark
   `<OWNER-DECISION-PLACEHOLDER>`; default to publish-as-`ariadne-cli`.
3. Make crates publishable: set `publish = true` (workspace or per-crate), and on
   every intra-workspace dependency add a `version` next to `path`, e.g.
   `ariadne-core = { path = "crates/ariadne-core", version = "1.0.0" }` — locally
   the path is used, on publish the registry version is used [src:
   https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html
   "Multiple locations"]. Ensure each crate has a `description` and the inherited
   `license` (publish requires both).
4. Dry-run the whole tree in dependency order (leaves first): for each crate
   `cargo publish -p <crate> --dry-run`, ending with the CLI. This both validates
   packaging and confirms crates.io accepts the PolyForm (non-OSI) SPDX license
   (R5). Root-cause any failure; do not weaken metadata to pass.
5. Automation note in `docs/releasing.md`: cargo-dist does not publish to
   crates.io, so publishing is a manual `cargo publish` per crate in order on
   each release. Adopting a publisher tool (`release-plz`, `cargo workspaces`) is
   a new dependency — out of scope here; flag as a follow-up requiring owner
   sign-off [src: CLAUDE.md `<rules>` "no new dependency without asking"].
6. Real publish — only after dry-runs pass, names are secured, and the owner
   explicitly authorizes (publishes are irreversible): publish leaves→root in the
   same order. Verify `cargo install ariadne-cli` (or the chosen name) from
   crates.io installs a working `ariadne`.
</steps>

<verification>
- Name check: a recorded list of member → crates.io status (free/taken).
- `cargo publish --dry-run` exits 0 for every crate in the tree, CLI last.
- `grep -r 'version = "1.0.0"' crates/*/Cargo.toml` shows versions added to
  intra-workspace deps (or confirm via `cargo metadata`).
- docs/releasing.md lists the topological publish order and the chosen CLI name.
- Post-publish (owner-gated): `cargo install <name>` from a clean machine yields a
  runnable `ariadne --version`.
</verification>

<rollback>
Pre-publish: `git checkout -- Cargo.toml crates/*/Cargo.toml docs/releasing.md`
reverts all metadata. Post-publish is NOT reversible — crates.io versions can only
be yanked (`cargo yank`), not deleted; treat step 6 as a point of no return and do
not run it without owner authorization.
</rollback>
</content>
