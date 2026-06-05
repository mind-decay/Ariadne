---
tier_id: tier-01
title: Activate SCIP occurrence ingest; range-mapped precise References/Imports edges
deps: []
exit_criteria:
  - "A failing test (committed red → green) on a multi-crate fixture: crate A calls an ambiguous-named fn defined in crate B (no import); with SCIP facts present the edge resolves A→B precisely; the same call with no SCIP facts keeps the tree-sitter behaviour"
  - "`ariadne-scip::extract_facts(&proto::Index) -> ScipFactsRaw` exists and is unit-tested; `ScipFactsRaw` is a pure type (no prost/redb), threaded core→salsa via a salsa input; `cargo test --test architecture` green (salsa never imports ariadne-scip)"
  - "A std-callee occurrence (`Vec::new()`, no indexed definition) yields NO edge; a covered file whose content hash no longer matches its SCIP facts falls back to tree-sitter edges (hash-gated coverage, D4)"
  - "After re-index with `--scip`, the dogfood cross-crate edge set recovers genuine calls ADR-0024 dropped (recall up, all true) and `apply_writes` still has no `new` edge; cold==warm and incremental==fresh parity green; same input → identical edge set"
status: pending
---

<context>
SCIP ingest is stubbed: `scip_symbols` returns empty [src: crates/ariadne-salsa/src/derived.rs:167-171],
`ScipDocInput` is always `None` [src: crates/ariadne-salsa/src/db.rs:146,185], so
every edge comes from the tree-sitter scoped resolver [src: crates/ariadne-salsa/src/derive.rs:220-278].
This tier turns SCIP on for EDGES only (Strategy B), keeping tree-sitter symbols
and identity untouched (plan D1). SCIP occurrences carry a globally-resolved
`symbol`, a `range`, and a `symbol_roles` bitset (Definition 0x1, Import 0x2)
[src: crates/ariadne-scip/proto/scip.proto; WebFetch scip.proto this session].

salsa may not import `ariadne-scip` [src: tests/architecture.rs:13-14,31-43], so
the proto is decoded to a pure `ScipFactsRaw` at the composition root and fed via
a salsa input — the `SyntacticFactsRaw`/`SyntacticFactsInput` pattern
[src: crates/ariadne-salsa/src/inputs.rs:66-70; plan D2].

Spike-first: confirm the proto decode site already in hand
[src: crates/ariadne-scip/src/indexer/mod.rs:85 `proto::Index::decode`] and how
the CLI cold path obtains per-file `Document`s, before any production edit.
</context>

<files>
- crates/ariadne-core/src/domain/** — `ScipFactsRaw` pure type: `Vec<ScipOccurrence{ symbol: String, byte_range: (u32,u32), roles: u32 }>` + the indexed content hash; no prost, no redb.
- crates/ariadne-scip/src/lib.rs + a new module — `pub fn extract_facts(&proto::Index) -> Vec<(path, ScipFactsRaw)>`; normalizes each occurrence symbol via `normalize_scip_symbol` so equivalent encodings key equal [src: crates/ariadne-scip/src/normalize/mod.rs:160-162].
- crates/ariadne-salsa/src/inputs.rs — replace `ScipDocInput.raw_proto: Option<Vec<u8>>` with a pure `ScipFactsInput { facts: ScipFactsRaw, indexed_hash }` (or add it) [src: inputs.rs:50-57].
- crates/ariadne-salsa/src/derived.rs — memoized `scip_facts_for_file`; retire the `scip_symbols` empty stub for the edge path (symbols stay tree-sitter, D1) [src: derived.rs:167-204].
- crates/ariadne-salsa/src/derive.rs — `resolve_scip_edges(facts_by_file, ts_symbols)`: build `scip_symbol → SymbolId` from Definition occurrences (range→`enclosing_symbol`), resolve non-def occurrences to `References`, Import-role to `Imports`; reuse `enclosing_symbol`/`span` [src: derive.rs:281-296].
- crates/ariadne-salsa/src/db.rs — in `commit_revision`, for hash-current covered files emit SCIP edges and skip `resolve_edges`; else tree-sitter (D4) [src: db.rs:204-258].
- crates/ariadne-cli/src/domain/mod.rs + crates/ariadne-daemon/** — composition roots: decode `Document`s, call `extract_facts`, set the input.
- crates/ariadne-salsa/tests/scip_edges.rs + a multi-crate fixture — the repro.
</files>

<steps>
1. SPIKE (no prod edit): trace a `Document`'s occurrences from `proto::Index::decode`
   [src: indexer/mod.rs:85] to where the CLI commits a revision. Build the
   multi-crate fixture (A calls B's ambiguous-named fn, no import). Commit a RED
   test asserting the precise A→B edge that the bare-name resolver cannot produce.
2. Define `ScipFactsRaw` (pure) in core and `extract_facts` in `ariadne-scip`
   (walk `documents[].occurrences`, normalize `symbol`, keep `range`+`symbol_roles`).
   Unit-test extraction on a checked-in `.scip` fixture.
3. Add the pure salsa input (D2) + memoized `scip_facts_for_file`; wire both
   composition roots to populate it after the indexers run.
4. Implement `resolve_scip_edges` (D3): Definition occ → `enclosing_symbol` gives
   `scip_symbol → SymbolId`; each other occ → edge `src`=enclosing ts symbol,
   `dst`=map lookup; drop unmapped `dst` / missing `src` / self-loop; sort
   occurrences by `(file, range)` for determinism.
5. Gate in `commit_revision` (D4): covered ⟺ SCIP facts present AND
   `indexed_hash == file content hash`; covered ⇒ SCIP edges, skip `resolve_edges`;
   else tree-sitter. Turn the repro GREEN; add the std-callee no-edge case and the
   edited-file → tree-sitter-fallback case.
6. Re-index the repo with `--scip`; assert recovered true cross-crate edges, no
   `apply_writes → new`. Run the full suite + parity + determinism; report
   `memory_report()` delta.
</steps>

<verification>
- `cargo nextest run --workspace` → repro + std-callee + hash-fallback tests green;
  navigation / find_references / blast_radius / cold==warm / incremental==fresh
  unchanged-green (legitimate edges not dropped).
- `cargo test --test architecture` green (salsa has no `ariadne-scip` dep).
- Dogfood `cargo run -p ariadne-cli -- index --scip` then inspect via
  `find_references` / graph: genuine cross-crate calls present, `apply_writes` has
  no `new` edge; index twice → identical edge set.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`;
  `cargo fmt --all --check`; `cargo deny check` (no new dep); `memory_report()`
  delta < budget (R7).
</verification>

<rollback>
`git checkout --` the new core type, `extract_facts`, the salsa input/query,
`resolve_scip_edges`, the `commit_revision` gate, and the test/fixture. The
`ScipDocInput`→`ScipFactsInput` swap is the only input-shape change; reverting it
restores the `None`/stub path (today's behaviour). If the tier overruns, keep the
RED repro `#[ignore]`d with this slug and revert production code.
</rollback>
