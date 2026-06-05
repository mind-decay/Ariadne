---
tier_id: tier-01
title: Activate SCIP occurrence ingest; range-mapped precise References/Imports edges
deps: []
exit_criteria:
  - "A committed RED→GREEN test on a multi-crate fixture: crate A calls an ambiguous-named Method/Path fn defined in crate B (no import); with SCIP facts present the edge resolves A→B precisely; with no SCIP facts it keeps the shape-gated tree-sitter behaviour (no edge, ADR-0025)"
  - "`ariadne-scip::extract_facts(&IngestReport) -> Vec<(path, ScipFacts)>` exists and is unit-tested; `ScipFacts` is a pure core type (no prost/redb), threaded core→salsa via a salsa input (whose field is the `ScipFactsRaw` mirror, per the `SyntacticFacts`/`SyntacticFactsRaw` precedent); `cargo test --test architecture` green (salsa never imports ariadne-scip)"
  - "A std-callee occurrence (`Vec::new()`, no indexed definition) yields NO edge; a covered file whose content hash no longer matches its SCIP facts falls back to the precise resolver (hash-gated coverage, D4)"
  - "After re-index, the dogfood cross-crate edge set recovers genuine Method/Path calls ADR-0025 dropped (recall up, all true) and `apply_writes` still has no `new` edge; cold==warm and incremental==fresh parity green; same input → identical edge set"
status: completed
completed: 2026-06-05
---

<context>
SCIP ingest is stubbed: `--scip` runs `IngestPlan` then discards the report
[src: crates/ariadne-cli/src/domain/mod.rs:155,253-258]; `scip_symbols` returns
empty [src: crates/ariadne-salsa/src/derived.rs:171-178]; `ScipDocInput.raw_proto`
is always `None` [src: inputs.rs:50-58]. So every edge comes from the now-precise
shape-gated resolver [src: crates/ariadne-salsa/src/derive.rs:236-316], which
abstains on Method/Path callees with no same-file def — dropping real cross-crate
recall [src: ADR-0025; r1-resolver-completion plan.md D1,D6].

This tier turns SCIP on for EDGES only (Strategy B), keeping tree-sitter symbols and
identity untouched (plan D1). SCIP occurrences carry a globally-resolved `symbol`, a
`range`, and a `symbol_roles` bitset — Definition `0x1`, Import `0x2`
[src: crates/ariadne-scip/proto/scip.proto:526-528,645-680]. salsa may not import
`ariadne-scip` [src: tests/architecture.rs:13-14,31-43], so the proto is decoded to a
pure `ScipFacts` at the composition root and fed via a salsa input — the
`SyntacticFactsInput` pattern [src: inputs.rs:60-71; plan D2].
</context>

