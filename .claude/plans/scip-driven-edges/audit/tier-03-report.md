---
tier_id: tier-03
audited: 2026-06-05
verdict: PASS
commit: bc82fbd13db7d59f862d7f9aedb2eb5bfb05df5e
---

<scope>
Adversarial audit of tier-03 (`Implements`/`TypeOf` edges from SCIP relationships;
`EdgeKindFilter` honest by production). Diff is the uncommitted working tree against
HEAD `bc82fbd` (tier-01/02 committed + PASS). 17 files, +644/-35, no untracked files
(fixtures are inline). Scope = the tier's `<files>` plus the necessary plumbing the
plan `<architecture>` calls for. Sibling `plan.md` read for `<decisions>` D1–D6.
</scope>

<checks_run>
- `cargo fmt --all --check` → clean.
- `cargo deny check` → advisories/bans/licenses/sources ok; only pre-existing
  unmatched-license-allowance warnings; **no new dependency** (D5/constraints honored).
- `cargo nextest run --workspace` → **473 passed**, 0 failed, 19 skipped. Includes
  parity (`incremental_sequence_equals_fresh_rebuild`,
  `warm_apply_equals_fresh_rebuild`), old-DB-open
  (`pre_reads_writes_db_reopens_with_old_tags_intact`),
  `architecture_invariants_hold`, and the salsa memory-budget probes
  (`derivation`/`incremental` `over_budget()` → R7 green).
- The 9 tier-03 tests by name → all PASS:
  `implementation_relationship_yields_implements_edge`,
  `type_definition_relationship_yields_typeof_edge`,
  `relationship_to_unindexed_symbol_drops_edge`,
  `extracts_implementation_relationship_with_normalized_keys`,
  `relationship_without_edge_flag_is_dropped`,
  `every_filter_maps_to_a_producible_edge_kind`,
  `previously_empty_filters_now_resolve_to_real_edges`,
  `blast_radius_answers_who_implements_and_typeof`,
  `edge_kind_all_variants_round_trip_with_stable_tags`.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` → clean.
- `cargo test --test architecture` → green (covered in suite as
  `ariadne-workspace::architecture`): `ariadne-salsa ⊥ ariadne-scip` holds; the
  salsa-side `ScipRelationshipRaw` is a local mirror, not an import of the adapter.
- Read end-to-end: records.rs, scip.rs, lib.rs, tags.rs, build.rs (`from_core`,
  `to_flag`), impact.rs (`filter_to_set` + tests), derive.rs
  (`resolve_scip_edges` passes 1–3), derived.rs, db.rs (coverage gate + sort),
  memory.rs, cli `run_scip_ingest`, scip `document_relationships`, and all tests.
</checks_run>

<findings>
| id | category | severity | location | problem | fix |
|---|---|---|---|---|---|
| F1 | plan_adherence | INFO | tier `<files>` vs diff | `<files>` lists `daemon/query.rs` and `storage/redb/**`, neither touched; the actual change touches `cli/domain/mod.rs`, `daemon/queries/impact.rs`, `salsa/{db,derived,lib,memory}.rs`, `graph/tests/synthetic.rs` — none enumerated. All are correct and required by the plan `<architecture>`/`<steps>` (composition root sets the input; `filter_to_set` is the real reconciliation site; new byte tags flow through the generic redb codec, exercised end-to-end by `scip_edges.rs`). | None required — note the `<files>` list under-enumerated the (correct) change set. |
| F2 | correctness | INFO | derive.rs:520-523,538 | A relationship edge's evidence `source_span` pairs `facts.file_id` (the doc declaring the relationship) with the `from` symbol's def range from a global key map that does not record the def's file. Under normal SCIP this coincides (a symbol's `relationships` ride the document that defines it, as the comment states); if an indexer ever emitted a relationship on a non-defining doc, file_id and byte range would mismatch. Edge identity (`src`/`kind`/`dst`) and the parity/determinism guarantees are unaffected — only the span metadata. | Store `(file_id, range)` in `def_range_of_key` and use the def's file for the span, or leave as best-effort (degraded-not-wrong, consistent with R2/R3). |
</findings>

<verdict>
**PASS.** Zero FAIL findings. Two INFO nits, neither gating.

All four `exit_criteria` independently verified:
1. RED→GREEN repros — `implementation_relationship_yields_implements_edge` asserts a
   `Dog -> Animal` `Implements` edge (graph `Overrides`), and
   `type_definition_relationship_yields_typeof_edge` a `binding -> Animal` `TypeOf`
   edge, both read back through real `RedbStorage` (so the new tag round-trips
   end-to-end). The Implements→Overrides graph mapping is pinned in `synthetic.rs`.
2. Derivation `EdgeKind` gains `Implements = 7` / `TypeOf = 8` (append-only stable
   tags); `tags.rs` round-trips every variant, pins 7/8, and asserts `from_byte(9)
   == None`; `from_core` maps Implements→`Overrides`, TypeOf→`TypeOf`; old DBs
   (tags 0–6) reopen (`pre_reads_writes…` green; tags append-only, no migration).
3. Total-mapping honesty: `every_filter_maps_to_a_producible_edge_kind` proves each
   of the 8 `EdgeKindFilter` variants resolves through `filter_to_set` →
   `EdgeKindSet` → a graph kind in `from_core`'s image; the 5 once-empty filters are
   re-checked by `previously_empty_filters_now_resolve_to_real_edges`. `Inherits`
   honestly aliases to `OVERRIDES` (SCIP conflates impl/override/inheritance, D5).
4. `blast_radius` filtered to `OVERRIDES` answers "who implements X"
   (`blast_radius_answers_who_implements_and_typeof`); cold==warm and
   incremental==fresh parity green; deterministic edge set (occurrences sorted by
   range, relationships full-tuple sorted, BTree/`seen` dedup keyed by
   `EdgeKey{src,kind,dst}` so impl/typeof never collide with occurrence edges);
   architecture test green.

Correctness spot-checks held: unmapped endpoint and self-loop drop (no false edge,
no fabrication — kinds emitted only on a present flag); the D4 coverage gate feeds
only hash-current files to the SCIP pass; `document_relationships` keeps only the two
edge-bearing flags and normalizes both endpoints to the same keys the occurrences
use. No smuggled dependency or pattern; hexagonal boundary intact.
</verdict>

<next_steps>
None required for PASS. Optional follow-ups (non-gating): consider F2's
`(file_id, range)` span fix if a non-defining-doc relationship is ever observed;
the implementer may reconcile the tier `<files>` list to the actual change set for
traceability (F1).
</next_steps>

<sources>
- SCIP relationship flags (is_implementation field 3, is_type_definition field 4):
  crates/ariadne-scip/proto/scip.proto:488-512 (cited by tier + plan).
- Two-enum mapping + filter honesty (D5): crates/ariadne-core/src/domain/records.rs:155-214;
  crates/ariadne-graph/src/build.rs:30-114; crates/ariadne-daemon/src/domain/queries/impact.rs:22-117.
- Resolution + determinism + coverage gate (D3/D4): crates/ariadne-salsa/src/derive.rs:438-547;
  crates/ariadne-salsa/src/db.rs:330-370.
- Code-health / reviewer standard: https://google.github.io/eng-practices/review/reviewer/standard.html
</sources>
