# Phase 1: Implementation Plan (v2)

**Spec:** `design/specs/2026-03-17-phase1-core-cli.md`
**Design docs:** `architecture.md`, `determinism.md`, `path-resolution.md`, `error-handling.md`, `performance.md`, `testing.md`, `distribution.md`
**Date:** 2026-03-17

## Chunk Overview

```
Chunk 1:  Project Scaffold + Data Model + Core Utils (no dependencies)
Chunk 2:  Tree-sitter Core + LanguageParser Trait (depends on Chunk 1)
Chunk 3:  Language Parsers — Low Complexity (depends on Chunk 2)
Chunk 4:  Language Parsers — High Complexity (depends on Chunk 2)
Chunk 5:  Detection — File Type + Architectural Layer (depends on Chunk 1)
Chunk 6:  Content Hashing + Clustering (depends on Chunk 1)
Chunk 7:  Workspace Detection (depends on Chunk 1)
Chunk 8:  Graph Builder (depends on Chunks 2, 3, 4, 5, 6, 7)
Chunk 9:  JSON Serialization (depends on Chunks 1, 8)
Chunk 10: CLI Interface (depends on Chunks 8, 9)
Chunk 11: Tests (depends on all previous chunks)
Chunk 12: GitHub Releases CI (depends on Chunk 10)
```

---

## Chunk 1: Project Scaffold + Data Model + Core Utils

**Goal:** Cargo project compiles, core types defined, warning system and path utilities exist.

### Task 1.1: Create Cargo project scaffold
- **Files:** `Cargo.toml`, `.gitignore`, `src/main.rs`, `src/lib.rs`
- **Source:** Spec D1, `distribution.md`
- **Key points:**
  - `Cargo.toml`: binary crate, name `ariadne`, version 0.1.0, edition 2021
  - License: `license = "MIT OR Apache-2.0"` (D-009)
  - Dependencies: clap, tree-sitter, all 7 grammar crates, serde, serde_json, xxhash-rust, walkdir, ignore, rayon, anyhow, dunce
  - Dev-dependencies: insta (yaml feature), proptest, criterion (html_reports feature), tempfile
  - `[[bench]]` entries for criterion benchmarks (name = "build_bench", name = "parser_bench")
  - `main.rs`: placeholder `fn main() {}`
  - `lib.rs`: module declarations for `graph`, `parser`, `detect`, `hash`, `warnings`, `resolve`
  - `.gitignore`: `/target/`
- **Commit:** `ariadne(core): create Cargo project scaffold`

### Task 1.2: Create empty module files
- **Files:** `src/graph/mod.rs`, `src/graph/model.rs`, `src/graph/serialize.rs`, `src/graph/cluster.rs`, `src/parser/mod.rs`, `src/detect/mod.rs`, `src/detect/patterns.rs`, `src/hash.rs`, `src/warnings.rs`, `src/resolve.rs`
- **Source:** Spec D1
- **Key points:**
  - Each file: module-level doc comment
  - `graph/mod.rs`, `parser/mod.rs`: re-export submodules
  - `warnings.rs` and `resolve.rs` are NEW modules not in the original plan
- **Commit:** (combined with Task 1.1)

### Task 1.3: Implement core data model
- **File:** `src/graph/model.rs`
- **Source:** Spec D2, `determinism.md`
- **Key points:**
  - `Node` struct: path, file_type, layer, arch_depth (default 0), lines, hash, exports (Vec<String>), cluster
  - `Edge` struct: from, to, edge_type, symbols (Vec<String>). Derive `Ord`/`PartialOrd` for sorting (D-006)
  - `FileType` enum: Source, Test, Config, Style, Asset, TypeDef — `#[serde(rename_all = "snake_case")]`
  - `EdgeType` enum: Imports, Tests, ReExports, TypeImports — same rename. Derive `Ord` for sort key
  - `ArchLayer` enum: Api, Service, Data, Util, Component, Hook, Config, Unknown — same rename
  - `ProjectGraph` struct: version (u32), project_root (String), nodes (`BTreeMap<String, Node>`), edges (Vec<Edge>). **NO `generated` field by default** (D-006)
  - `Cluster` struct: files (Vec<String>), file_count, internal_edges, external_edges, cohesion (f64)
  - `ClusterMap` struct: clusters (`BTreeMap<String, Cluster>`)
  - All types derive Debug, Clone, Serialize, Deserialize
