---
tier_id: tier-08
title: MCP daemon client — ariadne-mcp queries the warm daemon, cold-path fallback
deps: [tier-06]
exit_criteria:
  - Each MCP tool routes its query to the daemon over IPC instead of a cold redb read.
  - If no daemon is reachable, the MCP server auto-spawns one, or falls back to the v1 cold path.
  - All v1 MCP tool insta goldens pass unchanged in daemon-client mode.
  - `cargo nextest run -p ariadne-mcp` + architecture + clippy + fmt all green.
status: pending
---

<context>
v1 spawns the MCP server per Claude session; each tool cold-reads redb and rebuilds the graph. With the warm daemon (tier-06) the MCP server becomes a thin client: it forwards queries over IPC to the always-warm graph (plan RD6). The v1 cold path is retained as a fallback so a missing daemon never breaks Claude. Full context: plan.md.
</context>

<files>
- crates/ariadne-mcp/src/adapters/daemon_client.rs — new: thin `interprocess` client + framing (the per-adapter client module from ADR-0014).
- crates/ariadne-mcp/Cargo.toml — modify: add `interprocess = "=2.4.2"`.
- crates/ariadne-mcp/src/ — modify: each `#[tool]` handler routes through the daemon client, with a cold-path fallback.
- crates/ariadne-mcp/tests/ — modify: run the v1 tool goldens in daemon-client mode against a spawned daemon.
</files>

<steps>
1. Failing test first (`ariadne-mcp` tests): start a daemon on a fixture, run an MCP tool through the server, assert the result equals the v1 golden — and assert it was served by the daemon, not a cold read. Red — no daemon client exists.
2. Implement `daemon_client.rs`: connect to the `.ariadne/` local socket, length-prefix frame a `DaemonRequest`, read the `DaemonResponse` [src: https://docs.rs/interprocess/2.4.2/interprocess/local_socket/index.html ; ADR-0014].
3. Connection policy: try the socket; on refused/missing, auto-spawn `ariadne daemon start` and retry once; if it still fails, fall back to the v1 cold redb path. Every tool stays answerable (risk R-B3).
4. Send the client's last-known redb `revision` in the handshake so the daemon refreshes if the client is ahead of the daemon's build (tier-06 handshake).
5. Route each `#[tool]` handler: build the matching `DaemonRequest`, send, map `DaemonResponse` to the existing tool output type — no output shape changes, so v1 goldens stay valid.
6. Run the full v1 MCP golden suite in daemon-client mode; add one explicit cold-fallback test (daemon unavailable → tool still returns the golden).
</steps>

<verification>
- `cargo nextest run -p ariadne-mcp` — all v1 tool goldens green in daemon-client mode + cold-fallback test green.
- Manual: register `ariadne` MCP in a Claude Code project, `/mcp` lists tools, run "blast radius of X" — served by the warm daemon; kill the daemon mid-session, re-run — cold fallback still answers.
- `cargo test --test architecture`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check` — green.
</verification>

<rollback>
`git checkout -- crates/ariadne-mcp`. The MCP server reverts to the pure v1 cold path.
</rollback>
