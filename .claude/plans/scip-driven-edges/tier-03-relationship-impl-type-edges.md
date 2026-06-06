---
tier_id: tier-03
title: Implements/TypeOf edges from SCIP relationships; EdgeKindFilter honest by production
deps: [tier-02]
exit_criteria:
  - "A committed RED→GREEN test: a trait/interface-impl fixture yields an `Implements` edge (graph `Overrides`) between impl and trait symbols; a typed-binding fixture yields a `TypeOf` edge from the binding to its type symbol"
  - "The derivation `EdgeKind` gains `Implements` and `TypeOf` with stable byte tags; `to_byte`/`from_byte` round-trip; `from_core` maps them to graph `Overrides`/`TypeOf`; old redb files still open"
  - "Every `EdgeKindFilter` variant maps to a PRODUCIBLE `EdgeKind` — asserted by a total-mapping test over the daemon filter, `EdgeKindSet`, and `from_core`; the 5 previously-empty filters (TypeOf/Overrides/Reads/Writes/Inherits) now resolve to real edges"
  - "`blast_radius` filtered to `Implements` answers 'who implements X'; cold==warm and incremental==fresh parity green; same input → identical edge set; `cargo test --test architecture` green"
status: completed
completed: 2026-06-05
---

<context>
Occurrence roles (tiers 01–02) give call/ref/read/write edges. SCIP's other edge
signal is `SymbolInformation.relationships`: `is_implementation` (Find
implementations, field 3), `is_type_definition` (Go to type definition, field 4),
`is_reference` (2), `is_definition` (5)
[src: crates/ariadne-scip/proto/scip.proto:488-512]. These give trait/interface
implementation and type-of edges no syntactic pass can produce. This tier consumes
them and then makes the advertised filter honest BY PRODUCTION (plan D5, user:
all-tiers).

The honesty gap spans two enums: the daemon `EdgeKindFilter` advertises
`TypeOf/Overrides/Reads/Writes/Inherits` [src: crates/ariadne-core/src/domain/daemon/query.rs:14-31]
and `EdgeKindSet` mirrors them [src: crates/ariadne-graph/src/build.rs:82-95], but
`from_core` produces only `Calls/Imports/Defines` [src: build.rs:66-79] — so those
5 filters return empty. Tiers 02–03 add the derivation kinds and the `from_core`
arms so every advertised filter resolves to a real edge. `is_implementation`
conflates interface-impl, method-override, and inheritance across indexers; mapping
all to one `Implements` (→ graph `Overrides`/`Inherits`) is honest — no false
precision (plan D5, R3).
</context>

<files>
Reconciled to the actual 16-file change set per audit F1: the planned
`daemon/query.rs` and `storage/redb/**` were NOT the touch sites — the real
reconciliation lives in `daemon/queries/impact.rs::filter_to_set`, and the new
byte tags flow through the generic redb codec (no per-tech edit), exercised
end-to-end by `scip_edges.rs` [src: audit/tier-03-report.md F1].

Domain interior:
- crates/ariadne-core/src/domain/records.rs — add `Implements`/`TypeOf` to the
  derivation `EdgeKind`; extend `to_byte`/`from_byte` (append-only tags 7/8).
- crates/ariadne-core/src/domain/scip.rs — extend the SCIP facts type with
  relationships `{ from, to, is_implementation, is_type_definition }`.
- crates/ariadne-core/src/lib.rs — façade re-export of the new relationship type.
- crates/ariadne-graph/src/build.rs — extend `from_core` (`Implements`→graph
  `Overrides`, `TypeOf`→`TypeOf`) and `to_flag` so `EdgeKindSet`/`from_core` are
  total over producible kinds.
