# Phase 1: Implementation Plan

**Spec:** `design/specs/2026-03-17-phase1-core-cli.md`
**Date:** 2026-03-17

## Chunk Overview

```
Chunk 1: Project Scaffold + Data Model (no dependencies)
Chunk 2: Tree-sitter Core + LanguageParser Trait (depends on Chunk 1)
Chunk 3: Language Parsers — Low Complexity (depends on Chunk 2)
Chunk 4: Language Parsers — High Complexity (depends on Chunk 2)
Chunk 5: Detection — File Type + Architectural Layer (depends on Chunk 1)
Chunk 6: Content Hashing + Clustering (depends on Chunk 1)
Chunk 7: Graph Builder (depends on Chunks 2, 3, 4, 5, 6)
Chunk 8: JSON Serialization (depends on Chunks 1, 7)
Chunk 9: CLI Interface (depends on Chunks 7, 8)
Chunk 10: Tests (depends on all previous chunks)
Chunk 11: GitHub Releases CI (depends on Chunk 9)
```

---

## Chunk 1: Project Scaffold + Data Model

**Goal:** Cargo project exists, compiles, core types defined.

### Task 1.1: Create Cargo project scaffold
- **Files:** `Cargo.toml`, `.gitignore`, `src/main.rs`, `src/lib.rs`
- **Source:** Spec D1
- **Key points:**
  - `Cargo.toml`: binary crate with all dependencies from spec D1 (clap, tree-sitter, grammar crates, serde, serde_json, xxhash-rust, walkdir, ignore, rayon)
  - Edition 2021, version 0.1.0, name `ariadne`
  - `main.rs`: minimal placeholder with `fn main() {}`
  - `lib.rs`: empty module declarations for `graph`, `parser`, `detect`, `hash`
  - `.gitignore`: standard Rust (`/target/`, `Cargo.lock` excluded from ignore since binary crate)
- **Commit:** `ariadne(core): create Cargo project scaffold`

### Task 1.2: Create empty module files
- **Files:** `src/graph/mod.rs`, `src/graph/model.rs`, `src/graph/serialize.rs`, `src/graph/cluster.rs`, `src/parser/mod.rs`, `src/detect/mod.rs`, `src/detect/patterns.rs`, `src/hash.rs`
- **Source:** Spec D1 file structure
- **Key points:**
  - Each file starts with module-level doc comment describing its purpose
  - `graph/mod.rs` re-exports submodules
  - `parser/mod.rs` re-exports submodules (parser files created in Chunks 3-4)
- **Commit:** (combined with Task 1.1)

### Task 1.3: Implement core data model
- **File:** `src/graph/model.rs`
- **Source:** Spec D2, `architecture.md` Graph Data Model section
- **Key points:**
  - `Node` struct: path, file_type, layer, arch_depth (default 0), lines, hash, exports, cluster
  - `Edge` struct: from, to, edge_type, symbols
  - `FileType` enum: Source, Test, Config, Style, Asset, TypeDef — derives Serialize/Deserialize with `#[serde(rename_all = "snake_case")]`
  - `EdgeType` enum: Imports, Tests, ReExports, TypeImports — same serde rename
  - `ArchLayer` enum: Api, Service, Data, Util, Component, Hook, Config, Unknown — same serde rename
  - `ProjectGraph` struct: version (u32), generated (String), project_root (String), nodes (HashMap<String, Node>), edges (Vec<Edge>)
  - `Cluster` struct: files, file_count, internal_edges, external_edges, cohesion (f64)
  - `ClusterMap` struct: clusters (HashMap<String, Cluster>)
  - All types derive Debug, Clone, Serialize, Deserialize as appropriate
- **Commit:** `ariadne(core): implement core data model types`

### Task 1.4: Verify compilation
- **Action:** `cargo build`
- **Key points:** Must compile clean with all dependencies resolved. Fix any version conflicts.

---

## Chunk 2: Tree-sitter Core + LanguageParser Trait

**Goal:** Parser infrastructure exists — trait, registry, tree-sitter initialization.

