---
tier_id: tier-02
title: Reads/Writes edge kinds from SCIP access roles
deps: [tier-01]
exit_criteria:
  - "A committed RED→GREEN test on a fixture with a field/variable read and write: the read occurrence yields a `Reads` edge and the write occurrence a `Writes` edge, each only when the SCIP role bit is present"
  - "The derivation `EdgeKind` gains `Reads` and `Writes` with stable byte tags; `to_byte`/`from_byte` round-trip all variants; `from_core` maps them to the graph `Reads`/`Writes` (no longer collapsing to `Calls`); an old redb file (pre-Reads/Writes tags) still opens and decodes (no data migration)"
  - "An occurrence with neither ReadAccess nor WriteAccess set stays a plain `References` edge (no fabrication); `blast_radius` can filter on `Reads` and on `Writes` and returns the expected sets"
  - "cold==warm and incremental==fresh parity green; same input → identical edge set; `cargo test --test architecture` green"
status: completed
completed: 2026-06-05
---

<context>
Tier-01 emits precise `References`/`Imports` from SCIP occurrences. SCIP further
distinguishes access kind in the same bitset: `SymbolRole` WriteAccess `0x4`,
ReadAccess `0x8` [src: crates/ariadne-scip/proto/scip.proto:530-532]. This tier
promotes those occurrences to dedicated `Reads`/`Writes` edges so consumers can
separate mutation from read, and so the `from_core` mapping stops collapsing them
to `Calls` (plan D5; full filter reconciliation completes in tier-03).

Two `EdgeKind` enums: the derivation/storage one
[src: crates/ariadne-core/src/domain/records.rs:161-172] and the in-RAM graph one
which already declares `Reads = 5`/`Writes = 6`
[src: crates/ariadne-graph/src/build.rs:30-47]. Today `from_core` collapses every
non-Defines/Imports kind to `Calls` [src: build.rs:66-79], so the graph's
`Reads`/`Writes` are never populated. This tier adds the matching derivation
variants and the `from_core` arms. Edges are derived each index, so widening the
enum needs only a storage tag round-trip + re-index — no redb data migration
(old files predate the new tags). Population is indexer-dependent; emit only on a
present bit (plan R3).
</context>

<files>
- crates/ariadne-core/src/domain/records.rs — add `Reads` and `Writes` to the
  derivation `EdgeKind` with the next stable byte tags; extend `to_byte`/`from_byte`
  [src: records.rs:161-172].
- crates/ariadne-graph/src/build.rs — extend `from_core` to map the new derivation
  kinds to graph `Reads`/`Writes` [src: build.rs:66-79].
- crates/ariadne-storage/src/adapters/redb/** — confirm the edge-kind byte
  round-trips the new tags; a round-trip test over all variants.
- crates/ariadne-salsa/src/derive.rs — in `resolve_scip_edges`, branch the
  occurrence kind: WriteAccess→`Writes`, ReadAccess→`Reads`, else `References`
  (Import-role still `Imports`); precedence Write > Read when both set.
- crates/ariadne-salsa/tests/scip_edges.rs (+ fixture) — read/write repro.
- crates/ariadne-daemon/src/domain/queries/impact.rs + crates/ariadne-mcp/** —
  ensure `blast_radius` kind filtering passes `Reads`/`Writes` through (full
  `EdgeKindFilter` reconciliation is tier-03).
</files>

<steps>
1. Add a fixture with a clear field read and field write whose SCIP index carries
   ReadAccess / WriteAccess. Commit a RED test asserting a `Reads` and a `Writes`
   edge at those sites, and a plain-`References` edge where no access bit is set.
2. Extend the derivation `EdgeKind` with `Reads`/`Writes` (next tags); update
   `to_byte`/`from_byte`; extend `from_core` (build.rs); add an exhaustive byte
   round-trip test and an "old DB opens" test (a redb file written with the prior
   tags decodes unchanged).
3. Branch `resolve_scip_edges` on `symbol_roles`: mask WriteAccess `0x4` / ReadAccess
   `0x8`; choose `Writes` > `Reads` > `References`; leave Import-role `Imports`. No
   name-based logic, no fabrication when bits are absent.
4. Thread the new kinds through `blast_radius` filtering so a query can request only
   `Reads` or only `Writes`.
5. Re-index dogfood with `--scip`; confirm read/write edges appear where the indexer
   emits the roles and nowhere else. Run full suite + parity + determinism; report
   `memory_report()` delta.
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
`git checkout --` records.rs, build.rs `from_core`, the storage round-trip, the
`resolve_scip_edges` branch, the blast_radius wiring, and the fixture/test. Removing
the new tags reverts every SCIP occurrence to a `References` edge (tier-01
behaviour); no persisted data needs undoing (edges re-derive on the next index).
</rollback>