- **Commit:** `ariadne(core): implement core data model types`

### Task 1.4: Implement warning system
- **File:** `src/warnings.rs`
- **Source:** `error-handling.md`
- **Key points:**
  - `WarningCode` enum: W001..W009 with Display impl
  - `Warning` struct: code, file (Option<String>), message, detail (Option<String>)
  - `WarningCollector`: collects warnings during build, thread-safe (Arc<Mutex<Vec<Warning>>>)
  - `emit_warnings(collector, format: WarningFormat)`: print to stderr in human or JSON format
  - `WarningFormat` enum: Human, Json
  - `BuildSummary` struct: files_built, edges_created, clusters, elapsed, files_skipped (with reasons), imports_unresolved
  - `print_summary(summary)`: stdout one-liner + optional skip/unresolved counts
- **Commit:** `ariadne(core): implement structured warning system`

### Task 1.5: Implement path normalization
- **File:** `src/resolve.rs`
- **Source:** `path-resolution.md`
- **Key points:**
  - `normalize_path(path: &Path, project_root: &Path) -> String`: canonical relative format (forward slashes, no `./`, no `..`, no trailing slash)
  - `is_case_insensitive(root: &Path) -> bool`: probe filesystem with temp file, cache result
  - `validate_within_root(resolved: &Path, project_root: &Path) -> bool`: path traversal protection — resolved must start_with project_root
  - Uses `dunce::canonicalize` on Windows
  - All functions are pure/testable — no side effects beyond the FS probe
- **Commit:** `ariadne(core): implement path normalization and case sensitivity detection`

### Task 1.6: Verify compilation
- **Action:** `cargo build`
- **Key points:** All dependencies resolve, all modules compile

---

## Chunk 2: Tree-sitter Core + LanguageParser Trait

**Goal:** Parser infrastructure — trait, registry, tree-sitter initialization with partial parse support.

### Task 2.1: Define LanguageParser trait and supporting types
- **File:** `src/parser/mod.rs`
- **Source:** Spec D4, `architecture.md`
- **Key points:**
  - `Import` struct: module_path, symbols, is_type_only, is_dynamic
  - `Export` struct: name, is_reexport, source (Option<String>)
  - `LanguageParser` trait: Send + Sync, 6 methods (language, extensions, tree_sitter_language, extract_imports, extract_exports, resolve_import_path)
  - `ParserRegistry`: register(), parser_for_extension(), supported_languages(), new_with_defaults()
  - `parse_file(content: &[u8], parser: &dyn LanguageParser, collector: &WarningCollector) -> Option<(Vec<Import>, Vec<Export>)>`: shared parse logic with partial parse handling:
    - Parse with tree-sitter
    - Count ERROR nodes in top-level children
    - If >50% ERROR → W001, return None
    - If any ERROR but ≤50% → W007, extract from valid subtrees
    - If no ERROR → extract all
  - Source: `error-handling.md` Stage 2
- **Commit:** `ariadne(parser): define LanguageParser trait and parser registry`

---

## Chunk 3: Language Parsers — Low Complexity

**Goal:** Go, C#, Java parsers. Simplest import syntax.

### Task 3.1: Implement Go parser
- **File:** `src/parser/go.rs`
- **Source:** Spec D6
- **Key points:**
  - Extensions: `.go`
  - Imports: single, grouped, aliased, dot, blank
  - Exports: empty vec (Go capitalization convention)
  - Path resolution: parse `go.mod` for module path (W008 fallback if missing/broken), skip std lib / external, resolve internal against module root
  - Use `normalize_path()` from `resolve.rs` for resolved paths
- **Commit:** `ariadne(parser): implement Go language parser`

### Task 3.2: Implement C# parser
- **File:** `src/parser/csharp.rs`
- **Source:** Spec D9
- **Key points:**
  - Extensions: `.cs`
  - Imports: using, using static, aliased using, global using
  - Exports: public class/interface/struct/enum symbol names
  - Path resolution: namespace-to-directory heuristic. Accept false negatives
- **Commit:** `ariadne(parser): implement C# language parser`

### Task 3.3: Implement Java parser
- **File:** `src/parser/java.rs`
- **Source:** Spec D10
- **Key points:**
  - Extensions: `.java`
  - Imports: class, wildcard, static, static wildcard
  - Exports: public class/interface/enum/record symbol names
  - Path resolution: package-to-path (`com.example.Foo` → `com/example/Foo.java`), try `src/main/java/` and `src/`