- crates/ariadne-salsa/src/derive.rs — `resolve_scip_edges` pass 3 maps each
  relationship's `from`/`to` SCIP symbols through the same `scip_symbol →
  SymbolId` map to `Implements`/`TypeOf` edges; drop unmapped/self-loop.
- crates/ariadne-salsa/src/derived.rs — salsa-local `ScipRelationshipRaw` mirror
  (no `ariadne-scip` import) + facts plumbing.
- crates/ariadne-salsa/src/db.rs — relationships input + coverage gate + sort.
- crates/ariadne-salsa/src/lib.rs — relationships input wiring / re-export.
- crates/ariadne-salsa/src/memory.rs — account the relationship table (R7 probe).

Adapters / composition:
- crates/ariadne-daemon/src/domain/queries/impact.rs — `filter_to_set` reconciles
  `EdgeKindFilter`→`EdgeKindSet` (alias `Overrides`/`Inherits`→`Implements`, wire
  `TypeOf`/`Reads`/`Writes`); `blast_radius` filtering.
- crates/ariadne-scip/src/facts.rs — `document_relationships`: `extract_facts`
  populates relationships from `documents[].symbols[].relationships`, normalizing
  both endpoints to the occurrence keys.
- crates/ariadne-cli/src/domain/mod.rs — `run_scip_ingest` sets the relationships
  input on the salsa DB (composition root).

Tests / fixtures:
- crates/ariadne-core/tests/tags.rs — byte round-trip for tags 7/8; `from_byte(9)
  == None`; old tags 0–6 stable.
- crates/ariadne-graph/tests/synthetic.rs — pin `Implements`→graph `Overrides`.
- crates/ariadne-scip/tests/extract_facts.rs — relationship-extraction repro.
- crates/ariadne-salsa/tests/scip_edges.rs (inline fixtures) — impl + type-of
  repros; filter→`EdgeKind` total-mapping test; `blast_radius` who-implements.
</files>

<steps>
1. Add a trait/interface-impl fixture and a typed-binding fixture whose SCIP index
   carries `is_implementation` / `is_type_definition` relationships. Commit RED tests
   asserting the `Implements` and `TypeOf` edges.
2. Extend the derivation `EdgeKind` (`Implements`, `TypeOf`); update byte round-trip
   + old-DB-opens test; extend `from_core`.
3. Extend `ScipFactsRaw` + `extract_facts` with relationships (normalize both
   symbols). Map them in `resolve_scip_edges` via the existing symbol→id map; drop
   relationships whose endpoints are not indexed (external supertypes).
4. Reconcile `EdgeKindFilter` (D5): alias `Overrides`/`Inherits` to `Implements`,
   wire `TypeOf`/`Reads`/`Writes`; add a test asserting the mapping is TOTAL (every
   filter variant → a producible `EdgeKind`) and no filter variant is unproducible.
5. Wire `blast_radius` to filter `Implements`/`TypeOf`; confirm "who implements X"
   returns the impl symbols. Re-index dogfood with `--scip`; run full suite + parity
   + determinism; report `memory_report()` delta.
</steps>

<verification>
- `cargo nextest run --workspace` → impl + type-of repro green; filter total-mapping
  test green; byte round-trip + old-DB-opens green; cold==warm and incremental==fresh
  parity green; index twice → identical edges.
- `blast_radius` filtered to `Implements` / `TypeOf` returns the expected sets; the
  daemon advertises no edge-kind it cannot produce.
- `cargo test --test architecture`; `cargo clippy … -D warnings`;
  `cargo fmt --all --check`; `cargo deny check` (no new dep); `memory_report()`
  delta < budget (R7).
</verification>

<rollback>
`git checkout --` records.rs, build.rs `from_core`, query.rs, the `ScipFactsRaw`
relationship fields + `extract_facts`, the `resolve_scip_edges` relationship mapping,
the storage round-trip, blast_radius wiring, and the fixtures/tests. Removing the new
tags and restoring the prior filter reverts to tier-02 (occurrence-only edges); no
persisted data needs undoing (edges re-derive on the next index).
</rollback>
