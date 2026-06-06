---
tier_id: tier-04
audited: 2026-06-06
verdict: PASS
commit: bc82fbd13db7d59f862d7f9aedb2eb5bfb05df5e
---

<scope>
Adversarial audit of tier-04 (SCIP default-on, run out-of-band; precise resolver as
live fallback). Diff is the uncommitted working tree against HEAD `bc82fbd` (tiers
01–02 committed + PASS). Sibling `plan.md` read for `<decisions>` D1–D6.

The working tree mixes two uncommitted tiers: tier-03 (relationships:
`Implements`/`TypeOf`, `ScipRelationship`, `from_core`, the salsa relationship
plumbing) and tier-04. Tier-03 was already audited PASS against this same working
tree [src: audit/tier-03-report.md, commit bc82fbd, verdict PASS]. Per the
non-negotiable "other tier files belong to other audits", the tier-03 files
(core/records.rs, core/scip.rs, graph/build.rs, salsa/{derive,derived,db,memory,lib}.rs,
scip/facts.rs and their tests) are treated as the established, already-audited
baseline and not re-reviewed here; this audit scopes to tier-04's `<files>`.

Tier-04 `<files>` touched as intended:
- `cli/src/main.rs` — `--scip` inverted to `--no-scip` (`run(root, fresh, !no_scip)`).
- `cli/src/commands/index.rs` — doc only (flag semantics).
- `cli/src/domain/mod.rs` — `scip_facts()` (daemon-facing extract), Phase-4 out-of-band
  ingest, relationship wiring through `run_scip_ingest`.
- `cli/src/commands/status.rs` — default-on posture + degraded-mode surfacing.
- `cli/src/config.rs` — NOT modified; `INDEXER_BINARIES` already present, surfacing
  landed in status.rs. Acceptable (no change needed there).
- `daemon/**` — `serve_live` second hand-back (`Sender<ScipFactsBatch>`); pump drain +
  `apply_scip_facts`; `spawn_scip_pass` background thread; `filter_to_set` honesty;
  `Cargo.toml` blake3 dev-dep (already a workspace dep, not new).
- `docs/adr/0026-default-on-out-of-band-scip.md` — new, Accepted.
- `cli/tests/scip_default_on.rs`, `cli/tests/index_parity.rs` (`--no-scip` goldens),
  `daemon/tests/scip_pass.rs`, `daemon/tests/live_update.rs` (signature adapt).

Justified out-of-`<files>` touch: `ariadne-e2e/src/domain/mod.rs` `run_index_measured`
gains `--no-scip`. The committed SLO release gate (`e2e/tests/slo.rs`) is the home of
the cold/incr measurement (exit #2); default-on forces it to opt out so the gate keeps
measuring the synchronous fast path. The tier `<files>` named cli/daemon tests for SLO
work; the real gate lives in e2e. Change is +7, minimal, documented inline + in the
ADR consequences. Recorded, not faulted.
</scope>

<checks_run>
- `cargo fmt --all --check` → clean.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` → clean.
- `cargo deny check` → advisories/bans/licenses/sources ok; only pre-existing
  unmatched-license-allowance warnings; **no new external dependency** (D6/constraints).
- `cargo nextest run --workspace` → **475 passed**, 0 failed, 19 skipped (1 slow, 1 leaky).
- Tier-04 tests explicitly re-run, all PASS: `scip_pass::background_scip_pass_recovers_
  cross_crate_edge_without_blocking_queries`; `scip_default_on::default_on_index_
  degrades_to_resolver_without_indexers`; `impact::tests::{every_filter_maps_to_a_
  producible_edge_kind, previously_empty_filters_now_resolve_to_real_edges}`; all 11
  `index_parity::parity_*` goldens.
- Parity gates green: `salsa::incremental_sequence_equals_fresh_rebuild`,
  `daemon::incremental_warm::warm_apply_equals_fresh_rebuild` (incremental==fresh /
  cold==warm) → exit #4.
- `architecture_invariants_hold` PASS; `ariadne-daemon/Cargo.toml` confirms NO
  `ariadne-scip` link (core/graph/storage/salsa/parser only) → hexagonal isolation
  preserved (ADR-0026 maintainability force, D2).
- `daemon::memory_probe::warm_graph_tables_stay_within_the_per_table_budget` PASS → R7.
- End-to-end dogfood of `status`: `cargo run -p ariadne-cli -- status .` prints
  `scip: default-on, out-of-band (pass --no-scip to disable)`, the indexer matrix
  (resolved paths + `scip-java MISSING`), and `degraded: java missing — those languages
  index on the precise resolver (never a failure)` → exit #1 surfacing verified live.
- In-session verification limits (environmental, not defects, stated per non-negotiable):
  (1) `slo.rs` is `#[ignore]`d (multi-GB OSS corpus) — the <60s / p95<500ms NUMBERS
  cannot be re-run here; exit #2 is verified by construction (Phase 4 runs after the
  measured commit; the gate measures the unchanged `--no-scip` fast path) + the
  committed gate. (2) The live MCP daemon (rev 1534) runs the pre-tier-04 binary, so the
  "dogfood overview rides SCIP edges" half of exit #4 is verifiable only after the new
  binary is installed and re-indexes; the machinery is wired + tested.
