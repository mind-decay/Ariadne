---
tier_id: tier-15c
audited: 2026-06-02
verdict: PASS
commit: 58f5e3dac678a01c0d519adb4ac5eb9717828fe5
---

<scope>
Reviewed `tier-15c-diff-blast-radius-tool.md` against the working-tree diff
(HEAD = tier-15b `58f5e3d`; tier-15c is uncommitted, audited pre-commit per the
audit-gate). Diff spans the tier `<files>` plus consequential follow-ons (façade
re-exports, `Cargo.toml`/`.lock`, `tests/architecture.rs`, the CLI D3 refactor).
Read every changed source file end-to-end; re-ran the full `<verification>`
gate. The MCP `diff_blast_radius` tool wires tier-14's `GraphIndex::diff_blast`
+ `ariadne_git::diff` into a 17th MCP tool, daemon-routed (hunks over wire) with
a cold fallback.
</scope>

<checks_run>
- fmt: `cargo fmt --all --check` → FMT_OK.
- tests: `cargo nextest run -p ariadne-core -p ariadne-mcp -p ariadne-daemon` →
  91/91 pass, incl. new `tools_diff_blast` golden + the `diff_blast_arm_matches_cold_output`
  parity unit test.
- architecture: `cargo test --test architecture` → ok (new RD7 daemon-no-git
  clause non-vacuous: `ariadne-daemon` is a workspace member with deps).
- clippy: `cargo clippy --workspace --all-targets --all-features -- -D warnings` → clean.
- doc: `RUSTDOCFLAGS="-D warnings" cargo doc -p ariadne-core -p ariadne-mcp -p ariadne-daemon --no-deps` → clean.
- evidence: core `DiffSpec` ↔ `DiffSpecInput` exact mirror; `ariadne_git::diff(&Path,&DiffSpec)->Result<(Vec<LineHunk>,Vec<String>)>` matches the handler call; graph `diff_blast`/`DiffBlastReport`/`DiffSeed` mirrored field-for-field to the wire + MCP DTOs; `WarmCatalog`/cold `Catalog` carry `symbols`(byte_start/end/file), `path_to_id`, `graph`, `snap`/`root`.
</checks_run>

<findings>
| id | category | severity | location | problem | fix |
|---|---|---|---|---|---|
| F1 | tests | INFO | crates/ariadne-daemon/src/domain/queries/impact.rs:189-287 | The warm-daemon `impact::diff_blast` + `collect_span_sources` have no direct execution test; coverage is the cold golden + a protocol-level parity unit test. | Add a `DaemonQuery::DiffBlast` case to `warm_graph.rs`/`warm_analytics.rs`. Non-blocking: matches tier-15b's audited-PASS precedent (Hotspots/Complexity/CoChange daemon handlers are likewise warm-untested), and exit-criterion 5 explicitly scopes coverage to "cold golden + daemon/cold parity unit test". Logic is mirrored from the tested cold path. |
| F2 | correctness | INFO | crates/ariadne-daemon/src/domain/queries/impact.rs:269 | Daemon `collect_span_sources` swallows a snapshot read error (`cat.snap.file(fid).ok().flatten()`) where the cold path propagates it; a (near-impossible, in-RAM) error degrades the file to `unresolved` instead of erroring. | Acceptable by design — aligns with D3 ("staleness degrades to `unresolved`, never incorrect"). If exactness is wanted, mirror the cold path's `?` propagation. |
</findings>

<verdict>
PASS. Zero FAIL findings. Two INFO nits, neither gating.

All seven `exit_criteria` independently verified:
1. ✓ `diff_blast_radius` registered/discoverable; `DiffBlastInput{spec:DiffSpecInput(WorkingTree default|Commit|RefRange), depth?, kinds?}` (server.rs:530, types.rs:461-523, handshake snapshot).
2. ✓ `ariadne-mcp → ariadne-git` added with ADR/RD7 comment; architecture green incl. new daemon-no-git assertion; ADR-0023 (Accepted) records the asymmetry.
3. ✓ Handler runs `ariadne_git::diff` first, then `try_query_async(DiffBlast{hunks,changed_paths,depth,kinds})` → `project_daemon`, else cold `tools::diff_blast::handle`; daemon builds hash-guarded `FileSymbolSpans` via shared `spans_from` and runs `graph.diff_blast`.
4. ✓ Golden asserts `must∪may == ∪ blast_radius(seeds)` through the live tool; `unresolved` surfaced via tier-14 logic, never dropped.
5. ✓ Spawned-server cold golden over a real fixture git repo w/ uncommitted edit; `diff_blast_arm_matches_cold_output` parity unit test — both pass.
6. ✓ Handshake `EXPECTED_TOOLS=17` + re-accepted snapshots; README (17), CLAUDE.md catalog, and regenerated `codebase-overview.md` updated.
7. ✓ nextest (core/mcp/daemon) + architecture + clippy + fmt (+ doc) all green.

Decisions: D1 (mcp links git / daemon does not, ADR-0023, arch assertion) ✓; D2 (hunks-over-wire `DaemonQuery::DiffBlast`, cold fallback) ✓; D3 (shared `line_starts`+`spans_from` with blake3 guard, CLI refactored onto it, old private `line_starts` removed — no triplication) ✓; D4 (`DiffSpecInput` JsonSchema mirror, core stays schemars-free, default `WorkingTree`) ✓. Hexagonal invariants intact; no smuggled tech (`gix`/`blake3` both in plan tech_inventory). The codebase-overview.md churn (±~585 lines) is a dogfood regeneration (mermaid node-id renumber + new files/counts), not scope creep.
</verdict>

<next_steps>
None required to ship. Optional (non-gating): close F1 by adding a warm-daemon
`DaemonQuery::DiffBlast` test to `warm_graph.rs`, which would also retire the
same gap inherited from tier-15b.
</next_steps>

<sources>
- tier file: .claude/plans/post-v1-roadmap/tier-15c-diff-blast-radius-tool.md
- plan: .claude/plans/post-v1-roadmap/plan.md (RD6, RD7, D-blocks)
- ADR-0023: docs/adr/0023-mcp-git-diff-dependency.md
- graph use case: crates/ariadne-graph/src/diff_blast.rs:81; span_lines.rs:64-93
- daemon handler: crates/ariadne-daemon/src/domain/queries/impact.rs:189-287
- cold tool: crates/ariadne-mcp/src/tools/diff_blast.rs; handler: server.rs:530-567
- golden: crates/ariadne-mcp/tests/tools_diff_blast.rs
- architecture invariant: tests/architecture.rs:141-155
- OWASP Top 10 (input handling reviewed — revspecs go to pure-Rust gix, no shell): https://owasp.org/www-project-top-ten/
- Google eng-practices (code health over perfection): https://google.github.io/eng-practices/review/reviewer/standard.html
</sources>
