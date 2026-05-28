---
tier_id: tier-06
audited: 2026-05-28
verdict: PASS
commit: 623b4b247b6714c252b5e32a06a20db94d360d2d
---

<scope>
Tier-06 daemon skeleton — new `ariadne-daemon` driving-adapter crate hosting a long-running process bound to an `interprocess` 2.4.2 local socket; pure lifecycle policy (`DaemonPaths`/`Pid`/`DaemonStatus`/`reclaim_decision`) in `domain/lifecycle.rs`; transport + filesystem/process orchestration in `adapters/ipc.rs`; pure `DaemonRequest::Ping`/`DaemonResponse::Pong` wire types in `ariadne-core/domain/daemon.rs`; `ariadne daemon {start,stop,status}` wired in `ariadne-cli`; `tests/architecture.rs` adds `ariadne-daemon` to `DRIVING_ADAPTERS`; ADR-0015 records the D10 reversal. Audited against working-tree state (`HEAD = 623b4b2`, tier-06 work uncommitted) scoped to the tier `<files>` plus the new crate files the build created (`src/adapters/mod.rs`, `src/domain/mod.rs`, `tests/daemon.rs`).
</scope>

<checks_run>
Tier `<verification>` re-run end-to-end:
- `cargo nextest run -p ariadne-daemon` → 5/5 PASS (`domain::lifecycle::tests::{paths_root_under_ariadne_dir, pid_parse_trims_and_rejects_garbage, decision_is_alive_then_reclaim_then_fresh}`, `daemon::{ping_roundtrips_and_stop_is_clean, stale_pidfile_and_socket_are_reclaimed}`).
- `cargo test --test architecture` → 1/1 PASS. Re-ran with `ariadne-daemon` source touched to force a fresh compile, not a cache hit.
- `cargo clippy --workspace --all-targets -- -D warnings` → exit 0, 0 warnings/errors (pedantic is a workspace `warn` lint, so `-D warnings` promotes it; daemon crate re-linted from a `touch` to confirm a real check, not skipped). The `--workspace --all-targets` compile is also positive evidence the rest of the workspace still builds with the new crate wired in.
- `cargo fmt --all --check` → clean.

Manual end-to-end (tier `<verification>` step 2), against the built `target/debug/ariadne`:
- `ariadne daemon start <tmp>` → `daemon started (pid 38464)`; `.ariadne/` then holds `daemon.pid` + a real Unix domain socket (`srwxr-xr-x daemon.sock`).
- `ariadne daemon status <tmp>` → `daemon running (pid 38464)`.
- `ariadne daemon stop <tmp>` → `daemon stopped`; both `daemon.pid` and `daemon.sock` removed (`.ariadne/` empty); `status` then reports `daemon stopped`; `pgrep -fl "ariadne daemon"` → no lingering processes.
- Edge cases: idempotent `stop` on a not-running daemon → `Ok`/`daemon stopped`; double `start` → `daemon already running (pid …)`, exit 1 (`reclaim_decision` → `AlreadyRunning`); `default_value = "."` resolves the root from cwd (`status` with no arg from inside the project → `daemon running`). No orphan processes after cleanup.

Plan adherence reviewed end-to-end:
- `crates/ariadne-daemon/Cargo.toml` — deps exactly `ariadne-core` + `thiserror` + `interprocess = "=2.4.2"`, dev-dep `tempfile`; matches `<files>` and the RD5 tech inventory.
- `crates/ariadne-daemon/src/lib.rs` — re-export-only façade (`pub mod` + `pub use`, `#![deny(missing_docs)]`); no logic (folder-layout rule 3).
- `src/domain/lifecycle.rs:104-117` — `reclaim_decision(pidfile_present, socket_present, alive)` is pure: `alive → AlreadyRunning`, else residue → `Reclaim`, else `Fresh`; unit-tested at 144-165. Liveness is the `Ping` handshake, not a PID probe — exactly the R-B3 mitigation and the ADR-0015 `kill(0)`-rejection rationale.
- `src/adapters/ipc.rs:75-98` — length-prefixed framing (4-byte BE length + 1 discriminant byte) with a `MAX_FRAME = 1024` cap that rejects an oversized length prefix before allocation (DoS guard).
- `ipc.rs:227-279` (`serve`) — reclaim decision → write pidfile → bind listener → one accept/handle/`pidfile_is_ours` shutdown check per connection → `remove_residue`. Bind retries once after clearing a leftover socket on `AddrInUse`. A malformed client frame is swallowed (`let _ = serve_connection`) so it cannot kill the daemon.
- `ipc.rs:314-327` (`stop`) — removes the pidfile (the shutdown signal), wakes the blocking accept loop with one `Ping`, waits for the socket to stop answering, then clears residue; idempotent when already down.
- `ipc.rs:337-381` (`start`) — `RUN_ENV`-guarded re-exec of `current_exe()` with a canonicalized absolute root, detached (stdio nulled), then `wait_until_up`; the re-executed child `serve`s. No `fork`/daemonize dep — matches ADR-0015.
- `crates/ariadne-core/src/domain/daemon.rs` — `DaemonRequest`/`DaemonResponse` are pure enums carrying no serialization; deliberately not `#[non_exhaustive]` (the single transport adapter matches exhaustively), matching the ADR. `domain/mod.rs` + `lib.rs` register and re-export them.
- `crates/ariadne-cli/src/{main.rs,commands/daemon.rs,commands/mod.rs}` — `Daemon` subcommand with `{Start,Stop,Status}` actions; handlers are thin `anyhow`-wrapped shims over `ariadne_daemon`. CLI is the only crate depending on `ariadne-daemon` (composition root, ADR-0007).
- `tests/architecture.rs:49` — `ariadne-daemon` added to `DRIVING_ADAPTERS`; the invariant asserts only `ariadne-cli` may depend on it. Verified live.

