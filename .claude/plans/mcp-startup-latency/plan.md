---
slug: mcp-startup-latency
title: Cut MCP session-open latency — gitignore-aware watcher file-id cache + lazy cold catalog
created: 2026-05-31
owners: [user, claude]
review: [user, codex]
single_tier: false
tiers: [tier-01-watcher-gitignore-cache, tier-02-lazy-cold-catalog]
---

<context>
Opening a new Claude session spawns `ariadne serve --watch` (`.mcp.json`). Two
synchronous costs run before the MCP `initialize` handshake answers, so the
tools appear "slow to start":

1. `start_watcher` → `NotifyWatcher::start` → notify-debouncer-full
   `.watch(root, Recursive)`, which populates a `FileIdMap` by walking the
   **entire tree with `walkdir`**, stat-ing every entry — and notify is not
   gitignore-aware, so it scans `target/`, `.git/`, `node_modules/` too
   [src: crates/ariadne-watcher/src/adapters/notify.rs:80-84; file_id_map.rs:39-54].
2. `serve_stdio` → `build_server` → `Catalog::build`, an eager full read of the
   redb index into an in-RAM petgraph, on **every** session even when a warm
   daemon already holds the graph [src: crates/ariadne-mcp/src/serve.rs:78-82].

Measured this session (freshly-built schema-3 binary, repo = 318 files / 2998
symbols; `target/` = 35,494 files / 6.4 GB):
- `serve` (no watch): median 29 ms.
- `serve --watch` (full tree): median 182 ms — **+153 ms** spent in the watcher
  file-id scan of ignored dirs.
- cold-daemon first tool: ~200 ms; warm-daemon first tool: ~0 ms.
Both costs scale with project size (file count incl. ignored dirs; symbol+edge
count) → seconds on large repos.

Scope: eliminate both startup costs. Out of scope: recovering the wiped
tier-11 (git-history) / tier-12 (complexity) work and the redb schema-4 bump
(redone separately; the live index was re-built at schema 3 this session).
</context>

<constraints>
- TDD: each tier writes a failing test before implementation; no module-boundary
  mocks [src: .claude/plans/ariadne-core/plan.md `<constraints>`].
- Hexagonal: fix lives inside the owning adapter crate; no new driving→driving
  dep; ports unchanged [src: CLAUDE.md `<rules>` hexagonal boundary].
- No new dependency without sign-off — the watcher fix reuses the re-exported
  `notify_debouncer_full::file_id`, adding none [src: notify-debouncer-full-0.7.0/src/lib.rs:91].
- Determinism preserved: no behavioural change to event semantics or query
  results; only when/whether work runs [src: feedback_no_llm_features].
- Unit asserts are functional invariants, not wall-clock (anti-flake); wall-clock
  lives in criterion benches [src: feedback_validation_required].
</constraints>

<decisions>
- D1 — Watcher: replace the default `FileIdMap` with a **gitignore-aware custom
  `FileIdCache`** whose `add_path(_, Recursive)` walks via `ignore::WalkBuilder`
  (skipping `target/`/`node_modules/`/`.ariadne/` + `.gitignore`) instead of bare
  `walkdir`. Rejected: `NoCache` (loses rename-pair stitching → quality drop);
  deferring watcher start off the critical path (still wastes the full scan +
  opens a missed-event window). The custom cache keeps rename stitching for the
  indexed file set while never stat-ing ignored dirs [src: cache.rs:8-32;
  file_id_map.rs:39-54; reconcile.rs:18,56 already uses `ignore::WalkBuilder`].
- D2 — Wire via `new_debouncer_opt::<_,_,C>` (replaces `new_debouncer`) in
  `NotifyWatcher::start`. Single chokepoint: both `ariadne serve --watch` and the
  daemon's own watcher route through it, so one fix cuts session-open *and*
  cold-daemon start [src: lib.rs:639; serve.rs:43; commands/daemon.rs:43-45].
