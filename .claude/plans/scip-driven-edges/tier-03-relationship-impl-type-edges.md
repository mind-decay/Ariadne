---
tier_id: tier-03
title: Implements/TypeOf edges from SCIP relationships; reconcile EdgeKindFilter to producible kinds
deps: [tier-02]
exit_criteria:
  - "A failing test (red → green): a trait/interface-impl fixture yields an `Implements` edge between impl and trait symbols; a typed-binding fixture yields a `TypeOf` edge from the binding to its type symbol"
  - "`EdgeKind` gains `Implements = 7`, `TypeOf = 8` with `to_byte`/`from_byte` round-trip; old redb files (tags 0–6) still open"
  - "Every `EdgeKindFilter` variant maps to a producible `EdgeKind` — asserted by a total-mapping test; any advertised kind SCIP cannot supply is removed or explicitly aliased (closes the advertised-but-unproducible gap, plan D5)"
  - "`blast_radius` filtered to `Implements` answers 'who implements X' / 'what overrides Y'; cold==warm and incremental==fresh parity green; same input → identical edge set; `cargo test --test architecture` green"
status: pending
---

<context>
Occurrence roles (tiers 01–02) give call/ref/read/write edges. SCIP's other edge
signal is `SymbolInformation.relationships`: `is_implementation` (Find
implementations), `is_type_definition` (Go to type definition), `is_reference`,
`is_definition` [src: crates/ariadne-scip/proto/scip.proto; WebFetch scip.proto
this session]. These give trait/interface implementation and type-of edges that
no syntactic pass can produce. This tier consumes them and then makes the
advertised `EdgeKindFilter` honest: today it exposes TypeOf/Overrides/Reads/
Writes/Inherits [src: crates/ariadne-core/src/domain/daemon/query.rs:14-22] while
`EdgeKind` could produce none of them [src: crates/ariadne-core/src/domain/records.rs:161-191].
After this tier the filter maps 1:1 to producible kinds.

`is_implementation` conflates interface-impl, method-override, and inheritance
across indexers; mapping all to one `Implements` kind is honest (no false
precision) — the filter's `Overrides`/`Inherits` alias onto it (plan D5, R3).
</context>

<files>
- crates/ariadne-core/src/domain/records.rs — add `Implements = 7`, `TypeOf = 8`;
  extend `to_byte`/`from_byte` [src: records.rs:161-191].
- crates/ariadne-core/src/domain/daemon/query.rs — reconcile `EdgeKindFilter`:
  `Calls→References`, `Imports→Imports`, `Defines→Defines`, `Reads→Reads`,
  `Writes→Writes`, `TypeOf→TypeOf`, `Overrides→Implements`, `Inherits→Implements`;
  delete any variant with no producible kind [src: query.rs:14-22].
- crates/ariadne-scip/src/** + crates/ariadne-core/** — extend `ScipFactsRaw` with
  relationships `{ from: String, to: String, is_implementation, is_type_definition }`;
  `extract_facts` populates them from `documents[].symbols[].relationships`.
- crates/ariadne-salsa/src/derive.rs — in `resolve_scip_edges`, after occurrence
  edges, map each relationship's `from`/`to` SCIP symbols through the same
  `scip_symbol → SymbolId` map to `Implements` / `TypeOf` edges; drop unmapped.
- crates/ariadne-storage/src/adapters/redb/** — round-trip tags 7–8.
- crates/ariadne-salsa/tests/scip_edges.rs (+ fixtures) — impl + type-of repro;
  a `EdgeKindFilter`→`EdgeKind` total-mapping test.
</files>

<steps>
1. Add a trait/interface-impl fixture and a typed-binding fixture whose SCIP index
   carries `is_implementation` / `is_type_definition` relationships. Commit RED
   tests asserting the `Implements` and `TypeOf` edges.
2. Extend `EdgeKind` (`Implements = 7`, `TypeOf = 8`); update byte round-trip +
   old-DB-opens test.
3. Extend `ScipFactsRaw` + `extract_facts` with relationships (normalize both
   symbols). Map them in `resolve_scip_edges` via the existing symbol→id map; drop
   relationships whose endpoints are not indexed (external supertypes).
4. Reconcile `EdgeKindFilter` (D5): alias `Overrides`/`Inherits` to `Implements`,
   wire `TypeOf`; add a test asserting the mapping is TOTAL (every filter variant
   → a real `EdgeKind`) and that no filter variant is unproducible.
5. Wire `blast_radius` to filter `Implements`/`TypeOf`; confirm "who implements X"
   returns the impl symbols. Re-index dogfood with `--scip`; run full suite +
   parity + determinism; report `memory_report()` delta.
6. Note for handoff: docgen-overview-fidelity tier-03 re-enables the withheld
   Role / boundary-violation / cross-crate-cycle sections on these edges.
</steps>

<verification>
- `cargo nextest run --workspace` → impl + type-of repro green; filter
  total-mapping test green; byte round-trip + old-DB-opens green; cold==warm and
  incremental==fresh parity green; index twice → identical edges.
- `blast_radius` filtered to `Implements` / `TypeOf` returns the expected sets;
  the daemon advertises no edge-kind it cannot produce.
- `cargo test --test architecture`; `cargo clippy … -D warnings`;
  `cargo fmt --all --check`; `cargo deny check` (no new dep); `memory_report()`
  delta < budget (R7).
</verification>

<rollback>
`git checkout --` records.rs, query.rs, the `ScipFactsRaw` relationship fields +
`extract_facts`, the `resolve_scip_edges` relationship mapping, the storage
round-trip, blast_radius wiring, and the fixtures/tests. Removing tags 7–8 and
restoring the prior `EdgeKindFilter` reverts to tier-02 (occurrence-only edges);
no persisted data needs undoing (edges re-derive on next index).
</rollback>