</checks_run>

<findings>
| id | category | severity | location | problem | fix |
|---|---|---|---|---|---|
| F1 | reliability | INFO | crates/ariadne-cli/src/commands/daemon.rs `spawn_scip_pass` + the `scip.join()` on shutdown | After the 2 s settle, the external indexer run in `scip_facts(&root)` is uninterruptible (no `stop` check), yet the thread is `join()`ed on daemon shutdown, so `daemon stop` issued mid-build blocks until all 13 indexers finish — seconds to minutes. | Detach the pass thread (the `tx.send` is already best-effort and fails harmlessly on a dropped `rx`), or document the bounded shutdown delay; do not `join()` an in-flight indexer build. |
</findings>

<verdict>
PASS. Zero FAIL findings. All four exit criteria are satisfied:

- **#1 default-on + degraded:** `--scip`→`--no-scip` inverted (`main.rs`, `!no_scip`);
  `scip_default_on` proves an all-binaries-absent run succeeds with degraded warnings and
  still indexes call edges on the precise resolver; `status` surfaces the posture live.
- **#2 out-of-band, SLOs unchanged:** `run_index` Phase 4 runs SCIP AFTER the timed
  walk/parse/resolve/commit phases (separate `scip_ms`); the fast tree-sitter index
  commits first, then a second `commit_revision` over `ScipFactsInput` re-commits covered
  edges. The committed SLO gate measures the `--no-scip` fast path — identical before and
  after default-on — so the synchronous path the SLO governs is unchanged by construction.
- **#3 daemon background pass, non-blocking:** `spawn_scip_pass` runs the indexers on a
  dedicated thread holding no lock; `apply_scip_facts` folds on the pump thread, taking the
  warm-catalog write lock only for the re-commit + rebuild — the same discipline as the
  established accept-loop staleness rebuild (`ipc.rs:132`). `scip_pass` proves a cross-crate
  edge the resolver abstains on is recovered while live queries answer throughout.
- **#4 parity + ADR + determinism:** incremental==fresh and warm==cold parity tests green;
  `--no-scip` goldens stable; ADR-0026 records the scheduling + SLO-preservation + hexagonal
  isolation decision; `EdgeKindFilter` is honest by production (every variant maps to a
  `from_core`-producible kind, `Inherits`→`OVERRIDES` for SCIP's conflated `is_implementation`).

The daemon never links `ariadne-scip` — only pure-core `ScipFacts` cross the
`serve_live` channel, mirroring the RD7/ADR-0023 Git-hunk precedent. Hexagonal boundary,
determinism, memory budget, and no-new-dep constraints all hold.
</verdict>

<next_steps>
- F1 is INFO and does not gate; address opportunistically (detach the SCIP pass thread on
  shutdown) or note the bounded delay in the ADR consequences.
- Before relying on exit #2/#4 in production: run the `#[ignore]`d `slo_release_gate`
  against the corpus to capture the actual <60s / p95<500ms numbers, and reinstall the
  daemon binary + re-index so the dogfood/MCP overview rides the SCIP edges.
- Commit will bundle tiers 03 + 04 (both PASS); the audit-gate keys off this tier-04 state.
</next_steps>

<sources>
- Plan + decisions: .claude/plans/scip-driven-edges/plan.md D4, D6; tier-04 exit_criteria.
- ADR: docs/adr/0026-default-on-out-of-band-scip.md (Accepted).
- Prior tier audited baseline: .claude/plans/scip-driven-edges/audit/tier-03-report.md (PASS).
- Out-of-band structure: crates/ariadne-cli/src/domain/mod.rs run_index Phase 4 / run_scip_ingest.
- Daemon non-blocking: crates/ariadne-daemon/src/domain/live.rs apply_scip_facts/spawn_pump;
  crates/ariadne-cli/src/commands/daemon.rs spawn_scip_pass; crates/ariadne-daemon/src/adapters/ipc.rs:106,132.
- Hexagonal isolation: crates/ariadne-daemon/Cargo.toml (no ariadne-scip); ADR-0023 precedent.
- Sourcegraph dual precise+syntactic model: https://sourcegraph.com/docs/code-search/code-navigation/precise_code_navigation.
</sources>