- **Commit:** `ariadne(parser): implement Java language parser`

### Task 3.4: Register parsers
- **File:** `src/parser/mod.rs`
- **Key points:** Add `mod go; mod csharp; mod java;`, update `new_with_defaults()`
- **Commit:** (combined)

---

## Chunk 4: Language Parsers — High Complexity

**Goal:** TypeScript/JavaScript, Python, Rust parsers.

### Task 4.1: Implement TypeScript/JavaScript parser
- **File:** `src/parser/typescript.rs`
- **Source:** Spec D5, `path-resolution.md`
- **Key points:**
  - Extensions: `.ts`, `.tsx`, `.js`, `.jsx`, `.mjs`, `.cjs`
  - 7 import patterns + 5 export patterns (see spec D5 for full list)
  - Path resolution: relative with extension/index probing. `@/` and bare specifiers → skip
  - **Workspace-aware resolution**: if `WorkspaceInfo` is provided, check workspace member map before classifying as external (D-008). Resolve `@scope/name` workspace packages to member entry points
  - Use `normalize_path()` and `validate_within_root()` from `resolve.rs`
- **Commit:** `ariadne(parser): implement TypeScript/JavaScript language parser`

### Task 4.2: Implement Python parser
- **File:** `src/parser/python.rs`
- **Source:** Spec D7
- **Key points:**
  - Extensions: `.py`, `.pyi`
  - Imports: import, import-as, from-import, from-import-as, relative, `__future__` skip, TYPE_CHECKING guard
  - Exports: `__all__` extraction if present
  - Path resolution: relative against package dir, absolute against project root, try `module.py` / `module/__init__.py`
- **Commit:** `ariadne(parser): implement Python language parser`

### Task 4.3: Implement Rust parser
- **File:** `src/parser/rust_lang.rs`
- **Source:** Spec D8
- **Key points:**
  - Extensions: `.rs`
  - Imports: `use crate::`, `use super::`, `use self::`, `mod submodule;`, skip extern crate/std/core
  - Exports: pub items, pub use re-export
  - Path resolution: crate root, super/self relative, mod → file mapping
- **Commit:** `ariadne(parser): implement Rust language parser`

### Task 4.4: Register parsers
- **File:** `src/parser/mod.rs`
- **Key points:** Add `mod typescript; mod python; mod rust_lang;`, update `new_with_defaults()` for all 6
- **Commit:** (combined)

---

## Chunk 5: Detection — File Type + Architectural Layer

**Goal:** Files classified by type and layer.

### Task 5.1: Implement detection patterns
- **File:** `src/detect/patterns.rs`
- **Source:** Spec D11

### Task 5.2: Implement detection functions
- **File:** `src/detect/mod.rs`
- **Source:** Spec D11, D12
- **Key points:**
  - `detect_file_type(path: &Path) -> FileType`: rules in order (test → config → style → asset → type_def → source)
  - `infer_arch_layer(path: &Path) -> ArchLayer`: deepest directory match
- **Commit:** `ariadne(detect): implement file type detection and layer inference`

---

## Chunk 6: Content Hashing + Clustering

**Goal:** xxHash64 hashing and directory-based clustering.

### Task 6.1: Implement content hashing
- **File:** `src/hash.rs`
- **Source:** Spec D13
- **Key points:** `hash_file(path: &Path) -> Result<String>` — xxHash64, lowercase hex (16 chars)
- **Commit:** `ariadne(core): implement xxHash64 content hashing`

### Task 6.2: Implement directory-based clustering
- **File:** `src/graph/cluster.rs`
- **Source:** Spec D16, `determinism.md`
- **Key points:**
  - `assign_clusters(nodes: &mut BTreeMap<String, Node>)`: first meaningful directory segment
  - `compute_cluster_metrics(nodes, edges) -> ClusterMap`: BTreeMap, cohesion = 1.0 on zero division
  - **Sort `cluster.files` lexicographically** before returning (D-006)
- **Commit:** `ariadne(graph): implement directory-based clustering`

---

## Chunk 7: Workspace Detection

**Goal:** Detect monorepo workspace configuration and build member map.

