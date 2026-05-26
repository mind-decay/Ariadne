---
tier_id: tier-11
title: Git history ingestion — ariadne-git adapter on gix, churn + co-change tables
deps: [tier-02]
exit_criteria:
  - A new `ariadne-git` driven adapter walks commit history and per-commit changed files via `gix`.
  - Per-file churn (commit count, distinct authors, last-changed) + co-change pairs persist to new redb tables.
  - The new tables ship behind a tier-02 migration step (existing indexes upgrade in place, no rebuild).
  - `ariadne index` ingests history within a configurable bounded commit depth.
  - `cargo nextest run -p ariadne-git -p ariadne-storage` + architecture + clippy + fmt all green.
status: pending
---

<context>
v1 analytics are static-only — they ignore how code changed over time. Block C adds history-derived signal. This tier ingests it: a pure-Rust `gix` adapter walks commits and records per-file churn and co-change (files changed together), persisted for later metrics (plan RD7). Full context: plan.md.
</context>

<files>
- crates/ariadne-git/Cargo.toml — new: deps `ariadne-core`, `gix = "=0.83.0"` (no network/transport features), `thiserror`.
- crates/ariadne-git/src/lib.rs — new: façade.
- crates/ariadne-git/src/domain/ — new: history record types kept pure where shared (or in `ariadne-core`).
- crates/ariadne-git/src/adapters/gix.rs — new: commit walk + per-commit diff (one file, one tech).
- crates/ariadne-git/src/errors.rs — new: `thiserror` `GitError`.
- crates/ariadne-core/src/domain/ — modify: `FileChurn` + `CoChangePair` pure record types.
- crates/ariadne-storage/src/ — modify: `CHURN` + `CO_CHANGE` redb tables + a tier-02 migration step adding them.
- crates/ariadne-cli/src/ — modify: `ariadne index` runs history ingestion (bounded depth from `config.toml`).
- tests/architecture.rs — modify: classify `ariadne-git` as a driven adapter (depends only on `ariadne-core`).
- docs/adr/0016-git-history-adapter.md — new.
</files>

<steps>
1. Failing test first (`ariadne-git` tests): over a fixture repo with a known commit history, assert the adapter reports the expected per-file commit counts and co-change pairs. Red — the crate does not exist.
2. Scaffold `ariadne-git` per `docs/folder-layout.md`; `gix` with default features minus network/transport so the critical path stays pure-Rust (no curl/C) [src: https://lib.rs/crates/gix].
3. Implement `adapters/gix.rs`: open the repo, walk history via `head().ancestors().all()`; for each commit compute changed paths by diffing it against its parent tree (`gix-diff`) [src: https://github.com/GitoxideLabs/gitoxide].
4. Aggregate: per file → commit count, distinct author count, last-changed timestamp; per unordered file pair changed in the same commit → co-change count.
5. Bound the walk by a configurable commit depth (`config.toml`); use the commit-graph file when present for an efficient walk (risk R-C1).
6. Add `FileChurn`/`CoChangePair` pure record types to `ariadne-core`.
7. `ariadne-storage`: define `CHURN` + `CO_CHANGE` tables; register a tier-02 `MigrationStep` that creates them so existing indexes upgrade in place.
8. Wire history ingestion into `ariadne index`; classify `ariadne-git` in `tests/architecture.rs`; write ADR-0016 (decision = `gix`; rejected = shelling to `git`, `git2`/libgit2).
</steps>

<verification>
- `cargo nextest run -p ariadne-git -p ariadne-storage` — churn/co-change extraction + migration green.
- Manual: `ariadne index` on the ariadne_v2 self-index; spot-check a hot file's commit count against `git log --oneline -- <file> | wc -l`.
- `cargo test --test architecture`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check` — green.
</verification>

<rollback>
`git checkout -- .` and `rm -rf crates/ariadne-git docs/adr/0016-git-history-adapter.md`. The migration step is additive and reversible (drop the new tables).
</rollback>
