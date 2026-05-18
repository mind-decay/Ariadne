---
tier_id: tier-04
title: Salsa query layer (incremental computation over storage + parser + scip)
deps: [tier-01, tier-02, tier-03]
exit_criteria:
  - `AriadneDb` Salsa database with input queries (file_content, file_metadata, scip_doc_for_file, project_config) and derived queries (parse_tree, syntactic_facts, symbols_for_file, edges_for_file, blast_radius).
  - Durability levels assigned: stdlib/vendor inputs = `Durability::HIGH`; dep-tree = `MEDIUM`; project source = `LOW` [src: https://rust-analyzer.github.io/blog/2023/07/24/durable-incrementality.html].
  - Per-table memory probe exposed via `AriadneDb::memory_report()` returning per-table byte counts; warns if any table > 256MB.
  - Proptest: random edit sequence → derived query results equal a fresh-DB full recompute (100 iterations).
  - Criterion: single-file edit re-derives `symbols_for_file` for that file in <50ms p95; unrelated files re-derive in 0ms (cache hit).
status: pending
---

<context>
Brain of the system. Salsa converts manual cache invalidation into a query graph with automatic early-cutoff [src: https://github.com/salsa-rs/salsa]. Risk-aware tier because rust-analyzer's 2025 Salsa migration caused a 4x memory regression [src: https://github.com/rust-lang/rust-analyzer/issues/19402]; we measure per-table memory from day one.
</context>

<files>
- `crates/ariadne-salsa/Cargo.toml` — `salsa` (latest), workspace deps.
- `crates/ariadne-salsa/src/lib.rs` — re-exports `AriadneDb`, query traits, `Durability`.
- `crates/ariadne-salsa/src/db.rs` — `#[salsa::db] pub struct AriadneDb { storage: salsa::Storage, ... }`.
- `crates/ariadne-salsa/src/inputs.rs` — input queries (`#[salsa::input]`): `FileContentInput`, `FileMetadataInput`, `ScipDocInput`, `ProjectConfigInput`.
- `crates/ariadne-salsa/src/derived.rs` — `#[salsa::tracked]` fns: `parse_tree`, `syntactic_facts`, `symbols_for_file`, `edges_for_file`, `blast_radius`.
- `crates/ariadne-salsa/src/memory.rs` — per-table probe using `salsa::Storage::heap_size` + custom counters per `#[salsa::tracked]`.
- `crates/ariadne-salsa/tests/equivalence.rs` — proptest: full-rebuild vs incremental equivalence.
- `crates/ariadne-salsa/tests/durability.rs` — assert HIGH-durability input change does not invalidate unrelated LOW queries.
- `crates/ariadne-salsa/benches/edit.rs` — criterion micro-bench for single-file edit.
</files>

<steps>
1. Add `salsa` workspace dep (latest stable on crates.io; verify it is the new `salsa-rs/salsa` (not `rust-analyzer-salsa` legacy) [src: https://github.com/salsa-rs/salsa]).
2. **Failing test first** (`tests/equivalence.rs` skeleton): construct an `AriadneDb`, set 3 file inputs, query `symbols_for_file(file_a)`. Then mutate `file_b`'s content, re-query `symbols_for_file(file_a)`, assert result-equal to first call AND `salsa::DebugWithDb` shows cache hit (no recomputation). Fails until step 6.
3. Define inputs with `#[salsa::input]`:
   - `FileContentInput { path: String, content: Arc<[u8]>, hash: [u8; 32] }`
   - `FileMetadataInput { lang: Lang, size: u64, mtime_ns: u64 }`
   - `ScipDocInput { path: String, raw_proto: Option<Arc<[u8]>> }`
   - `ProjectConfigInput { root: PathBuf, enabled_langs: Vec<Lang>, ignore: Vec<String> }`
4. Define derived queries with `#[salsa::tracked]`:
   - `fn parse_tree(db, file: FileContentInput) -> Arc<tree_sitter::Tree>` — calls `ariadne-parser`.
   - `fn syntactic_facts(db, file: FileContentInput) -> Arc<SyntacticFacts>` — depends on `parse_tree`.
   - `fn scip_symbols(db, scip: ScipDocInput) -> Arc<Vec<SymbolRecord>>` — calls `ariadne-scip` (tier-05; stub for now returning empty).
   - `fn symbols_for_file(db, file: FileContentInput, scip: ScipDocInput) -> Arc<Vec<SymbolRecord>>` — merges syntactic + scip; `scip` takes precedence per symbol.
   - `fn edges_for_file(...) -> Arc<Vec<EdgeRecord>>`.
   - `fn blast_radius(db, sym: SymbolId, depth: u8) -> Arc<Vec<SymbolId>>` — stub now (real algo in tier-07).
5. Durability assignments on input setters:
   - `set_content(...).with_durability(Durability::LOW)` for files under project root.
   - `Durability::MEDIUM` for files under `node_modules`/`vendor`/`target`/`.venv`.
   - `Durability::HIGH` for stdlib paths.
   Policy lives in `inputs::durability_for(path)` so it is testable in isolation.
6. Implement `AriadneDb::new(storage: ariadne_storage::Storage)` — wires inputs into the Salsa DB. Provide `seed_from_disk()` that reads redb on cold start and creates inputs.
7. Memory probe (`memory.rs`): expose `memory_report() -> BTreeMap<&'static str, u64>` aggregating `salsa::Storage::reportable_memory()` per tracked-fn (call `salsa`'s `reportable_memory_usage` API; if absent in pinned version, fall back to counter+`mem::size_of_val` walk and document the gap).
8. Property test (`tests/equivalence.rs`): generate a fixture project of 20 files; apply N random edits; after each step, compare `symbols_for_file(f)` for every file vs a fresh `AriadneDb` reloaded from disk and queried; must be identical (proptest 100 iterations).
9. Durability test (`tests/durability.rs`): seed stdlib input with HIGH durability; set a LOW-durability project file; mutate the stdlib content; assert that downstream LOW queries that did NOT depend on stdlib do not re-run (Salsa's debug counters confirm).
10. Criterion (`benches/edit.rs`): edit 1 file in a 1K-file fixture; assert `symbols_for_file(edited)` recomputes in <50ms p95, `symbols_for_file(other)` returns from cache in <50µs.
11. Wire `AriadneDb` to write back deltas via `ariadne_storage::WriteTxn` in a `commit_revision()` method called by the watcher (tier-06). For tier-04 it is exposed but not yet driven.
</steps>

<verification>
- `cargo nextest run -p ariadne-salsa` green; proptest 100 iterations stable.
- `cargo bench -p ariadne-salsa` numbers within budget; criterion baseline saved for CI gating.
- `AriadneDb::memory_report()` printed by `ariadne-cli debug mem` (stub) on a 10K-file fixture; assert no table > 256MB.
- Manual: induce a runaway derived-fn by setting durability=LOW on stdlib, observe memory grow, then revert and confirm reduction. Document in tier-04 audit report.
</verification>

<rollback>
`git rm -r crates/ariadne-salsa` + workspace member removal. No on-disk state owned (all goes via ariadne-storage).
</rollback>
