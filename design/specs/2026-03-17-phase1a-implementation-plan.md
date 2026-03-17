# Phase 1a: Implementation Plan

**Spec:** `design/specs/2026-03-17-phase1a-mvp.md`
**Date:** 2026-03-17

## Chunk Overview

```
Chunk 1: Scaffold + Data Model (no dependencies)
Chunk 2: LanguageParser Trait (depends on 1)
Chunk 3: Parsers — Go, C#, Java (depends on 2)
Chunk 4: Parsers — TS/JS, Python, Rust (depends on 2)
Chunk 5: Detection + Hashing + Clustering (depends on 1)
Chunk 6: Graph Builder (depends on 2, 3, 4, 5)
Chunk 7: Serialization + CLI (depends on 6)
Chunk 8: Tests (depends on all)
```

---

## Chunk 1: Scaffold + Data Model

### Task 1.1: Cargo project
- **Files:** `Cargo.toml`, `.gitignore`, `src/main.rs`, `src/lib.rs`
- **Source:** Spec D1
- **Key points:**
  - name = `ariadne-graph`, `[[bin]] name = "ariadne"` (D-010)
  - All deps from spec D1. Dev-deps: insta (yaml), tempfile
  - `main.rs`: placeholder `fn main() {}`
  - `lib.rs`: `pub mod graph; pub mod parser; pub mod detect; pub mod hash;`
- **Commit:** `ariadne(core): create project scaffold`

### Task 1.2: Empty module files
- **Files:** `src/graph/{mod,model,serialize,cluster}.rs`, `src/parser/mod.rs`, `src/detect/{mod,patterns}.rs`, `src/hash.rs`
- **Commit:** (combined with 1.1)

### Task 1.3: Data model
- **File:** `src/graph/model.rs`
- **Source:** Spec D2, `determinism.md`
- **Key points:**
  - All types per spec D2. **BTreeMap** for nodes and clusters
  - EdgeType derives `Ord` (for sort key)
  - `#[serde(rename_all = "snake_case")]` on all enums
  - No `generated` field in ProjectGraph
- **Commit:** `ariadne(core): implement data model`

### Task 1.4: Verify `cargo build` compiles

---

## Chunk 2: LanguageParser Trait

### Task 2.1: Trait + registry + parse helper
- **File:** `src/parser/mod.rs`
- **Source:** Spec D4
- **Key points:**
  - `Import`, `Export` structs
  - `LanguageParser` trait (Send + Sync, 6 methods)
  - `ParserRegistry` with register/lookup/new_with_defaults
  - `parse_file(content, parser) -> Option<(Vec<Import>, Vec<Export>)>`: tree-sitter parse + partial error handling (>50% ERROR nodes → None + stderr, else extract valid)
- **Commit:** `ariadne(parser): define trait and registry`

---

## Chunk 3: Parsers — Low Complexity

### Task 3.1: Go parser (`src/parser/go.rs`)
- **Source:** Spec D6. Extensions: `.go`. Imports: single, grouped, aliased, dot, blank. Exports: empty. Resolution: `go.mod` module path, skip std/external.
- **Commit:** `ariadne(parser): implement Go parser`

### Task 3.2: C# parser (`src/parser/csharp.rs`)
- **Source:** Spec D9. Extensions: `.cs`. Imports: using, using static, aliased, global. Exports: public symbols. Resolution: namespace-to-directory heuristic.
- **Commit:** `ariadne(parser): implement C# parser`

### Task 3.3: Java parser (`src/parser/java.rs`)
- **Source:** Spec D10. Extensions: `.java`. Imports: class, wildcard, static, static wildcard. Exports: public symbols. Resolution: package-to-path.
- **Commit:** `ariadne(parser): implement Java parser`

### Task 3.4: Register in `new_with_defaults()`
- **Commit:** (combined)

---

## Chunk 4: Parsers — High Complexity

### Task 4.1: TypeScript/JavaScript parser (`src/parser/typescript.rs`)
- **Source:** Spec D5. Extensions: `.ts`, `.tsx`, `.js`, `.jsx`, `.mjs`, `.cjs`. 7 import + 5 export patterns. Resolution: relative + extension/index probing, skip `@/` and bare specifiers.
- **Commit:** `ariadne(parser): implement TypeScript/JavaScript parser`

### Task 4.2: Python parser (`src/parser/python.rs`)
- **Source:** Spec D7. Extensions: `.py`, `.pyi`. Imports: import, from-import, relative, TYPE_CHECKING, skip `__future__`. Exports: `__all__`. Resolution: package dir + project root.
- **Commit:** `ariadne(parser): implement Python parser`

