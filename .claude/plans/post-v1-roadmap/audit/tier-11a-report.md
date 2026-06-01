---
tier_id: tier-11a
audited: 2026-06-01
verdict: PASS
commit: ac386d2198c53275b899ee61ef020ef50c0c63bb
---

<scope>
Tier-11a "Incremental git history — HEAD-oid watermark, re-walk only new commits,
daemon-lifecycle wiring". Diff scoped to the tier `<files>` plus the five
build-created files. HEAD `ac386d2`; tier-11a changes are an uncommitted
working-tree diff on top (audit-gate hook gates the subsequent commit).

Files read end-to-end:
- crates/ariadne-storage/src/adapters/redb/tables.rs (SCHEMA_VERSION 4→5, HISTORY_META)
- crates/ariadne-storage/src/adapters/redb/mod.rs (Storage impl + bootstrap)
- crates/ariadne-storage/src/adapters/redb/history.rs (new: merge_history + watermark)
- crates/ariadne-storage/src/domain/migration.rs (v4→v5 step)
- crates/ariadne-core/src/domain/ports.rs (3 new Storage methods)
- crates/ariadne-git/src/adapters/gix/mod.rs (renamed; accumulate + head_oid)
- crates/ariadne-git/src/adapters/gix/incremental.rs (new: walk_since + is_ancestor)
- crates/ariadne-git/src/adapters/mod.rs, lib.rs (façade re-exports)
- crates/ariadne-cli/src/commands/index.rs (refresh_history)
- crates/ariadne-cli/src/commands/daemon.rs (spawn_history_rewalk + wait_or_stop)
- tests: ariadne-git/tests/incremental.rs, ariadne-storage/tests/history_merge.rs,
  ariadne-cli/tests/incremental_history.rs, plus migration/changeset test edits
</scope>

<checks_run>
All `<verification>` commands re-run from a clean tree:
- `cargo fmt --all --check` — green (exit 0).
- `cargo nextest run -p ariadne-git -p ariadne-storage -p ariadne-cli` — 69 passed,
  2 skipped, 0 failed (exit 0). New tests observed PASS: the 3 `ariadne-git::incremental`,
  the 3 `ariadne-storage::history_merge`, and the v4→v5 / v1→v5 migration coverage.
- `cargo nextest run -p ariadne-cli --test incremental_history` (the head-truncated
  E2E pair) — 2 passed: `incremental_ingest_equals_full_cold_walk` (divergence-0 gate)
  and `force_pushed_history_falls_back_to_full_replace`.