### Task 7.1: Implement workspace detection
- **File:** `src/resolve.rs` (extend existing file)
- **Source:** `path-resolution.md` Monorepo section, D-008
- **Key points:**
  - `WorkspaceKind` enum: Npm, Pnpm, Yarn, Cargo, Go, Nx, Turbo
  - `WorkspaceMember` struct: name, path, entry_point
  - `WorkspaceInfo` struct: kind, members vec
  - `detect_workspace(project_root: &Path) -> Option<WorkspaceInfo>`: scan for indicators:
    - `package.json` with `"workspaces"` → parse glob patterns → resolve member dirs → read each member's `package.json` for name + main/module
    - `pnpm-workspace.yaml` → similar
    - Other workspace types: detect but don't parse (log W008 if detected but not yet supported)
  - Phase 1 scope: npm/yarn/pnpm workspaces only. Others detected + warned
  - W008 fallback if workspace config can't be parsed
- **Commit:** `ariadne(core): implement workspace detection for npm/yarn/pnpm`

---

## Chunk 8: Graph Builder

**Goal:** Full pipeline — walk, validate, parse, resolve, connect, cluster, sort.

### Task 8.1: Implement graph builder
- **File:** `src/graph/mod.rs`
- **Source:** Spec D14, `error-handling.md`, `determinism.md`, `performance.md`, `path-resolution.md`
- **Key points:**
  - `BuildOptions` struct: max_file_size (default 1MB), max_files (default 50000), verbose (bool), timestamp (bool)
  - `build_graph(project_root, registry, options) -> Result<(ProjectGraph, ClusterMap, WarningCollector, BuildSummary)>`
  - **Pipeline stages with timing** (per `performance.md` --verbose output):
    1. **[walk]** Walk directory via `ignore` crate → collect into `Vec<PathBuf>` → **sort by path** (D-006)
    2. **[read+validate]** For each file (sequential pre-filter):
       - Check max_files limit → warn and stop if exceeded
       - Read bytes → W002 on failure
       - Check size > max_file_size → W003
       - Check null bytes in first 8KB → W004 (binary)
       - Check UTF-8 validity → W009
       - `normalize_path()` for canonical node key
    3. **[parse]** Parallel via `rayon` on sorted file list (preserves order):
       - `parse_file()` from Chunk 2 (handles W001/W007 partial parse)
       - extract imports + exports
    4. **[resolve]** For each file's imports:
       - Classify: RELATIVE / WORKSPACE / STDLIB / EXTERNAL
       - Resolve per `path-resolution.md` pipeline
       - `validate_within_root()` for path traversal protection
       - Case-insensitive fallback if `is_case_insensitive()` (cached)
       - Create Edge. Unresolved → W006 (verbose only), increment counter
    5. **[post-process]**
       - Infer test edges (naming convention + imports)
       - Detect re-export edges (barrel files)
    6. **[cluster]** assign_clusters + compute_cluster_metrics
    7. **[sort]** Before returning (D-006):
       - Sort `edges` by (from, to, edge_type)
       - Sort each `node.exports`
       - Sort each `edge.symbols`
       - (cluster.files already sorted in Chunk 6)
  - Set version=1, project_root. `generated` field only if options.timestamp=true
  - Record per-stage elapsed times in BuildSummary
- **Commit:** `ariadne(graph): implement graph builder pipeline`

---

## Chunk 9: JSON Serialization

**Goal:** Deterministic, atomic JSON output.

### Task 9.1: Implement serialization
- **File:** `src/graph/serialize.rs`
- **Source:** Spec D15, `determinism.md`, `error-handling.md`
- **Key points:**
  - Custom serialization for graph.json (not plain serde derive):
    - Top-level: `version`, `project_root`, `node_count`, `edge_count`, `nodes`, `edges`
    - Optional `generated` field (only if --timestamp)
    - Nodes: BTreeMap → stable key order in JSON
    - Edges: already sorted → stable array order
    - Compact tuple format for edges: `[from, to, edge_type_str, symbols]`
  - `write_graph(graph, output_dir, timestamp) -> Result<()>`
  - `write_clusters(clusters, output_dir) -> Result<()>`
  - **Atomic writes**: write to `.tmp` file, then `fs::rename` (per `error-handling.md`)
  - **Buffered writing**: `BufWriter<File>` + `serde_json::to_writer_pretty` — O(1) serialization memory (per `performance.md`)
  - Create output directory if missing. E003 on failure
