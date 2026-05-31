---
tier_id: tier-01
audited: 2026-05-31
verdict: PASS
commit: 7e948ab0425c7478afdbbf18b7fae9a1b71e0c31
---

<scope>
Tier-01 "Gitignore-aware watcher file-id cache" of plan `mcp-startup-latency`.
Reviewed the working-tree diff scoped to the tier `<files>`:

- `crates/ariadne-watcher/src/adapters/file_id_cache.rs` (new) — `GitignoreFileIdCache`.
- `crates/ariadne-watcher/src/adapters/ignore.rs` — `.git/` added to `DEFAULT_IGNORES` + test.
- `crates/ariadne-watcher/src/adapters/mod.rs` — module registration.
- `crates/ariadne-watcher/src/adapters/notify.rs` — `new_debouncer` → `new_debouncer_opt`.
- `crates/ariadne-watcher/tests/file_id_cache.rs` (new) — invariant test.
- `crates/ariadne-watcher/src/lib.rs` — root re-export (NOT listed in tier `<files>`).

HEAD at audit: `7e948ab`. All changes are uncommitted working-tree state.
Library behaviour verified against the pinned source in the local cargo
registry (`notify-debouncer-full-0.7.0`), as Context7 quota was exhausted this
session per the plan's `<tech_inventory>`.
</scope>

<checks_run>
- plan_adherence: every `<files>` entry accounted for; the one out-of-scope
  edit (`lib.rs`) identified and assessed (INFO-1).
- correctness: re-derived the `add_path(Recursive)` pruning against the
  `FileIdCache` trait and the default `FileIdMap` (same crate version).
- architecture: re-ran `cargo test --test architecture` — green; no new
  in-workspace crate dependency introduced (`ignore` / `notify` /
  `notify_debouncer_full` were already watcher deps).
- tests: `cargo nextest run -p ariadne-watcher` — 20 passed (1 pre-existing
  leaky test `ignore::ariadne_dir_is_always_ignored`, unrelated to this tier).
- lint/format: `cargo clippy -p ariadne-watcher --all-targets -- -D warnings`
  clean; `cargo fmt --all --check` clean.
- exit_criteria: all four verified independently (below).
- library API: `new_debouncer_opt` signature and `FileIdCache` trait shape
  read from the pinned crate source.

Exit-criteria verification:
1. "never stat-s paths under target/ / node_modules/ / .ariadne/ / .gitignore" —
   `add_path(Recursive)` prunes via `WalkBuilder::filter_entry` keyed on the
   `Ignore` matcher (file_id_cache.rs:67-70); pruned directories are not
   descended, and `get_file_id` is called only on yielded (non-ignored)
   entries (file_id_cache.rs:72-77). `.gitignore` patterns are part of the
   `Ignore` matcher (ignore.rs:59-63). VERIFIED.
2. "builds the debouncer via new_debouncer_opt; new_debouncer no longer used" —
   notify.rs:85-93 uses `new_debouncer_opt`; no remaining call to the non-opt
   `new_debouncer` in the crate (sole textual match is a doc comment,
   notify.rs:3 — see INFO-2). VERIFIED.
3. "failing-first test: cache holds a source file but none under a synthetic
   ignored dir" — `caches_source_file_but_not_ignored_dir` asserts `src/a.rs`
   is_some, `target/debug/big.rs` is_none, `.git/HEAD` is_none against a real
   temp tree with no module-boundary mocks (tests/file_id_cache.rs:14-44).
   VERIFIED (test passes; TDD red-first ordering not auditable on uncommitted
   work, accepted on the realistic test content).
4. "nextest -p ariadne-watcher, clippy -D warnings, fmt --check,
   cargo test --test architecture all green" — all four re-run green (above).
   VERIFIED.
</checks_run>

<findings>
| id | category | severity | location | problem | fix |
|----|----------|----------|----------|---------|-----|
| INFO-1 | plan_adherence | INFO | crates/ariadne-watcher/src/lib.rs:13 | `pub use ...GitignoreFileIdCache;` is added in a file not listed in the tier `<files>`, and the re-export is referenced by nothing (the test imports the module path; `notify.rs` uses the `crate::` path). | Drop the re-export, or accept it as deliberate API-consistency with the crate's existing `NotifyWatcher`/`Ignore`/`Reconciler` root re-exports. Non-blocking. |
| INFO-2 | docs | INFO | crates/ariadne-watcher/src/adapters/notify.rs:3 | Module doc still says it "Wraps `notify_debouncer_full::new_debouncer`"; the code now wraps `new_debouncer_opt`. | Update the doc line to `new_debouncer_opt`. |
</findings>

<verdict>
PASS. Zero FAIL findings.

The implementation is correct and matches the plan's `<decisions>` (D1 custom
gitignore-aware `FileIdCache`; D2 wired via `new_debouncer_opt` at the single
`NotifyWatcher::start` chokepoint). The `new_debouncer_opt` argument order in
notify.rs:85-93 matches the real signature
`(timeout, tick_rate, handler, file_id_cache, config)` at
notify-debouncer-full-0.7.0/src/lib.rs:639 — note the plan's step-4 example
listed `config` before the cache; the implementer correctly verified against
source and used the right order. `remove_path` mirrors the default `FileIdMap`
exactly; the un-overridden `rescan` default re-walks through the same
gitignore-aware `add_path`, so dropped-event rescans also exclude ignored dirs.
The `.git/` `DEFAULT_IGNORES` addition and walk-level `filter_entry` pruning are
documented in the tier `<notes>` as plan-owner-approved refinements; the
behavioural narrowing (watcher stops emitting `.git/` events) leaves query
results unchanged because `.git/` was never indexed — the determinism
constraint holds. No new dependency, no cross-adapter dep, no port change; the
architecture boundary test stays green.

The wall-clock e2e probe (rebuild release CLI, `cp -Rc` clone, re-index, time
`serve --watch` spawn→`initialize`) could not be re-run in-audit: its harness
`/tmp/probe3.py` is session-scoped and no longer present, and a 7.1 GB `target/`
clone is impractical here. This does not gate the verdict: the binding gates are
the four functional exit-criteria (all reproduced green), wall-clock lives in
benches per the plan `<constraints>`, and the tier `<notes>` record the measured
result (watcher overhead ~18 ms vs the prior +153 ms; median ratio 1.55×, under
the stated 2× gate). The two INFO items are non-blocking nits.
</verdict>

<next_steps>
None required for PASS. Optional cleanups (do not block commit):
- Decide INFO-1: either remove the unused `lib.rs:13` re-export or keep it for
  root-API consistency with the crate's other adapter re-exports.
- Fix INFO-2: refresh the notify.rs module doc to name `new_debouncer_opt`.
</next_steps>

<sources>
- FileIdCache trait + default rescan — notify-debouncer-full-0.7.0/src/cache.rs:8-63
- FileIdMap add_path/remove_path — notify-debouncer-full-0.7.0/src/file_id_map.rs:34-62
- new_debouncer_opt signature (timeout, tick_rate, handler, file_id_cache, config) — notify-debouncer-full-0.7.0/src/lib.rs:639
- ignore WalkBuilder filter_entry pruning — https://docs.rs/ignore/0.4.25/ignore/struct.WalkBuilder.html
- Reviewer standard / comment severity — https://google.github.io/eng-practices/review/reviewer/standard.html
</sources>
