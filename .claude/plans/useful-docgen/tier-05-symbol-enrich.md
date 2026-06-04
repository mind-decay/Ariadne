---
tier_id: tier-05
title: Symbol doc enrichment (doc_for / DocForReport)
deps: [tier-01]
exit_criteria:
  - "DocForReport carries additive enrichment fields (role, file_risk = churn x complexity, blast_must, blast_may) and all existing fields are unchanged in name and order"
  - "daemon doc_for, mcp doc_for renderer, and cli doc_for renderer all populate/display the new fields"
  - "public_refs are scope-filtered through DocScope (no fixture/test neighbours)"
  - "cold (cli/mcp) and warm (daemon) DocForReport are identical for a sample symbol (parity)"
  - "structured-output test on DocForReport green; cargo clippy/fmt/deny/architecture green"
status: completed
completed: 2026-06-04
---

<context>
The symbol surface returns a structured `DocForReport` rendered client-side, not Markdown
[src: crates/ariadne-daemon/src/domain/queries/docs.rs:18-50;
crates/ariadne-core/src/domain/daemon/response.rs:71-82]. Enrich it with deterministic, system-only
context: a role one-liner, the file's churn×complexity risk, and blast-radius summary counts —
additive only, so no consumer breaks. Both catalogs already carry `churn` + per-symbol
`complexity`, so `file_risk` is computable on the cold and warm paths alike, preserving parity
[src: crates/ariadne-mcp/src/catalog.rs:50,84-92; crates/ariadne-daemon/src/domain/catalog.rs:147-152].
Consumes tier-01 scope. Full context: plan.md.
</context>

<files>
Actual touch set (audit F1 corrected the original list, which named `codec.rs`/`query.rs` —
never touched — and omitted the graph helpers, `types.rs`, and the test files):
- crates/ariadne-core/src/domain/daemon/response.rs — `DocForReport` appends `role: String`,
  `file_risk: Option<f32>`, `blast_must: u32`, `blast_may: u32` after the stable
  `signature`/`kind`/`file`/`brief`/`public_refs` prefix [src: CLAUDE.md D13 — core owns the type].
- crates/ariadne-graph/src/{hotspot.rs,doc_model.rs,lib.rs} — pure use cases `file_risk` (churn×complexity)
  and `symbol_role` (kind + hexagonal layer) + façade re-export [src: plan.md tier-05 D6 — graph-pure helper].
- crates/ariadne-daemon/src/domain/queries/docs.rs — `doc_for` enrichment + `file_complexity`; captures
  `must_touch`/`may_touch` lengths as `blast_must`/`blast_may`; `public_refs` scoped via `DocScope.include`.
- crates/ariadne-mcp/src/tools/doc_for.rs — cold handler computes the same fields from the cold `Catalog`.
- crates/ariadne-mcp/src/types.rs — `DocForOutput` DTO mirror (field-for-field for parity).
- crates/ariadne-mcp/src/server.rs — `doc_for` response arm + parity unit test.
- crates/ariadne-mcp/tests/{tools_doc_for.rs,support.rs}, crates/ariadne-daemon/tests/warm_analytics.rs,
  crates/ariadne-graph/tests/hotspot.rs — structured + parity + helper-regression tests.
- NOT touched (additive serialization needs neither): crates/ariadne-daemon/src/adapters/codec.rs
  (whole-struct postcard, no field-by-field mirror) and crates/ariadne-cli/src/commands/query.rs
  (renders via field-agnostic `json(&report)`).
</files>

<steps>
1. Write a failing test in `tests/tools_doc_for.rs`: assert `DocForReport` now exposes `role`,
   `file_risk`, `blast_must`, `blast_may`; that a fixture neighbour is absent from `public_refs`;
   that pre-existing fields are unchanged; and that cold and warm reports are equal for one symbol.
2. Extend the `DocForReport` core type with the additive fields, mirrored field-for-field by the
   `DocForOutput` DTO so cold/warm JSON stays parity-equal — keep serialization backward-tolerant.
   `codec.rs` is NOT touched: it postcards the whole `DaemonResponse`, so additive fields flow with no
   field-by-field mirror [src: crates/ariadne-mcp/src/types.rs — DocForOutput mirror; audit F1].
