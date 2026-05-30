---
tier_id: tier-10
audited: 2026-05-30
verdict: PASS
commit: f6b6ae56e514104d6eead95176cc1a9fdf14d565
---

<scope>
Re-audit of tier-10 ("CLI daemon client + warm-query SLO and daemon memory
probe") against its sibling `plan.md`. A prior audit at this same commit
returned FAIL on F1 (the CLI daemon-routing path shipped with no automated
test). The working tree is uncommitted on top of `f6b6ae5` and blends tiers
07a–10; the diff was scoped to tier-10's `<files>` plus the new files the build
added:
- `crates/ariadne-cli/src/adapters/{mod,daemon_client}.rs` (new thin client)
- `crates/ariadne-cli/src/commands/query.rs` (daemon routing + cold fallback)
- `crates/ariadne-cli/src/main.rs` (`mod adapters`; `query` help text)
- `crates/ariadne-e2e/tests/cli_daemon_parity.rs` (NEW — the F1 fix)
- `crates/ariadne-e2e/tests/slo.rs` (warm-query SLO + daemon-RSS stage)
- `crates/ariadne-daemon/benches/warm_query.rs` + `Cargo.toml` `[[bench]]`
- `crates/ariadne-daemon/tests/memory_probe.rs` (new)
Tier-07a/07b/08/09 changes in the same tree were treated as out of scope.
</scope>

<checks_run>
- `cargo fmt --all --check` → clean.
- `cargo clippy --workspace --all-targets -- -D warnings` → exit 0, no warnings.
- `cargo nextest run --workspace` → 279 passed, 15 skipped, 0 failed (1 slow).
  Verified individually: `ariadne-e2e::cli_daemon_parity` PASS,
  `ariadne-daemon::memory_probe` PASS, `ariadne-workspace::architecture` PASS,
  `ariadne-daemon::incremental_warm` PASS.
- `cargo bench --workspace --no-run` → exit 0, all benches build (incl. warm_query).
- Measured `cargo bench -p ariadne-daemon --bench warm_query` → `samples=100
  p50=0.020ms p95=0.025ms p99=0.050ms (budget 10.0ms)`; criterion
  `warm_query_blast_radius` ≈ 28.6 µs. Exit criterion #2 satisfied (p95 ≪ 10 ms).
- Read every in-scope file end-to-end. `query.rs` 13-arm `build_query` map and
  14-arm `project` response map are exhaustive over the daemon protocol; daemon
  `dispatch.rs` handles all 13 CLI-emitted variants + `Ping`; `query.rs::project`
  exhaustively maps every `DaemonResponse` variant. `to_core_kinds` mirrors
  `ariadne-mcp/src/server.rs`.
- Cold-fallback correctness: `ariadne_daemon::query` → `round_trip` →
  `Stream::connect(...)?` returns `Err` immediately when no daemon listens, so
  `DaemonClient::try_query` falls through to auto-spawn-or-`None`→cold path. No
  panic, no hang.
- F1 fix (`cli_daemon_parity.rs`) is a genuine red→green, default-running test:
  it drives the real `ariadne query` binary down both routes, asserts
  byte-identical JSON across 5 tools, then deletes `index.redb` and proves the
  warm route is daemon-served (only the in-RAM graph can answer; the cold path
  bails on the absent index). Without the CLI daemon path that leg is red.
- F2 fix: `main.rs:76-77` `query` help now reads "Route one tool query to the
  warm daemon (cold in-process fallback)…", no longer "in-process".
- Step-2 topology decision (recorded per the tier): the CLI is the composition
  root (ADR-0007) and already depends on `ariadne-daemon`, so it reuses the
  daemon's canonical transport via `ariadne_daemon::query`/`ping` rather than
  adding `interprocess` to its Cargo.toml. Client transport duplication stays at
  exactly one file (the MCP client), so ADR-0015's `ariadne-ipc` crate correctly
  stays deferred. Sound, documented deviation from the `<files>` note; not a
  finding.
</checks_run>

<findings>
| id | category | severity | location | problem | fix |
|---|---|---|---|---|---|
| F1 | tests | INFO | crates/ariadne-daemon/tests/memory_probe.rs:46-57; root cause crates/ariadne-salsa/src/memory.rs:48-56 (out of tier-10 scope) | The per-table memory probe is vacuous: `AriadneDb::memory_report()` is a tier-04 zero-stub that unconditionally inserts `0` for every tracked table (the "later tiers feed counters in" obligation was never met; `LiveEngine::memory_report` at live.rs:204-205 just delegates to it), so `over_budget().is_empty()` is tautologically true and the probe can never detect a >256 MiB table. The doc comment ("asserts no warm-graph table exceeds the 256 MiB budget") overstates what it verifies. Tier-10 used the mechanism step 5 prescribed ("v1 tier-04 mechanism") and the real R1 backstop — daemon RSS < 4 GiB — is honestly wired in `slo.rs`, so no exit criterion is literally violated; but the per-table half of R1 is currently unenforced. | Schedule the deferred tier-04 counter population (a `mem::size_of_val` walk or salsa table enumeration when the pinned API exposes one) so `memory_report()` returns real per-table bytes; until then, soften the probe's doc comment to state it asserts the table set, not real sizes. |
</findings>

<verdict>
PASS. Zero FAIL findings. The prior blocker (F1: the CLI daemon-routing
deliverable shipped with no automated coverage and a falsely-described step-1
TDD link) is genuinely resolved: `cli_daemon_parity.rs` is a default-running,
real-binary red→green test that exercises both the warm and cold routes, asserts
byte-identical JSON, and proves the warm route is daemon-served by deleting the
on-disk index. The prior INFO (F2 stale help text) is also fixed. Exit criteria:
#1 CLI routes to the daemon with cold fallback (implemented + tested); #2 warm
query p95 < 10 ms (measured 0.025 ms); #4 nextest + bench-no-run + architecture +
clippy + fmt all green. The step-2 transport-reuse decision is sound and keeps
ADR-0015's `ariadne-ipc` crate correctly deferred. One INFO (vacuous per-table
memory probe) is rooted in pre-existing, plan-authorized tier-04 code outside
this tier's file scope and does not gate.