- `cargo test --test architecture` — green (exit 0): no `ariadne-daemon → ariadne-git` edge.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` — green (exit 0).

Evidence beyond the build:
- `with_hidden(Some(oid))` ancestor-hiding semantics proven by execution:
  `walk_since_visits_only_commits_after_watermark` asserts only the post-watermark
  commit's files appear; `walk_since_unreachable_watermark_falls_back_to_full` proves
  the rebase/force-push fallback. Real `.git` fixtures, no module-boundary mocks.
- `replace_history_inner` (mod.rs:84-86) `delete_table`s CHURN+CO_CHANGE before
  rewrite, so the force-push full-replace path equals a fresh cold walk (no stale
  residue) — confirmed by `force_pushed_history_falls_back_to_full_replace`.
- ariadne-daemon Cargo.toml deps: core/graph/storage/salsa/parser only — no ariadne-git.
- Default `HistoryConfig::depth = None` (config.rs:75), so divergence-0 holds at the
  shipped default.
</checks_run>

<exit_criteria_check>
1. Watermark persists in redb; re-ingest walks only newer commits, merges into
   CHURN/CO_CHANGE — VERIFIED (history.rs merge_history; walk_since hide; tests).
2. Incremental re-walk == full cold walk, divergence 0 — VERIFIED
   (incremental_ingest_equals_full_cold_walk, depth=None).
3. Rewritten/force-pushed history falls back to full replace, no corruption/panic —
   VERIFIED (is_ancestor via merge_base → hide=None; force-push tests).
4. Daemon never depends on ariadne-git; CLI composition root drives re-walks; daemon
   reads churn from redb per op — VERIFIED (architecture test green; daemon.rs schedules
   the thread at the CLI root; daemon Cargo has no ariadne-git).
5. nextest (3 crates) + architecture + clippy + fmt all green — VERIFIED.
</exit_criteria_check>

<findings>
| id | category | severity | location | problem | fix |
|----|----------|----------|----------|---------|-----|
| I1 | reliability | INFO | crates/ariadne-cli/src/commands/daemon.rs:114-124 (spawn → refresh_history) vs crates/ariadne-daemon/src/domain/live.rs:148-151,163-166 + adapters/ipc.rs:124-134 | The tier-08 invariant serializes *every* in-process redb open under the warm-catalog write lock so no two opens collide (single-open-per-process; tier-08 audit I1, which hardened `serve_connection` to open under the lock). The new re-walk thread opens redb via `refresh_history` **outside** that lock, in the same daemon-child process as the pump and accept loop. A collision now makes the *daemon's own* ops lose the race: a pump `apply` errors and is dropped (recovered ≤30s later by `RECONCILE_INTERVAL`), and a stale-rebuild query returns `DaemonResponse::Error`. Recoverable, no corruption/panic — same class and severity the tier-08 audit assigned I1. | Acquire the catalog write lock (or a dedicated redb mutex) around the re-walk's storage access so all opens stay serialized, matching live.rs/ipc.rs. |
| I2 | performance | INFO | crates/ariadne-cli/src/commands/index.rs:55-66 | `refresh_history` opens `storage` (redb) *before* `walk_since` and uses it after, holding the single-open handle across the entire `gix` history walk. The walk needs no redb; only the watermark read and the merge do. This widens the I1 exclusion window from merge-duration to walk+merge-duration — potentially seconds on a large no-commit-graph repo, every 60s — amplifying I1's collisions against daemon ops. | Read the watermark, drop `storage`, run `walk_since`, then reopen redb only for `merge_history`/`replace_history` + `set_last_ingested_commit`. |
| I3 | correctness | INFO | crates/ariadne-cli/src/commands/index.rs:56-77 + crates/ariadne-cli/src/config.rs:54-58 | With `depth = Some(N)`, the first cold walk is capped at N commits and sets the watermark at HEAD, but later incremental merges append new commits onto that already-N-deep base, so the effective window grows past N over the daemon lifetime and diverges from a full depth-N cold walk. The divergence-0 invariant holds only for `depth = None` (the default and the tested config); the plan commits only to depth capping "the first cold walk" (step 5), so this is undocumented behavior, not a broken guarantee. | Document the limitation, or take the full `replace_history` path when `depth.is_some()` so the bounded window stays exact. |
</findings>

<verdict>
PASS. Zero FAIL findings. Every exit criterion is independently verified and all
five `<verification>` commands re-run green. The three INFO items are recoverable
robustness/clarity nits — the most material (I1/I2) is the same single-open redb
race the tier-08 audit already accepted as non-blocking INFO, now reintroduced by an
unsynchronized re-walk opener and worsened by holding the handle across the git walk.
The merge is ACID (watermark advances in the merge transaction), the force-push
fallback is clean, author-set/co-change merges are byte-identical to a cold walk, and
the daemon stays adapter-isolated from ariadne-git.
</verdict>

<next_steps>
None gate the commit. Optional hardening before relying on the daemon under load:
fold I1+I2 together — serialize the re-walk's redb access under the catalog lock and
scope `storage` to just the watermark read + merge, leaving the git walk lock-free.
Consider I3 if a non-default `depth` is ever shipped.
</next_steps>

<sources>
- tier-11a tier file + plan.md RD7, R-C1/R-C4: .claude/plans/post-v1-roadmap/
- tier-08 single-open invariant + audit I1: crates/ariadne-daemon/src/domain/live.rs:10-15,
  crates/ariadne-daemon/src/adapters/ipc.rs:124-134, audit/tier-08-report.md:70
- redb WriteTransaction (ACID single-txn merge): https://docs.rs/redb/4.1.0/redb/struct.WriteTransaction.html
- gix rev-walk with_hidden / merge_base (behavior confirmed by passing fixtures):
  https://docs.rs/gix/0.84.0/gix/struct.Repository.html
- Google eng-practices reviewer standard (code health over perfection):
  https://google.github.io/eng-practices/review/reviewer/standard.html
</sources>