### Task 2.1: Define LanguageParser trait and supporting types
- **File:** `src/parser/mod.rs`
- **Source:** Spec D4, `architecture.md` Language Support section
- **Key points:**
  - `Import` struct: module_path, symbols, is_type_only, is_dynamic
  - `Export` struct: name, is_reexport, source (Option<String>)
  - `LanguageParser` trait: Send + Sync, 6 methods (language, extensions, tree_sitter_language, extract_imports, extract_exports, resolve_import_path)
  - `ParserRegistry` struct: register(), parser_for_extension(), supported_languages(), new_with_defaults()
  - Registry internally maps extension → parser (HashMap<String, Arc<dyn LanguageParser>>)
  - `new_with_defaults()` initially empty — parsers registered as they're implemented in Chunks 3-4
- **Commit:** `ariadne(parser): define LanguageParser trait and parser registry`

---

## Chunk 3: Language Parsers — Low Complexity

**Goal:** Go, C#, Java parsers implemented. These have the simplest import syntax.

### Task 3.1: Implement Go parser
- **File:** `src/parser/go.rs`
- **Source:** Spec D6, `architecture.md` Language Support table
- **Key points:**
  - Extensions: `.go`
  - tree-sitter-go grammar
  - Extract imports: single import, grouped import, aliased, dot, blank
  - Tree-sitter query targets: `import_declaration`, `import_spec` nodes
  - Exports: empty vec (Go uses capitalization, not extracted)
  - Path resolution: parse `go.mod` for module path, skip std lib / external, resolve internal paths against module root
- **Commit:** `ariadne(parser): implement Go language parser`

### Task 3.2: Implement C# parser
- **File:** `src/parser/csharp.rs`
- **Source:** Spec D9
- **Key points:**
  - Extensions: `.cs`
  - tree-sitter-c-sharp grammar
  - Extract imports: using, using static, aliased using, global using
  - Tree-sitter query targets: `using_directive` nodes
  - Exports: public class/interface/struct/enum → extract symbol names
  - Path resolution: namespace-to-directory heuristic mapping. Accept false negatives.
- **Commit:** `ariadne(parser): implement C# language parser`

### Task 3.3: Implement Java parser
- **File:** `src/parser/java.rs`
- **Source:** Spec D10
- **Key points:**
  - Extensions: `.java`
  - tree-sitter-java grammar
  - Extract imports: class import, wildcard, static import, static wildcard
  - Tree-sitter query targets: `import_declaration` nodes
  - Exports: public class/interface/enum/record → extract symbol names
  - Path resolution: package-to-path convention (`com.example.Foo` → `com/example/Foo.java`), try `src/main/java/` and `src/` roots
- **Commit:** `ariadne(parser): implement Java language parser`

### Task 3.4: Register parsers in registry
- **File:** `src/parser/mod.rs`
- **Source:** Spec D4
- **Key points:**
  - Add `mod go; mod csharp; mod java;` to parser/mod.rs
  - Update `new_with_defaults()` to register Go, C#, Java parsers
- **Commit:** (combined with parser commits)

---

## Chunk 4: Language Parsers — High Complexity

**Goal:** TypeScript/JavaScript, Python, Rust parsers implemented.

### Task 4.1: Implement TypeScript/JavaScript parser
- **File:** `src/parser/typescript.rs`
- **Source:** Spec D5, `architecture.md` Language Support table
- **Key points:**
  - Extensions: `.ts`, `.tsx`, `.js`, `.jsx`, `.mjs`, `.cjs`
  - Uses both tree-sitter-typescript and tree-sitter-javascript grammars (select by extension)
  - Import patterns (7): named, default, namespace, side-effect, require, dynamic import(), type-only
  - Export patterns (5): named, default, re-export, barrel re-export, declaration
  - Tree-sitter queries: `import_statement`, `import_clause`, `call_expression` (for require/dynamic), `export_statement`
  - `is_type_only`: detect `import type` syntax via tree-sitter node type
  - `is_dynamic`: detect `import()` call expression
  - Path resolution: relative paths with extension/index probing (.ts, .tsx, .js, .jsx, .mjs, .cjs, index.*). `@/` and bare specifiers → skip.
  - Re-export detection: `export { x } from './y'` and `export * from './y'` → set is_reexport=true, source=path
- **Commit:** `ariadne(parser): implement TypeScript/JavaScript language parser`

