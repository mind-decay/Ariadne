---
tier_id: tier-02
title: redb-backed storage adapter — Storage port + MVCC read/write txns + postcard codec
deps: [tier-01]
exit_criteria:
  - `ariadne-core::ports::Storage` trait + `Changeset`, `RevisionId`, `StorageError` (SchemaMismatch, NotFound, Corrupted, Io) declared in core; no redb types leak past the adapter boundary.
  - `ariadne_storage::RedbStorage::open(path)` creates `.ariadne/index.redb`, writes `schema_version = 1` to META, returns `StorageError::SchemaMismatch { found, expected }` on mismatch (no migration).
  - 4 tables (META, FILES, SYMBOLS, EDGES) + 1 multimap (EDGES_BY_FILE) defined via redb 4.1 `TableDefinition` / `MultimapTableDefinition`; postcard 1.1 codec for record bodies; tier-01 `IdEncode` for fixed-width keys.
  - Proptest 1.11 codec roundtrip ≥10K cases per record type, no shrink failures.
  - `WriteTxn::apply(&Changeset)` is atomic (single redb write txn); criterion bench reports p50/p95 over 1K/10K/100K-edge changesets.
  - `ReadSnapshot` survives 16 reader threads + 1 writer thread, 5s, no torn reads, writer not blocked (redb MVCC).
  - insta golden of a 5-file / 20-edge changeset (pretty-printed per-table dump) committed.
  - Bench output records redb file size + RSS delta on 10K-edge changeset; no hard gate this tier (baseline for R1).
status: completed
completed: 2026-05-19
---

<context>
Persistence backbone. Fills the `Storage` port left empty by tier-01. Every downstream tier writes through this port; redb stays confined to `ariadne-storage::adapters::redb`.

