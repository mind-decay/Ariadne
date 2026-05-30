---
tier_id: tier-09
audited: 2026-05-30
verdict: PASS
commit: f6b6ae56e514104d6eead95176cc1a9fdf14d565
---

<scope>
Tier-09 — "MCP daemon client": the `ariadne-mcp` driving adapter becomes a thin
client to the warm daemon (RD6), routing each `#[tool]` query over the
`<root>/.ariadne/daemon.sock` local socket and falling back to the v1 cold redb
path when no daemon answers. Scoped diff (uncommitted working tree on
`f6b6ae5`), confined to the tier's `<files>`:

- `crates/ariadne-mcp/src/adapters/daemon_client.rs` — NEW thin `interprocess`
  client + length-prefixed postcard framing + connection policy.
- `crates/ariadne-mcp/src/adapters/mod.rs` — NEW module decl.
- `crates/ariadne-mcp/Cargo.toml` — adds `interprocess = "=2.4.2"`, `postcard = "=1.1.3"`.
- `crates/ariadne-mcp/src/server.rs` — all 13 `#[tool]` handlers route through
  the daemon client; `project_daemon` + `to_core_kinds` mappers added.
- `crates/ariadne-mcp/src/lib.rs` — exposes `adapters` + re-exports `DaemonClient`.
- `crates/ariadne-mcp/tests/daemon_client.rs` — NEW: 3 tests (IPC round-trip,
  daemon-served-not-cold, cold fallback).
- `crates/ariadne-mcp/tests/support.rs` — adds a real stub-daemon socket server.

Out of scope (other tiers, not audited here): the daemon/salsa/cli/graph
working-tree changes belong to tiers 07a/07b/08. Core daemon protocol types
(`DaemonQuery`/`DaemonResponse`/`DaemonRequest`) were committed by tier-07 and
consumed here unmodified — `ariadne-core` is not touched by this tier.
</scope>

<checks_run>
- plan_adherence: every change lands inside the tier `<files>`; nothing leaks to
  other crates. `ariadne-mcp` gains no workspace dep on `ariadne-daemon` — it
  embeds the one-file client per ADR-0015. Verified by reading the full diff +
  `git status` scope filter.
- correctness: `project_daemon` maps each `DaemonResponse` variant to `wire(&x)`;
  the `ariadne-core` report rows (`rows.rs`) mirror the `crate::types` MCP output
  structs field-for-field, name-for-name, order-for-order — so the daemon-path
  JSON is byte-identical to the cold path by construction (read both type
  families end-to-end). Connection policy (try → auto-spawn once → retry → cold)
  matches `<steps>` 3; revision handshake (`self.revision()` = `catalog.revision`)
  matches step 4; `to_core_kinds` maps all 8 edge-kind variants exhaustively.
- security: local socket only, no network/TCP (RD5). Auto-spawn uses `Command`
  with explicit args (`daemon start <root>`), no shell → no command injection.
  `read_frame` caps payloads at 64 MiB, rejecting a malformed length prefix
  (mirrors the daemon codec) [src: https://owasp.org/www-project-top-ten/ A03
  injection / resource-exhaustion guard]. `<root>` is the indexed-project path,
  not MCP-client input.
- performance: daemon round-trip is one short-lived socket exchange per query;
  read-only `Arc<Catalog>` unchanged. See INFO-1 (blocking IO on the async
  worker) — within tier-09's exit criteria (no latency SLO is gated here).
- architecture: `cargo test --test architecture` green — adapter-isolation
  invariant holds; client duplication sanctioned by ADR-0015/RD5/RD6.
