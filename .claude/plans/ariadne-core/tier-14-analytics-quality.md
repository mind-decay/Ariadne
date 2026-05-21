---
tier_id: tier-14
title: Analytics-quality fixes — blast-radius empty/absent disambiguation + god-module signal denoise
deps: [tier-13]
exit_criteria:
  - "`GraphIndex::blast_radius` returns `Option<BlastRadius>`: `None` for a `SymbolId` absent from the graph node set, `Some(_)` (radius possibly empty) for a present symbol. A failing-first graph test covers both arms."
  - "`BlastRadiusOutput` carries a `symbol: SymbolSummary` echoing the resolved target; an MCP integration test asserts a resolved symbol with zero inbound edges returns the echoed symbol plus empty `must_touch`/`may_touch`."
  - "`weak_spots` god-module detection excludes files under `tests/`, `benches/`, `examples/` and any `build.rs`; a failing-first integration test asserts a non-library-target file with high efferent coupling is excluded while a library file with high efferent coupling is kept."
  - "A dogfood re-run records the `weak_spots.god_modules` count before/after exclusion in the tier-14 audit; `GOD_THRESHOLD` is raised only if the post-exclusion count is still noisy, with the new value justified by that measurement."
  - "`WeakSpotsOutput.dead_symbols` rustdoc records the known syntactic-only false positives and points at the `--scip` path; no behavioural change to `dead_code`."
  - "`cargo build --workspace`, `clippy -D warnings`, `fmt --check`, `cargo test --test architecture`, `cargo nextest run --workspace`, `RUSTDOCFLAGS=-D warnings cargo doc`, `cargo bench --workspace --no-run` all green."
status: completed
completed: 2026-05-21
---

<context>
Post-v1 analytics-quality tier. Dogfooding the shipped v1 on 2026-05-21
(`ariadne index` + MCP queries against Ariadne's own repo: 202 files /
2032 symbols / 1889 edges) surfaced two heuristic/UX defects. Neither is a
correctness bug — the tier-13 SLO release gate is green and out of scope.

F1 — `blast_radius` returns `{must_touch:[], may_touch:[], depth_used:0}`
for a symbol that exists but has no inbound edges, indistinguishable from
the graph-level result for an absent symbol: `GraphIndex::blast_radius`
collapses both the `index` miss and the empty-`preds` path into
`BlastRadius::default()` [src: crates/ariadne-graph/src/blast.rs:58-64]. A
caller cannot tell "no dependents" from "not analysed".

F2 — `weak_spots.god_modules` flagged ~40 of 202 modules (~20%), including
`tests/*.rs` and `benches/*.rs`. The signal is `coupling.rows` filtered by
`efferent > GOD_THRESHOLD` (8), one module per file
[src: crates/ariadne-mcp/src/tools/weak_spots.rs:9,21-30 ;
crates/ariadne-mcp/src/tools/coupling_report.rs:27-50]. A 20% hit rate is
noise, not an actionable tail.

Scope confirmed by reading the crates. F1 touches `ariadne-graph`
(`BlastRadius`, `blast_radius`) and `ariadne-mcp` only — both are
use-case / driving-adapter API, **not an `ariadne-core` port**, so no ADR
is owed [src: crates/ariadne-graph/src/lib.rs:21-29 — `BlastRadius` is
re-exported from `ariadne-graph`]. F2 lives entirely in `ariadne-mcp`; the
brief's "start from heuristics.rs" is inaccurate — `heuristics.rs` feeds
only `refactor.rs`/`docgen.rs`, never the `weak_spots` god-module path.
`dead_symbols` false positives are OUT OF SCOPE (correct behaviour for a
syntactic-only graph; `--scip` is the semantic path) — documented only. No
new dependency, crate, or cross-crate edge. Full context: plan.md.
</context>

<files>
- crates/ariadne-graph/src/blast.rs — `blast_radius` → `Option<BlastRadius>`;
  `None` on the `index` miss, `Some(_)` otherwise (empty `preds` included).
