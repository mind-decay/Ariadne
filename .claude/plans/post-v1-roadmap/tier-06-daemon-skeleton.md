---
tier_id: tier-06
title: Daemon skeleton — ariadne-daemon crate, interprocess local socket, lifecycle
deps: []
exit_criteria:
  - A new `ariadne-daemon` crate hosts a long-running process bound to an `interprocess` local socket.
  - `ariadne daemon {start,stop,status}` manage the process via a pidfile + socket under `.ariadne/`.
  - A client `Ping` over the socket receives `Pong`; a stale socket/pidfile is detected and reclaimed.
  - ADR-0015 records the D10 reversal; `tests/architecture.rs` classifies `ariadne-daemon` as a driving adapter.
  - `cargo nextest run -p ariadne-daemon` + architecture + clippy + fmt all green.
status: pending
---

<context>
v1 D10 runs an MCP process per Claude session, cold-reading redb each time. Block B reverses this into a warm daemon (plan RD5/RD6). This tier ships only the skeleton: the crate, the `interprocess` listener, lifecycle management, and a trivial `Ping`/`Pong`. The warm graph (tier-07) and watcher loop (tier-08) come later. Full context: plan.md.
</context>

<files>
- crates/ariadne-daemon/Cargo.toml — new: deps `ariadne-core`, `interprocess = "=2.4.2"`, `thiserror`; auto-joins the `crates/*` workspace.
- crates/ariadne-daemon/src/lib.rs — new: façade, re-exports only.
- crates/ariadne-daemon/src/domain/ — new: lifecycle state machine + pidfile/socket-path policy (pure).
- crates/ariadne-daemon/src/adapters/ipc.rs — new: `interprocess` local-socket listener (one file, one tech).
- crates/ariadne-daemon/src/errors.rs — new: `thiserror` `DaemonError` enum.
- crates/ariadne-core/src/domain/ — modify: add pure `DaemonRequest::Ping` / `DaemonResponse::Pong` wire types.
- crates/ariadne-cli — modify: add a `daemon {start,stop,status}` subcommand (CLI is the composition root, ADR-0007).
- tests/architecture.rs — modify: add `ariadne-daemon` to `DRIVING_ADAPTERS`.
- docs/adr/0015-daemon-mode-ipc.md — new: per `docs/adr/_template.md`.
</files>

<steps>
1. Failing test first (`ariadne-daemon` tests): spawn the daemon, connect over the socket, send `Ping`, assert `Pong`, stop it cleanly. Red — the crate does not exist.
2. Scaffold the crate per `docs/folder-layout.md` (`lib.rs` façade, `domain/`, `adapters/`, `errors.rs`); `members = ["crates/*"]` auto-includes it.
3. Add `DaemonRequest`/`DaemonResponse` pure enums to `ariadne-core` (start with `Ping`/`Pong`); serialization stays out of core — the adapter owns framing.
4. Implement `adapters/ipc.rs`: bind a named local socket via `interprocess::local_socket` `ListenerOptions` — Unix domain socket on Unix, named pipe on Windows, one API [src: https://docs.rs/interprocess/2.4.2/interprocess/local_socket/index.html]. Socket name derives from the `.ariadne/` path.
5. Lifecycle (`domain/`): `start` writes a pidfile + socket under `.ariadne/`; `stop` signals shutdown and removes both; `status` reports running/stopped. On `start`, a pidfile whose PID is dead or whose socket fails a `Ping` handshake is treated as stale and reclaimed (risk R-B3).
6. Length-prefix frame `Ping`/`Pong` over the stream; one accept loop, one connection handler.
7. Wire `ariadne daemon {start,stop,status}` in `ariadne-cli`.
8. Amend `tests/architecture.rs`: `ariadne-daemon` is a driving adapter — nothing in the workspace may depend on it.
9. Write ADR-0015: decision = reverse D10 to a warm daemon; transport = `interprocess` local socket; rejected = TCP loopback, D-Bus. Record the IPC-topology question deferred to tier-07.
</steps>

<verification>
- `cargo nextest run -p ariadne-daemon` — start/ping/stop + stale-pidfile-reclaim tests green.
- Manual: `ariadne daemon start` → `ariadne daemon status` reports running → `ariadne daemon stop` → socket + pidfile gone.
- `cargo test --test architecture`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check` — green.
</verification>

<rollback>
`git checkout -- .` and `rm -rf crates/ariadne-daemon docs/adr/0015-daemon-mode-ipc.md`. No prior tier depends on the daemon; v1 cold-path is untouched.
</rollback>