### Task 4.2: Implement Python parser
- **File:** `src/parser/python.rs`
- **Source:** Spec D7
- **Key points:**
  - Extensions: `.py`, `.pyi`
  - tree-sitter-python grammar
  - Import patterns: import, import as, from-import, from-import as, relative (., ..), `__future__` → skip
  - TYPE_CHECKING guard: detect `if TYPE_CHECKING:` block, mark enclosed imports as `is_type_only: true`
  - Tree-sitter queries: `import_statement`, `import_from_statement`, `if_statement` (for TYPE_CHECKING)
  - Exports: `__all__` list extraction if present, otherwise empty vec
  - Path resolution: relative → resolve against package dir, absolute → resolve against project root, try `module.py` and `module/__init__.py`
- **Commit:** `ariadne(parser): implement Python language parser`

### Task 4.3: Implement Rust parser
- **File:** `src/parser/rust_lang.rs`
- **Source:** Spec D8
- **Key points:**
  - Extensions: `.rs`
  - tree-sitter-rust grammar
  - Import patterns: `use crate::`, `use super::`, `use self::`, `mod submodule;`, skip `extern crate`, skip `std::`/`core::`
  - `mod submodule;` → creates edge to `submodule.rs` or `submodule/mod.rs`
  - Tree-sitter queries: `use_declaration`, `mod_item`, `extern_crate_declaration`
  - Exports: `pub fn/struct/enum/trait/type/const/static` → extract name. `pub use` → re-export
  - Path resolution: `crate::` from crate root, `super::` from parent, `self::` from current, `mod` from relative
- **Commit:** `ariadne(parser): implement Rust language parser`

### Task 4.4: Register parsers in registry
- **File:** `src/parser/mod.rs`
- **Key points:**
  - Add `mod typescript; mod python; mod rust_lang;`
  - Update `new_with_defaults()` to register all 6 parsers
- **Commit:** (combined with parser commits)

---

## Chunk 5: Detection — File Type + Architectural Layer

**Goal:** Files can be classified by type and layer.

### Task 5.1: Implement file type detection patterns
- **File:** `src/detect/patterns.rs`
- **Source:** Spec D11
- **Key points:**
  - Define pattern lists for each file type (test, config, style, asset, type_def)
  - Test patterns: directory names (`test`, `tests`, `__tests__`, `spec`), suffixes (`_test.go`, `.test.ts`, `.spec.ts`, etc.), prefixes (`test_*.py`), C# `Tests/`, Java `Test.java` suffix
  - Config patterns: extensions (.json, .yaml, .yml, .toml, .xml, .ini, .env), specific filenames (tsconfig, webpack, package.json, Cargo.toml, go.mod, etc.)
  - Style patterns: .css, .scss, .sass, .less, .styl, *.styles.ts, *.styled.ts
  - Asset patterns: image/font extensions
  - TypeDef patterns: .d.ts, .pyi, `@types/` directory

### Task 5.2: Implement detection functions
- **File:** `src/detect/mod.rs`
- **Source:** Spec D11, D12
- **Key points:**
  - `detect_file_type(path: &Path) -> FileType`: evaluate rules in order (test → config → style → asset → type_def → source)
  - `infer_arch_layer(path: &Path) -> ArchLayer`: check path segments against directory pattern table. Deepest match wins. Return Unknown if no match.
  - Layer patterns from spec D12 table: api, service, data, util, component, hook, config directories

- **Commit:** `ariadne(detect): implement file type detection and layer inference`

---

## Chunk 6: Content Hashing + Clustering

**Goal:** xxHash64 file hashing and directory-based clustering work independently.

### Task 6.1: Implement content hashing
- **File:** `src/hash.rs`
- **Source:** Spec D13
- **Key points:**
  - `hash_file(path: &Path) -> Result<String>`: read file bytes, xxHash64, return lowercase hex (16 chars)
  - Use `xxhash_rust::xxh64::xxh64()` function
  - Error propagation if file unreadable
- **Commit:** `ariadne(core): implement xxHash64 content hashing`

### Task 6.2: Implement directory-based clustering
- **File:** `src/graph/cluster.rs`
- **Source:** Spec D16
- **Key points:**
  - `assign_clusters(nodes: &mut HashMap<String, Node>)`: for each node, extract first meaningful directory segment under source root
  - Common source root prefixes to strip: `src/`, `lib/`, `app/`, `pkg/`, `internal/`, `cmd/`
  - Files directly in root → cluster "root"
  - `compute_cluster_metrics(nodes: &HashMap<String, Node>, edges: &[Edge]) -> ClusterMap`: compute per-cluster file_count, internal_edges, external_edges, cohesion
  - Cohesion = internal_edges / (internal_edges + external_edges). Zero-division: cohesion = 1.0 (isolated cluster is perfectly cohesive, per spec D16)
