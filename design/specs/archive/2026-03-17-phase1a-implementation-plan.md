# Phase 1a: Implementation Plan

**Spec:** `design/specs/2026-03-17-phase1a-mvp.md`
**Date:** 2026-03-17
**Updated:** 2026-03-18 (D-017 through D-023 architectural decisions)

## Chunk Overview

```
Chunk 1: Scaffold + Data Model + Diagnostics (no dependencies)
Chunk 2: Traits + Registry (depends on 1)
Chunk 3: Parsers — Go, C#, Java (depends on 2)
Chunk 4: Parsers — TS/JS, Python, Rust (depends on 2)
Chunk 5: Detection + Hashing + Clustering (depends on 1)
Chunk 6: Pipeline (depends on 2, 3, 4, 5)
Chunk 7: Serialization + CLI (depends on 6)
Chunk 8: Tests (depends on all)
```

---

## Chunk 1: Scaffold + Data Model + Diagnostics

### Task 1.1: Cargo project + module scaffold
- **Files:** `Cargo.toml`, `.gitignore`, `src/main.rs`, `src/lib.rs`, empty module files
- **Source:** Spec D1
- **Key points:**
  - name = `ariadne-graph`, `[[bin]] name = "ariadne"` (D-010)
  - Deps: clap, tree-sitter + grammar crates for 6 Tier 1 languages, serde/serde_json, xxhash-rust, ignore, rayon, thiserror
  - Dev-deps: insta (yaml), tempfile
  - `main.rs`: placeholder `fn main() {}`
  - `lib.rs`: `pub mod model; pub mod parser; pub mod pipeline; pub mod detect; pub mod cluster; pub mod serial; pub mod diagnostic; pub mod hash;`
  - Empty module files for all directories per D-023:
    - `src/model/{mod,types,node,edge,graph}.rs`
    - `src/parser/{mod,traits,registry}.rs`
    - `src/pipeline/{mod,walk,read,resolve,build}.rs`
    - `src/detect/{mod,filetype,layer}.rs`
    - `src/cluster/mod.rs`
    - `src/serial/{mod,json}.rs`
    - `src/diagnostic.rs`, `src/hash.rs`
- **Commit:** `ariadne(core): create project scaffold`

### Task 1.2: Newtypes (`src/model/types.rs`)
- **Source:** Spec D2, D-017, D-024
- **Key points:**
  - `CanonicalPath(String)`: constructors enforce normalization (relative, forward slashes, no `./`, no `..`, no trailing slash). Impl `Ord`, `Hash`, `Serialize`.
  - `ContentHash(String)`: created only by hash module. Impl `Eq`, `Serialize`.
  - `ClusterId(String)`: cluster identifier. Impl `Ord`, `Serialize`.
  - `Symbol(String)`: export/import symbol name. Impl `Ord`, `Serialize`.
  - `FileSet(BTreeSet<CanonicalPath>)`: set of known files for import resolution (D-024). Lives here so `parser/` can reference it without depending on `pipeline/`.
  - All implement `Debug`, `Clone`, `PartialEq`, `Eq`.
  - `as_str() -> &str` accessor on each newtype. `into_string() -> String` for output model conversion.
- **Commit:** `ariadne(core): implement domain newtypes`

### Task 1.3: Data model (`src/model/`)
- **Source:** Spec D2, `determinism.md`
- **Key points:**
  - `src/model/node.rs`: `Node`, `FileType` enum (Copy + Ord + serde), `ArchLayer` enum (Copy + Ord + serde)
  - `src/model/edge.rs`: `Edge` (with `CanonicalPath`, `EdgeType`, `Vec<Symbol>`), `EdgeType` enum (Copy + Ord + serde)
  - `src/model/graph.rs`: `ProjectGraph { nodes: BTreeMap<CanonicalPath, Node>, edges: Vec<Edge> }`, `ClusterMap { clusters: BTreeMap<ClusterId, Cluster> }`, `Cluster` struct
  - All enums: `#[serde(rename_all = "snake_case")]`, derive `Copy, Ord`
  - No `generated` field in ProjectGraph
