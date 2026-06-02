---
tier_id: tier-15c
title: diff_blast_radius MCP tool — impact of a working-tree / commit / ref-range diff
deps: [tier-15b, tier-14]
exit_criteria:
  - `diff_blast_radius` is registered on `AriadneServer`, discoverable; input is a `DiffSpec` (WorkingTree default | Commit | RefRange) + optional `depth`/`kinds`.
  - `ariadne-mcp` gains an `ariadne-git` dependency for the live diff; `tests/architecture.rs` stays green (git is a driven adapter; the daemon stays git-free per RD7); recorded in ADR-0023.
  - The tool runs `ariadne_git::diff` → (hunks, paths) in the MCP process, then routes the graph join — daemon path sends `DaemonQuery::DiffBlast { hunks, changed_paths, depth, kinds }`, the daemon builds `FileSymbolSpans` (hash-guarded) and runs `GraphIndex::diff_blast`; cold fallback runs it in-process.
  - The returned must∪may equals the union of per-seed v1 `blast_radius` (tier-14 invariant re-asserted through the live path); a changed file with no resolved symbol is returned as unresolved, never dropped.
  - A spawned-server golden exercises the tool on a fixture git repo with an uncommitted edit; a daemon/cold parity unit test passes.
  - Discoverability finalized: handshake snapshots at 17 tools; `docs/codebase-overview.md`, `README.md`, and CLAUDE.md's "Ariadne code intelligence" catalog updated (CLAUDE.md via `/rules-writer`).
  - `cargo nextest run -p ariadne-core -p ariadne-mcp -p ariadne-daemon` + architecture + clippy + fmt all green.
status: completed
completed: 2026-06-02
---

<context>
tier-14 built `GraphIndex::diff_blast(symbol_lines, hunks, changed_paths, depth, kinds)` and the `ariadne_git::diff(root, spec)` reader [src: crates/ariadne-graph/src/diff_blast.rs:81; crates/ariadne-git/src/adapters/gix/diff.rs:38, re-exported at crates/ariadne-git/src/lib.rs:17]. This tier wires them into the `diff_blast_radius` MCP tool — the reviewer's "what does *this change* affect". The git diff must run where `ariadne-git` is linked; RD7 bars the daemon from `ariadne-git`, so the MCP server (already a cold composition root depending on `ariadne-storage` [src: crates/ariadne-mcp/Cargo.toml:20-21]) takes the git dep and computes hunks, then routes the graph+span join to the warm daemon over a new query, or runs it cold. `FileSymbolSpans` is built from catalog symbols + the changed files' bytes, hash-guarded, exactly as the CLI does [src: crates/ariadne-cli/src/commands/index.rs:215-276]. Full context: plan.md; tier-14.
</context>

