---
tier_id: tier-11a
title: Incremental git history — HEAD-oid watermark, re-walk only new commits, daemon-lifecycle wiring
deps: [tier-11, tier-08]
exit_criteria:
  - A HEAD-commit-oid watermark persists in redb; re-ingestion walks only commits newer than the watermark and merges deltas into `CHURN`/`CO_CHANGE`.
  - An incremental re-walk after N new commits yields churn/co-change byte-identical to a full cold walk over the whole history (divergence 0).
  - A rewritten/force-pushed history (watermark not an ancestor of HEAD) falls back to a full `replace_history` walk — no corruption, no panic.
  - The daemon never depends on `ariadne-git`; the CLI composition root drives re-walks and the daemon reads churn from redb per analytics op (tier-08 adapter-isolation precedent).
  - `cargo nextest run -p ariadne-git -p ariadne-storage -p ariadne-cli` + architecture + clippy + fmt all green.
status: pending
---

<context>
tier-11 ingests file-level churn + co-change once at `ariadne index` (cold). This tier keeps it current cheaply: a commit-oid watermark lets re-ingestion walk only commits added since the last run and merge the deltas, so history stays fresh over the daemon lifecycle without re-walking the whole repo each time (plan RD7, risk R-C1). No new architectural pattern — it reuses tier-08's "CLI composition root wires lifecycle work, daemon stays adapter-isolated" decision, so no ADR. Full context: plan.md.
</context>

<files>
- crates/ariadne-storage/src/adapters/redb/tables.rs — modify: add a `HISTORY_META` table (`&str` → `&[u8]`) for the watermark; bump `SCHEMA_VERSION` by 1 (the META table is `&str`→`u64`, so a commit oid needs a byte-valued table) [src: crates/ariadne-storage/src/adapters/redb/tables.rs:12-17].
- crates/ariadne-storage/src/domain/migration.rs — modify: register the next `MigrationStep` opening `HISTORY_META` [src: crates/ariadne-storage/src/domain/migration.rs:47-62].
- crates/ariadne-core/src/domain/ports.rs — modify: `Storage` gains `last_ingested_commit()`/`set_last_ingested_commit()` + `merge_history(churn_delta, pair_delta)`.
- crates/ariadne-git/src/adapters/gix.rs — modify: `walk_since(watermark: Option<gix::ObjectId>)` hides the watermark's ancestors from the rev-walk so only new commits are visited.
- crates/ariadne-git/src/lib.rs — modify: expose the incremental walk on the port.
- crates/ariadne-cli/src/commands/index.rs — modify: pick full vs incremental walk by watermark validity.
- crates/ariadne-cli/src/commands/daemon.rs — modify: schedule periodic re-walk alongside the watcher wiring (the daemon child is the long-running host; the CLI owns the git adapter) [src: tier-08 build_notes "Clean-wiring design"].
</files>

<steps>
1. Failing test first (`ariadne-git`/`ariadne-storage` tests): fixture repo; cold-ingest the first K commits; commit N more; incremental re-walk; assert `CHURN`/`CO_CHANGE` equal a full cold walk over all `K+N` commits (divergence 0). Red — no watermark, no incremental walk.
2. Add `HISTORY_META` + the `KEY_LAST_INGESTED_COMMIT` key; register the next migration step opening it (additive; pre-existing dbs upgrade in place — no rebuild) [src: crates/ariadne-storage/src/domain/migration.rs:98-100].
3. `ariadne-git::walk_since(watermark)`: when `Some(oid)` and `oid` is reachable from HEAD, hide its ancestors from the rev-walk (only commits newer than the watermark are visited); when `None` or `oid` is unreachable (rebase/force-push), do a full walk [src: https://docs.rs/gix/0.84.0/gix/struct.Repository.html — rev-walk tips + hidden/ends; exact hide method verified at build].
4. `Storage::merge_history`: inside one `WriteTransaction` — add new commit counts, union `author_keys`, take the max `last_changed_ns`, add co-change counts — then set the watermark to the current HEAD oid in the same transaction so a crash never half-applies (ACID) [src: https://docs.rs/redb/4.1.0/redb/struct.WriteTransaction.html].
5. CLI: read the watermark; if present and an ancestor of HEAD → `walk_since` + `merge_history`; else full `replace_history` (tier-11) + set the watermark. Bounded `depth` (tier-11 config) still caps the first cold walk.
6. Daemon lifecycle — honour tier-08's adapter-isolation decision: the daemon does NOT depend on `ariadne-git` (driving→driven is barred by `tests/architecture.rs`; dev-deps count too). The CLI composition root that spawns the daemon child schedules the periodic re-walk and writes deltas via the `Storage` port; the daemon serves analytics by reading `CHURN`/`CO_CHANGE` from redb per op (its transient-open model, tier-08), so it sees fresh churn with no in-RAM history state [src: tier-08 build_notes decision 3 + "Clean-wiring design"].
7. Determinism: the incremental==full invariant is the gate (mirrors tier-08's divergence-0 proptest); no wall-clock, no RNG in aggregation.
</steps>

<verification>
- `cargo nextest run -p ariadne-git -p ariadne-storage -p ariadne-cli` — incremental==full (divergence 0) + the rebase/force-push full-fallback test green.
- Manual: `ariadne index` this repo; `git commit` a change; re-run; confirm only the new commit's files incremented (cross-check `git log`), watermark advanced to HEAD.
- `cargo test --test architecture` (no daemon→`ariadne-git` edge), `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check` — green.
</verification>

<rollback>
`git checkout -- crates`. Drop the `HISTORY_META` table + watermark key and revert `SCHEMA_VERSION`; re-ingestion reverts to tier-11's full cold walk every run. No ADR to remove.
</rollback>
</content>
