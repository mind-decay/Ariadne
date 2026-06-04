---
tier_id: tier-02
title: R1 — scope index-time callee resolution; stop name-collision phantom edges
deps: []
exit_criteria:
  - "A failing test on a synthetic multi-crate fixture reproduces name-collision mis-resolution (a call to a name defined in ≥2 crates binds to a symbol outside the caller's resolution scope); committed red → green"
  - "The index-time callee→SymbolId resolver is located and its file:line recorded in an ADR under docs/adr/NNNN-scoped-call-resolution.md (status Accepted)"
  - "Resolution is scoped: a callee resolves to an in-scope definition (same file → same crate → import-visible) when one exists; a callee with no in-scope definition (e.g. std `Vec::new`) yields NO edge — both asserted by tests"
  - "After re-index, `apply_writes` carries no outbound edge to a `new` defined in another crate (asserted via the graph or an equivalent fixture)"
  - "`cargo test --test architecture` green; warm==cold parity green; existing navigation/reference tests unchanged-green; same input → identical edge set (determinism)"
status: pending
---

<context>
Root cause R1: a callee is captured as bare identifier text
[src: crates/ariadne-parser/src/adapters/treesitter/facts.rs:125 `callee: String`],
and the index-time linker that turns that text into a target `SymbolId` binds it
to a same-named workspace symbol regardless of scope, so every `Vec::new()` /
`.build()` collapses onto one arbitrary symbol. PROOF: `apply_writes`
(`crates/ariadne-storage/src/adapters/redb/apply.rs`) calls only std `Vec::new()`
yet the graph has `apply_writes → new` (cross-crate); the architecture test is
green so no real Cargo-level adapter→adapter dep exists [src: plan.md R1].

`SymbolId = blake3(canonical SCIP symbol)`, so SCIP-resolved targets are precise;
the phantom comes from name-based linking, NOT from `SymbolId` hashing and NOT
from the query-side `find_symbol` first-match (which resolves user queries, not
edges) [src: crates/ariadne-scip/src/normalize/mod.rs:101,145-149;
crates/ariadne-daemon/src/domain/catalog.rs:253-254].

This tier is spike-first: the exact linker locus (SCIP ingestion vs tree-sitter
fact-linking) is confirmed before any edit. If the scoped-resolution fix proves
larger than one session, narrow this tier to located + failing test + ADR and
spawn a dedicated R1-implementation plan (tier-03 then blocks on it)
[src: plan.md `<risks>`].
</context>

<files>
- docs/adr/NNNN-scoped-call-resolution.md — new ADR: the phantom mechanism, the
  located resolver file:line, the chosen scoping rule, rejected alternatives.
- crates/ariadne-scip/src/** and/or crates/ariadne-parser/src/** — the resolver
  located in the spike (exact path recorded in the ADR); scope the callee→target
  match and emit no edge when no in-scope definition exists.
- crates/ariadne-core/src/domain/** — only if the resolver needs scope context
  (file/crate/import set) it does not already carry; confirm in the spike.
- a new test + fixture under the owning crate's `tests/` — the multi-crate
  collision repro and the std-callee no-edge case.
</files>

<steps>
1. SPIKE (no production edit): trace one call's path from `@call.callee` text to
   the persisted `edges_added` dst `SymbolId`. Pin the resolver file:line. Build
   the minimal fixture that reproduces a wrong bind (two crates define `helper`;
   crate A calls its own `helper`; assert the edge currently targets the wrong
   crate's symbol). Commit the test RED.
2. Record findings + the chosen rule in the ADR. Rule (language-agnostic intent):
   resolve a callee to a definition in scope precedence same-file → same-crate →
   import-visible module; on zero in-scope matches, emit no edge (do not bind to
   an arbitrary global same-name symbol). No symbol-name denylist [src: plan.md
   D2; god-module-suggestion-fix D2].
3. Implement the scoped resolution at the located resolver. Keep `SymbolId`
   derivation and SCIP-precise edges untouched [src: normalize/mod.rs:145-149].
4. Turn the repro test GREEN; add the std-callee case (`Vec::new()` → no edge).
   Re-index the repo (or an equivalent fixture) and assert `apply_writes` has no
   cross-crate `new` edge.
5. Run the full suite; confirm navigation/reference/blast-radius tests still pass
   (scoping must not drop legitimate same-name resolutions) and warm==cold holds.
</steps>

<verification>
- `cargo nextest run --workspace` → repro + std-callee tests green; navigation,
  find_references, blast_radius, warm==cold parity unchanged-green.
- `cargo test --test architecture` green.
- Re-index dogfood (`cargo run -p ariadne-cli -- index` then inspect via
  `find_references`/graph) → no `apply_writes` → cross-crate `new` edge.
- Determinism: index the same input twice → identical edge set (existing
  insertion-order/determinism tests stay green).
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`;
  `cargo fmt --all --check`; `cargo deny check` (no new dep).
</verification>

<rollback>
`git checkout -- <resolver files recorded in ADR> <new test/fixture> docs/adr/NNNN-*.md`.
If the tier narrows to spike-only (fix deferred to a new plan), keep the ADR +
RED repro test (marked `#[ignore]` with the tracking plan slug) and revert no
production code; tier-03 then declares a dep on the new R1 plan.
</rollback>