<files>
- crates/ariadne-core/src/domain/** — `ScipFacts` pure type: `Vec<ScipOccurrence{ symbol: String, byte_range: (u32,u32), roles: u32 }>` + indexed content hash; no prost, no redb.
- crates/ariadne-scip/src/lib.rs + a new module — `pub fn extract_facts(&IngestReport) -> Vec<(path, ScipFacts)>`; normalize each occurrence symbol via `normalize_scip_symbol` so equivalent encodings key equal [src: crates/ariadne-scip/src/normalize/mod.rs:160-162].
- crates/ariadne-salsa/src/inputs.rs — replace `ScipDocInput.raw_proto: Option<Vec<u8>>` with a pure `ScipFactsInput { facts: ScipFactsRaw, indexed_hash }` [src: inputs.rs:50-58].
- crates/ariadne-salsa/src/derived.rs — memoized `scip_facts_for_file`; retire the `scip_symbols` empty stub for the edge path (symbols stay tree-sitter, D1) [src: derived.rs:171-211].
- crates/ariadne-salsa/src/derive.rs — `resolve_scip_edges(facts_by_file, ts_symbols)`: build `scip_symbol → SymbolId` from Definition occurrences (range→enclosing symbol), resolve non-def occurrences to `References`, Import-role to `Imports`; reuse `enclosing_symbol`/`span` [src: derive.rs:236-316].
- crates/ariadne-salsa/src/db.rs — in `commit_revision`, for hash-current covered files emit SCIP edges and skip `resolve_edges`; else the precise resolver (D4) [src: db.rs:329-336].
- crates/ariadne-cli/src/domain/mod.rs + crates/ariadne-daemon/** — composition roots: pass the `IngestReport` to `extract_facts`, set the input (default run path lands in T4; here wire it behind the existing `--scip`).
- crates/ariadne-salsa/tests/scip_edges.rs + a multi-crate fixture — the repro.
</files>

<steps>
1. SPIKE (no prod edit): trace a `Document`'s occurrences from `IngestPlan::ingest`
   [src: indexer/mod.rs:85; plan.rs] to the CLI commit. Build the multi-crate
   fixture (A calls B's ambiguous-named Method/Path fn, no import). Commit a RED test
   asserting the precise A→B edge the shape-gated resolver now refuses (ADR-0025).
2. Define `ScipFacts` (pure) in core and `extract_facts` in `ariadne-scip` (walk
   `documents[].occurrences`, normalize `symbol`, keep `range`+`symbol_roles`).
   Unit-test extraction on a checked-in `.scip` fixture.
3. Add the pure salsa input (D2) + memoized `scip_facts_for_file`; wire both
   composition roots to populate it from the report after the indexers run.
4. Implement `resolve_scip_edges` (D3): Definition occ → `enclosing_symbol` gives
   `scip_symbol → SymbolId`; each other occ → edge `src`=enclosing ts symbol,
   `dst`=map lookup; drop unmapped `dst` / missing `src` / self-loop; sort
   occurrences by `(file, range)` for determinism.
5. Gate in `commit_revision` (D4): covered ⟺ SCIP facts present AND
   `indexed_hash == file content hash`; covered ⇒ SCIP edges, skip `resolve_edges`;
   else the precise resolver. Turn the repro GREEN; add the std-callee no-edge case
   and the edited-file → resolver-fallback case.
6. Re-index with `--scip`; assert recovered true cross-crate edges, no
   `apply_writes → new`. Run the full suite + parity + determinism; report
   `memory_report()` delta.
</steps>

<verification>
- `cargo nextest run --workspace` → repro + std-callee + hash-fallback tests green;
  navigation / find_references / blast_radius / cold==warm / incremental==fresh
  unchanged-green (legitimate edges not dropped).
- `cargo test --test architecture` green (salsa has no `ariadne-scip` dep).
- Dogfood `cargo run -p ariadne-cli -- index --scip` then inspect via
  `find_references`: genuine cross-crate Method/Path calls present, `apply_writes`
  has no `new` edge; index twice → identical edge set.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`;
  `cargo fmt --all --check`; `cargo deny check` (no new dep); `memory_report()`
  delta < budget (R7).
</verification>

<rollback>
`git checkout --` the new core type, `extract_facts`, the salsa input/query,
`resolve_scip_edges`, the `commit_revision` gate, and the test/fixture. The
`ScipDocInput`→`ScipFactsInput` swap is the only input-shape change; reverting it
restores the stub path (today's behaviour). If the tier overruns, keep the RED
repro `#[ignore]`d with this slug and revert production code.
</rollback>

<remediation>
Resolves audit `tier-01-report.md` (FAIL @ `6011ea2`), verification half of step 6.
- **F1** — re-baselined the two stale cold-parity goldens
  `crates/ariadne-cli/tests/goldens/parity_{java,csharp}.txt` via
  `UPDATE_GOLDENS=1`. Each lost exactly one `References` (tag 1) edge:
  `run→helper` (`Caller.run` calling cross-file `Callee.helper()`) and
  `Run→Helper` (`Callee.Helper()`). Both are Path/Method-qualified, cross-file,
  no import, no same-file def — the intended ADR-0025 abstention. Goldens were
  written once in `16520b9`, before the abstention landed in `985116d`/`97f122a`;
  not a tier-01 regression (audit reproduced both at HEAD with no diff). Parity
  indexes without `--scip`, so the syntactic-only goldens now match the precise
  resolver. R4 edge delta: −1 edge per fixture; files/symbols unchanged. The
  bare-identifier fixtures (rust/go/ts/python/c) keep their edge — free-call
  resolution is retained.
- **F2** — `cargo nextest run --workspace` now green (460/460, 19 skipped);
  fmt/clippy/deny clean, no new dep. `status: completed` re-asserted truthfully.
- **I1** — reconciled exit-criterion/`<files>`/`<steps>` wording: the pure core
  type is `ScipFacts` (impl), `ScipFactsRaw` is the salsa mirror field type
  (`SyntacticFacts`/`SyntacticFactsRaw` precedent). No code change.
</remediation>
