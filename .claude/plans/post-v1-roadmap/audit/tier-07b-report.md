---
tier_id: tier-07b
audited: 2026-05-30
verdict: PASS
commit: f6b6ae56e514104d6eead95176cc1a9fdf14d565
---

<scope>
Reviewed tier-07b "Incremental per-file re-derivation — edit-stable SymbolId,
stale-record removal, incremental==full-rebuild invariant" against its
`<files>`, `<steps>`, `<exit_criteria>`, plan RD12 / R-B4 / R-B5, and ADR-0017.
Diff is uncommitted in the working tree on top of HEAD f6b6ae5 (tier-07a is also
uncommitted on the same tree; only the tier-07b `<files>` were reviewed —
`derive.rs::symbol_id`, the diff-aware `commit_revision` + `rederive_file` /
`forget_file` in `db.rs`, `tests/incremental.rs`, the re-baselined cli goldens,
ADR-0017). tier-07a artifacts (derived.rs, inputs.rs, lib.rs, the CLI refactor)
belong to the tier-07a audit (already PASS) and were not re-litigated.

Files read end-to-end: crates/ariadne-salsa/src/derive.rs,
crates/ariadne-salsa/src/db.rs, crates/ariadne-salsa/tests/incremental.rs,
crates/ariadne-salsa/src/memory.rs, crates/ariadne-core/src/domain/changeset.rs,
crates/ariadne-core/src/domain/ports.rs,
crates/ariadne-storage/src/adapters/redb/apply.rs,
crates/ariadne-cli/tests/index_parity.rs, crates/ariadne-cli/src/domain/mod.rs
(run_index), docs/adr/0017-incremental-id-stability.md.
</scope>

<checks_run>
All re-run this session; full output captured.
- `cargo nextest run -p ariadne-salsa` — 13/13 PASS. Stability test
  `symbol_id_is_edit_stable_across_offset_shift` green; divergence-0 proptest
  `incremental_sequence_equals_fresh_rebuild` green (100 cases, 17.2s).
- `cargo nextest run --workspace` — 257/257 PASS, 15 skipped (external-indexer
  gated). Architecture invariant `architecture_invariants_hold` PASS.
- `cargo nextest run -p ariadne-cli --test index_parity` — 11/11 PASS (cold
  goldens for the 4 framework + 7 single-lang fixtures match under the new id).
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` — exit 0.
- `cargo fmt --all --check` — exit 0.
- `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` — exit 0.
- Self-index dogfood: built `ariadne`, indexed a clean copy of the repo twice
  (cold full, then `touch` derive.rs + re-index). Identical counts both runs —
  files=305, symbols=2861, edges=3234 (revision 1→2). Cold 871ms / 732ms,
  well under the 60s SLO.
- Memory probe: proptest asserts `memory_report().over_budget()` empty (see I2).
- Verified no offset-id remnants (`name}@` / `@{offset}`) anywhere in crates;
  `fn symbol_id` exists only in derive.rs (single derivation source).
</checks_run>

<findings>
| id | category | severity | location | problem | fix |
|----|----------|----------|----------|---------|-----|
| I1 | tests | INFO | crates/ariadne-salsa/tests/incremental.rs (whole file) | No test exercises `symbol_id` with `nth>0`: every fixture (stability test, proptest, goldens) uses unique `(name,kind)` per file, so the intra-file occurrence-index disambiguator and the R-B5 residual-churn claim in ADR-0017 are unverified by execution (divergence-0 still holds by construction). | Add a 1-file fixture with two same-name same-kind decls; assert distinct ids and divergence-0 after an offset-shifting edit + a before-insert. |
| I2 | tests | INFO | crates/ariadne-salsa/src/memory.rs:48-56 (asserted at incremental.rs:332) | The proptest's R1 memory-probe assertion is vacuous: `memory_report()` is still the tier-04 zero-baseline stub, so `over_budget()` can never be non-empty. Pre-existing, tier-04-documented/authorized stub — not tier-07b code. | Note the probe is a placeholder, or wire real per-table counters in a future tier as planned. |
| I3 | performance | INFO | crates/ariadne-salsa/src/db.rs:399-401 + 413-420 | Forgetting then re-creating a path orphans the prior `FileContentInput`/`SyntacticFactsInput` in salsa storage (dropped from `self.files`, never removed); long-running daemon (tier-08) delete+recreate churn accumulates them. No divergence/correctness impact; negligible in the bounded proptest and unmeasured given I2. | Track/evict orphaned inputs when wiring the tier-08 watcher, or document the bound. |
</findings>

<verdict>
PASS. Zero FAIL findings. Every exit criterion is independently verified:
(1) edit-stable id — stability test green; (2) stale removals consistent with a
full rebuild — `fill_stale_deletes` diffs the prior committed set against the
derived set and the divergence-0 proptest confirms it; (3) single-file driver
API — `rederive_file`/`forget_file` exist, are exported (`FileDerivation` in
lib.rs), and drive the proptest; (4) divergence-0 proptest — 100 cases green;
(5) ADR-0017 records the `{path}#{kind}#{name}#{nth}` scheme, the R-B5 collision
policy, and the stale-removal contract, and the cold goldens are re-baselined
and pass; (6) workspace + architecture + clippy + fmt + doc + self-index
dogfood all green. The apply path (apply.rs:24-70) makes the explicit delete
vectors idempotent against the file_deletes cascade, and EDGES_BY_FILE stays
consistent because a stable `src` id pins an edge's source file. The three INFO
items are coverage/latent-memory notes; none names a behaviour that violates an
exit criterion, a non-negotiable, or a stated budget.
</verdict>

<next_steps>
None required for PASS. Optional follow-ups, all deferrable to tier-08:
- I1: add an `nth>0` (same-name/kind sibling) fixture to lock the disambiguator.
- I2/I3: when the watcher lands, replace the stub memory probe with real
  per-table counters and verify forget+recreate churn stays bounded.
Commit/push gate may proceed: audit-state.json updated to verdict PASS.
</next_steps>

<sources>
- tier-07b-incremental-id-stability.md `<exit_criteria>`, `<steps>`, `<verification>`
- post-v1-roadmap plan.md RD12, R-B4, R-B5
- docs/adr/0017-incremental-id-stability.md
- crates/ariadne-core/src/domain/changeset.rs:16-29 ; ports.rs:108-131
- crates/ariadne-storage/src/adapters/redb/apply.rs:14-76
- CLAUDE.md `<rules>` R1 memory probe ; SLO (cold <60s)
</sources>
