---
tier_id: tier-02
title: Reads/Writes edge kinds from SCIP access roles
deps: [tier-01]
exit_criteria:
  - "A failing test (red → green) on a fixture with a field/variable read and write: the read occurrence yields a `Reads` edge and the write occurrence a `Writes` edge, each only when the SCIP role bit is present"
  - "`EdgeKind` gains `Reads` and `Writes` with stable byte tags 5 and 6; `to_byte`/`from_byte` round-trip all variants; an old redb file (tags 0–4 only) still opens and decodes (no data migration)"
  - "An occurrence with neither ReadAccess nor WriteAccess set stays a plain `References` edge (no fabrication); `blast_radius` can filter on `Reads` and on `Writes`"
  - "cold==warm and incremental==fresh parity green; same input → identical edge set; `cargo test --test architecture` green"
status: pending
---

<context>
Tier-01 emits precise `References`/`Imports` from SCIP occurrences. SCIP further
distinguishes access kind in the same bitset: `SymbolRole` WriteAccess 0x4,
ReadAccess 0x8 [src: crates/ariadne-scip/proto/scip.proto; WebFetch scip.proto
this session]. This tier promotes those occurrences to dedicated `Reads`/`Writes`
edges so consumers can separate mutation from read (data-flow-ish queries) and so
the advertised `EdgeKindFilter` Reads/Writes start mapping to real kinds (plan
D5; reconciliation completes in tier-03).

Edges are derived each index, so widening the enum needs only a storage tag
round-trip + re-index — no redb data migration (old files only ever hold tags
0–4) [src: crates/ariadne-core/src/domain/records.rs:161-191]. Population is
indexer-dependent; emit only on a present bit (plan R3).
</context>

<files>
- crates/ariadne-core/src/domain/records.rs — add `Reads = 5`, `Writes = 6` to
  `EdgeKind`; extend `to_byte`/`from_byte` [src: records.rs:161-191].
- crates/ariadne-storage/src/adapters/redb/** — confirm the edge-kind byte
  round-trips tags 5–6; a round-trip test over all variants.
- crates/ariadne-salsa/src/derive.rs — in `resolve_scip_edges`, branch the
  occurrence kind: WriteAccess→`Writes`, ReadAccess→`Reads`, else `References`
  (Import-role still `Imports`); precedence Write > Read when both set.
- crates/ariadne-salsa/tests/scip_edges.rs (+ fixture) — read/write repro.
- crates/ariadne-mcp/** / crates/ariadne-daemon/src/domain/queries/impact.rs —
  ensure `blast_radius` kind filtering passes `Reads`/`Writes` through (full
  `EdgeKindFilter` reconciliation is tier-03).
</files>

<steps>
1. Add a fixture with a clear field read and field write whose SCIP index carries
   ReadAccess / WriteAccess. Commit a RED test asserting a `Reads` and a `Writes`
   edge at those sites, and a plain-`References` edge where no access bit is set.
2. Extend `EdgeKind` with `Reads = 5`, `Writes = 6`; update `to_byte`/`from_byte`;
   add an exhaustive byte round-trip test and an "old DB opens" test (a redb file
   written with tags 0–4 decodes unchanged).
3. Branch `resolve_scip_edges` on `symbol_roles`: mask WriteAccess 0x4 / ReadAccess
   0x8; choose `Writes` > `Reads` > `References`; leave Import-role `Imports`.
   No name-based logic, no fabrication when bits are absent.
4. Thread the new kinds through `blast_radius` filtering so a query can request
   only `Reads` or only `Writes`.
5. Re-index dogfood with `--scip`; confirm read/write edges appear where the
   indexer emits the roles and nowhere else. Run full suite + parity + determinism;
   report `memory_report()` delta.
</steps>

<verification>
- `cargo nextest run --workspace` → read/write repro green; `References` unchanged
  where no access bit; byte round-trip + old-DB-opens tests green; cold==warm and
  incremental==fresh parity green; index twice → identical edges.
- `blast_radius` filtered to `Reads` / `Writes` returns the expected sets.
- `cargo test --test architecture`; `cargo clippy … -D warnings`;
  `cargo fmt --all --check`; `cargo deny check` (no new dep); `memory_report()`
  delta < budget (R7).
</verification>

<rollback>
`git checkout --` records.rs, the storage round-trip, the `resolve_scip_edges`
branch, the blast_radius filter wiring, and the fixture/test. Removing tags 5–6
reverts every SCIP occurrence to a `References` edge (tier-01 behaviour); no
persisted data needs undoing (edges are re-derived on the next index).
</rollback>
