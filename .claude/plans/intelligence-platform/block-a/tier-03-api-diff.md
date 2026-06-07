---
tier_id: tier-03
title: A2 — api_surface_diff semver classifier + MCP/CLI surfaces
deps: [tier-02]
exit_criteria:
  - "`cargo nextest run --workspace` green; new failing-first tests now pass"
  - "graph unit test: removed public symbol → Major; added → Minor; signature-changed → Major; no surface change → None"
  - "e2e golden: `api-diff` over a 2-commit fixture (one removed, one added, one signature-changed public item) returns verdict=Major with the exact added/removed/changed lists, byte-identical across two runs"
  - "real run `ariadne api-diff HEAD~1..HEAD` on this repo prints a verdict + lists; MCP `api_surface_diff` returns the same for the same refs"
  - "`cargo test --test architecture` green: `ariadne-daemon` still does NOT link `ariadne-git`; `ariadne-mcp → ariadne-parser` is accepted (driving→driven)"
  - "`cargo clippy ... -D warnings`, `cargo fmt --all --check` green; ADR-0027 committed"
status: pending
---

<context>
Adds A2's verdict: classify the public-surface delta between two refs as None/Patch/Minor/Major per the Cargo SemVer taxonomy — removed or signature-changed = major, added = minor [src: https://doc.rust-lang.org/cargo/reference/semver.html]. Consumes tier-02's `PublicSymbol` + `read_blobs_at` + `public_surface`. Runs entirely in the querying process; no daemon leg (plan D6). Full context: ./plan.md.
</context>

<files>
- `crates/ariadne-graph/src/api_surface.rs` (new) + `lib.rs` re-export — `enum SemverBump { None, Patch, Minor, Major }`, `struct ApiDiffReport { verdict, added, removed, changed }`, pure `api_surface_diff(base: &[PublicSymbol], head: &[PublicSymbol]) -> ApiDiffReport`.
- `crates/ariadne-mcp/Cargo.toml` (modify) — add `ariadne-parser` (driving→driven; ADR-0027).
- `crates/ariadne-mcp/src/{tools/api_surface_diff.rs (new), tools/mod.rs, types.rs, server.rs}` — self-contained `handle` (no `DaemonQuery` variant): resolve refs → blobs → surfaces → classify.
- `crates/ariadne-cli/src/commands/{api_diff.rs (new), mod.rs}` + `main.rs` (modify) — `ariadne api-diff <base>..<head>` (same in-process composition; CLI already links git+parser+graph).
- `docs/adr/0027-mcp-parser-dependency.md` (new) — records the `ariadne-mcp → ariadne-parser` edge and why A2 has no daemon leg (D6).
- Tests: inline `#[cfg(test)]` in `api_surface.rs`; e2e 2-commit golden in `crates/ariadne-e2e` (mirror crates/ariadne-cli/tests/incremental_history.rs).
</files>

<steps>
1. Write failing graph unit tests first (TDD): synthetic base/head `PublicSymbol` lists exercise each verdict (removed→Major, added→Minor, signature-changed→Major, identical→None) [src: CLAUDE.md TDD rule; cargo SemVer reference].
2. Implement `api_surface_diff` (pure): identity key = `(name, kind)`; in-head-only → `added` (Minor); in-base-only → `removed` (Major); in-both with differing `signature` → `changed` (Major); verdict = max bump over all deltas (`Major > Minor > Patch > None`); sort `added`/`removed`/`changed`. Only `Visibility::Public` symbols are in the inputs, so a visibility narrowing surfaces as a removal [src: https://doc.rust-lang.org/cargo/reference/semver.html].
3. Write ADR-0027 (template under docs/adr/) and add `ariadne-parser` to `crates/ariadne-mcp/Cargo.toml`. The edge is a permitted driving→driven dep (mirrors the existing `ariadne-mcp → ariadne-git`); `api_surface_diff` runs in-process so no `DaemonQuery` variant is added and `ariadne-daemon` stays git-free [src: tests/architecture.rs:121-154; docs/adr/0023].
4. MCP `api_surface_diff` handler: input `{ base, head }` revspecs → `ariadne_git::diff(RefRange { from: base, to: head })` for `changed_paths` → `read_blobs_at(base, changed_paths)` and `read_blobs_at(head, changed_paths)` → for each blob detect `Lang` via the indexer's existing path→Lang detection and call `public_surface` → concat per side → `api_surface_diff` → wire output (verdict string + symbol rows). Wire `types.rs`/`server.rs`/`tools/mod.rs` like an existing tool [src: crates/ariadne-mcp/src/tools/diff_blast.rs; crates/ariadne-parser/src/lib.rs:16].
5. CLI `api-diff <base>..<head>` subcommand: split the `base..head` arg, run the identical in-process composition, print the report as JSON. Exit 0 (informational — the verdict is in the payload for the Block-B PR-risk bot) [src: crates/ariadne-cli/src/commands/mod.rs].
6. Add the e2e golden: a fixture repo with two commits seeding a removed `pub`, an added `pub`, and a signature-changed `pub`; assert `verdict == Major`, the three lists exactly, byte-identical re-run, and the run completes well under the 500ms incremental budget on the multi-file diff (BR3).
</steps>

<verification>
- `cargo nextest run --workspace` green (graph unit + e2e golden; red before step 2).
- Unit: each verdict matches the SemVer taxonomy; verdict is the max over mixed deltas.
- E2e: correct verdict + exact lists; re-run byte-identical (determinism); latency under budget.
- Real run: `ariadne api-diff HEAD~1..HEAD` prints a verdict; the MCP `api_surface_diff` tool returns the same for those refs (parity).
- `cargo test --test architecture` green — assert the daemon still does not link `ariadne-git` (invariant 5) and the new `mcp → parser` edge passes (invariant 4).
- `cargo clippy ... -D warnings`, `cargo fmt --all --check` green.
Fail loudly: a wrong verdict, a leaked `gix`/`tree-sitter` type, a daemon git edge, or non-determinism is a hard fail [src: CLAUDE.md `<rules>`].
</verification>

<rollback>
Revert the commit. The only non-additive change is the `ariadne-mcp → ariadne-parser` Cargo edge; removing it (and ADR-0027) returns the dependency graph to the tier-02 baseline. The graph use-case, tool, and command files are additive.
</rollback>