Verification limitation (not a finding): exit criterion #3's daemon-RSS-<4GB and
the warm SLO stage's p95-on-100K are obtainable only from the `#[ignore]`d
`slo_release_gate` (multi-GB OSS corpus), not run this session — consistent with
v1 release-gate practice. The 4 GiB daemon-RSS assertion and the warm-query
stage are correctly wired in `slo.rs`; only the corpus clone is deferred.
</verdict>

<next_steps>
None blocking — tier-10 may commit. Recommended (non-gating): address the F1
INFO by scheduling the deferred per-table byte-counter population so the R1
per-table hard-fail can actually fire across all Salsa-touching tiers, and
correct `memory_probe.rs`'s doc comment to match what it currently verifies.
Run the `#[ignore]`d `slo_release_gate` against the real corpus at release to
confirm warm p95 < 10 ms and daemon RSS < 4 GiB on 100K files.
</next_steps>

<sources>
- CLAUDE.md `<rules>` — TDD mandatory; "Validate by execution"; R1 per-tier
  memory probe (>256MB/table hard fail).
- .claude/plans/post-v1-roadmap/plan.md `<constraints>`/RD5/RD6/risk R1; `<steps>`,
  `<exit_criteria>`, `<verification>` of tier-10-cli-daemon-client-slo.md.
- docs/adr/0015-daemon-mode-ipc.md `<consequences>` (one-file client duplication;
  deferred `ariadne-ipc`); docs/adr/0007-cli-composition-root.md.
- crates/ariadne-salsa/src/memory.rs:48-71 (zero-stub + TRACKED_TABLES);
  crates/ariadne-daemon/src/domain/live.rs:204-205 (delegation);
  crates/ariadne-daemon/src/adapters/ipc.rs:78-83,369-380 (transport/cold-fallback).
- Measured this session: warm_query p95 = 0.025 ms; nextest 279 passed/15 skipped;
  clippy exit 0; fmt clean; bench --no-run exit 0; memory_probe tables all 0 bytes.
- Google eng-practices, code-health-over-perfection standard:
  https://google.github.io/eng-practices/review/reviewer/standard.html
</sources>
</content>
</invoke>