- tests: 3 new tests use a REAL stub-daemon socket with INDEPENDENT framing
  (not the client's codec) as a protocol oracle, and a real rmcp child process —
  no boundary mocks. Failure modes are loud (`expect`/`panic`).
- exit_criteria: each verified independently — see `<verdict>`.
- verification commands re-run — all green (`<checks_run>` evidence below).
</checks_run>

<verification_output>
- `cargo fmt --all --check` → exit 0.
- `cargo nextest run -p ariadne-mcp` → 27/27 passed (13 v1 tool goldens
  unchanged + 3 new daemon tests + handshake/shutdown/smoke).
- `cargo test --test architecture` → 1 passed (`architecture_invariants_hold`).
- `cargo clippy -p ariadne-mcp --all-targets --all-features -- -D warnings` → clean.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` → clean.
- NOT runnable in-session: the manual VS Code / `/mcp` / kill-daemon-mid-session
  walk-through (`<verification>` bullet 2) needs a live daemon binary + editor;
  stated, not claimed green.
</verification_output>

<findings>
| id | category | severity | location | problem | fix |
|----|----------|----------|----------|---------|-----|
| INFO-1 | performance | INFO | daemon_client.rs:93-131; server.rs tool handlers | `try_query` does blocking std socket IO and an up-to-10s `std::thread::sleep` poll loop on the tokio worker thread inside async `#[tool]` fns; the server.rs doc claim "the same parallelism holds for the daemon path" overstates this — a blocked future pins its worker, so a missing-daemon burst can stall workers until spawn-wait clears. | Wrap the call in `tokio::task::spawn_blocking`, or use `interprocess`'s tokio feature, so the executor is never blocked. |
| INFO-2 | tests | INFO | tests/daemon_client.rs:56-80 | Only `find_definition` is exercised end-to-end on the daemon-served path; the other 12 `project_daemon` arms rely on type-shape parity (correct by construction) but have no daemon-served assertion. | Add a stub-daemon parity test (or a `project_daemon` unit test) per response variant. |
| INFO-3 | correctness | INFO | server.rs:391 (`project_daemon` Error arm) | A query-level `DaemonResponse::Error(msg)` maps to `internal_error(msg)`; the JSON-RPC error *code* matches the cold `McpError::into_rmcp` (also `internal_error`), but message-string parity with the cold `"not found: <x>"` wording is unverified and depends on the daemon's phrasing (tier-07). | Assert error-message parity once a real daemon serves, or normalise the not-found message at the projection boundary. |
</findings>

<verdict>
PASS — zero FAIL findings.

Exit criteria, each independently verified:
1. "Each MCP tool routes its query to the daemon over IPC instead of a cold redb
   read." ✓ — all 13 handlers call `self.daemon.try_query(...)` first and only
   reach `tools::*::handle` on `None` (server.rs diff, every handler).
2. "If no daemon is reachable, the MCP server auto-spawns one, or falls back to
   the v1 cold path." ✓ — `try_query` (daemon_client.rs:74-85) tries the socket,
   auto-spawns `<exe> daemon start <root>` and retries once, else returns `None`
   → cold path. In production `.mcp.json` launches `ariadne serve`, so
   `current_exe()` is the `ariadne` CLI and `daemon start` is a valid subcommand.
   `server_cold_fallback_when_daemon_unavailable` proves the fallback answer.
3. "All v1 MCP tool insta goldens pass unchanged in daemon-client mode." ✓ — the
   13 tool golden tests pass with the daemon-client routing compiled in (they
   take the cold fallback as no daemon is up; no `.snap.new` produced). Daemon-vs-
   cold byte-parity holds by construction (mirrored row types) and is shown live
   for `find_definition` (INFO-2 notes the coverage gap on the rest).
4. "`cargo nextest run -p ariadne-mcp` + architecture + clippy + fmt all green." ✓
   — see `<verification_output>`.
</verdict>

<next_steps>
None blocking. Optional follow-ups (non-gating): address INFO-1 by moving the
blocking IO off the async executor (`spawn_blocking` / tokio transport) before
the Block-B latency SLOs are gated in a later tier; close INFO-2's per-tool
daemon-served parity gap.
</next_steps>

<sources>
- Tier file: .claude/plans/post-v1-roadmap/tier-09-mcp-daemon-client.md
- Sibling plan: .claude/plans/post-v1-roadmap/plan.md (RD5, RD6, R-B3)
- ADR-0015 daemon-mode IPC (one-file client duplication): docs/adr/0015-daemon-mode-ipc.md
- interprocess 2.4.2 local socket: https://docs.rs/interprocess/2.4.2/interprocess/local_socket/index.html
- OWASP Top 10 (injection / resource exhaustion): https://owasp.org/www-project-top-ten/
- Code-review standard (health over perfection): https://google.github.io/eng-practices/review/reviewer/standard.html
</sources>
