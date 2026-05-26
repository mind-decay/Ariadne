---
tier_id: tier-08
title: Daemon live updates — watcher loop feeds incremental warm-graph invalidation
deps: [tier-07]
exit_criteria:
  - The daemon owns the `ariadne-watcher` event loop and reacts to filesystem changes.
  - A file edit re-derives the affected subset via Salsa and applies a delta to the warm graph.
  - redb deltas are persisted; the daemon revision advances on each applied change.
  - A proptest of random edit sequences shows zero divergence between the warm graph and a full rebuild.
  - `cargo nextest run -p ariadne-daemon` + architecture + clippy + fmt all green.
status: pending
---

<context>
tier-07 made the daemon serve queries from a warm graph built once at startup. This tier keeps that graph live: the daemon hosts the watcher, and every filesystem change flows through Salsa re-derivation into an incremental `apply_delta` on the warm petgraph (plan RD6, dataflow in plan `<architecture>`). Full context: plan.md.
</context>

<files>
- crates/ariadne-daemon/Cargo.toml — modify: add `ariadne-watcher`.
- crates/ariadne-daemon/src/domain/ — modify: the update pipeline (fs event → invalidate → re-derive → delta → persist).
- crates/ariadne-daemon/src/ — modify: own the watcher event loop alongside the IPC accept loop.
- crates/ariadne-daemon/tests/ — new: live-update integration test + incrementality proptest.
</files>

<steps>
1. Failing test first (`ariadne-daemon` tests): start the daemon on a fixture, mutate a file on disk, wait for debounce, query the affected symbol over IPC, assert the warm graph reflects the edit. Red — the daemon has no watcher.
2. Read v1 tier-06's watcher → invalidation pipeline and `AriadneDb::commit_revision`, and v1 tier-07's `GraphIndex::apply_delta` (+`EdgeDelta`) [src: .claude/plans/ariadne-core/tier-06-watcher.md ; tier-07-graph-analytics.md].
3. Spawn the `ariadne-watcher` `NotifyWatcher` inside the daemon; debounced fs events feed an internal channel consumed by the update pipeline. The IPC accept loop and the watcher loop run concurrently; the warm graph `RwLock` serializes reads vs the delta apply.
4. Update pipeline per file event: invalidate the Salsa file input → re-derive parse/symbols/graph subset → `commit_revision` writes redb deltas → `GraphIndex::apply_delta` mutates the warm petgraph → bump the daemon revision.
5. Reconcile the v1 watcher safeguard: union fs events with the periodic gitignore-aware scan + content-hash check (v1 risk R7) so a missed event cannot leave the warm graph stale.
6. Incrementality proptest: 100 random edit sequences; assert the warm graph after deltas is identical to a fresh `build_from_snapshot` (mirrors the v1 plan `<verification>` divergence=0 invariant).
</steps>

<verification>
- `cargo nextest run -p ariadne-daemon` — live-update test + incrementality proptest (divergence 0) green.
- Manual: daemon on the self-index; edit a `.rs` file; confirm `find_references` over IPC reflects the change within the debounce window.
- `cargo test --test architecture`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check` — green.
</verification>

<rollback>
`git checkout -- crates/ariadne-daemon`. The daemon falls back to the tier-07 build-once-at-startup behaviour.
</rollback>
