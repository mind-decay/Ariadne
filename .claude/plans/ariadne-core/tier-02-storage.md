---
tier_id: tier-02
title: redb-backed storage layer (tables, codecs, write txns, MVCC read snapshots)
deps: [tier-01]
exit_criteria:
  - `Storage::open(path)` opens/creates `.ariadne/index.redb` with on-disk schema_version table; mismatch triggers rebuild_required error.
  - All 6 core tables defined with typed codecs and proptest round-trip suite (10K random records each).
  - `WriteTxn::apply_changeset(cs)` atomically updates files/symbols/edges in <10ms p95 for 1K-edge changeset (criterion).
  - `ReadSnapshot` supports N concurrent readers + 1 writer without blocking (proptest concurrent stress).
  - insta snapshot fixture of an empty + populated DB dump (pretty-printed by-table) committed for diff-review.
status: pending
---

<context>
Persistence backbone. Every downstream tier writes here. redb chosen for: pure Rust, ACID, MVCC, stable on-disk format [src: https://github.com/cberner/redb]. Goal: small surface, strict codec separation, no leakage of redb types to upstream crates.
</context>

<files>
- `crates/ariadne-storage/Cargo.toml` — adds `redb`, `bincode`, `blake3`, `bytes`, workspace `serde`/`thiserror`/`tracing`.
- `crates/ariadne-storage/src/lib.rs` — re-exports `Storage`, `WriteTxn`, `ReadSnapshot`, `Changeset`, `StorageError`.
- `crates/ariadne-storage/src/schema.rs` — `TableDefinition` constants + schema_version constant.
- `crates/ariadne-storage/src/codec.rs` — bincode-based `redb::Value` impls for `FileRecord`, `SymbolRecord`, `EdgeRecord`, `ParseCache`, `ScipDoc`.
- `crates/ariadne-storage/src/changeset.rs` — `Changeset` builder + `WriteTxn::apply_changeset` impl.
- `crates/ariadne-storage/src/snapshot.rs` — `ReadSnapshot` thin wrapper around `redb::ReadTransaction`.
- `crates/ariadne-storage/tests/roundtrip.rs` — proptest roundtrip for each codec.
- `crates/ariadne-storage/tests/mvcc.rs` — concurrent reader/writer proptest.
- `crates/ariadne-storage/tests/changeset.rs` — golden insta snapshot for a small known changeset.
- `crates/ariadne-storage/benches/apply.rs` — criterion for `apply_changeset` over 1K/10K/100K edges.
</files>

<steps>
1. Add `redb` workspace dep (pin exact version in `Cargo.lock`). Confirm pure-Rust by `cargo tree -i redb` showing no `-sys` deps [src: https://github.com/cberner/redb].
2. Define `SCHEMA_VERSION: u32 = 1` + `META_TABLE: TableDefinition<&str, u32>` ("schema_version", "tool_version") [src: https://docs.rs/redb/latest/redb/struct.TableDefinition.html].
3. Define 6 typed tables:
   - `FILES: TableDefinition<FileIdBytes, FileRecordBytes>` (`FileRecord = { path, lang, size, blake3_hash, mtime_ns, scip_doc_present }`).
   - `SYMBOLS: TableDefinition<SymbolIdBytes, SymbolRecordBytes>` (`SymbolRecord = { canonical_name, kind, defining_file, defining_span, doc?, signature? }`).
   - `EDGES: TableDefinition<EdgeKeyBytes, EdgeRecordBytes>` (`EdgeKey = (src_symbol, kind, dst_symbol)`; `EdgeRecord = { source_span, evidence_lang, weight }`).
   - `PARSE_CACHE: TableDefinition<FileIdBytes, ParseCacheBytes>` (`ParseCache = { lang, ts_tree_serialized, content_hash }`; bincode-encoded `tree_sitter::Tree::serialize` bytes — feature gate behind `parser` consumer).
   - `SCIP_DOCS: TableDefinition<&str, ScipDocBytes>` (path → raw SCIP protobuf bytes).
   - `MULTI_FILE_EDGES: MultimapTableDefinition<FileIdBytes, EdgeIdBytes>` (reverse index for file → outgoing edges; used by watcher invalidation) [src: https://docs.rs/redb/latest/redb/struct.MultimapTableDefinition.html].
4. Codecs: implement `redb::Value` + `redb::Key` for each record via `bincode::serde::encode_to_vec` / `decode_from_slice` [src: https://docs.rs/bincode]. ID byte forms come from `IdEncode` in ariadne-core (tier-01).
5. Write **failing tests first** (`tests/roundtrip.rs`): for each record type, generate 10K random instances with `proptest`, write in a txn, read back, assert equality [src: https://proptest-rs.github.io].
6. Implement `Storage::open(path: &Path) -> Result<Self>`:
   - `Database::create(path)` (creates parent dirs).
   - In a write txn, read META `schema_version`; if absent insert `SCHEMA_VERSION`; if present and mismatched return `StorageError::SchemaMismatch { found, expected }` (no auto-migration in v1).
7. `WriteTxn` wraps `redb::WriteTransaction`. Expose `apply_changeset(&Changeset)` which:
   - Inserts new/updated files (+ recomputes content hash); deletes tombstoned files.
   - Inserts/updates symbols/edges; removes dangling edges via `MULTI_FILE_EDGES` lookup.
   - Bumps a monotonic `revision: u64` in META table.
   - Returns `RevisionId` to upstream.
8. `ReadSnapshot::open(&Storage) -> Result<Self>` wraps `redb::ReadTransaction`. Exposes typed accessors `file(FileId) -> Option<FileRecord>`, `symbols_in_file(FileId) -> Vec<SymbolRecord>`, `outgoing_edges(SymbolId) -> Vec<EdgeRecord>`, `incoming_edges(SymbolId) -> Vec<EdgeRecord>`.
9. Concurrent stress test (`tests/mvcc.rs`): 16 reader threads + 1 writer thread for 5s, proptest random read sequences interleaved with writes; assert no reader observes torn writes, writer never blocks readers [src: https://github.com/cberner/redb (MVCC docs)].
10. Criterion bench (`benches/apply.rs`): 1K / 10K / 100K random-edge changesets; record p50/p95; gate p95 ≤10ms / ≤100ms / ≤1s respectively in CI.
11. Golden snapshot (`tests/changeset.rs`): apply a hand-crafted 5-file changeset → use insta to assert a stable pretty-printed dump of all 6 tables.
12. Document the public API in `crates/ariadne-storage/src/lib.rs` doc comment (one line each — no prose docs per project rules).
</steps>

<verification>
- `cargo nextest run -p ariadne-storage` green; proptest covers 10K cases per table without shrinking failures.
- `cargo bench -p ariadne-storage` reports p95 within budgets above; CI gate enforced by `criterion`'s `--save-baseline`.
- Open redb file from a second process via `Storage::open` to confirm MVCC isolation behavior.
- Manual: `ariadne-cli` (stub) writes a fake changeset, inspect `.ariadne/index.redb` with `redb-cli` [src: https://docs.rs/redb/latest/redb/]. Expected: file exists, schema_version=1.
</verification>

<rollback>
Storage layer is additive. Rollback = `git rm -r crates/ariadne-storage` + remove from workspace `Cargo.toml` members. Any on-disk `.ariadne/index.redb` left over is safe to delete.
</rollback>
