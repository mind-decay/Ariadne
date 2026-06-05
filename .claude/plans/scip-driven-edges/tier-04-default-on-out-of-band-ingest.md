---
tier_id: tier-04
title: SCIP default-on, run out-of-band; precise resolver as live fallback
deps: [tier-03]
exit_criteria:
  - "`--scip` is inverted to default-on with a `--no-scip` opt-out; an `IngestPlan` whose every indexer binary is absent is a warning, never a failure, and the index still completes on the precise resolver (degraded mode)"
  - "SCIP runs OUT-OF-BAND: the fast tree-sitter index commits first, then a separate SCIP pass re-commits covered edges; a committed measurement shows cold full-index <60s and incremental p95 <500ms UNCHANGED vs the pre-tier baseline (SCIP not on either synchronous path, R9)"
  - "The daemon runs SCIP as a background/idle pass (not on the synchronous query or incremental-commit path); a query during an in-flight SCIP pass returns the current (precise-resolver or last-covered) edges, never blocks"
  - "Fresh re-index of the committed binary twice → identical edge set; the dogfood/MCP overview rides SCIP edges where an indexer is present and the precise resolver elsewhere; ADR-0026 records the scheduling + SLO-preservation decision; cold==warm and incremental==fresh parity green"
status: pending
---

<context>
Tiers 01–03 produce precise SCIP edges, but only behind `--scip`, which runs
`IngestPlan` INLINE — counted in cold-index time [src: crates/ariadne-cli/src/domain/mod.rs:155,253-258].
So the committed/dogfood/MCP graph the LLM consumes stays tree-sitter-only unless a
human passes the flag. This tier makes SCIP edges the DEFAULT without regressing the
SLOs (plan D6, user: most effective/quality) — Sourcegraph's auto-index endpoint:
"automatically uses Precise whenever available, search-based as fallback"
[src: https://sourcegraph.com/docs/code-search/code-navigation/precise_code_navigation].

The orchestration already supports default-on: `IngestPlan` walks drivers, asks each
`detect(root)`, runs survivors in parallel, and treats a missing binary as a WARNING
in degraded mode, never a hard failure [src: crates/ariadne-scip/src/indexer/plan.rs:1-39].
The two missing pieces are (1) inverting the flag and moving the run OFF the
synchronous index path so cold<60s / incr-p95<500ms hold (R9), and (2) the daemon,
which has ZERO SCIP wiring today, running it as a background pass.
</context>

<files>
- crates/ariadne-cli/src/main.rs + commands/index.rs — invert `--scip` to default-on
  with a `--no-scip` opt-out; the index command commits the fast tree-sitter pass,
  THEN runs SCIP and re-commits covered edges [src: index.rs:22-32].
- crates/ariadne-cli/src/domain/mod.rs — split `run_index` so `IngestPlan::ingest`
  runs AFTER the fast commit (a second `commit_revision` over `ScipFactsInput`), not
  inline in the timed parse/resolve/commit phases [src: domain/mod.rs:253-258].
- crates/ariadne-daemon/** — a background/idle SCIP pass: after a commit settles (or
  on idle), run `IngestPlan`, set `ScipFactsInput`, re-commit; never on the
  synchronous query or incremental-commit path. New wiring (daemon has none today).
- crates/ariadne-cli/src/config.rs + commands/status.rs — surface default-on + the
  degraded-mode warnings (missing indexer binaries) in `status` [src: config.rs:24-29].
- docs/adr/0026-default-on-out-of-band-scip.md — the scheduling + SLO-preservation
  decision (out-of-band pass; precise resolver as live fallback).
- crates/ariadne-cli/tests/** + crates/ariadne-daemon/tests/** — SLO + degraded-mode
  + daemon-non-blocking tests.
</files>

<steps>
1. Write ADR-0026: SCIP default-on, out-of-band; the fast index never blocks on SCIP;
   the precise resolver is the live fallback (D4); degraded mode on missing binaries.
2. Invert the flag: default-on, `--no-scip` opt-out. Commit a RED test asserting an
   all-binaries-absent run completes (degraded warning) and indexes on the resolver.
3. Move the CLI SCIP run out-of-band: fast tree-sitter index commits, THEN
   `IngestPlan::ingest` → `extract_facts` → `ScipFactsInput` → a second
   `commit_revision`. Add a committed cold/incr measurement asserting <60s / p95<500ms
   vs the pre-tier baseline (R9).
4. Wire the daemon background/idle SCIP pass; add a test that a query during an
   in-flight pass returns current edges and never blocks.
5. Re-index the committed binary twice → identical edge set; confirm the dogfood
   overview rides SCIP edges where indexers are present. Run full suite + parity +
   determinism; report `memory_report()` delta.
</steps>

<verification>
- `cargo nextest run --workspace` → degraded-mode + daemon-non-blocking + SLO tests
  green; cold==warm and incremental==fresh parity green; index twice → identical edges.
- Committed measurement: cold full-index <60s and incremental p95 <500ms unchanged
  vs baseline with SCIP default-on (R9).
- Dogfood: `cargo run -p ariadne-cli -- index` (no flag) produces SCIP edges where an
  indexer is present; `status` lists any missing-indexer warnings.
- `cargo test --test architecture`; `cargo clippy … -D warnings`;
  `cargo fmt --all --check`; `cargo deny check` (no new dep); `memory_report()`
  delta < budget (R7).
</verification>

<rollback>
`git checkout --` the flag inversion, the `run_index` split, the daemon background
pass, the status surfacing, ADR-0026, and the tests. Reverting restores `--scip`
opt-in inline (tier-01–03 behaviour); no persisted data needs undoing (edges
re-derive on the next index). If the daemon pass overruns, ship CLI default-on only
and keep the daemon pass `#[ignore]`d with this slug.
</rollback>