- **Commit:** `ariadne(graph): implement deterministic atomic JSON serialization`

---

## Chunk 10: CLI Interface

**Goal:** `ariadne build` with all flags and `ariadne info` work end-to-end.

### Task 10.1: Implement CLI with clap
- **File:** `src/main.rs`
- **Source:** Spec D17, `error-handling.md`
- **Key points:**
  - clap derive API
  - Subcommand `build`:
    - Positional: `project_root`
    - `--output <dir>` (default: `{project_root}/.ariadne/graph/`)
    - `--max-file-size <bytes>` (default: 1048576)
    - `--max-files <count>` (default: 50000)
    - `--verbose` (show all warnings + per-stage timing)
    - `--warnings <format>` (human|json, default: human)
    - `--strict` (exit 1 on any warning)
    - `--timestamp` (include `generated` field in output, D-006)
  - Subcommand `info`: version + supported languages
  - Build flow: validate project_root (E001/E002) → create registry → detect workspace → build_graph → write output → emit warnings → print summary
  - `--verbose`: print per-stage timing from BuildSummary
  - `--strict`: if collector has any warnings → exit 1
  - Exit codes: 0 success, 1 fatal or strict-mode warnings
  - Error handling: `anyhow` for fatal errors, pretty-print to stderr
- **Commit:** `ariadne(cli): implement CLI with all flags`

### Task 10.2: Add README.md
- **File:** `README.md`
- **Source:** Spec D1, `distribution.md`
- **Key points:**
  - What it is, installation (cargo install, install.sh, prebuilt), usage examples, supported languages, output format, license badge
- **Commit:** (combined with Task 10.1)

---

## Chunk 11: Tests

**Goal:** 4-level test suite per `testing.md`.

### Task 11.1: Create fixture projects
- **Files:** `tests/fixtures/typescript-app/`, `go-service/`, `python-package/`, `mixed-project/`, `edge-cases/`, `workspace-project/`
- **Source:** Spec D19b, `testing.md`, `path-resolution.md`, `error-handling.md`
- **Key points:**
  - `typescript-app/`: ~10 files — barrel index.ts, api/services/utils layers, type-only imports, test file, config files
  - `go-service/`: ~8 files — go.mod, internal/handler, pkg/utils
  - `python-package/`: ~8 files — __init__.py barrel, relative imports, TYPE_CHECKING
  - `mixed-project/`: ~6 files — Go backend, TS frontend, Python scripts
  - `edge-cases/`: empty file, syntax error, circular imports, deep nesting, unicode filename, **binary file (with .ts ext), partial-error file (valid imports + broken syntax), non-utf8 file**
  - `workspace-project/` **(NEW)**: npm workspace with 3 packages (@myapp/auth, @myapp/api, @myapp/shared), cross-package imports
- **Commit:** `ariadne(test): add fixture projects`

### Task 11.2: Create test helpers and invariant checker
- **Files:** `tests/helpers.rs`, `tests/invariants.rs`
- **Source:** `testing.md` L3, `determinism.md`
- **Key points:**
  - `helpers.rs`: `generate_synthetic_project()` for benchmarks
  - `invariants.rs`: `check_all_invariants(graph, clusters) -> Result<()>`
  - 13 invariants (INV-1 through INV-13) per `testing.md`
  - **INV-11 updated**: byte-identical determinism check, NOT set comparison (per `determinism.md`)
- **Commit:** `ariadne(test): add test helpers and invariant checker`

### Task 11.3: Implement L1 parser snapshot tests
- **Files:** `tests/parsers/mod.rs`, `test_typescript.rs`, `test_go.rs`, `test_python.rs`, `test_rust.rs`, `test_csharp.rs`, `test_java.rs`
- **Source:** Spec D19a, `testing.md` L1
- **Key points:**
  - ~44 parser snapshot tests + ~20 path resolution snapshot tests using `insta::assert_yaml_snapshot!()`
  - **Path normalization tests** (NEW): 7 cases from `path-resolution.md` (leading `./`, `..`, `.`, double slashes, backslashes, trailing slashes)
  - **Case sensitivity tests** (NEW, conditional): 3 cases from `path-resolution.md`
  - **Workspace resolution tests** (NEW): 3 cases — workspace package import, subpath, non-workspace scoped package
  - **Path traversal tests** (NEW): 2 cases — `../../../etc/passwd`, sibling project