3. In `doc_for`: derive `role` (deterministic from `meta.kind` + owning-module coupling shape);
   set `file_risk` from the shared helper (`None` only when `churn` is empty); pass the existing
   `must_touch`/`may_touch` lengths as `blast_must`/`blast_may` [src: docs.rs:30-41].
4. Filter `public_refs` (currently the blast-radius `must_touch ∪ may_touch`, `Vec<SymbolSummary>`
   from core rows.rs:9) via `DocScope.include` so fixture/test neighbours drop.
5. Update the mcp `doc_for` renderer (tools/doc_for.rs + server.rs) and the cli `query doc_for`
   renderer to display the new fields.
6. Confirm cold (cli/mcp) and warm (daemon) routes produce equal structured output (parity).
</steps>

<verification>
- `cargo nextest run -p ariadne-mcp -p ariadne-daemon -p ariadne-cli` → doc_for tests green.
- `cargo test --test architecture` (core still owns the type; adapters only render) [src: CLAUDE.md D13].
- `cargo clippy … -D warnings`; `cargo fmt --all --check`; `cargo deny check`.
- Parity: assert cold and warm `DocForReport` are identical for a sample symbol.
</verification>

<rollback>
`git checkout -- crates/ariadne-core crates/ariadne-daemon crates/ariadne-mcp crates/ariadne-cli`.
Additive fields mean reverting is a clean type rollback; no stored-data migration involved.
</rollback>

<amendments>
Post-audit follow-up resolving the three INFO findings in `audit/tier-05-report.md`
(user-authorized; none gated the PASS verdict). Behavioural deltas re-verified against
`<verification>` and exit criteria EC2/EC4.
- F1 (plan_adherence): `<files>` rewritten above to match the real touch set.
- F2 (correctness): the `doc_for` blast call moved from depth 1 to depth 3 (`DOC_BLAST_DEPTH`,
  the `blast_radius` tool default) on both the warm (`docs.rs`) and cold (`tools/doc_for.rs`)
  paths, so `blast_may` carries the transitive (non-funnel) callers instead of being structurally
  `0`. `public_refs` now takes the must-touch (funnel) set only — at depth 1 `may_touch` was empty,
  so dropping the old `.chain(may_touch)` keeps `public_refs` and `blast_must` identical while
  `blast_may` gains signal. `blast_must`/`blast_may`/`public_refs` field docs reworded to the
  dominator/depth-3 semantics. Parity preserved (both paths share `DOC_BLAST_DEPTH`).
- F3 (performance): `ariadne_graph::file_risk` now scores the queried file directly — max-normalizing
  churn × complexity over the churn set exactly as `file_hotspots`/`rank` do — instead of ranking and
  sorting every file then searching one row. Output is byte-identical (locked by
  `file_risk_matches_ranked_score`); only the O(files·log files) sort + report allocation is dropped.
  The O(symbols) `file_complexity` build is retained — `max_complexity` needs it and it carries no sort.

Re-audit follow-up resolving the two INFO findings in the second-pass `audit/tier-05-report.md`
(user-authorized; neither gated the PASS verdict). Status stays `completed` — both deltas re-verified
against `<verification>` and EC1–EC5.
- F1 (docs): `<steps>` step 2 reworded above to drop the stale "`codec.rs` DTO mirror" clause and name
  the real mirror — the `DocForOutput` DTO in `mcp/src/types.rs` — matching the amended `<files>` and
  the code (`codec.rs` postcards the whole `DaemonResponse`, untouched). Documentation only.
- F2 (performance): both `file_complexity` builders (warm `docs.rs`, cold `tools/doc_for.rs`) now scope
  accumulation to the churn-set paths (a `HashSet<&str>` over `cat.churn`), since `file_risk` ranks only
  churn files and never reads a non-churn entry — so the dead per-file allocation for non-churn files
  (e.g. fixtures with symbols but no Git history) is dropped. Byte-identical: every churn-path lookup
  returns the same sum, so `file_risk`, `public_refs`, blast counts, and cold/warm parity are unchanged
  (proven by `doc_for_matches_cold` + `doc_for_arm_matches_cold_output`). This is the audit's "scope the
  build" remedy; the per-call O(symbols) scan stays, so a full catalog-level cache remains the ≥10 perf-
  gate option if `doc_for` p95 breaches the <100ms budget at 100K files.
</amendments>