- **Commit:** `ariadne(core): implement data model`

### Task 1.4: Diagnostics (`src/diagnostic.rs`)
- **Source:** Spec D5, D-021, `error-handling.md`
- **Key points:**
  - `FatalError` enum via `thiserror` (E001-E005)
  - `WarningCode` enum (W001-W004, W006-W009 — W005 removed per `error-handling.md`), `Warning` struct (code, path, message, detail)
  - `DiagnosticCollector`: `Mutex<Vec<Warning>>`, `Mutex<DiagnosticCounts>`
  - `warn(&self, warning)`, `drain(self) -> DiagnosticReport` (sorted by path, code)
  - `DiagnosticCounts { files_skipped, imports_unresolved, partial_parses }`
- **Commit:** `ariadne(core): implement diagnostic collector`

### Task 1.5: Verify `cargo build` compiles

---

## Chunk 2: Traits + Registry

### Task 2.1: Parser traits (`src/parser/traits.rs`)
- **Source:** Spec D4, D-018
- **Key points:**
  - `RawImport { path: String, symbols: Vec<String>, is_type_only: bool }`
  - `RawExport { name: String, is_re_export: bool, source: Option<String> }`
  - `LanguageParser` trait (Send + Sync, 5 methods): `language`, `extensions`, `tree_sitter_language`, `extract_imports`, `extract_exports`
  - `ImportResolver` trait (Send + Sync): `resolve(import, from_file, known_files) -> Option<CanonicalPath>`
  - `FileSet` newtype (BTreeSet<CanonicalPath>) lives in `model/types.rs` — referenced by ImportResolver trait
- **Commit:** `ariadne(parser): define parser and resolver traits`

### Task 2.2: Parser registry (`src/parser/registry.rs`)
- **Source:** Spec D4
- **Key points:**
  - `ParserRegistry` stores `Vec<Box<dyn LanguageParser>>` + `HashMap<String, usize>` extension index
  - Also stores resolvers: `Vec<Box<dyn ImportResolver>>` paired with parsers
  - `register(parser, resolver)`, `parser_for(extension)`, `resolver_for(extension)`
  - `with_tier1()` — registers all 6 Tier 1 languages (called from main.rs)
  - `parse_source(content, parser) -> Option<(Tree, Vec<RawImport>, Vec<RawExport>)>`: tree-sitter parse + partial error handling (>50% ERROR → None, else extract valid)
- **Commit:** `ariadne(parser): implement parser registry`

### Task 2.3: Pipeline traits (`src/pipeline/walk.rs`, `src/pipeline/read.rs`, `src/serial/mod.rs`)
- **Source:** Spec D5, D-019
- **Key points:**
  - `FileEntry { path: PathBuf, extension: String }` — output of walk
  - `FileContent { path: CanonicalPath, bytes: Vec<u8>, hash: ContentHash, lines: u32 }` — output of read
  - `ParsedFile { path: CanonicalPath, imports: Vec<RawImport>, exports: Vec<RawExport> }` — output of parse (per `architecture.md` Intermediate Types; FileType and ArchLayer are determined later in `resolve_and_build`, not during parsing)
  - `BuildOutput { graph_path, clusters_path, file_count, edge_count, cluster_count, warnings }` — final output (per architecture.md D-024)
  - `FileWalker` trait: `walk(&self, root, config) -> Result<Vec<FileEntry>, FatalError>`
  - `FileReader` trait: `read(&self, entry) -> Result<FileContent, FileSkipReason>`
  - `GraphSerializer` trait: `write_graph(&self, output, dir)`, `write_clusters(&self, clusters, dir)`
  - `WalkConfig` struct with `Default::default()` (max_files: 50k, max_file_size: 1MB, exclude_dirs: [".ariadne"]) — per `architecture.md` D-024. Note: max_depth (64) is a hardcoded internal constant in `FsWalker`, not a `WalkConfig` field.
- **Commit:** `ariadne(pipeline): define stage traits and intermediate types`

---

## Chunk 3: Parsers — Low Complexity