- **Commit:** `ariadne(graph): implement directory-based clustering`

---

## Chunk 7: Graph Builder

**Goal:** Full graph build pipeline — walk, parse, connect, cluster.

### Task 7.1: Implement graph builder
- **File:** `src/graph/mod.rs`
- **Source:** Spec D14
- **Key points:**
  - `build_graph(project_root: &Path, registry: &ParserRegistry) -> Result<(ProjectGraph, ClusterMap)>`
  - Step 1: Walk directory using `ignore` crate (respects .gitignore)
  - Step 2: Filter to files with extensions recognized by registry
  - Step 3: Parallel file processing with `rayon`:
    - For each file: detect_file_type, infer_arch_layer, count lines, hash_file, parse (extract imports + exports), create Node
  - Step 4: Edge creation — for each file's imports, call resolve_import_path → create Edge. Unresolved imports → log warning to stderr, skip
  - Step 5: Post-processing:
    - Test edge inference: for each test file, find subject by naming convention (remove test prefix/suffix) and/or by imports → create Tests edge
    - Re-export detection: for barrel files (index.ts, __init__.py) where an import is immediately re-exported → create ReExports edge
  - Step 6: assign_clusters
  - Step 7: compute_cluster_metrics
  - Set version=1, generated=ISO 8601 now, project_root
  - Graceful degradation: unparseable files logged to stderr, excluded from graph, exit code still 0
- **Commit:** `ariadne(graph): implement graph builder pipeline`

---

## Chunk 8: JSON Serialization

**Goal:** ProjectGraph and ClusterMap serialize to JSON matching `architecture.md` format.