- D3 — Lazy catalog: hold `tokio::sync::OnceCell<Arc<Catalog>>`, built on the
  first cold-fallback miss via `spawn_blocking`; never built when the daemon
  answers. Read the redb `revision` cheaply at startup (single `KEY_REVISION`
  read, transient handle dropped) to preserve the daemon staleness handshake.
  Rejected: sending `revision: 0` like the one-shot CLI client — loses MCP
  staleness precision for a long-lived session [src: server.rs:88,112-113,130;
  adapters/redb/tables.rs:9 KEY_SCHEMA_VERSION/KEY_REVISION; cli daemon_client.rs:64-68].
- D4 — Pass project root through `ServeOpts`/`AriadneServer::new` so the
  `DaemonClient` no longer reads `catalog.root` (catalog may be unbuilt)
  [src: server.rs:88].
</decisions>

<architecture>
- ariadne-watcher (driven adapter): new `domain`/adapter type
  `GitignoreFileIdCache` implementing `FileIdCache`; `NotifyWatcher::start`
  constructs it from the `Ignore` it already receives and feeds it to
  `new_debouncer_opt`. No public-API change; callers (CLI serve, daemon)
  unchanged. [tier-01]
- ariadne-mcp (driving adapter): `AriadneServer` swaps `catalog: Arc<Catalog>`
  for a lazy cell + a cheap `revision: u64`; `build_server` stops calling
  `Catalog::build`. Cold-fallback arms build-on-demand. [tier-02]
- The two tiers touch disjoint crates and share no symbols → independent; either
  order is valid.
</architecture>

<tech_inventory>
| tech | version | doc fetched this session |
|------|---------|--------------------------|
| notify-debouncer-full | 0.7.0 | https://docs.rs/notify-debouncer-full/latest + local src cache.rs/file_id_map.rs/lib.rs |
| notify | 8.2 | RecursiveMode (transitive, local src) |
| ignore | 0.4.25 | WalkBuilder — already a watcher dep [src: crates/ariadne-watcher/Cargo.toml:21] |
| file-id | 0.2.3 | re-exported as `notify_debouncer_full::file_id` (no direct dep) [src: lib.rs:91] |
| tokio | 1.52.3 | OnceCell + spawn_blocking |
| rmcp | 1.7.0 | initialize handshake (unchanged) |

Context7 quota was exhausted this session; notify-debouncer-full behaviour was
verified against the pinned crate source in the local cargo registry.
</tech_inventory>

<risks>
| risk | likelihood | mitigation | owner |
|------|-----------|------------|-------|
| FSEvents still delivers events for ignored paths after start | high | per-event `add_path` is one stat; `dispatch` already drops ignored paths before the sink [src: notify.rs:103] | tier-01 |
| Concurrent first cold-miss tool calls each build the catalog | med | `OnceCell::get_or_try_init` builds once; others await | tier-02 |
| Timing tests flake in CI | high | assert invariants (cache excludes ignored paths; catalog unbuilt on warm path), wall-clock only in benches | both |
| Lazy revision drifts vs daemon | low | cheap `KEY_REVISION` read at startup keeps handshake exact | tier-02 |
</risks>

<verification>
- Re-run the session probe (commands recorded in each tier `<verification>`):
  `serve --watch` session-open must no longer scale with ignored-dir file count,
  and warm-daemon session-open must not invoke `Catalog::build`.
- Workspace gates green: `cargo nextest run --workspace`, clippy `-D warnings`,
  `cargo fmt --all --check`, `cargo deny check`, `cargo test --test architecture`
  [src: CLAUDE.md `<commands>`].
- Memory probe per Salsa/graph-touching tier where applicable (R1) — neither
  tier grows an in-RAM table; note "no delta" explicitly [src: CLAUDE.md `<rules>`].
</verification>

<sources>
- notify-debouncer-full 0.7.0 — https://docs.rs/notify-debouncer-full/latest/notify_debouncer_full/
- notify-debouncer-full source — ~/.cargo/registry/.../notify-debouncer-full-0.7.0/src/{cache.rs,file_id_map.rs,lib.rs}
- ignore 0.4 WalkBuilder — https://docs.rs/ignore/latest/ignore/struct.WalkBuilder.html
- tokio OnceCell — https://docs.rs/tokio/latest/tokio/sync/struct.OnceCell.html
- Hexagonal boundary — CLAUDE.md `<architecture>` / `<rules>`
</sources>