- crates/ariadne-graph/tests/golden_repo.rs — unwrap the `Option` in
  `golden_blast_radius_user_struct`; NEW test for the absent/empty arms.
- crates/ariadne-graph/benches/blast.rs — unwrap the `Option` in the bench loop.
- crates/ariadne-mcp/src/types.rs — `BlastRadiusOutput` gains `symbol: SymbolSummary`.
- crates/ariadne-mcp/src/tools/blast_radius.rs — populate `symbol`; map a
  graph `None` to `McpError::NotFound` (catalog/graph desync guard).
- crates/ariadne-mcp/src/tools/weak_spots.rs — `is_library_target` filter on
  the god-module candidate rows; conditional `GOD_THRESHOLD` change.
- crates/ariadne-mcp/tests/support.rs — NEW `seed_god_module_project` fixture.
- crates/ariadne-mcp/tests/tools_weak_spots.rs — NEW god-module exclusion test.
- crates/ariadne-mcp/tests/tools_blast_radius.rs — NEW resolved-but-empty test.
</files>

<steps>
1. **F1 failing test (graph).** In `golden_repo.rs`, add a test that builds a
   fresh `GraphIndex` with two symbols + one `A→B` edge via `add_edge`
   [src: crates/ariadne-graph/src/build.rs:168] and asserts: `blast_radius`
   on a `SymbolId` never inserted returns `None`; `blast_radius` on `A` (a
   node with zero inbound edges) returns `Some` with empty `must_touch`/
   `may_touch`. Fails to compile first — return type is `BlastRadius`.

2. **F1 graph impl.** Change `GraphIndex::blast_radius` to
   `-> Option<BlastRadius>`: the `index` miss returns `None`; a present
   symbol returns `Some(_)`, and the `preds.is_empty()` path returns
   `Some(BlastRadius::default())` — a true "no dependents" answer. Rewrite
   the rustdoc to state the `None`/`Some` contract
   [src: crates/ariadne-graph/src/blast.rs:53-86].

3. **F1 fix graph callers.** Update every in-workspace caller of
   `blast_radius` (`cargo build --workspace` is the backstop): in
   `golden_repo.rs` `golden_blast_radius_user_struct` use
   `.expect("sid(5) present")` — `sid(5)` has predecessors so the snapshot
   content is unchanged [src: crates/ariadne-graph/tests/golden_repo.rs:159-162];
   in `benches/blast.rs` unwrap the `Option` inside the timed loop
   [src: crates/ariadne-graph/benches/blast.rs — `graph.blast_radius(*s, 3, …)`].

4. **F1 failing test (MCP).** In `tools_blast_radius.rs`, add a test:
   `blast_radius` on `crate::main` — the canonical fixture's only fan_in=0
   symbol [src: crates/ariadne-mcp/tests/tools_weak_spots.rs:44-46] — returns
   JSON whose `symbol` object resolves to `crate::main` and whose
   `must_touch`/`may_touch` are empty. Fails first: `BlastRadiusOutput` has
   no `symbol` field.

5. **F1 MCP impl.** In `types.rs` add `symbol: SymbolSummary` to
   `BlastRadiusOutput` with a `///` doc line [src: crates/ariadne-mcp/src/types.rs:102-111].
   In `tools/blast_radius.rs` populate it via `summarize(cat, id)`
   [src: crates/ariadne-mcp/src/tools/mod.rs:30]; when
   `cat.graph.blast_radius(id, …)` is `None`, return `McpError::NotFound`
   (defensive — `build_from_snapshot` adds every symbol as a node, so this
   is currently unreachable [src: crates/ariadne-graph/src/build.rs:217-221]).

6. **F2 failing test.** Add `seed_god_module_project()` to MCP `support.rs`:
   one library file `src/hub.rs` and one non-library-target file
   `tests/big_suite.rs`, each holding a symbol whose outgoing edges reach
   ≥9 distinct external symbols (efferent > `GOD_THRESHOLD` = 8
   [src: crates/ariadne-mcp/src/tools/weak_spots.rs:9]). In
   `tools_weak_spots.rs`, add a test asserting `god_modules` contains
   `src/hub.rs` and does **not** contain `tests/big_suite.rs`. Fails first —
   the tool flags both [src: crates/ariadne-mcp/src/tools/weak_spots.rs:18-30].