redb chosen for pure-Rust + ACID + MVCC concurrent readers without blocking the writer [src: https://docs.rs/redb/4.1.0/redb/]. Bincode is OUT — RUSTSEC-2025-0141 (2026-01-07) marks bincode permanently unmaintained; cargo-audit + deny.toml advisories=deny would fail [src: https://rustsec.org/advisories/RUSTSEC-2025-0141]. Replaced with postcard 1.1.3 (serde-compatible, stable wire format, recommended by the bincode advisory) [src: https://docs.rs/postcard/1.1.3, https://crates.io/crates/postcard].

Out of scope (encoded as hard limits): schema migration (rebuild_required on mismatch), at-rest encryption, compression, secondary indexes beyond `EDGES_BY_FILE`.
</context>

<files>
- `crates/ariadne-core/src/domain/ports.rs` — flesh out `Storage`, `WriteTxn`, `ReadSnapshot` trait shapes (signatures only; no IO).
- `crates/ariadne-core/src/domain/changeset.rs` — pure `Changeset { file_upserts, file_deletes, symbol_upserts, symbol_deletes, edges_added, edges_removed }` + builder; `RevisionId(u64)`.
- `crates/ariadne-core/src/domain/records.rs` — `FileRecord { path, lang, size, blake3: [u8;32], mtime_ns }`, `SymbolRecord { canonical_name, kind, defining_file, defining_span }`, `EdgeKind` enum, `EdgeKey { src: SymbolId, kind: EdgeKind, dst: SymbolId }`, `EdgeRecord { source_span, evidence_lang, weight }`. All `#[derive(Serialize, Deserialize)]` for postcard.
- `crates/ariadne-core/src/errors.rs` — extend with `StorageError` (thiserror) variants above.
- `crates/ariadne-storage/Cargo.toml` — exact pins `redb = "=4.1.0"`, `postcard = { version = "=1.1.3", features = ["use-std"] }`, `blake3 = "=1.8.5"`; workspace `serde`/`thiserror`/`tracing`; dev `proptest`, `insta`, `criterion`, `tempfile`.
- `crates/ariadne-storage/src/lib.rs` — façade: `pub use adapters::redb::RedbStorage;` only. No prose.
- `crates/ariadne-storage/src/domain/mod.rs` — adapter-internal helpers (e.g. internal `WriteOp` enum). No IO.
- `crates/ariadne-storage/src/adapters/redb.rs` — `RedbStorage`, `RedbWriteTxn`, `RedbReadSnapshot` (implement core ports). One file per external tech [src: tier-00 docs/folder-layout.md].
- `crates/ariadne-storage/src/adapters/codec.rs` — `redb::Value` / `redb::Key` impls for `FileId`, `SymbolId`, `EdgeKey`, `FileRecord`, `SymbolRecord`, `EdgeRecord`. Keys = fixed-width big-endian via `IdEncode`; values = postcard.
- `crates/ariadne-storage/src/errors.rs` — `RedbStorageError` (thiserror) mapping `redb::Error` / `postcard::Error` → `ariadne_core::StorageError` via `From`.
- `crates/ariadne-storage/tests/roundtrip.rs` — proptest 10K cases per record type. Arbitrary impls in `tests/support.rs`.
- `crates/ariadne-storage/tests/mvcc.rs` — 16 reader threads + 1 writer thread, 5s stress.
- `crates/ariadne-storage/tests/changeset.rs` — insta golden snapshot of a 5-file / 20-edge changeset.
- `crates/ariadne-storage/benches/apply.rs` — criterion benches at 1K/10K/100K edges; logs file size + RSS delta.
</files>

<steps>
1. **Failing tests first** (`tests/roundtrip.rs`): proptest 10K cases per record type. Generate arbitrary record → `RedbStorage::open(tempdir)` → write in txn → re-open `ReadSnapshot` → assert equality. Uses `proptest::collection::vec` + `prop_assert_eq!` [src: https://docs.rs/proptest/1.11.0].
2. Add deps to workspace `Cargo.toml` and `crates/ariadne-storage/Cargo.toml` with exact pins. Verify pure-Rust: `cargo tree -i redb`, `cargo tree -i postcard`, `cargo tree -i blake3` show zero `-sys` deps. blake3 simd uses pure Rust on non-x86 fallback [src: https://docs.rs/blake3/1.8.5].
3. Fill `ariadne-core::ports`:
   ```rust
   pub trait Storage: Send + Sync {
       type Write<'a>: WriteTxn + 'a where Self: 'a;
       type Read<'a>: ReadSnapshot + 'a where Self: 'a;
       fn begin_write(&self) -> Result<Self::Write<'_>, StorageError>;
       fn snapshot(&self) -> Result<Self::Read<'_>, StorageError>;
       fn revision(&self) -> RevisionId;
   }
   pub trait WriteTxn {
       fn apply(self, cs: &Changeset) -> Result<RevisionId, StorageError>;
   }
   pub trait ReadSnapshot {
       fn file(&self, id: FileId) -> Result<Option<FileRecord>, StorageError>;
       fn symbols_in_file(&self, id: FileId) -> Result<Vec<SymbolRecord>, StorageError>;
       fn outgoing_edges(&self, src: SymbolId) -> Result<Vec<(EdgeKey, EdgeRecord)>, StorageError>;
       fn incoming_edges(&self, dst: SymbolId) -> Result<Vec<(EdgeKey, EdgeRecord)>, StorageError>;
       fn edges_in_file(&self, file: FileId) -> Result<Vec<EdgeKey>, StorageError>;
   }
   ```
   GAT-style associated types keep redb's txn lifetimes out of `StorageError`. `Send + Sync` per hexagonal Rust idiom [src: https://www.howtocodeit.com/guides/master-hexagonal-architecture-in-rust].
4. Declare tables in `adapters/redb.rs` against redb 4.1 [src: https://docs.rs/redb/4.1.0/redb/struct.TableDefinition.html, https://docs.rs/redb/4.1.0/redb/struct.MultimapTableDefinition.html]:
   - `META: TableDefinition<&str, u64>` — `"schema_version"`, `"revision"`.
   - `FILES: TableDefinition<&[u8], &[u8]>` — key = `FileId::to_bytes()`, value = postcard(FileRecord).
   - `SYMBOLS: TableDefinition<&[u8], &[u8]>` — key = `SymbolId::to_bytes()`, value = postcard(SymbolRecord).
   - `EDGES: TableDefinition<&[u8], &[u8]>` — key = 17-byte fixed `[src(8) | kind(1) | dst(8)]` (big-endian, lex-ordered); value = postcard(EdgeRecord).
   - `EDGES_BY_FILE: MultimapTableDefinition<&[u8], &[u8]>` — `FileId::to_bytes()` → set of EdgeKey bytes; populated when an edge's `source_span.file` is the upserted file. Drives watcher invalidation in tier-06.
5. Codec impls in `adapters/codec.rs` against redb 4.1 trait shape [src: https://docs.rs/redb/4.1.0/redb/trait.Value.html, https://docs.rs/redb/4.1.0/redb/trait.Key.html]:
   - `Value::SelfType<'a> = Self; Value::AsBytes<'a> = Vec<u8>; fixed_width() = None;` body via `postcard::to_stdvec` / `postcard::from_bytes` [src: https://docs.rs/postcard/1.1.3].
   - For IDs and EdgeKey, fixed-width keys via `IdEncode` (tier-01); implement `Key::compare(a, b) = a.cmp(b)` (lex byte compare — sound because big-endian fixed-width).
   - DO NOT postcard-encode keys: postcard varint LEB128 is not order-preserving across byte-length boundaries [src: https://postcard.jamesmunns.com/wire-format]. Property test asserts: for 1K random ID pairs, `Key::compare(a.as_bytes(), b.as_bytes()) == a.cmp(&b)`.
   - `from_bytes` returns the decoded value; corruption panics with `expect` — file-scoped `#![allow(clippy::expect_used)]` documented inline; rationale: redb 4.1 `Value::from_bytes` signature has no error channel [src: https://docs.rs/redb/4.1.0/redb/trait.Value.html#tymethod.from_bytes].
6. `RedbStorage::open(path: &Path) -> Result<Self, StorageError>`:
   - `std::fs::create_dir_all(path.parent())`; `redb::Database::create(path)` [src: https://docs.rs/redb/4.1.0/redb/struct.Database.html#method.create].
   - In a `begin_write` txn, open META; read `"schema_version"`. Absent → insert `1u64`, commit. Present and `!= 1` → `StorageError::SchemaMismatch { found, expected: 1 }`.
   - Read `"revision"` (default 0) into `AtomicU64` on the struct; `Storage::revision()` returns it without txn.
7. `RedbWriteTxn::apply(&Changeset)` in a single `db.begin_write()` txn [src: https://docs.rs/redb/4.1.0/redb/struct.Database.html#method.begin_write]:
   1. For each `file_deletes`: drain `EDGES_BY_FILE[file]` → for each edge key remove from `EDGES`; remove all `SYMBOLS` whose `defining_file == file` (single-pass range scan); remove `FILES[file]`; drain the multimap entry.
   2. Upsert `file_upserts` into `FILES` (caller provides blake3; storage does not hash).
   3. Upsert `symbol_upserts` into `SYMBOLS`; apply `symbol_deletes`.
   4. Apply `edges_removed` (drop from `EDGES`; remove multimap pair `(source_span.file, key)`).
   5. Apply `edges_added` (insert into `EDGES`; insert multimap `(source_span.file, key)`).
   6. Increment META `"revision"`; commit; update `AtomicU64`; return new `RevisionId`.
   On commit failure, redb auto-rolls back [src: https://docs.rs/redb/4.1.0/redb/struct.WriteTransaction.html].
8. `RedbReadSnapshot::open(&self)` wraps `db.begin_read()`; typed accessors close over the read txn (lifetime-bound via `Storage::Read<'_>` GAT). Reads do not block writers; writer does not block readers [src: https://docs.rs/redb/4.1.0/redb/#features].
9. **MVCC stress** (`tests/mvcc.rs`): spawn 16 reader threads (random `outgoing_edges`/`file` queries) + 1 writer thread (apply a 100-edge changeset per iter); run 5s. Assertions: (a) every read returns records consistent with some single committed revision (revision recorded in each FileRecord via blake3-of-Changeset → unused field skipped; instead readers re-read `Storage::revision()` and assert monotonic non-decrease); (b) writer median wall-time ≤ 1.25× single-threaded baseline (writer-not-blocked-by-readers smoke check).
10. **Criterion bench** (`benches/apply.rs`): sizes 1K / 10K / 100K random edges over 50 files. Record p50/p95 per size. After each iter, capture: `std::fs::metadata(path).len()` and process RSS via `sysinfo` 0.31 (cross-platform, pure-Rust [src: https://docs.rs/sysinfo]). Log to stdout; no CI gate this tier (baseline for tier-04 R1 probe).
11. **Insta golden** (`tests/changeset.rs`): hand-build deterministic 5-file × 20-edge changeset (FileId 1..=5, SymbolId 1..=15, EdgeKind cycling through `Defines`/`References`/`Imports`). Apply, then iterate every table in sorted order, format each entry via `{:#?}`, concatenate to a single string, `insta::assert_snapshot!(...)` [src: https://docs.rs/insta/1.47.2].
12. Façade `lib.rs`: re-exports only; one-line rustdoc per item per project rule. No prose docs in adapter source.
</steps>

<verification>
- `cargo nextest run -p ariadne-storage` green; proptest covers ≥10K cases per record type without shrink failures.
- `cargo bench -p ariadne-storage --no-run` builds; `cargo bench -p ariadne-storage` emits p50/p95 + file-size + RSS-delta lines (manual visual check this tier; tier-04 starts gating).
- `cargo deny check` clean — postcard / redb / blake3 / sysinfo all under tier-00 license allowlist (MIT / Apache-2.0).
- `cargo audit` clean — bincode absent confirms RUSTSEC-2025-0141 not triggered [src: https://rustsec.org/advisories/RUSTSEC-2025-0141].
- `cargo test --test architecture` (tier-00 invariant) still green: `cargo tree -p ariadne-storage` shows only `ariadne-core` from the workspace.
- Manual cross-process MVCC check: a second `RedbStorage::open` of the file produced by `changeset.rs` reads `schema_version = 1`, `revision` matching.
- Failure modes that must be loud (no silencing): corruption → panic via codec `expect` (intentional, redb API constraint); schema mismatch → `StorageError::SchemaMismatch`; redb error → mapped variant. No `try`/`ignore`/`unwrap_or_default` on IO paths.
</verification>

<rollback>
Tier is additive. Rollback steps:
1. `git rm -r crates/ariadne-storage/src/adapters crates/ariadne-storage/src/domain crates/ariadne-storage/tests crates/ariadne-storage/benches crates/ariadne-storage/src/errors.rs`.
2. Revert `ariadne-core` ports/records/changeset/errors additions (`git checkout main -- crates/ariadne-core/src/domain crates/ariadne-core/src/errors.rs`).
3. Drop redb / postcard / blake3 / sysinfo from workspace `Cargo.toml`.
4. Delete any leftover `.ariadne/index.redb` test artefacts (safe — no production data).
Completed prior tiers (00, 01) remain untouched.
</rollback>
