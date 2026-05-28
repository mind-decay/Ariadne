# ADR-0015: Daemon Mode And Local-Socket IPC

<status>
Accepted
Date: 2026-05-28
Decider: claude
</status>

<context>
v1 decision D10 runs Ariadne as a per-session MCP stdio process: every Claude
session spawns a fresh `ariadne` that cold-reads redb and rebuilds its
in-RAM graph before answering. The post-v1 roadmap reverses this (RD5/RD6):
a long-running daemon owns the warm graph and thin clients query it
[src: .claude/plans/post-v1-roadmap/plan.md RD5, RD6].

This ADR fixes the daemon's process model and transport for the tier-06
skeleton. Forces: reliability (a second daemon must never race the first; a
crashed daemon must not wedge the next start), efficiency (IPC overhead is on
the warm-query hot path whose target tightens to p95 <10ms — RD6),
maintainability (the transport must abstract Unix/Windows behind one API so
the daemon stays a single crate), and the standing constraint that the
critical path is pure-Rust with no second runtime
[src: .claude/plans/post-v1-roadmap/plan.md `<constraints>`; ADR-0002 D5].
The warm graph itself and the watcher loop are out of scope here (tiers 07/08).
</context>

<decision>
Add `ariadne daemon {start,stop,status}`, hosted by a new driving-adapter
crate `ariadne-daemon`. Clients and daemon communicate over an `interprocess`
2.4.2 local socket addressed by `<root>/.ariadne/daemon.sock`. The
request/response wire types (`DaemonRequest`/`DaemonResponse`, `Ping`/`Pong`
for the skeleton) are pure and live in `ariadne-core/domain`; the transport
adapter owns all framing.
</decision>

<rationale>
- **`interprocess` local socket — maintainability + reliability.** One
  `local_socket` API maps to a Unix domain socket on Unix/macOS and a named
  pipe on Windows, so the daemon needs no per-OS transport code
  [src: https://docs.rs/interprocess/2.4.2/interprocess/local_socket/index.html].
  It is pure-Rust, honouring D5 (no cgo / second runtime).
- **Socket under `.ariadne/` — reliability.** Anchoring the socket (and
  pidfile) in the project's `.ariadne/` directory isolates one daemon per
  project and makes the leftover socket file the concrete thing a stale-start
  reclaims [src: tier-06 step 4].
- **Wire types in `ariadne-core` — maintainability.** They are pure data with
  no IO, so they belong in the domain interior; keeping serialization in the
  adapter stops a codec choice from leaking past the hexagonal boundary
  [src: ADR-0001; tier-06 step 3].
- **Liveness by `Ping` handshake — reliability + efficiency.** A start treats
  a pidfile with no live responder as stale and reclaims it. Liveness is
  decided by the `Ping`/`Pong` handshake rather than an OS `kill(0)` probe: a
  dead process cannot answer, the handshake is portable, and it adds no
  `libc`/`nix` dependency outside the tech inventory (risk R-B3)
  [src: .claude/plans/post-v1-roadmap/plan.md risk R-B3, `<tech_inventory>`].
- **Shutdown via pidfile removal — reliability.** `stop` removes the pidfile,
  then opens one connection to wake the blocking accept loop; the daemon
  answers, notices its pidfile is gone, and exits. This keeps the wire
  protocol to `Ping`/`Pong` and needs no signal/`kill` syscall
  [src: tier-06 steps 5, 6].
</rationale>

<alternatives>
- **TCP loopback** — rejected: binds a port (conflicts, exhaustion) and can
  trigger host firewall prompts, hurting reliability on developer machines
  [src: .claude/plans/post-v1-roadmap/plan.md RD5].
- **D-Bus** — rejected: Linux-only, so it breaks the cross-platform single
  binary [src: RD5].
- **`kill(0)` PID-liveness probe** — rejected: requires a `libc`/`nix`
  dependency not in the tech inventory; the `Ping` handshake subsumes the
  dead-PID case [src: `<tech_inventory>`].
- **Filesystem-socket-only addressing** is the skeleton's choice on the Unix
  test target; the same `interprocess` API reaches Windows named pipes via the
  namespaced backend, so a Windows port switches the backend, not the crate.
</alternatives>

<consequences>
- A new driving-adapter crate `ariadne-daemon` joins the workspace. The
  architecture invariant adds it to `DRIVING_ADAPTERS`: nothing but the
  composition root `ariadne-cli` may depend on it
  [src: tests/architecture.rs; ADR-0007].
- `ariadne-core` gains `domain::daemon` (`DaemonRequest`/`DaemonResponse`).
  These enums are deliberately **not** `#[non_exhaustive]` so a future variant
  fails to compile until the single transport adapter frames it.
- **IPC topology — resolved by tier-07.** The protocol stays split exactly as
  the skeleton placed it: the pure `DaemonRequest`/`DaemonResponse` wire types
  (tier-07 added one variant per v1 read query plus a `revision` handshake)
  live in `ariadne-core/domain`; the `interprocess` transport, the postcard
  framing codec, and the warm-graph query dispatcher all live in
  `ariadne-daemon`; each driving-adapter client (mcp/cli/lsp — tiers 09/10/16)
  embeds a thin `daemon_client` that calls the public `ariadne_daemon::query`.
  A shared `ariadne-ipc` crate stays **deferred**: it is warranted only if
  per-adapter client duplication later exceeds one file, at which point it
  ships with an explicit `tests/architecture.rs` exception (precedent: ADR-0007
  carved out the composition root). tier-07 added no such duplication — the
  daemon is the sole transport owner and `ariadne-core` already holds the wire
  types — so no new crate and no invariant exception were introduced
  [src: .claude/plans/post-v1-roadmap/tier-07-daemon-warm-graph.md step 3].
- **Warm-graph staleness handshake.** Every request carries the client's
  last-observed redb `revision`; when it exceeds the revision the daemon built
  its warm graph from, the daemon rebuilds (reopening redb transiently, then
  dropping the handle) before answering (risk R-B2). The daemon holds the
  single-open redb lock only during build/refresh, never while idle, so an
  external indexer can advance the file between requests
  [src: .claude/plans/post-v1-roadmap/tier-07-daemon-warm-graph.md steps 4, 6].
- Detaching the daemon uses a re-exec of the `ariadne` binary guarded by the
  `ARIADNE_DAEMON_RUN` environment marker (no `fork`/daemonize dependency).
  Hardening (session leadership, idle-reap) is left to tier-07/08.
- The v1 cold per-session path is untouched; daemon mode is additive, and a
  later tier adds the auto-fallback to cold mode when no daemon is reachable
  (RD6).
</consequences>

<sources>
- `[src: .claude/plans/post-v1-roadmap/plan.md RD5, RD6, risk R-B3]`
- `[src: .claude/plans/post-v1-roadmap/tier-06-daemon-skeleton.md]`
- `[src: https://docs.rs/interprocess/2.4.2/interprocess/local_socket/index.html]`
- `[src: docs/adr/0001-architecture-style.md]`
- `[src: docs/adr/0007-cli-composition-root.md]`
</sources>