### Task 3.1: Go parser + resolver (`src/parser/go.rs`)
- **Source:** Spec D7. Extensions: `.go`.
- **Key points:**
  - `LanguageParser` impl: single, grouped, aliased, dot, blank imports. Exports: empty.
  - `ImportResolver` impl: `go.mod` module path, skip std/external.
- **Commit:** `ariadne(parser): implement Go parser`

### Task 3.2: C# parser + resolver (`src/parser/csharp.rs`)
- **Source:** Spec D10. Extensions: `.cs`.
- **Key points:**
  - `LanguageParser` impl: using, using static, aliased, global. Exports: public symbols.
  - `ImportResolver` impl: namespace-to-directory heuristic.
- **Commit:** `ariadne(parser): implement C# parser`

### Task 3.3: Java parser + resolver (`src/parser/java.rs`)
- **Source:** Spec D11. Extensions: `.java`.
- **Key points:**
  - `LanguageParser` impl: class, wildcard, static, static wildcard. Exports: public symbols.
  - `ImportResolver` impl: package-to-path.
- **Commit:** `ariadne(parser): implement Java parser`

### Task 3.4: Register in `with_tier1()`
- **Commit:** (combined)

---

## Chunk 4: Parsers — High Complexity

### Task 4.1: TypeScript/JavaScript parser + resolver (`src/parser/typescript.rs`)
- **Source:** Spec D6. Extensions: `.ts`, `.tsx`, `.js`, `.jsx`, `.mjs`, `.cjs`.
- **Key points:**
  - `LanguageParser` impl: 7 import + 5 export patterns. One struct handles both TS and JS.
  - `ImportResolver` impl: relative + extension/index probing, skip `@/` and bare specifiers.
- **Commit:** `ariadne(parser): implement TypeScript/JavaScript parser`

### Task 4.2: Python parser + resolver (`src/parser/python.rs`)
- **Source:** Spec D8. Extensions: `.py`, `.pyi`.
- **Key points:**
  - `LanguageParser` impl: import, from-import, relative, TYPE_CHECKING, skip `__future__`. Exports: `__all__`.
  - `ImportResolver` impl: package dir + project root.
- **Commit:** `ariadne(parser): implement Python parser`

### Task 4.3: Rust parser + resolver (`src/parser/rust_lang.rs`)
- **Source:** Spec D9. Extensions: `.rs`.
- **Key points:**
  - `LanguageParser` impl: use crate/super/self, mod, skip extern/std. Exports: pub items, pub use.
  - `ImportResolver` impl: crate root relative.
- **Commit:** `ariadne(parser): implement Rust parser`

### Task 4.4: Register all 6 parsers in `with_tier1()`
- **Commit:** (combined)

---

## Chunk 5: Detection + Hashing + Clustering

### Task 5.1: File type detection (`src/detect/filetype.rs`, `src/detect/layer.rs`)
- **Source:** Spec D12, D13, `architecture.md` File Type Detection + Architectural Layer Heuristics
- **Key points:**
  - `detect_file_type(path) -> FileType`: implement the 6-level priority table from `architecture.md` (known config filenames → per-language test patterns → .d.ts → style → asset → default source). Filename-specific rules take precedence over extension rules.
  - `infer_arch_layer(path) -> ArchLayer`: implement the directory pattern table from `architecture.md` (api → service → data → util → component → hook → config → unknown). Covers both frontend and backend conventions.
  - Both return model enums, depend only on `model/`
- **Commit:** `ariadne(detect): implement file type and layer detection`

### Task 5.2: Content hashing (`src/hash.rs`)
- **Source:** Spec D14.
- **Key points:** `hash_content(bytes: &[u8]) -> ContentHash` — xxHash64, lowercase hex
- **Commit:** `ariadne(core): implement xxHash64 hashing`

### Task 5.3: Clustering (`src/cluster/mod.rs`)
- **Source:** Spec D17, `determinism.md`
- **Key points:**
  - `assign_clusters(graph) -> ClusterMap`: group by first meaningful directory segment
  - `compute_cluster_metrics(cluster, graph)`: internal/external edges, cohesion
  - Returns `BTreeMap<ClusterId, Cluster>`. Sorted file lists. Cohesion rounded to 4 decimal places (per `determinism.md`). Cohesion = 1.0 on zero division.
