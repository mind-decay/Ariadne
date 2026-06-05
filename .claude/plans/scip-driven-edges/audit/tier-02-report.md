---
tier_id: tier-02
audited: 2026-06-05
verdict: PASS
commit: 6011ea2df1b69d99de2a2db9c806dc98a9d440f1
---

<scope>
Tier-02 "Reads/Writes edge kinds from SCIP access roles" of the
`scip-driven-edges` plan. Tier-01 and tier-02 both sit uncommitted in the
working tree (no SCIP commit in the log; the SCIP fact files are untracked);
tier-01 was audited PASS at the same HEAD. The scoped tier-02 delta — confined
to the access-role surface — is:

- `crates/ariadne-core/src/domain/records.rs:156-202` — derivation `EdgeKind`
  gains `Reads = 5` / `Writes = 6`; `to_byte`/`from_byte` extended.
- `crates/ariadne-graph/src/build.rs:65-81` — `from_core` maps
  `Reads`/`Writes` to the graph alphabet (no longer collapsing to `Calls`).
- `crates/ariadne-salsa/src/derive.rs:382-385,455-468` — `resolve_scip_edges`
  branches on `symbol_roles`: Import > Write > Read > References.
- `crates/ariadne-salsa/tests/scip_edges.rs:304-404` — access + precedence repro.
- `crates/ariadne-core/tests/tags.rs:89-126` — all-variants tag round-trip.
- `crates/ariadne-storage/tests/roundtrip.rs:136-194` — old-DB reopen.
- `crates/ariadne-graph/tests/synthetic.rs:95-167` — blast filter Reads/Writes.

Out of tier-02 scope (tier-01 footprint, audited separately): `scip.rs`,
`facts.rs`, `extract_facts.rs`, salsa `db.rs/derived.rs/inputs.rs/memory.rs`,
cli/core `mod.rs`, and the two parity goldens (a tier-01 resolver-abstention
`References`-edge removal — verified a real edge no longer produced, not a
weakened assertion). `impact.rs` / `ariadne-mcp` needed no change: the
`EdgeKindFilter::{Reads,Writes}` → `EdgeKindSet::{READS,WRITES}` mapping
(`impact.rs:34-35`) and the MCP filter variants (`mcp/src/types.rs:27,29`)
pre-date this tier — tier-02's `<files>` says "ensure ... passes through".
</scope>

<checks_run>
- `cargo fmt --all --check` → clean.
- `cargo nextest run --workspace` → 465 passed, 19 skipped, 0 failed. Includes
  `ariadne-workspace::architecture` (salsa ⊥ ariadne-scip), cold==warm
  (`ariadne-salsa::equivalence`), and incremental==fresh
  (`ariadne-daemon::incremental_warm`, `ariadne-salsa::incremental`).
- Targeted tier-02: `scip_edges` (access + precedence), `tags`, `roundtrip`,
  `synthetic` → 29 passed.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` → clean.
- `cargo deny check` → advisories/bans/licenses/sources ok; no new dependency
  (the three warnings are pre-existing unmatched-license allowances).
- Read end-to-end: `records.rs`, `build.rs`, `derive.rs`, `scip_edges.rs`,
  `tags.rs`, `roundtrip.rs`, `synthetic.rs`; `db.rs` SCIP wiring
  (occurrences sorted by range `:348`, files by id `:377` before
  `resolve_scip_edges` `:378` — deterministic).
- `memory_report()` (R7): tier-02 adds no salsa memo table and no record-width
  growth (`EdgeKind` stays `repr(u8)`, 1 byte; `EdgeRecord` unchanged). Edges
  live in the redb `EDGES` table, not a tracked memo table — the delta over
  tier-01 is structurally zero, far under the 256MB budget.
- `mcp__ariadne__project_status` → revision 1420, fresh.
</checks_run>

<findings>
| id | category | severity | location | problem | fix |
|----|----------|----------|----------|---------|-----|
| — | — | — | — | No FAIL or INFO findings. | — |
</findings>

<verdict>
PASS. All four `exit_criteria` independently verified:

1. **RED→GREEN read/write fixture, each only on a present bit.**
   `access_roles_drive_reads_and_writes_edges` (`scip_edges.rs:310`) asserts the
   `0x4` occurrence → one `Writes` edge, the `0x8` → one `Reads`, and the
   bit-free occurrence stays a plain `References` (no fabrication). Green.

2. **EdgeKind gains stable tags; round-trip; from_core; old DB opens.**
   `Reads = 5`/`Writes = 6` (`records.rs:175-178`); `tags.rs:89` pins the bytes,
   round-trips all 7 variants through `to_byte`/`from_byte` and the composite
   `EdgeKey`, and asserts tag 7 → `None`. `from_core` maps `Reads`/`Writes`
   (`build.rs:73-75`); `synthetic.rs:104-111` asserts they no longer collapse to
   `Calls`. `pre_reads_writes_db_reopens_with_old_tags_intact`
   (`roundtrip.rs:142`) writes tags 0–4, closes, reopens with the widened enum,
   and decodes every edge unchanged — no migration. Green.

3. **Neither bit → References; blast_radius filters Reads/Writes.** The
   References arm above covers the no-fabrication case;
   `blast_radius_filters_reads_and_writes_independently` (`synthetic.rs:96`)
   drives `READS`, `WRITES`, `READS|WRITES`, and `CALLS` filters end-to-end
   through `from_core` and asserts each returns exactly its expected reachable
   set. Green.

4. **Parity green; identical edge set; architecture green.** cold==warm,
   incremental==fresh, and the architecture invariant all pass in the workspace
   run. Determinism holds: occurrences/files are sorted before
   `resolve_scip_edges`, and the `sym_of_key` map is read-only in the
   edge-emitting pass (first-wins by sorted iteration). Green.

Decision adherence: D5 (kinds track SCIP signal, honest by production, emit only
on a present bit — Import>Write>Read>References, no name logic, no fabrication)
and D1 (symbols untouched; SCIP feeds edges only) are met. Definition-role
occurrences are correctly excluded from access-edge emission (they build the
key map; a self-referential def would self-loop and drop regardless). No
smuggled dependency or pattern.
</verdict>

<next_steps>
None. Tier-02 is accepted. Proceed to tier-03 (Implements/TypeOf relationships +
full `EdgeKindFilter` honesty-by-production), which the plan defers the
remaining filter reconciliation to.
</next_steps>

<sources>
- SCIP `SymbolRole` access bits: crates/ariadne-scip/proto/scip.proto:526-532
  (WriteAccess 0x4, ReadAccess 0x8) [cited by tier file + derive.rs consts].
- Two `EdgeKind` enums + the lossy `from_core`: records.rs:161-202 ;
  build.rs:30-81 ; daemon `EdgeKindFilter` core/domain/daemon/query.rs:14-31.
- Plan decisions D1/D5 + R3/R7: .claude/plans/scip-driven-edges/plan.md.
- Verification commands: CLAUDE.md `<commands>`.
</sources>
