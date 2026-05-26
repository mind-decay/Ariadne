---
tier_id: tier-02
audited: 2026-05-22
verdict: PASS
commit: d363c2a27cd0b573da8bb871811fc535d0edf5cf
---

<scope>
Audited tier-02 "redb schema migration — versioned vN->vN+1 steps replace
rebuild-on-mismatch" of the `post-v1-roadmap` plan. Working-tree diff scoped to
the tier `<files>`:
- `crates/ariadne-storage/src/domain/migration.rs` — new (160 lines).
- `crates/ariadne-storage/src/adapters/redb/mod.rs` — modified open path.
- `crates/ariadne-storage/src/adapters/redb/tables.rs` — `SCHEMA_VERSION` 1→2.
- `crates/ariadne-storage/src/domain/mod.rs` — declares `migration` module.
- `crates/ariadne-storage/src/errors.rs` — new `Migration` error variant.
- `crates/ariadne-storage/tests/migration.rs` — new (250 lines).
- `crates/ariadne-storage/tests/changeset.rs` — adjusted mismatch test.
- `crates/ariadne-storage/fixtures/schema-v1.redb` — new committed v1 fixture.

The tier `<files>` names `adapters/redb.rs`; the adapter is already a
directory (`redb/mod.rs` + `tables.rs`/`apply.rs`/`scan.rs`/`snapshot.rs`),
so the modification landed in `redb/mod.rs`. `domain/mod.rs` and
`tests/changeset.rs` are outside the literal `<files>` list but their edits
are forced by `<steps>` 1 and 6: declaring a new module requires editing
`domain/mod.rs`, and bumping `SCHEMA_VERSION` to 2 makes the old
`changeset.rs` mismatch test (which used `2u64`) no longer a mismatch, so it
had to move to `3u64`. Both are justified, in-scope adjustments.
</scope>

<checks_run>
- `cargo nextest run -p ariadne-storage` — 23 passed, 1 skipped (the
  `#[ignore]`d `generate_v1_schema_fixture` helper). Green.
- `cargo test --test architecture` — `architecture_invariants_hold` ok. Green.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` —
  exit 0, no warnings. Green.
- `cargo fmt --all --check` — exit 0. Green.
- Read every changed file end-to-end (`migration.rs`, `redb/mod.rs`,
  `tables.rs`, `domain/mod.rs`, `errors.rs`, `migration.rs` test,
  `changeset.rs` diff).
- Verified the committed `fixtures/schema-v1.redb` is a real v1 database
  (`file` → `data`, 1.1M; test asserts `on_disk_schema_version == Some(1)`
  pre-open and `Some(2)` post-open).
- Traced `MigrationRegistry::plan` against `<steps>` 3–5 and re-derived the
  three branch outcomes (registered path, version > current, unregistered
  gap) by hand; cross-checked against `WriteTransaction` ACID semantics
  [src: https://docs.rs/redb/4.1.0/redb/struct.WriteTransaction.html].
</checks_run>

<findings>
| id | category | severity | location | problem | fix |
|---|---|---|---|---|---|
| F1 | architecture | INFO | `crates/ariadne-storage/src/errors.rs:74-76` | A failed migration step (rolled-back txn, DB intact at its original version) is mapped to `StorageError::Corrupted`, which connotes on-disk damage that did not occur. | Acceptable within tier scope (no `ariadne-core` `StorageError` variant exists for "migration failed" and adding one is outside `<files>`); the message is explicit and the path is currently unreachable (only a no-op step is registered). Consider a dedicated core variant if a non-identity step is ever added. |
</findings>

<verdict>
PASS. Zero FAIL findings.

All four `exit_criteria` independently verified:
1. `MigrationRegistry` runs ordered `vN->vN+1` steps inside one
   `WriteTransaction` — `run_migration` takes `&WriteTransaction` (the single
   txn `bootstrap` opens), iterates the planned chain, and the version bump +
   chain commit atomically; a pre-commit crash leaves the file at its
   original version.
2. Opening a `current-1` file migrates in place with all records intact —
   `v1_fixture_migrates_in_place_with_all_records_intact` opens the committed
   v1 fixture, asserts every file/symbol/edge survives and the revision
   counter is preserved, and confirms the on-disk version advances 1→2.
3. A version gap with no registered path still returns `SchemaMismatch` —
   covered in both directions: `older_version_with_no_migration_path_returns_
   schema_mismatch` (v0, below current) and the retained
   `reopen_with_mismatched_schema_version_returns_schema_mismatch` (v3, above
   current). `plan` returns `None` for `from >= to` and for an unregistered
   start, and `run_migration` converts that to the unchanged `SchemaMismatch`
   — no silent data loss.
4. `cargo nextest run -p ariadne-storage` + architecture + clippy + fmt all
   green (see `<checks_run>`).

The `migrate_v1_to_v2` identity step is correct: the v1→v2 diff bumps only
the version constant and changes no table layout or record codec, so the
layouts are byte-identical and the no-op is sound — exactly what `<steps>` 6
authorizes. Tests are realistic (real redb files, real adapter surface, no
module-boundary mocks) and fail loudly. The `Migration` error keeps
`thiserror` — no `anyhow` smuggled into the adapter crate. No new dependency
introduced.
</verdict>

<next_steps>
None blocking. F1 is INFO and does not gate. The tier may proceed to commit;
ensure `crates/ariadne-storage/fixtures/schema-v1.redb` is staged (currently
untracked) so the committed fixture the round-trip test depends on lands with
the change.
</next_steps>

<sources>
- redb WriteTransaction (ACID, single-txn migration): https://docs.rs/redb/4.1.0/redb/struct.WriteTransaction.html
- Reviewer standard / code-health-over-perfection: https://google.github.io/eng-practices/review/reviewer/standard.html
- Comment severity guidance: https://google.github.io/eng-practices/review/reviewer/comments.html
- Tier spec: .claude/plans/post-v1-roadmap/tier-02-redb-schema-migration.md
- Plan RD2: .claude/plans/post-v1-roadmap/plan.md
</sources>