Architecture / dependency hygiene:
- `cargo tree -p ariadne-daemon` shows only `ariadne-core`, `interprocess` (→ `libc`, pure-Rust `recvmsg`/`widestring`, build-only `doctest-file`), `thiserror`. New Cargo.lock packages: `ariadne-daemon`, `interprocess`, `recvmsg`, `widestring`, `doctest-file` — all pure-Rust + `libc` bindings; no cgo, no async runtime. `cargo tree -i tokio -e normal` is empty → `interprocess`'s optional tokio feature is off; D5 holds.
- `thiserror` `DaemonError` in the adapter's public API; `anyhow` confined to `ariadne-cli`. No `interprocess`/`std::io` types leak past the boundary (errors stringified at `From<io::Error>`).
- Diff confined to the tier `<files>`; the only tier-file edit is the `status: pending → completed` flip + `completed:` date. No exit-criterion text weakened.

Exit-criteria reconciliation:
1. *`ariadne-daemon` crate hosts a long-running process on an `interprocess` local socket.* ✓ — crate exists, builds, E2E created a real Unix domain socket and served over it.
2. *`ariadne daemon {start,stop,status}` manage the process via a pidfile + socket under `.ariadne/`.* ✓ — manual walk created/removed `.ariadne/daemon.{pid,sock}`.
3. *Client `Ping` → `Pong`; stale socket/pidfile detected and reclaimed.* ✓ — `ping_roundtrips_and_stop_is_clean` + `stale_pidfile_and_socket_are_reclaimed` (planted bogus PID + dangling socket file, daemon rebinds and overwrites the pidfile).
4. *ADR-0015 records the D10 reversal; `tests/architecture.rs` classifies `ariadne-daemon` as a driving adapter.* ✓ — ADR-0015 present and Accepted; arch test re-run green with the new entry.
5. *`cargo nextest run -p ariadne-daemon` + architecture + clippy + fmt all green.* ✓ — all four re-run green this audit.
</checks_run>

<findings>
| id | category | severity | location | problem | fix | sources |
|---|---|---|---|---|---|---|
| F1 | correctness | INFO | `crates/ariadne-daemon/src/adapters/ipc.rs:254-259` | On the `AddrInUse` bind-retry branch, a failing second `create_sync()?` propagates without removing the pidfile, whereas the sibling error branch (260-263) does remove it — leaving a stale pidfile on that one failure path. | Remove the pidfile before returning in the retry-failure case too (mirror lines 260-263). No functional impact: the next `start`/`serve` sees `pidfile_present && !alive` and `Reclaim`s it. | n/a |
| F2 | reliability | INFO | `crates/ariadne-daemon/src/adapters/ipc.rs:366-380` | `start` does not observe the detached child's early exit; a child that dies immediately (only reachable via a `start`/`start` race, since the parent already gates on `AlreadyRunning`) makes the parent block the full 10s `STARTUP_TIMEOUT` and report `Timeout` rather than the real cause. | Tier-07/08 hardening (session leadership, child-exit detection) is explicitly deferred by ADR-0015 `<consequences>` + risk R-B3; acceptable for the skeleton. | ADR-0015 `<consequences>`; plan.md risk R-B3 |

No FAIL findings.
</findings>

<verdict>
PASS. The daemon skeleton is correctly implemented: a pure lifecycle policy split cleanly from the `interprocess` transport, a length-prefixed `Ping`/`Pong` protocol with an allocation cap, handshake-based liveness and stale-residue reclamation, and a thin CLI composition-root wiring. All five exit criteria are independently verified — unit + integration tests, the architecture invariant, clippy, fmt, and a real `start → status → stop` walk that creates and tears down the socket + pidfile with no orphan processes. Dependencies stay pure-Rust (no tokio, no cgo), and the diff stays inside the tier `<files>`. Two INFO findings (a pidfile-cleanup asymmetry on a self-healing failure path; deferred child-exit detection) — neither blocks the tier.
</verdict>

<next_steps>
- Tier-06 is shippable. No code changes required to merge.
- Optional `fix` follow-up for F1: mirror the pidfile cleanup into the `AddrInUse` retry-failure branch of `serve` (`ipc.rs:254-259`).
- Carry F2 forward as the tier-07/08 hardening item already named in ADR-0015 (`fork`-less detach → child-exit detection, session leadership, idle-reap) so a fast-failing daemon surfaces its cause instead of a 10s timeout.
</next_steps>

<sources>
- tier-06 plan: `.claude/plans/post-v1-roadmap/tier-06-daemon-skeleton.md`
- post-v1 plan (RD5, RD6, risk R-B3): `.claude/plans/post-v1-roadmap/plan.md`
- ADR-0015 (daemon mode + local-socket IPC): `docs/adr/0015-daemon-mode-ipc.md`
- composition-root precedent: `docs/adr/0007-cli-composition-root.md`
- interprocess 2.4.2 local socket API: https://docs.rs/interprocess/2.4.2/interprocess/local_socket/index.html
- Code-review standard (block only on real defects; INFO otherwise): https://google.github.io/eng-practices/review/reviewer/standard.html
</sources>
</content>
</invoke>
