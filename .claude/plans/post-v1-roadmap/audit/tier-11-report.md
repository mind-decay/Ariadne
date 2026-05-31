---
tier_id: tier-11
audited: 2026-06-01
verdict: PASS
commit: 7a1d738dd6516b32959ebe4fa8218c0d62ed3dc8
---

<scope>
Tier-11 — Git history ingestion (file-level, cold): new `ariadne-git` driven
adapter on `gix`, per-file churn + unordered co-change persisted to new redb
`CHURN` + `CO_CHANGE` tables behind a v3→v4 migration, wired into `ariadne
index` at the CLI composition root.

Scoped diff (vs HEAD 7a1d738): new crate `crates/ariadne-git/` (Cargo.toml,
lib.rs, adapters/{mod,gix}.rs, errors.rs, tests/history.rs); core
records.rs + ports.rs + lib.rs; storage redb/{mod,tables}.rs +
migration.rs + tests/{history,migration,changeset}.rs; cli config.rs +
commands/index.rs + Cargo.toml; tests/architecture.rs; docs/adr/0018; plus
mechanical infra (Cargo.lock, ci.yml, cog.toml `git` scope) and the tier-file
status flip. Out-of-`<files>` touches (test files, Cargo.toml dep, ci.yml,
cog.toml) are each forced by the listed changes and verified justified — see
`<checks_run>`.
</scope>

<checks_run>
- plan_adherence: every `<files>` entry touched as intended. Out-of-list files
  all justified: `ariadne-cli/Cargo.toml` (path dep required to wire the
  adapter), `ariadne-git/src/adapters/mod.rs` (module decl), the three storage
  test files + `ariadne-git/tests/history.rs` (mandated by step 1 +
  `<verification>` round-trip/migration), `ci.yml`+`cog.toml` (register the new
  `git` crate scope so its commits pass Conventional-Commits lint), `Cargo.lock`
  (mechanical). No smuggled crate.
- correctness: walk = `head()` → `rev_walk(Some(head)).sorting(ByCommitTime
  NewestFirst).all()`; per commit first-parent `diff_tree_to_tree`; root commit
  diffs vs `None` parent (empty tree); `change_path` keeps only
  `is_blob_or_symlink()` entries (directory/tree entries skipped — proved by
  `records_full_blob_paths_and_skips_directory_entries`). Co-change pairs are
  the canonical `a<b` cross-product, skipped when `touched.len() >
  max_files_per_commit`. `co_change_key` = `a \0 b` with a `0x00` separator —
  injective (paths hold no NUL) and lex-order-preserving over `(a,b)`, so reads
  return deterministically sorted. Spot-check below confirms exactness.
- security: no untrusted input, no secrets, no injection surface. `git`
  subprocess appears only in the test harness, args are static, identity/dates
  pinned via env and isolated (`GIT_CONFIG_GLOBAL=/dev/null`). N/A to OWASP.
- performance: walk runs once at index time; large-commit O(n²) pair explosion
  bounded by `max_files_per_commit` (default 50); bounded `depth`; uses the
  commit-graph file via `gix` when present (R-C1). Aggregation is BTreeMap, no
  N+1 or hot-path sync IO beyond the unavoidable object reads.
- architecture: `tests/architecture.rs` reclassifies `ariadne-git` as a driven
  adapter; test green (deps ⊆ {ariadne-core}). No `gix` type crosses the public
  API (lib.rs re-exports only `HistoryOptions`/`HistoryReport`/`walk_history`/
  `GitError`; `GitError` flattens `gix` errors to `String`). Daemon dep set
  stays git-free (CLI is the sole consumer). D5 pure-Rust verified — see
  sources: no `*-sys`, `cc`, openssl, curl, or libgit2 in the `gix` subtree;
  `sha1` v0.10.6 (RustCrypto) + `zlib-rs` (pure-Rust) + `libc` (bindings only).
- tests: realistic — fixture repos built with the real `git` binary, adapter
  reads a real `.git` (no boundary mocks). Asserts behaviour (counts, authors,
  last-changed ns, pair sets, exclusion, depth, blob-only paths). Storage tests
  assert round-trip, empty-on-fresh, and replace-not-merge. Migration test
  synthesizes a real v3 db (drops tables + downgrades version) and proves v3→v4
  recreates tables with files/symbols/edges intact.