- **Commit:** `ariadne(graph): implement clustering`

---

## Chunk 6: Pipeline

### Task 6.1: FsWalker impl (`src/pipeline/walk.rs`)
- **Source:** Spec D5, `error-handling.md` Stage 1
- **Key points:**
  - `FsWalker` implements `FileWalker` trait
  - Walk via `ignore` crate (respects .gitignore)
  - Validate root: E001 (not found), E002 (not directory)
  - Always excludes `.ariadne/` directory (hardcoded, per D-024 `WalkConfig.exclude_dirs`)
  - Collect `Vec<FileEntry>` → **sort by path** (D-006)
  - Respect max_files limit from `WalkConfig`
- **Commit:** `ariadne(pipeline): implement file walker`

### Task 6.2: FsReader impl (`src/pipeline/read.rs`)
- **Source:** Spec D5, `error-handling.md` Stage 1
- **Key points:**
  - `FsReader` implements `FileReader` trait
  - Read bytes, check UTF-8, compute hash, count lines
  - Returns `FileContent` with `CanonicalPath` (normalize here)
  - Errors → `FileSkipReason` (read error, too large, encoding). Note: `BinaryFile` variant defined but not emitted in Phase 1a (null-byte check is Phase 1b)
- **Commit:** `ariadne(pipeline): implement file reader`

### Task 6.3: Build pipeline (`src/pipeline/mod.rs`, `src/pipeline/resolve.rs`, `src/pipeline/build.rs`)
- **Source:** Spec D15, D-025, `architecture.md` resolve_and_build sub-responsibilities, `determinism.md`, `performance.md`
- **Key points:**
  - `BuildPipeline::new(walker, reader, registry, serializer)` (D-019, D-020)
  - `BuildPipeline::run(&self, root, config) -> Result<BuildOutput, FatalError>`
  - Walk → read (with diagnostics) → **parallel parse** via rayon on sorted list → resolve_and_build → cluster → convert → serialize
  - `DiagnosticCollector` shared via `&` in rayon closures (D-021)
  - Parallel parse: `file_contents.par_iter().filter_map(|f| parse_single(f, &registry, &diagnostics)).collect()`
  - **resolve_and_build sub-responsibilities** (per `architecture.md`, all 9 items):
    1. Build `FileSet` from successfully-read files (not walked files — TOCTOU protection)
    2. For each `ParsedFile`, call `detect_file_type(path)` → `FileType`
    3. For each `ParsedFile`, call `infer_arch_layer(path)` → `ArchLayer`
    4. Resolve imports: call `resolver.resolve()` for each import, create edges
    5. Classify edges: `tests` (if source is test file), `re_exports` (from `RawExport.is_re_export`), `type_imports` (if `is_type_only`), else `imports`
    6. Apply naming-convention test edge inference (per `architecture.md` Edge Type Inference)
    7. Deduplicate edges: same (from, to, edge_type) → merge symbols (union, sorted)
    8. Set `arch_depth = 0` for all nodes (D-025 placeholder, computed in Phase 2)
    9. Assemble `ProjectGraph` with sorted exports and symbols
  - **Sort edges** by (from, to, edge_type). **Sort** node.exports, edge.symbols (D-006)
  - `ClusterMap → ClusterOutput` conversion happens here in `pipeline/mod.rs`, NOT in `serial/` (preserves `serial/` dependency rules)
  - Set version=1, project_root. No timestamp.
  - E004 if no parseable files after walk+read+parse
- **Commit:** `ariadne(pipeline): implement build pipeline`

---

## Chunk 7: Serialization + CLI