### Task 4.3: Rust parser (`src/parser/rust_lang.rs`)
- **Source:** Spec D8. Extensions: `.rs`. Imports: use crate/super/self, mod, skip extern/std. Exports: pub items, pub use. Resolution: crate root relative.
- **Commit:** `ariadne(parser): implement Rust parser`

### Task 4.4: Register all 6 parsers
- **Commit:** (combined)

---

## Chunk 5: Detection + Hashing + Clustering

### Task 5.1: File type detection (`src/detect/patterns.rs`, `src/detect/mod.rs`)
- **Source:** Spec D11, D12
- **Key points:** `detect_file_type()` rules in order. `infer_arch_layer()` deepest directory match.
- **Commit:** `ariadne(detect): implement file type and layer detection`

### Task 5.2: Content hashing (`src/hash.rs`)
- **Source:** Spec D13. `hash_file()` → xxHash64, lowercase hex.
- **Commit:** `ariadne(core): implement xxHash64 hashing`

### Task 5.3: Clustering (`src/graph/cluster.rs`)
- **Source:** Spec D16, `determinism.md`
- **Key points:** `assign_clusters()`, `compute_cluster_metrics()`. Sorted file lists. Cohesion = 1.0 on zero division.
- **Commit:** `ariadne(graph): implement clustering`

---

## Chunk 6: Graph Builder

### Task 6.1: Build pipeline (`src/graph/mod.rs`)
- **Source:** Spec D14, `determinism.md`, `performance.md`
- **Key points:**
  - `build_graph(project_root, registry) -> Result<(ProjectGraph, ClusterMap)>`
  - Walk via `ignore` crate → collect `Vec<PathBuf>` → **sort** (D-006)
  - Read each file: skip on read error / not UTF-8 → stderr warning
  - Parallel parse via `rayon` on sorted list (preserves order)
  - Resolve imports → create edges. Unresolved → skip silently
  - Post-process: test edges (naming convention), re-export edges (barrel files)
  - Assign clusters, compute metrics
  - **Sort edges** by (from, to, edge_type). **Sort** node.exports, edge.symbols (D-006)
  - Set version=1, project_root. No timestamp.
- **Commit:** `ariadne(graph): implement build pipeline`

---

## Chunk 7: Serialization + CLI

### Task 7.1: JSON serialization (`src/graph/serialize.rs`)
- **Source:** Spec D15, `determinism.md`, `performance.md`
- **Key points:**
  - Custom serialization: nodes as BTreeMap (sorted keys), edges as compact tuples `[from, to, type, symbols]`
  - `write_graph()`, `write_clusters()`: atomic writes (.tmp + rename), `BufWriter` + `to_writer_pretty`
  - Create output dir if missing
- **Commit:** `ariadne(graph): implement JSON serialization`

### Task 7.2: CLI (`src/main.rs`)
- **Source:** Spec D17
- **Key points:**
  - clap derive. `build <path> [--output <dir>]` (default: `.ariadne/graph/`). `info`.
  - Build flow: validate path → create registry → build_graph → write → print summary
  - Exit 0 success, exit 1 fatal (path not found, not dir, output not writable, no parseable files)
  - Summary: `"Built graph: N files, E edges, C clusters in Tms"` + skipped count if any
- **Commit:** `ariadne(cli): implement build and info commands`

---

## Chunk 8: Tests

### Task 8.1: Fixture projects (`tests/fixtures/`)
- 5 fixtures: typescript-app, go-service, python-package, mixed-project, edge-cases
- Edge-cases: empty file, syntax error file, circular imports (A↔B), deeply nested, unicode filename
- **Commit:** `ariadne(test): add fixture projects`

### Task 8.2: Test infrastructure (`tests/helpers.rs`, `tests/invariants.rs`)
- `invariants.rs`: 13 INV checks. Byte-identical determinism test.
- `helpers.rs`: shared utilities
- **Commit:** `ariadne(test): add invariant checker and helpers`

### Task 8.3: L1 parser snapshot tests (`tests/parsers/*.rs`)
- ~44 snapshot tests via `insta::assert_yaml_snapshot!()`
- One test per import/export pattern per language
- Path resolution snapshot tests (~20)
- **Commit:** `ariadne(test): add parser snapshot tests`

### Task 8.4: L2 fixture graph tests (`tests/graph_tests.rs`)
- Build each fixture → snapshot graph.json + clusters.json
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
