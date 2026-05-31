---
tier_id: tier-11
title: Git history ingestion (file-level, cold) — ariadne-git on gix, churn + co-change tables
deps: [tier-02]
exit_criteria:
  - A new `ariadne-git` driven adapter walks bounded commit history and per-commit changed paths via `gix`, depending only on `ariadne-core`.
  - Per-file churn (commit count, distinct authors, last-changed ns) + unordered file-pair co-change counts persist to new `CHURN` + `CO_CHANGE` redb tables.
  - The new tables ship behind one registered migration step (`SCHEMA_VERSION` → +1); a pre-existing db opens and upgrades in place — no rebuild.
  - `ariadne index` ingests history within a configurable bounded commit depth read from `config.toml`; commits touching more than a configurable file count are excluded from co-change.
  - `cargo nextest run -p ariadne-git -p ariadne-storage -p ariadne-cli` + architecture + clippy + fmt all green.
status: completed
completed: 2026-06-01
---

<context>
v1 analytics are static-only — they ignore how code changed over time. Block C adds history-derived signal; this tier ingests the file-level slice: a pure-Rust `gix` adapter walks commits and records per-file churn and co-change (files changed together), persisted for the tier-13 hotspot/coupling metrics. Incremental re-walk is tier-11a; per-symbol attribution is tier-11b. Full context + RD7: plan.md.
</context>

<files>
- crates/ariadne-git/Cargo.toml — new: deps `ariadne-core` (path), `gix = { version = "=0.84.0", default-features = false, features = ["blob-diff", "revision", "sha1"] }`, `thiserror` (workspace). All three are local/non-network features (`revision` for `rev_walk`, `sha1` for pure-Rust object decoding); no network/transport features [src: https://lib.rs/crates/gix ; docs/adr/0018-git-history-adapter.md].
- crates/ariadne-git/src/lib.rs — new: façade; re-exports the port impl + `GitError` only (no `gix` types leak).
- crates/ariadne-git/src/adapters/gix.rs — new: commit walk + per-commit tree diff (one file, one tech).
- crates/ariadne-git/src/errors.rs — new: `thiserror` `GitError`.
- crates/ariadne-core/src/domain/records.rs — modify: add `FileChurn` + `CoChangePair` owned record types beside `FileRecord` [src: crates/ariadne-core/src/domain/records.rs:11-23].
- crates/ariadne-core/src/domain/ports.rs — modify: extend the `Storage` port with `replace_history(churn, pairs)` + read accessors.
- crates/ariadne-storage/src/adapters/redb/tables.rs — modify: add `CHURN` + `CO_CHANGE` `TableDefinition`s; bump `SCHEMA_VERSION` by 1 [src: crates/ariadne-storage/src/adapters/redb/tables.rs:12-17].
- crates/ariadne-storage/src/domain/migration.rs — modify: register one `MigrationStep { from: prev, to: prev+1, apply }` that opens (creates) the two tables [src: crates/ariadne-storage/src/domain/migration.rs:47-62,98-100].
- crates/ariadne-cli/src/config.rs — modify: add a `#[serde(default)]` `[history]` block (`depth: Option<u32>`, `max_files_per_commit: u32`) [src: crates/ariadne-cli/src/config.rs:34-45].
- crates/ariadne-cli/src/commands/index.rs — modify: after the symbol commit, walk history and `replace_history` [src: crates/ariadne-cli/src/commands/index.rs:17-19].
- tests/architecture.rs — modify: classify `ariadne-git` as a driven adapter (deps ⊆ {core}).
- docs/adr/0018-git-history-adapter.md — new (authored at build).
</files>

<steps>
1. Failing test first (`ariadne-git` tests): build a fixture repo (a `#[test]` helper commits a known sequence), assert the adapter reports the expected per-file commit counts, distinct-author counts, and unordered co-change pairs. Red — the crate does not exist.
2. Scaffold `ariadne-git` per `docs/folder-layout.md`; `gix` with `default-features = false` + `blob-diff` so the critical path is pure-Rust (no curl/C, no transport) [src: https://lib.rs/crates/gix ; plan.md D5].
3. `adapters/gix.rs`: open via `gix::open`; `repo.head_commit()`; walk ancestors with `repo.rev_walk([head]).all()` (uses the commit-graph file when present — R-C1) [src: https://docs.rs/gix/0.84.0/gix/struct.Repository.html].
4. Per commit: `commit.tree()` vs its first parent's tree via `repo.diff_tree_to_tree(Some(parent_tree), Some(tree), ..)` for changed paths (root commit diffs against an empty tree); read `commit.author()` for identity + time [src: https://docs.rs/gix/0.84.0/gix/struct.Repository.html].
5. Aggregate: per path → commit count, distinct author set size, max committer-time ns (last-changed); per unordered path pair changed in one commit → co-change count. Skip co-change for commits whose changed-file count exceeds `max_files_per_commit` — large commits are co-change noise and the pair set is O(n²) [src: Tornhill, "Your Code as a Crime Scene", 2015; https://understandlegacycode.com/blog/key-points-of-software-design-x-rays/].
6. Bound the walk by `depth` (default: full history; capped when set) read from `config.toml`.
7. Add `FileChurn { path, commits, author_keys: Vec<[u8;8]>, last_changed_ns }` + `CoChangePair { a, b, count }` to `ariadne-core` (owned, `Serialize`/`Deserialize`); distinct-author count = `author_keys.len()` (an `authors()` accessor). Storing the set — not a bare count — lets tier-11a merge incrementally by union with no second record migration [src: crates/ariadne-core/src/domain/records.rs:1-23].
8. `ariadne-storage`: define `CHURN` (`&[u8]` path → postcard `FileChurn`) + `CO_CHANGE` (`&[u8]` ordered-pair key → postcard `CoChangePair`); register the next `MigrationStep` opening both tables; implement the `Storage::replace_history` methods via `encode_value`/`decode_value` [src: crates/ariadne-storage/src/adapters/redb/tables.rs:12-17 ; crates/ariadne-storage/src/domain/migration.rs:98-100].
9. Wire into `ariadne index`; classify `ariadne-git` in `tests/architecture.rs`; write ADR-0018 (decision = `gix` 0.84.0 no-network; rejected = shelling to `git`, `git2`/libgit2 = C).
</steps>

<verification>
- `cargo nextest run -p ariadne-git -p ariadne-storage -p ariadne-cli` — churn/co-change extraction, the large-commit exclusion, table creation + migration round-trip all green.
- Migration: a redb file at `SCHEMA_VERSION-1` opens, gains the two tables, and every pre-existing record survives (no rebuild); a version with no registered path still returns `SchemaMismatch`.
- Manual: `ariadne index` on this repo (41 commits); spot-check a hot file's count against `git log --oneline -- <file> | wc -l`.
- `cargo test --test architecture`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check` — green.
</verification>

<rollback>
`git checkout -- .` and `rm -rf crates/ariadne-git docs/adr/0018-git-history-adapter.md`. The migration step is additive (drop the two tables; revert `SCHEMA_VERSION`); `Storage` port + config additions are removed with the checkout.
</rollback>
