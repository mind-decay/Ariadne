---
tier_id: tier-10
title: CLI daemon client + warm-query SLO and daemon memory probe
deps: [tier-08, tier-09]
exit_criteria:
  - `ariadne` query subcommands route to the daemon with the same cold-path fallback as the MCP server.
  - A criterion bench shows warm query p95 < 10ms (vs the 100ms v1 cold SLO).
  - A memory probe shows daemon RSS < 4GB on the 100K-file workload (v1 risk R1).
  - `cargo nextest run --workspace` + `cargo bench --workspace --no-run` + architecture + clippy + fmt all green.
status: completed
completed: 2026-05-30
---

<context>
Closes Block B. The CLI becomes a daemon client like the MCP server (tier-09), and the warm path is held to a tighter SLO than the v1 cold path. This tier also runs the mandatory per-tier memory probe for the daemon's warm graph (CLAUDE.md R1: >256MB per table is a hard fail; daemon RSS bounded by the 4GB ceiling). Full context: plan.md.
</context>

<files>
- crates/ariadne-cli/src/adapters/daemon_client.rs — new: thin `interprocess` client (mirrors the MCP client; ADR-0015 per-adapter module).
- crates/ariadne-cli/Cargo.toml — modify: add `interprocess = "=2.4.2"`.
- crates/ariadne-cli/src/ — modify: query subcommands route through the daemon client with cold fallback.
- crates/ariadne-e2e/tests/slo.rs — modify: add the warm-query SLO stage.
- crates/ariadne-e2e/benches/ or crates/ariadne-daemon/benches/ — new: criterion warm-query bench.
- crates/ariadne-daemon/tests/ — new: memory-probe test reporting warm-graph table deltas.
</files>

<steps>
1. Failing test first (`ariadne-e2e` `slo.rs`): assert warm query p95 < 10ms over a daemon-served run on the 100K-file workload. Red — the CLI has no daemon path.
2. Implement `ariadne-cli/src/adapters/daemon_client.rs` reusing the tier-09 connect/auto-spawn/cold-fallback policy. If client transport now exceeds one file across mcp+cli, trigger the ADR-0015 shared-`ariadne-ipc`-crate path (record the decision in this tier's audit report).
3. Route the CLI query subcommands (`blast-radius`, `coupling`, `weak-spots`, etc.) through the daemon client.
4. Criterion warm-query bench: measure p95 over 100 sampled queries against a warm daemon; gate at < 10ms.
5. Memory probe: after the daemon builds the warm graph on the 100K-file workload, report `memory_report()` per-table deltas (v1 tier-04 mechanism) and peak RSS; > 256MB per table or > 4GB RSS is a hard fail (CLAUDE.md R1).
6. Extend `slo.rs`: the warm stage runs after the v1 cold/incremental/query stages, so the existing SLO gate is not weakened — only extended.
</steps>

<verification>
- `cargo nextest run --workspace` — full suite green including the new warm SLO stage.
- `cargo bench --workspace --no-run` — warm-query bench builds; a measured run records p95 < 10ms in the audit report.
- Memory probe output in the audit report: every warm-graph table < 256MB, daemon RSS < 4GB.
- `cargo test --test architecture`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check` — green.
</verification>

<rollback>
`git checkout -- crates/ariadne-cli crates/ariadne-e2e crates/ariadne-daemon`. The CLI reverts to the v1 in-process query path.
</rollback>