### Task 8.1: Implement graph.json serialization
- **File:** `src/graph/serialize.rs`
- **Source:** Spec D15, `architecture.md` graph.json format
- **Key points:**
  - Custom serialization for graph.json — not plain serde derive:
    - Top-level: `version`, `generated`, `project_root`, `node_count`, `edge_count`, `nodes`, `edges`
    - Nodes: object map keyed by path, values are node data (without path field — it's the key)
    - Edges: array of compact tuples `[from, to, edge_type_str, symbols]` — NOT Edge struct directly
  - `write_graph(graph: &ProjectGraph, output_dir: &Path) -> Result<()>`: serialize to `graph.json`
  - `write_clusters(clusters: &ClusterMap, output_dir: &Path) -> Result<()>`: serialize to `clusters.json`
  - Create output directory if it doesn't exist
  - Pretty-print JSON for readability (serde_json::to_string_pretty)
- **Commit:** `ariadne(graph): implement JSON serialization for graph and clusters`

---

## Chunk 9: CLI Interface

**Goal:** `ariadne build` and `ariadne info` work end-to-end.

### Task 9.1: Implement CLI with clap
- **File:** `src/main.rs`
- **Source:** Spec D17
- **Key points:**
  - Use clap derive API
  - Subcommand `build`: positional arg `project_root`, optional `--output <dir>` (default: `{project_root}/.ariadne/graph/`)
  - Subcommand `info`: no args
  - `build` flow: create ParserRegistry::new_with_defaults(), call build_graph(), write_graph(), write_clusters(), print summary to stdout
  - `info` flow: print version (from Cargo.toml via env!("CARGO_PKG_VERSION")), list supported languages from registry
  - Exit codes: 0 success, 1 fatal error (process::exit)
  - Errors: use `anyhow` for error handling (per spec D1 dependencies)
- **Commit:** `ariadne(cli): implement CLI interface (build + info commands)`

### Task 9.2: Add README.md
- **File:** `README.md`
- **Source:** Spec D1
- **Key points:**
  - What it is (structural dependency graph builder)
  - Installation (cargo install, prebuilt binaries)
  - Usage examples (build, info)
  - Supported languages table
  - Output format summary
- **Commit:** (combined with Task 9.1)

---

## Chunk 10: Tests

**Goal:** Parser unit tests, integration test, performance benchmark.

### Task 10.1: Create test fixtures
- **Files:** `tests/fixtures/typescript/`, `go/`, `python/`, `rust_project/`, `csharp/`, `java/`, `mixed/`
- **Source:** Spec D19
- **Key points:**
  - Each language directory: 2-4 small files with known import relationships
  - TypeScript fixtures: files with all 7 import patterns, barrel index.ts, type-only imports
  - Go fixtures: files with single/grouped imports, internal package reference
  - Python fixtures: files with import/from-import, relative import, TYPE_CHECKING block
  - Rust fixtures: files with use crate/super, mod declaration, pub use re-export
  - C# fixtures: files with using/using static, namespace-based structure
  - Java fixtures: files with import/static import, package structure
  - Mixed: one file per language importing from each other where possible (cross-language project)
- **Commit:** `ariadne(test): add test fixtures for all Tier 1 languages`

### Task 10.2: Implement parser unit tests
- **File:** `tests/parser_tests.rs`
- **Source:** Spec D19a
- **Key points:**
  - Per-language test functions using inline source strings
  - Each test: create source, init tree-sitter parser, parse, call extract_imports/extract_exports, assert
  - TypeScript: 7 import patterns + 5 export patterns + re-exports
  - Go: 5 import patterns
  - Python: 6 import patterns + TYPE_CHECKING + __all__ export
  - Rust: 5 import patterns + pub exports + pub use
  - C#: 4 import patterns + public exports
  - Java: 4 import patterns + public exports
- **Commit:** `ariadne(test): add parser unit tests for all Tier 1 languages`

### Task 10.3: Implement integration test
- **File:** `tests/integration_test.rs`
- **Source:** Spec D19b
- **Key points:**
  - Build graph on `tests/fixtures/mixed/`
  - Assert: correct number of files discovered
  - Assert: specific expected edges exist between fixture files
  - Assert: file types correct (test files detected, config files detected)
  - Assert: layers assigned reasonably
  - Assert: clusters match directory structure
  - Assert: graph.json and clusters.json written and valid JSON
  - Deserialize output files and verify schema
- **Commit:** `ariadne(test): add integration test for mixed-language project`

### Task 10.4: Implement performance benchmark
- **File:** `tests/bench_test.rs`
- **Source:** Spec D19c
- **Key points:**
  - `#[test] #[ignore]` attribute — run with `cargo test -- --ignored`
  - Generate synthetic project: 1000+ TypeScript files in temp directory with simple import chains
  - Build graph, measure elapsed time, assert under 3 seconds
  - Cleanup temp directory
- **Commit:** `ariadne(test): add performance benchmark test`

---

## Chunk 11: GitHub Releases CI

**Goal:** Automated cross-compilation and release publishing.

### Task 11.1: Create release workflow
- **File:** `.github/workflows/release.yml`
- **Source:** Spec D18
- **Key points:**
  - Trigger: `on: push: tags: ['v*']`
  - Matrix strategy: 5 targets (linux-x64, linux-arm64, macos-x64, macos-arm64, windows-x64)
  - Per-target steps: checkout, install Rust toolchain + target, cargo build --release --target, rename binary, upload release asset
  - Binary naming: `ariadne-{os}-{arch}[.exe]`
  - Use `actions/upload-artifact` + `softprops/action-gh-release` (or similar)
- **Commit:** `ariadne(ci): add GitHub Actions release workflow`

### Task 11.2: Create CI test workflow
- **File:** `.github/workflows/ci.yml`
- **Source:** Spec D18 ("cargo test runs on every push/PR")
- **Key points:**
  - Trigger: push + pull_request
  - Steps: checkout, install Rust, cargo test, cargo clippy, cargo fmt --check
- **Commit:** (combined with Task 11.1)

---

## Dependency Graph

```
Chunk 1 ──┬── Chunk 2 ──┬── Chunk 3 ──┐
           │             └── Chunk 4 ──┤
           ├── Chunk 5 ────────────────┤
           └── Chunk 6 ────────────────┤
                                       ▼
                                   Chunk 7 ── Chunk 8 ── Chunk 9 ── Chunk 10 ── Chunk 11
```

**Parallel opportunities:**
- Chunks 3 and 4 can be developed in parallel (independent parser implementations)
- Chunks 5 and 6 can be developed in parallel with Chunks 3-4 (no parser dependency)
- Chunks 3, 4, 5, 6 all only depend on Chunks 1-2
- Chunk 11 (CI) can be started after Chunk 9 but before Chunk 10 tests are finalized
