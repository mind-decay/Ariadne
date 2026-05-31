---
tier_id: tier-01
title: Gitignore-aware watcher file-id cache — kill the startup scan of ignored dirs
deps: []
exit_criteria:
  - "A GitignoreFileIdCache implements notify_debouncer_full::FileIdCache and, on add_path(root, Recursive), never stat-s paths under target/ / node_modules/ / .ariadne/ / .gitignore."
  - "NotifyWatcher::start builds the debouncer via new_debouncer_opt with the custom cache; new_debouncer is no longer used."
  - "Failing-first test proves the cache holds file-ids for a source file but none for a file under a synthetic ignored dir."
  - "cargo nextest run -p ariadne-watcher, clippy -D warnings, fmt --check, cargo test --test architecture all green."
status: completed
completed: 2026-05-31
---

<context>
The watcher's startup cost is notify-debouncer-full populating its `FileIdMap`
by walking the whole tree with `walkdir` (no gitignore filter) at `watch()`
time — measured +153 ms on this repo's 35,494-file `target/`. Replace the cache
with a gitignore-aware one so the initial scan visits only the indexed file set,
while keeping rename-pair stitching for those files. Single fix point: both
`ariadne serve --watch` and the daemon's watcher call `NotifyWatcher::start`.
Full context: plan.md.
</context>

<files>
- crates/ariadne-watcher/src/adapters/file_id_cache.rs — new: `GitignoreFileIdCache`
  holding `HashMap<PathBuf, FileId>` + an `Arc<Ignore>`; impl `FileIdCache`.
- crates/ariadne-watcher/src/adapters/mod.rs — register the new module.
- crates/ariadne-watcher/src/adapters/notify.rs — swap `new_debouncer` →
  `new_debouncer_opt`; pass the cache built from the `Ignore` already received.
- crates/ariadne-watcher/src/adapters/ignore.rs — if needed, expose an
  `is_ignored(path, is_dir)` already present [src: ignore.rs:98-104].
- crates/ariadne-watcher/tests/file_id_cache.rs — new: failing-first invariant test.
- crates/ariadne-watcher/benches/ — optional criterion bench: watcher-start time
  vs ignored-dir file count (wall-clock lives here, not in unit tests).
</files>

<steps>
1. Failing test first (`tests/file_id_cache.rs`): build a temp tree with
   `src/a.rs` and `target/debug/big.rs`; construct `GitignoreFileIdCache` from
   `Ignore::build(root)`; call `add_path(root, RecursiveMode::Recursive)`; assert
   `cached_file_id("src/a.rs").is_some()` and `cached_file_id("target/debug/big.rs").is_none()`.
   Red — type does not exist. [src: cache.rs:8-32 trait shape]
2. Implement `GitignoreFileIdCache`. `add_path` with `Recursive`: walk with
   `ignore::WalkBuilder::new(path).hidden(false).git_ignore(true).build()`,
   mirroring the reconciler so ignore semantics match, plus the watcher's
   `DEFAULT_IGNORES` overrides; for each non-ignored entry call
   `notify_debouncer_full::file_id::get_file_id(&p)` and insert. Non-recursive
   `add_path`: single entry. `cached_file_id` = map lookup; `remove_path` =
   `retain(|p,_| !p.starts_with(path))` (matches `FileIdMap`)
   [src: file_id_map.rs:35-58; reconcile.rs:56; ignore.rs:14 DEFAULT_IGNORES].
3. Reuse the re-exported types — `use notify_debouncer_full::file_id::{FileId, get_file_id};`
   No new dependency [src: notify-debouncer-full-0.7.0/src/lib.rs:91].
4. In `NotifyWatcher::start`, replace
   `new_debouncer(DEBOUNCE_PERIOD, None, tx)` with
   `new_debouncer_opt::<_, notify::RecommendedWatcher, GitignoreFileIdCache>(
       DEBOUNCE_PERIOD, None, tx, notify::Config::default(),
       GitignoreFileIdCache::new(Arc::clone(&ignore)))`,
   then `.watch(root, RecursiveMode::Recursive)` unchanged. Confirm the exact
   `new_debouncer_opt` arg order against the source before wiring
   [src: notify.rs:80-84; lib.rs:639].
5. Verify rename stitching still works for source files: keep the existing
   `dispatch` path untouched; the cache only changes which paths get ids.
6. Run the full gate set; record the probe delta in `<verification>`.
</steps>

<verification>
- `cargo nextest run -p ariadne-watcher` — new invariant test green.
- `cargo clippy -p ariadne-watcher --all-targets -- -D warnings`; `cargo fmt --all --check`.
- `cargo test --test architecture` — boundary unchanged (no new public type leaks,
  no new cross-adapter dep).
- End-to-end probe (real run): rebuild `cargo build --release -p ariadne-cli`,
  install, clone repo with `target/` via `cp -Rc`, re-index `--fresh`, then time
  `serve --watch <root>` spawn→`initialize` (the harness used this session:
  /tmp/probe3.py with ROOT set to the clone). Expected: `serve --watch` median
  drops from ~182 ms toward the ~29 ms no-watch baseline and no longer grows with
  `target/` file count. Compare against the stated baseline; a >2× residual gap
  is a fail to root-cause, not to accept.
- Memory: no in-RAM table added — state "no memory_report delta" (R1).
</verification>

<rollback>
Revert the three edited files + delete the new module/test. `new_debouncer` and
the default `FileIdMap` restore the prior (slow but correct) behaviour; no data
or schema migration is involved, so rollback is a pure code revert.
</rollback>

<notes>
Two refinements were approved by the plan owner during build (the e2e
`<verification>` ">2× residual gap" gate forced root-causing the residual):

1. `.git/` added to `DEFAULT_IGNORES` (ignore.rs). Root cause: `.git/` is never
   in a repo's own `.gitignore`, so the WalkBuilder descended into its 1661
   objects on this repo. Treating it like the other VCS/build dirs (plan-owner
   decision: "ignore .git like the other directories") makes the matcher exclude
   it uniformly (cache filter + event dispatch + reconcile). Behavioural note:
   the watcher now also drops `.git/` events — those paths were never indexed, so
   query results are unchanged (the determinism constraint is on query results).

2. `add_path(Recursive)` prunes ignored directories at the walk level via
   `WalkBuilder::filter_entry` keyed on the same `Ignore` matcher, not only the
   per-entry `is_ignored` filter of step 2. Without pruning, the walk still
   `readdir`-traversed `.git/` (DEFAULT_IGNORES are not in `.gitignore`, so the
   gitignore reader does not prune them). `filter_entry` skips descent, so the
   non-ignored file set inserted is identical — only efficiency improves
   [src: ignore-0.4.25/src/walk.rs:960-973 filter_entry pruning semantics].

Measured (this repo: 318 indexed files; `target/` 40,128 files / 7.1 GB;
`.git/` 1,661 files):
- `add_path` walk: inserts 720 file-ids in ~13.5 ms, independent of `target/`
  and `.git/` file count (both pruned) — was the full-tree `walkdir` of ~42k
  entries before.
- `serve --watch` spawn→`initialize`: min 46.9 ms vs no-watch baseline min
  29.2 ms → ~18 ms watcher overhead (was +153 ms). Median ratio 1.55×, min ratio
  1.6× — under the 2× gate. (`serve` no-watch median is noisier than `serve
  --watch` due to catalog-build/page-cache variance; the watcher cost is stable.)
- Memory: no new in-RAM/Salsa table; the cache's `HashMap` replaces the default
  `FileIdMap` and now holds fewer entries — no `memory_report` delta (R1).
</notes>