- docs/verification: re-ran every `<verification>` command (results below).
- exit_criteria: all five independently verified (below).

Commands re-run (full):
- `cargo nextest run -p ariadne-git -p ariadne-storage -p ariadne-cli` → 59
  passed, 2 skipped, 0 failed (incl. all 4 ariadne-git tests, 3 storage history
  tests, v3→v4 migration test).
- `cargo test --test architecture` → 1 passed.
- `cargo clippy --workspace --all-targets -- -D warnings` → clean, exit 0.
- `cargo fmt --all --check` → clean, exit 0.
- Manual `ariadne index` on this repo → `[index] history: 599 files, 9333
  co-change pairs`; persisted (revision 2). Spot-check (throwaway read-back test,
  removed after): CLAUDE.md commits=3/authors=1, redb/mod.rs 3/1, records.rs 3/1
  — exactly equal to `git log --oneline -- <f>` / distinct `%ae` (repo is linear,
  0 merges, so first-parent diff == git-log simplified history).

exit_criteria verification:
1. `ariadne-git` walks bounded history + per-commit changed paths via `gix`,
   deps ⊆ {core} — PASS (arch test + adapter source + tests).
2. churn (commits, distinct authors, last-changed ns) + unordered co-change
   persist to `CHURN`+`CO_CHANGE` — PASS (round-trip test + e2e run).
3. one v3→v4 migration step; pre-existing db opens & upgrades in place, no
   rebuild — PASS (`v3_database_gains_history_tables_with_records_intact`).
4. `ariadne index` ingests within configurable bounded depth from config.toml;
   commits over `max_files_per_commit` excluded from co-change — PASS
   (`HistoryConfig` default 50, e2e run, `excludes_large_commits_from_co_change`).
5. nextest + architecture + clippy + fmt all green — PASS.
</checks_run>

<findings>
| id | category | severity | location | problem | fix |
|----|----------|----------|----------|---------|-----|
| F1 | architecture | INFO | crates/ariadne-git/Cargo.toml:24-28 | `gix` features are `["blob-diff","revision","sha1"]`; the tier `<files>` (line 20) and `<tech_inventory>` (plan.md:102) list only `blob-diff`. | None required — `revision` is needed for the plan's own `rev_walk` (step 3) and `sha1` for object decoding; both are local/non-network/pure-Rust, the deviation is disclosed and justified in ADR-0018, and the "pure-Rust, no C/curl" intent is verified intact (no `*-sys`/`cc`/openssl/libgit2 in the subtree). Non-blocking. |
</findings>

<verdict>
PASS. Zero FAIL findings. The adapter, records, port extension, redb tables +
v3→v4 migration, config block, and CLI wiring all match the tier `<steps>` and
`<decisions>`; every `<verification>` command re-ran green; the end-to-end
`ariadne index` run ingested history and the per-file churn matched `git log`
exactly. The single INFO is a documented, verified-safe feature-set deviation
that does not gate.
</verdict>

<next_steps>
None to redo. Tier-11 is accepted. The HEAD-oid watermark incremental re-walk
(tier-11a) and per-symbol attribution (tier-11b) build on this adapter.
</next_steps>

<sources>
- gix crate / pure-Rust Git: https://lib.rs/crates/gix
- gix 0.84 Repository API (head_commit / rev_walk / diff_tree_to_tree):
  https://docs.rs/gix/0.84.0/gix/struct.Repository.html
- gix 0.84 feature flags (blob-diff / revision / sha1; default = network):
  https://docs.rs/crate/gix/0.84.0/features
- redb WriteTransaction (single-txn migration): https://docs.rs/redb/4.1.0/redb/struct.WriteTransaction.html
- postcard non-self-describing wire format: https://postcard.jamesmunns.com/wire-format
- co-change / large-commit-as-noise rationale: Tornhill, "Your Code as a Crime
  Scene", 2015
- dep-tree C/sys scan: `cargo tree -p ariadne-git -e features --no-default-features`
  and `-e build` — no `*-sys`/`cc`/openssl/curl/libgit2; `sha1` v0.10.6
  (RustCrypto), `zlib-rs` pure-Rust, `libc` bindings only.
- OWASP Top 10 (security checklist): https://owasp.org/www-project-top-ten/
</sources>