<decisions>
- D1 — `ariadne-mcp` depends on `ariadne-git`; the daemon does not (ADR-0023). The git diff is a driven-adapter read; the MCP server already wires a driven adapter (`ariadne-storage`) for its cold path, so taking `ariadne-git` is the same composition-root pattern (ADR-0007). The architecture test permits it — `ariadne-git` stays `deps ⊆ {core}`, and nothing depends on a driving adapter [src: tests/architecture.rs:40-45,121-140]. The daemon receives pre-computed hunks over the wire and never links git, preserving RD7. *Rejected:* git in the daemon (violates RD7 + adapter isolation); a fourth driving adapter just to run git (surface for nothing).
- D2 — daemon-routed via hunks-over-wire (user-chosen). `DaemonQuery::DiffBlast { hunks: Vec<LineHunk>, changed_paths, depth, kinds }` carries the diff — `LineHunk` already lives in `ariadne-core` (tier-11b); the daemon builds `FileSymbolSpans` from its warm `symbols` + reads the changed files' bytes for `line_starts`, then runs `diff_blast` on the warm graph. Cold fallback builds a `Catalog` and does the same in-process. *Rejected:* cold-only (pays a graph build per call; deviates from the stub's daemon-routing); sending whole `FileSymbolSpans` over the wire (the daemon already holds the symbols — only the diff is client-side).
- D3 — span building is hash-guarded and shared, not triplicated. Extract the pure `line_starts(&[u8]) -> Vec<u32>` + the "group changed symbols by file, read bytes, drop on `blake3` mismatch" shape into one reusable helper so the CLI, daemon, and MCP-cold paths agree [src: index.rs:215-276]. A file whose on-disk bytes no longer match its indexed `blake3` (stale offsets) contributes a path but no spans → it surfaces as `unresolved`, never a wrong seed. `diff_blast_radius` is therefore most precise against a fresh index (the daemon's live watcher keeps it so — tier-08); staleness degrades to `unresolved`, never to incorrect impact. *Rejected:* reading HEAD blobs for `line_starts` (a second content source to reconcile with indexed spans; tier-14 already fixed the spans-at-HEAD semantics).
- D4 — the MCP `DiffSpec` input mirrors `ariadne_core::DiffSpec` behind a `JsonSchema`-deriving `DiffSpecInput`, mapped at the handler. core's `DiffSpec` need not derive `schemars` (it is a wire/domain type); the MCP input layer owns the schema, as it already does for `EdgeKindFilter` [src: crates/ariadne-mcp/src/types.rs; server.rs:507-522]. Default = `WorkingTree`.
</decisions>

<files>
- crates/ariadne-mcp/Cargo.toml — modify: add `ariadne-git = { workspace = true }` (D1) with a comment citing ADR-0023 + RD7.
- crates/ariadne-core/src/domain/daemon/{query.rs,response.rs,rows.rs} — modify: `DaemonQuery::DiffBlast {…}` + `DaemonResponse::DiffBlast(DiffBlastReport)` + `DiffBlastReport`/`DiffSeed` mirror DTOs.
- crates/ariadne-graph/src/span_lines.rs — modify: expose the pure `line_starts` + a `spans_from(symbols_by_file, contents)` builder for reuse (D3); behaviour-guarded by tier-11b/14 goldens [src: crates/ariadne-graph/src/span_lines.rs:30-70].
- crates/ariadne-daemon/src/domain/queries/impact.rs + dispatch.rs — modify: a `diff_blast` handler — build spans from `WarmCatalog` + file reads, run `GraphIndex::diff_blast`, project via the existing `summarize`; route it [src: crates/ariadne-daemon/src/domain/dispatch.rs:21-26,46-65].
- crates/ariadne-mcp/src/types.rs — modify: `DiffBlastInput { spec: DiffSpecInput, depth, kinds }`, `DiffSpecInput`, `DiffBlastOutput`/`DiffSeedRow` — all `JsonSchema`.
- crates/ariadne-mcp/src/tools/diff_blast.rs + tools/mod.rs — new/modify: cold path — run git diff, build `Catalog`, build spans, run `diff_blast`.
- crates/ariadne-mcp/src/server.rs — modify: the `diff_blast_radius` `#[tool]` (run git diff first, then daemon-route the hunks or cold-run) + the `project_daemon` arm + description.
- crates/ariadne-mcp/tests/ + snapshots/ — new: a fixture-git-repo golden (uncommitted edit) + a daemon/cold parity test; re-accepted handshake snapshots (17 tools).
- docs/codebase-overview.md, README.md, CLAUDE.md — modify: final 17-tool catalog (CLAUDE.md via `/rules-writer`).
- docs/adr/0023-mcp-git-diff-dependency.md — new: D1 asymmetry (MCP may link git, daemon may not), citing ADR-0007/0015 + RD7.
</files>

<steps>
1. Failing test first: a spawned-server golden builds a fixture git repo (commit + uncommitted edit), seeds `.ariadne/index.redb` matching the worktree, spawns the MCP server (autospawn off → cold path), calls `diff_blast_radius` with `WorkingTree`, and asserts a stable insta golden + that must∪may equals the union of `blast_radius` over the changed seeds. Red — the tool is unregistered [src: crates/ariadne-git/tests/diff.rs fixture-repo helper; tier-14 step 6].
2. Add `DiffBlast` to the daemon protocol (`query.rs`/`response.rs`/`rows.rs`) carrying `Vec<LineHunk>` + paths; mirror `DiffBlastReport`/`DiffSeed` from the graph types [src: diff_blast.rs:81-110; tier-14 D4].
3. (D3) Extract/confirm the shared span builder: pure `line_starts` + a `spans_from` over (symbols-by-file, file bytes) with the `blake3` guard [src: index.rs:215-276]; keep tier-11b/14 goldens green.
4. Daemon `impact::diff_blast`: from `WarmCatalog.symbols` group changed-path symbols, read bytes under `root`, build `FileSymbolSpans`, run `self.graph.diff_blast(...)`, project `SymbolId → SymbolSummary` via the existing `summarize` [src: dispatch.rs:46-65]; route in `dispatch.rs`.
5. MCP cold `tools/diff_blast.rs`: `ariadne_git::diff(&root, &spec)` → (hunks, paths) [src: gix/diff.rs:38], build `Catalog`, build spans, `graph.diff_blast(...)`, project to the `types.rs` output.
6. `server.rs` `diff_blast_radius` `#[tool]`: run the git diff first (both paths need it), then `try_query_async(DiffBlast{hunks,paths,…})` → `project_daemon`, else cold `tools::diff_blast::handle`; add the `project_daemon` arm + a template description (triggers: "blast radius of my diff", "what does this change affect") [src: server.rs:248-268,489].
7. Parity unit test for the `DiffBlast` arm; hand-review the golden. Finalize discoverability: re-accept handshake snapshots (17 tools), update `docs/codebase-overview.md` + `README.md`, update CLAUDE.md's tool catalog via `/rules-writer`. Write ADR-0023. Optionally add a one-line `tests/architecture.rs` assertion that `ariadne-daemon` does not depend on `ariadne-git` (enforces RD7). Run the full gate + a live self-index dogfood (`diff_blast_radius` on a real ariadne_v2 working-tree edit).
</steps>

<verification>
- `cargo nextest run -p ariadne-core -p ariadne-mcp -p ariadne-daemon` — the fixture-repo golden, the union-equality assert, the daemon/cold parity test, and the re-accepted handshake snapshots (17 tools) all green; tier-11b/14 span goldens unchanged after the D3 extraction.
- End-to-end (real, not stub): the golden runs the real `ariadne_git::diff` over an on-disk fixture repo with an uncommitted edit and the real `diff_blast`; the dogfood run invokes `diff_blast_radius` on a live ariadne_v2 branch edit and the impact set is reviewed against the union of per-file `blast_radius`.
- Manual: in a Claude Code session ask "what does my current diff affect" — confirm Claude selects `diff_blast_radius`; kill the daemon → cold fallback still answers.
- `cargo test --test architecture` (stays green: `ariadne-mcp → ariadne-git` is permitted; RD7 preserved by construction / the optional daemon-no-git assertion), `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo fmt --all --check`, `RUSTDOCFLAGS=-D warnings cargo doc -p ariadne-core -p ariadne-mcp -p ariadne-daemon --no-deps` — green.
</verification>

<rollback>
`git checkout -- crates docs README.md CLAUDE.md` and `rm -f crates/ariadne-mcp/src/tools/diff_blast.rs docs/adr/0023-mcp-git-diff-dependency.md` plus the new snapshots; drop the `ariadne-git` line from `crates/ariadne-mcp/Cargo.toml`. The tool + its protocol variant + the git dep are additive; tiers 15a/15b and tier-14 are untouched.
</rollback>
