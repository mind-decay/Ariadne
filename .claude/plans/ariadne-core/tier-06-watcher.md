---
tier_id: tier-06
title: File watcher + invalidation pipeline (notify-rs → Salsa input updates)
deps: [tier-01, tier-02, tier-04]
exit_criteria:
  - `Watcher::start(root, ignore_patterns, tx)` spawns a notify-rs RecommendedWatcher with notify-debouncer-full at 100ms quiet period.
  - Watcher honors .gitignore + .ariadneignore via the `ignore` crate; verified by proptest generating random ignored/tracked files.
  - Filesystem events translate to `Invalidation { path, kind }` and are applied to `AriadneDb` inputs via `apply_invalidation` in tier-04.
  - Reconciliation pass: every 60s a full gitignore-aware walk computes content hashes for files unseen via notify and reconciles drift (R7 mitigation).
  - End-to-end test: edit a file via tokio::fs, assert that within 500ms `symbols_for_file` reflects the change (insta snapshot before/after).
status: completed
completed: 2026-05-20
---

<context>
Without a reliable watcher, "always reacts to changes" promise breaks. notify-rs is the same library rust-analyzer, zed, watchexec use; it abstracts FSEvents (macOS), inotify (Linux), ReadDirectoryChangesW (Windows) [src: https://github.com/notify-rs/notify]. FSEvents on macOS misses events under load (R7), so we union with a periodic scan.
</context>

<files>
- crates/ariadne-watcher/Cargo.toml — notify, notify-debouncer-full, ignore, blake3, crossbeam-channel, tokio (optional), workspace deps.
- crates/ariadne-watcher/src/lib.rs — re-exports `Watcher`, `Invalidation`, `ReconciliationReport`, `WatcherError`.
- crates/ariadne-watcher/src/watch.rs — wraps `notify_debouncer_full::new_debouncer`; spawns dedicated OS thread.
- crates/ariadne-watcher/src/ignore.rs — builds `ignore::gitignore::GitignoreBuilder` from `.gitignore` + `.ariadneignore` + hard-coded defaults (target/, node_modules/, .ariadne/).
- crates/ariadne-watcher/src/reconcile.rs — periodic full-walk that computes content hash via blake3 and emits Invalidation::HashDrift for changed files missed by events.
- crates/ariadne-watcher/src/sink.rs — `WatcherSink` trait, in-process impl that calls `AriadneDb::apply_invalidation` from tier-04.
- crates/ariadne-watcher/tests/events.rs — temp dir + tokio fs ops; assert event types translate correctly.
- crates/ariadne-watcher/tests/ignore.rs — proptest on path patterns.
- crates/ariadne-watcher/tests/reconcile.rs — simulate a missed event by writing a file via syscall-bypass (or directly via `std::fs`); assert reconciliation finds it within next pass.
- crates/ariadne-watcher/benches/sink.rs — criterion for `apply_invalidation` throughput on a 10K-file fixture.
</files>

<steps>
1. **Failing test first** (tests/events.rs): create temp dir, start Watcher, write 1 file via `tokio::fs::write`, await on rx channel for Invalidation; assert kind == `Created` and path matches. Fails until step 4.
2. Add deps. notify-debouncer-full is the canonical wrapper that coalesces rapid event bursts (a single editor save can fire Modify+Create+Modify+Modify) [src: https://github.com/notify-rs/notify/tree/main/notify-debouncer-full].
3. Build Ignore matcher: `ignore::WalkBuilder::new(root).git_global(true).hidden(false).add_custom_ignore_filename(".ariadneignore")` [src: https://docs.rs/ignore].
4. Watcher::start signature:
   ```rust
   pub fn start(
       root: PathBuf,
       ignore: Arc<Ignore>,
       sink: Box<dyn WatcherSink>,
       reconcile_interval: Duration,
   ) -> Result<WatcherHandle>
   ```
   Spawns: (a) notify-debouncer-full thread, (b) reconcile thread, (c) join handle for graceful shutdown via stop()`.
5. Translate notify events:
   - `DebouncedEvent { kind: Create, paths }` → `Invalidation::Created { path }`
   - `kind: Modify(Data*)` → `Invalidation::Modified { path }`
   - `kind: Remove` → `Invalidation::Removed { path }`
   - `kind: Modify(Name)` (rename) → emit `Removed { old }` + `Created { new }`
   Filter out events where `ignore.matched_path_or_any_parents(p, p.is_dir()).is_ignore()`.
6. Reconciliation: every `reconcile_interval`, run `ignore::WalkBuilder` → for each file compute `blake3(content)` (streamed, no full load >16MB). Diff against the last-known hash recorded by sink. Emit `Invalidation::HashDrift { path, old_hash, new_hash }` for differences.
7. WatcherSink in-process impl: holds `Arc<Mutex<AriadneDb>>`; on each Invalidation, looks up `FileContentInput`, sets new content + new hash with proper durability tier (`inputs::durability_for(path)`).
8. **Failing test first** (tests/ignore.rs): proptest generates random path strings; for `target/foo.rs` assert ignored; for `src/foo.rs` assert tracked; for `.ariadneignore`-declared `*.snap` assert ignored. Fails until step 3 implementation.
9. **Failing test first** (tests/reconcile.rs): mock a "missed event" by toggling watcher to a noop sink, mutate a file, swap sink back to real, wait reconcile interval; assert HashDrift fires.
10. Bench (benches/sink.rs): apply 10K random Invalidations through WatcherSink into a stub AriadneDb; assert throughput ≥10K/s; gate in CI.
11. Document macOS FSEvents caveat in src/watch.rs doc-comment: events may be coalesced or dropped under high load, hence the reconcile pass [src: https://github.com/notify-rs/notify (platform notes)].
12. Wire the watcher into ariadne-cli `watch` command (stub in tier-01; full impl in tier-10).
</steps>

<verification>
- `cargo nextest run -p ariadne-watcher` green.
- `cargo bench -p ariadne-watcher` ≥10K invalidations/s.
- Manual: in a fixture repo, `touch src/foo.rs`, observe `tracing::info!` log line for Invalidation within 200ms (debounce 100ms + slack). Then break expectations by chmodding the file (no content change) — assert NO HashDrift (hash unchanged).
- Stress: spawn 1K concurrent `fs::write` ops in temp dir, assert no panic, all changes eventually observed (event or reconcile).
</verification>

<rollback>
`git rm -r crates/ariadne-watcher` + workspace member removal. Watcher holds no on-disk state.
</rollback>
