---
tier_id: tier-02
audited: 2026-05-31
verdict: PASS
commit: 7e948ab0425c7478afdbbf18b7fae9a1b71e0c31
---

<scope>
Tier-02 "Lazy cold catalog" — defer the in-RAM `Catalog`/petgraph build off
session-open so a warm-daemon session pays O(1) startup, building the cold
fallback once on the first daemon miss.

Diff under review (scoped to the tier `<files>` + the build's necessary ripple):
- `crates/ariadne-mcp/src/serve.rs` — `build_server` drops `Catalog::build`,
  reads `storage.revision().0` from a transient handle, passes
  `(storage_path, root, revision)` to `AriadneServer::new`.
- `crates/ariadne-mcp/src/server.rs` — `catalog: Arc<OnceCell<Arc<Catalog>>>` +
  `revision: u64` + `root: PathBuf`; lazy `catalog()` (spawn_blocking +
  get_or_try_init), sync `catalog_arc()`, `catalog_built()`; all 13 cold-arms
  call `self.catalog().await?`; `revision()` returns the stored field.
- `crates/ariadne-mcp/tests/lazy_catalog.rs` — new behaviour test (warm = no
  build; cold = build-once + answer).
- `crates/ariadne-mcp/benches/concurrent.rs` — signature-change ripple (not in
  `<files>`; justified, see findings).
- `crates/ariadne-storage/src/adapters/redb/mod.rs` — listed in `<files>`;
  unchanged (the cheap `revision()` read already exists on the port).
</scope>

<checks_run>
All commands re-run at HEAD 7e948ab; full output captured.
- `cargo nextest run -p ariadne-mcp` → **102 passed, 0 failed** (incl. the two
  new `lazy_catalog` tests and the pre-existing async cold-fallback test).
- `cargo clippy -p ariadne-mcp --all-targets -- -D warnings` → **EXIT=0**.
- `cargo fmt --all --check` → **EXIT=0**.
- `cargo test --test architecture` → **EXIT=0** (no port/domain leak from the
  `AriadneServer::new` signature change).
- Workspace consistency: `grep` confirms the only `AriadneServer::new` callers
  are `serve.rs:86` and `benches/concurrent.rs:53` (both updated); no
  `.catalog.revision` or stray eager `Catalog::build` remains; no `ariadne-cli`
  / `ariadne-e2e` caller of the changed signature.
- Storage `revision()` cost: `redb/mod.rs:151-153` is an `AtomicU64` load,
  seeded from a single `KEY_REVISION` lookup at `open` (`mod.rs:90`,
  `tables.rs:10`) — no graph traversal. Confirms D3 "cheap startup-read".
- End-to-end wall-clock probe (`/tmp/probe4.py`): the session harness is
  ephemeral and no longer present, so the timing arm could not be re-run. The
  invariant it would demonstrate — no `Catalog::build` on the warm path — is
  proven deterministically by `reachable_daemon_answers_without_building_catalog`
  (catalog cell empty after a daemon-served call), which the plan's own
  anti-flake constraint designates as the canonical check (wall-clock lives in
  benches, not gates). Memory R1 "0 on warm path" follows: the empty cell means
  the petgraph is never allocated when the daemon answers.
</checks_run>

<findings>
| id | category | severity | location | problem | fix |
|----|----------|----------|----------|---------|-----|
| INFO-1 | correctness | INFO | server.rs:124-137 | The sync `catalog_arc()` has no build lock: two concurrent first callers can each run `Catalog::build`, the loser's catalog being dropped — wasted CPU, never a wrong result. Confined to test/bench callers; the bench forces the build once before fan-out, so the race never triggers in practice, and the doc comment already records it. | None required; if ever called concurrently in production, route through the async `catalog()` (which is race-free via `get_or_try_init`). |
</findings>

<verdict>
**PASS.** Zero FAIL findings. Every exit criterion is independently verified:

1. *build_server no longer calls `Catalog::build`; initialize without reading the
   full index* — `serve.rs:73-91` reads only `storage.revision().0` (atomic
   load) from a dropped transient handle; no graph work. ✓
2. *Lazily-built catalog (OnceCell) + cheap startup revision; built only on a
   cold miss* — `server.rs:54-113` holds `Arc<OnceCell<Arc<Catalog>>>`,
   `revision: u64`, `root: PathBuf`; `catalog()` (`server.rs:158-175`) builds via
   `spawn_blocking` + `get_or_try_init`; all 13 cold-arms gate on it. ✓
3. *Test proves warm = no build, cold = build-once + correct answer* —
   `lazy_catalog.rs` `reachable_daemon_answers_without_building_catalog`
   (daemon served, `!catalog_built()`) and `cold_access_builds_catalog_once_and_answers`
   (`Arc::ptr_eq` idempotence + seeded-index answer); the production *async*
   cold-fallback arm is additionally exercised by
   `daemon_client.rs::server_cold_fallback_when_daemon_unavailable` (real binary,
   autospawn off). All green. ✓
4. *nextest -p ariadne-mcp, clippy -D warnings, fmt --check, architecture all
   green* — re-run, all EXIT=0 / 102 passed. ✓

Architecture/decision adherence: change is contained to the `ariadne-mcp`
driving adapter; `tokio::sync::OnceCell` is an existing dep (D3); root threaded
through `new` so `DaemonClient` no longer reads `catalog.root` (D4); storage
port unchanged. No smuggled dependency or pattern.

Plan-adherence note (not a defect): `redb/mod.rs` was listed in `<files>` but
needed no change — `revision()` was already public on the port, exactly the
"confirm; expose if not already" the step allowed. `benches/concurrent.rs` is
outside `<files>` but is the unavoidable ripple of the public `new` signature
change, explicitly anticipated by step 6; the edit only retargets the signature
and forces the lazy build once up front so the timed loops measure query
latency, not a cold build.
</verdict>

<next_steps>
None. Tier-02 is accepted. The lone INFO is non-blocking and already documented
in-code; no rework required. Commit/push gate may proceed for this tier.
</next_steps>

<sources>
- redb `revision()` cost — `crates/ariadne-storage/src/adapters/redb/mod.rs:90,151-153`; `tables.rs:10` (`KEY_REVISION`).
- tokio `OnceCell::get_or_try_init` (build-once, error-not-cached) — https://docs.rs/tokio/latest/tokio/sync/struct.OnceCell.html
- Google eng-practices, reviewer standard (code-health-over-perfection) — https://google.github.io/eng-practices/review/reviewer/standard.html
- Tier spec — `.claude/plans/mcp-startup-latency/tier-02-lazy-cold-catalog.md`; plan `<decisions>` D3/D4 — `.claude/plans/mcp-startup-latency/plan.md`.
</sources>
