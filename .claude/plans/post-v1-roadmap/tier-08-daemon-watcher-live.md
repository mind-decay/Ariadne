---
tier_id: tier-08
title: Daemon live updates — watcher loop feeds incremental warm-graph invalidation
deps: [tier-07b]
exit_criteria:
  - The daemon owns the `ariadne-watcher` event loop and reacts to filesystem changes.
  - A file edit re-derives the affected subset via Salsa and applies a delta to the warm graph.
  - redb deltas are persisted; the daemon revision advances on each applied change.
  - A proptest of random edit sequences shows zero divergence between the warm graph and a full rebuild.
  - `cargo nextest run -p ariadne-daemon` + architecture + clippy + fmt all green.
status: completed
completed: 2026-05-30
---

<context>
tier-07 made the daemon serve queries from a warm graph built once at startup; tier-07a/07b then built the shared, edit-stable per-file derivation in `ariadne-salsa` (`rederive_file`/`forget_file` + diff-aware `commit_revision`, divergence-0 vs full rebuild — plan RD11/RD12). This tier keeps the warm graph live: the daemon hosts the watcher, and every filesystem change flows through the shared salsa re-derivation into an incremental `apply_delta` on the warm petgraph (plan RD6, dataflow in plan `<architecture>`). The earlier blocker — derivation locked in the CLI and stubbed salsa — is resolved by tier-07b. Full context: plan.md.
</context>

<files>
- crates/ariadne-daemon/Cargo.toml — modify: add `ariadne-watcher` (fs events), `ariadne-salsa` (the `rederive_file`/`commit_revision` driver), `ariadne-parser` + `ariadne-scip` (the daemon parses changed files — it is the warm-mode composition root, ADR-0007). The tier-07 Cargo.toml deferral note on `ariadne-salsa` is resolved here [src: crates/ariadne-daemon/Cargo.toml:20].
- crates/ariadne-daemon/src/domain/ — modify: the update pipeline (fs event → parse → `rederive_file` → `Changeset` delta → `GraphIndex::apply_delta` → revision bump).
- crates/ariadne-daemon/src/ — modify: own the watcher event loop alongside the IPC accept loop; hold the `AriadneDb` next to the warm `GraphIndex` behind the existing `RwLock`.
- crates/ariadne-daemon/tests/ — new: live-update integration test + incrementality proptest.
</files>

<steps>
1. Failing test first (`ariadne-daemon` tests): start the daemon on a fixture, mutate a file on disk, wait for debounce, query the affected symbol over IPC, assert the warm graph reflects the edit. Red — the daemon has no watcher.
2. Read the tier-07b driver API (`AriadneDb::rederive_file`/`forget_file` + diff-aware `commit_revision` returning a `Changeset`/`RevisionId`) and v1 tier-07's `GraphIndex::apply_delta` (+`EdgeDelta`) [src: crates/ariadne-salsa/src/db.rs ; crates/ariadne-graph/src/build.rs:121,247].
3. Spawn the `ariadne-watcher` `NotifyWatcher` inside the daemon; debounced fs events feed an internal channel consumed by the update pipeline. The IPC accept loop and the watcher loop run concurrently; the warm graph `RwLock` serializes reads vs the delta apply.
4. Update pipeline per file event: parse the changed file via `ariadne-parser`, convert to `SyntacticFactsRaw`, call `AriadneDb::rederive_file` (or `forget_file` on a `WatcherEvent` removal [src: crates/ariadne-core/src/domain/watcher.rs:32]); translate the returned `Changeset` (symbol_upserts/deletes + edges_added/removed) into a `GraphIndex::apply_delta` call against the warm petgraph; bump the daemon revision to the new `RevisionId`.
5. Reconcile the v1 watcher safeguard: union fs events with the periodic gitignore-aware scan + content-hash check (v1 risk R7) so a missed event cannot leave the warm graph stale.
6. Incrementality proptest: 100 random edit sequences; assert the warm graph after deltas is identical to a fresh `build_from_snapshot` (mirrors the v1 plan `<verification>` divergence=0 invariant).
</steps>