- **Commit:** `ariadne(test): add L1 parser and resolution snapshot tests`

### Task 11.4: Implement L2 fixture graph tests
- **File:** `tests/graph_tests.rs`
- **Source:** Spec D19b, `testing.md` L2
- **Key points:**
  - Snapshot full graph.json + clusters.json per fixture via insta
  - Call `check_all_invariants()` after each build
  - Edge-cases: verify binary file skipped (W004), syntax error skipped (W001), partial-error extracts valid imports (W007), non-utf8 skipped (W009)
  - Workspace-project: verify cross-package imports produce edges
- **Commit:** `ariadne(test): add L2 fixture graph snapshot tests`

### Task 11.5: Implement L3 property-based and determinism tests
- **File:** `tests/graph_tests.rs` (continued)
- **Source:** `testing.md` L3, `determinism.md`
- **Key points:**
  - `proptest!`: random valid TS files → build → check_all_invariants()
  - **Determinism test**: build same fixture twice → `assert_eq!(output1, output2)` — **byte-identical** (NOT set comparison)
  - Hash determinism: hash same file twice → assert equal
- **Commit:** `ariadne(test): add L3 property-based and determinism tests`

### Task 11.6: Implement L4 performance benchmarks
- **Files:** `benches/build_bench.rs`, `benches/parser_bench.rs`, `benches/helpers.rs`
- **Source:** Spec D19d, `testing.md` L4, `performance.md`
- **Key points:**
  - Build benchmarks: small (100 files, <200ms), medium (1000, <3s), large (3000, <10s)
  - Parser benchmarks: per-language (single file with many imports)
  - **Hash benchmark** (NEW): xxHash64 1MB file, <1ms
  - **Clustering benchmark** (NEW): 3000 nodes, <100ms
  - **Serialization benchmark** (NEW): 3000-node graph, <500ms
  - All using `criterion` with statistical analysis
- **Commit:** `ariadne(test): add L4 criterion performance benchmarks`

---

## Chunk 12: GitHub Releases CI

**Goal:** Automated CI and cross-compilation releases.

### Task 12.1: Create CI test workflow
- **File:** `.github/workflows/ci.yml`
- **Source:** Spec D18, `distribution.md`
- **Key points:**
  - Trigger: push + pull_request
  - Steps: checkout, install Rust, `cargo test`, `cargo insta test --check`, `cargo clippy -- -D warnings`, `cargo fmt --check`
- **Commit:** `ariadne(ci): add CI workflow`

### Task 12.2: Create release workflow
- **File:** `.github/workflows/release.yml`
- **Source:** Spec D18, `distribution.md`
- **Key points:**
  - Trigger: tag push `v*`
  - Matrix: 5 targets (linux-x64, linux-arm64, macos-x64, macos-arm64, windows-x64)
  - Steps: checkout, toolchain + target, `cargo build --release --target`, strip, rename, SHA-256 checksum, upload release asset
- **Commit:** `ariadne(ci): add release workflow`

### Task 12.3: Create install script
- **File:** `install.sh`
- **Source:** `distribution.md`
- **Key points:**
  - Detect OS (`uname -s`) and arch (`uname -m`)
  - Download correct binary from latest GitHub Release
  - Verify SHA-256 checksum
  - Install to `/usr/local/bin/` or `~/.local/bin/`
  - `--version <tag>` flag for specific version
- **Commit:** (combined with Task 12.2)

---

## Dependency Graph

```
Chunk 1 ──┬── Chunk 2 ──┬── Chunk 3 ──┐
           │             └── Chunk 4 ──┤
           ├── Chunk 5 ────────────────┤
           ├── Chunk 6 ────────────────┤
           └── Chunk 7 ────────────────┤
                                       ▼
                                   Chunk 8 ── Chunk 9 ── Chunk 10 ── Chunk 11 ── Chunk 12
```

**Parallel opportunities:**
- Chunks 3 and 4 can run in parallel (independent parser implementations)
- Chunks 5, 6, 7 can run in parallel with each other and with Chunks 3-4
- All of Chunks 3-7 only depend on Chunk 1 (except 3-4 also need Chunk 2)
- Chunk 12 (CI) can start after Chunk 10, in parallel with Chunk 11 (test files)