7. **F2 impl.** In `weak_spots.rs` add a private
   `is_library_target(path: &str) -> bool` — false when any `/`-split path
   component equals `tests`, `benches`, or `examples`, or the file name is
   `build.rs` (Cargo target conventions; rustdoc states the basis and that
   per-language target classification is future work). Filter the
   god-module candidate rows through it before the `efferent` test
   [src: crates/ariadne-mcp/src/tools/weak_spots.rs:18-30]. `coupling_report`
   stays untouched — exclusion is scoped to the `weak_spots` signal.

8. **F2 measure + threshold.** Run `ariadne index` on the Ariadne repo, then
   invoke the `weak_spots` MCP tool; record the `god_modules` count
   before/after exclusion in the tier-14 audit. If the post-exclusion count
   is still not a small actionable tail, raise `GOD_THRESHOLD` and cite the
   measured before/after count as the justification; otherwise leave it at
   8. If the const changes, update the step-6 fixture/assertion to match.

9. **dead_symbols doc note.** Extend the rustdoc on
   `WeakSpotsOutput.dead_symbols` [src: crates/ariadne-mcp/src/types.rs:213-214]
   to record the known syntactic-only false positives (`#[test]` functions,
   `build.rs::main`, serde-derived structs) and point at the `--scip`
   semantic path. Doc only — no change to `dead_code` behaviour.

10. **Verify.** Run the full gate listed below. Confirm
    `handshake__tools_list.snap` is unchanged — rmcp `#[tool]` advertises the
    input schema only, never the output type
    [src: https://docs.rs/rmcp/1.7.0/rmcp/attr.tool.html] — and the
    `golden_repo` blast snapshot is unchanged. Record the F1 + F2 dogfood
    observations in the tier-14 audit.
</steps>

<verification>
- `cargo build --workspace` — clean; catches any missed `blast_radius` caller.
- `cargo nextest run --workspace` — green: the step-1/4/6 failing tests now
  pass; `tools_blast_radius`, `tools_weak_spots`, `golden_repo` still pass.
- `cargo test --test architecture` — green: no new cross-crate edge;
  `ariadne-graph` and `ariadne-mcp` are already wired, no new dependency.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`,
  `cargo fmt --all --check`, `RUSTDOCFLAGS=-D warnings cargo doc --workspace
  --no-deps --document-private-items` — clean (new `symbol` field + revised
  `blast_radius` rustdoc satisfy `#![deny(missing_docs)]`).
- `cargo bench --workspace --no-run` — `benches/blast.rs` compiles against
  the `Option<BlastRadius>` signature.
- Snapshots: `golden_repo__golden_blast_radius_user_struct.snap` content
  unchanged (`sid(5)` has predecessors → `Some`); `handshake__tools_list.snap`
  unchanged (output schema is never advertised). Any unexpected snapshot
  drift fails loud — never accept via `cargo insta accept` without cause.
- Dogfood, recorded in the tier-14 audit: `blast_radius` on `FactExtractor`
  now returns an output whose `symbol` field proves the symbol resolved;
  `weak_spots.god_modules` count is a small actionable tail with no
  `tests/`/`benches/` entries. A still-noisy count after exclusion is
  root-caused (threshold raised with the measurement cited), not silenced.
</verification>

<rollback>
`git revert` the `blast.rs` signature change, the `types.rs` field, the
`weak_spots.rs` `is_library_target` filter (and any `GOD_THRESHOLD` change),
the `dead_symbols` rustdoc, and the test/fixture additions. All changes are
behavioural or wire-additive only — no on-disk format, no `SCHEMA_VERSION`,
no MCP input schema is touched, so reverting needs no data migration and
leaves a correct, slightly-less-precise analytics surface.
</rollback>