### Task 7.1: Output types + JSON serialization (`src/serial/mod.rs`, `src/serial/json.rs`)
- **Source:** Spec D16, D-022, `determinism.md`, `performance.md`
- **Note:** This task extends `src/serial/mod.rs` created in Task 2.3 (adds output types alongside the existing `GraphSerializer` trait definition). Does NOT replace the file.
- **Key points:**
  - `GraphOutput`, `NodeOutput`, `ClusterOutput` — output-only types with Serialize (type definitions only in `serial/`)
  - `impl From<ProjectGraph> for GraphOutput`: converts newtypes to strings, edges to compact tuples (D-012), enforces all sort points
  - **Note:** `impl From<ClusterMap> for ClusterOutput` lives in `pipeline/mod.rs` (Task 6.3), NOT here — `serial/` depends on `model/` only, and `ClusterMap` is a `model/` type so the conversion must happen in `pipeline/` to preserve the dependency rule
  - `JsonSerializer` implements `GraphSerializer` trait
  - Atomic writes (.tmp + rename), `BufWriter` + `to_writer_pretty`
  - Create output dir if missing
- **Commit:** `ariadne(serial): implement JSON serialization`

### Task 7.2: CLI + Composition Root (`src/main.rs`)
- **Source:** Spec D18, D-020
- **Key points:**
  - clap derive. `build <path> [--output <dir>]` (default: `.ariadne/graph/`). `info`.
  - **Composition Root:** create `BuildPipeline::new(Box::new(FsWalker::new()), Box::new(FsReader::new()), ParserRegistry::with_tier1(), Box::new(JsonSerializer))`
  - Build flow: `pipeline.run(root, config)` → print summary → exit code
  - Exit 0 success, exit 1 on `FatalError`
  - Summary: `"Built graph: N files, E edges, C clusters in Tms"` + skipped count if any
- **Commit:** `ariadne(cli): implement build and info commands`

---

## Chunk 8: Tests

### Task 8.1: Fixture projects (`tests/fixtures/`)
- 8 fixtures: typescript-app, go-service, python-package, rust-crate, csharp-project, java-project, mixed-project, edge-cases
- Edge-cases: empty file, syntax error file, circular imports (A↔B), deeply nested, unicode filename
- **Commit:** `ariadne(test): add fixture projects`

### Task 8.2: Test infrastructure (`tests/helpers.rs`, `tests/invariants.rs`)
- `invariants.rs`: 5 basic INV checks for Phase 1a — INV-1 (edge referential integrity), INV-2 (no self-imports), INV-8 (counts match), INV-9 (no duplicates), INV-11 (byte-identical determinism). Full 13-invariant suite deferred to Phase 1b with proptest.
- `helpers.rs`: shared utilities, `build_fixture(path) -> BuildOutput` helper
- Mock implementations: `MockWalker`, `MockReader` for pipeline unit tests
- **Commit:** `ariadne(test): add invariant checker and helpers`

### Task 8.3: L1 parser snapshot tests (`tests/parsers/*.rs`)
- ~50 snapshot tests via `insta::assert_yaml_snapshot!()` (per testing.md Coverage Matrix)
- One test per import/export pattern per language
- Path resolution snapshot tests (~20) — test `ImportResolver` impls (per testing.md)
- **Commit:** `ariadne(test): add parser snapshot tests`

### Task 8.4: Pipeline unit tests (`tests/pipeline_tests.rs`)
- Test pipeline with `MockWalker`/`MockReader` (D-019)
- Test DiagnosticCollector: warning aggregation, sorting, counts
- Test resolution logic with controlled file sets
- **Commit:** `ariadne(test): add pipeline unit tests`

### Task 8.5: L2 fixture graph tests (`tests/graph_tests.rs`)
- Build each fixture with real `FsWalker`/`FsReader`/`JsonSerializer` → snapshot graph.json + clusters.json
- Run `check_all_invariants()` on each
- Verify edge-cases: syntax error skipped, circular edges exist, empty file is node
- Determinism test: build twice → byte-identical
- **Commit:** `ariadne(test): add fixture graph tests`

---

## Dependency Graph

```
Chunk 1 ──┬── Chunk 2 ──┬── Chunk 3 ──┐
           │             └── Chunk 4 ──┤
           └── Chunk 5 ────────────────┤
                                       ▼
                                   Chunk 6 ── Chunk 7 ── Chunk 8
```

**Parallel:** Chunks 3, 4, 5 can all run in parallel. Chunk 8 can start fixtures (8.1) in parallel with Chunk 7.