<verification>
- `cargo nextest run -p ariadne-daemon` — live-update test + incrementality proptest (divergence 0) green.
- Manual: daemon on the self-index; edit a `.rs` file; confirm `find_references` over IPC reflects the change within the debounce window.
- `cargo test --test architecture`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check` — green.
</verification>

<build_notes>
Partial build, paused for a fresh `/spec-build` session. Decisions below were
taken with the user this session; honour them — do not re-litigate.

Resolved decisions:
1. **Surface the `Changeset` from salsa (DONE on disk).** Step 4 needs the
   committed changeset for `apply_delta`, but tier-07b's drivers returned only
   `RevisionId`. `crates/ariadne-salsa/src/db.rs` now: `commit_revision`
   unchanged; new private `commit_changeset`/`build_changeset`;
   `rederive_file`/`forget_file` return `(RevisionId, Changeset)`. All existing
   callers discard the value, so nothing broke. (task complete)
2. **Defer `ariadne-scip` (deviation from `<files>`).** No `<step>` consumes it
   and `scip_symbols` is a stub [src: crates/ariadne-salsa/src/derived.rs:158].
   Add only `ariadne-salsa` (workspace), `ariadne-parser` (path), `blake3`
   (workspace) to the daemon; `proptest` (workspace) as a dev-dep.
3. **Preserve the strict hexagonal invariant — daemon does NOT depend on
   `ariadne-watcher` (overrides the `<files>` line that adds it).** A
   driving→driving dep is forbidden by `tests/architecture.rs:125`
   (and `cargo_metadata` counts dev-deps, so daemon *tests* are barred too). No
   invariant relaxation, no ADR-0018. Instead the cli (the established
   composition root, ADR-0007) wires the watcher to the daemon over the
   `ariadne_core::WatcherSink` port via a `Receiver<Invalidation>` channel.

Clean-wiring design for the remaining work:
- `ariadne-daemon`: add `LiveEngine` (transient redb open per op — never holds
  redb idle, so tier-07 `warm_analytics` staleness tests stay green; holds
  `AriadneDb` + path→`FileId` map + `Arc<RwLock<WarmCatalog>>`). Refactor
  `WarmSnapshot` to `BTreeMap<EdgeKey,EdgeRecord>` + out/in/file `BTreeSet`
  indices (preserve scan order) + `apply(&Changeset)`. Add
  `WarmCatalog::apply_changeset` (snap + paths + symbols/by_name +
  `graph.apply_delta` + revision) and a `canonical_dump` for the proptest.
  Replicate the cli's `convert_facts`/`decl_kind_tag`/parse path (RD11 allows
  per-root parsing). Add `serve_live(root, events: Receiver<Invalidation>)`
  (build engine, spawn an update thread draining `events`, share the accept
  loop with `serve`) and `running_as_daemon_child()`. `serve(root)` UNCHANGED.
  Startup seed = iterate the catalog snap's stored files, re-read+parse from
  disk, `seed_file` with the stored `FileId` (no walk, no `ignore` dep).
- `ariadne-cli` `commands/daemon.rs`: if `running_as_daemon_child()` → build
  `ChannelSink::pair()` + `NotifyWatcher::start(.., Box::new(sink), 30s)` +
  `serve_live(root, rx)` (blocks), then `watcher.stop()`; else spawn detached.
- Tests in `crates/ariadne-daemon/tests/`: live-update feeds a
  `Receiver<Invalidation>` manually (NO watcher dep — invariant-safe) and
  asserts a query over IPC reflects the edit; incrementality proptest drives
  `LiveEngine` Set/Del and asserts `dump()==dump_fresh()` + equal graph counts.
  The real fs→`NotifyWatcher`→daemon path is covered by the manual self-index
  verification (translation/debounce are `ariadne-watcher`'s own tested
  concern). `convert_facts` must match the cli exactly or the first incremental
  commit churns un-edited files — guard via the divergence-0 proptest.

Sandbox/architecture facts confirmed this session: `RedbStorage` is single-open
per process (transient opens only); `EdgeKey` `Ord` == `to_bytes` order;
`commit_revision` emits a full upsert set + targeted stale deletes each call;
list_symbols iterates `cat.symbols` (id order) so `by_name` ordering only
matters for `find_definition` (first == lowest id, keep sorted).
</build_notes>

<rollback>
`git checkout -- crates/ariadne-daemon crates/ariadne-salsa`. The daemon falls
back to the tier-07 build-once-at-startup behaviour and the salsa drivers to
their tier-07b `RevisionId`-only return.
</rollback>
