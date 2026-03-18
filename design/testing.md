# Testing Strategy

## Principles

1. **Any behavioral change must break a test.** If a parser stops extracting an import pattern, a test must fail — even if the binary doesn't crash.
2. **Snapshots over assertions.** Snapshot testing catches unexpected changes. Hand-written assertions only catch what you thought to check.
3. **Every import pattern has a dedicated test.** No pattern is "too simple to test." The test is also documentation of what the parser handles.
4. **Performance is a correctness property.** A 3x slowdown is a bug, not an optimization opportunity.
5. **Tests must scale with the system.** Adding a new language = adding fixtures + snapshots, not rewriting test infrastructure.

## Test Levels

### L1: Parser Snapshot Tests

**Purpose:** Verify that each language parser extracts the correct `RawImport` and `RawExport` structs from known source patterns.

**Technique:** Snapshot testing via `insta` crate. Each test:
1. Provides a source code string with a specific import/export pattern
2. Calls `extract_imports()` or `extract_exports()`
3. Compares the result against a committed `.snap` file

**Why snapshots, not assertions:**
- Assertions check what you expect. Snapshots catch what you didn't expect.
- If a tree-sitter grammar update changes how `import type` is parsed, a snapshot test fails immediately. An assertion like `assert_eq!(imports.len(), 3)` might still pass if a different import was added by mistake.
- Snapshots are self-documenting — the `.snap` file shows exactly what the parser produces.

**Structure:**
```
tests/
├── snapshots/                    # insta snapshot files (committed)
│   ├── parser_typescript__named_import.snap
│   ├── parser_typescript__default_import.snap
│   ├── parser_typescript__dynamic_import.snap
│   ├── parser_typescript__type_only_import.snap
│   ├── parser_typescript__require.snap
│   ├── parser_typescript__barrel_reexport.snap
│   ├── parser_typescript__named_exports.snap
│   ├── parser_go__single_import.snap
│   ├── parser_go__grouped_import.snap
│   ├── ...
│   └── parser_java__static_wildcard_import.snap
└── parsers/
    ├── mod.rs                    # shared test utilities
    ├── test_typescript.rs        # TS/JS parser tests
    ├── test_go.rs                # Go parser tests
    ├── test_python.rs            # Python parser tests
    ├── test_rust.rs              # Rust parser tests
    ├── test_csharp.rs            # C# parser tests
    └── test_java.rs              # Java parser tests
```

**Coverage matrix — every cell must have a snapshot test:**

| Language | Patterns to test |
|----------|-----------------|
| TypeScript/JS | named import, default import, namespace import, side-effect import, require, dynamic import, type-only import, named export, default export, re-export, barrel re-export, declaration export |
| Go | single import, grouped import, aliased import, dot import, blank import |
| Python | import, import-as, from-import, from-import-as, relative import (.), relative import (..), `__future__` skip, TYPE_CHECKING guard, `__all__` export |
| Rust | use crate::, use super::, use self::, mod declaration, extern crate skip, std:: skip, pub items, pub use re-export |
| C# | using, using static, aliased using, global using, public class/interface export |
| Java | class import, wildcard import, static import, static wildcard import, public class/interface/enum/record export |

**Total: ~50 snapshot tests** for L1.

**Path resolution tests** — separate snapshot tests per language:
- Relative path resolution (./foo, ../foo)
- Index/init file resolution (directory imports)
- External package skip
- Standard library skip
- Language-specific quirks (Go module path, Python package __init__.py, Rust mod resolution, C# namespace heuristic, Java package-to-path)

**Total: ~20 path resolution snapshot tests.**

**Workflow:**
```bash
# Run all tests
cargo test

# If a snapshot changed intentionally, review and accept:
cargo insta review

# This updates .snap files — shows in git diff for code review
```

**Dependency:** `insta` crate (with `yaml` feature for readable snapshots).

### Pipeline Unit Tests (via D-019 Trait Injection)

**Purpose:** Verify pipeline stage logic in isolation, without filesystem access.

**Technique:** Mock implementations of `FileWalker`, `FileReader`, `GraphSerializer` traits:

```rust
// MockWalker returns predefined file list — no FS needed
struct MockWalker { files: Vec<FileEntry> }
impl FileWalker for MockWalker { ... }

// MockReader returns predefined file contents — no FS needed
struct MockReader { contents: HashMap<PathBuf, Vec<u8>> }
impl FileReader for MockReader { ... }
```

**What these test:**
- Pipeline orchestration logic (stage sequencing, error propagation)
- DiagnosticCollector warning aggregation and sorting
- Resolution logic with controlled file sets
- Clustering logic with known graph topologies

These complement L2 fixture tests. Fixtures test end-to-end correctness; pipeline unit tests test stage logic in isolation with controlled inputs.

### L2: Fixture Graph Tests

**Purpose:** Verify that the full pipeline (parse → resolve → edge creation → clustering) produces the correct graph for known projects.

**Technique:** Build graph on fixture projects using real `FsWalker`/`FsReader`/`JsonSerializer`, snapshot the entire output (graph.json, clusters.json).

**Fixtures:**

```
tests/fixtures/
├── typescript-app/           # ~10 files, known import graph
│   ├── src/
│   │   ├── index.ts          # barrel re-exports
│   │   ├── api/
│   │   │   └── router.ts     # imports from services
│   │   ├── services/
│   │   │   ├── auth.ts       # imports from utils
│   │   │   └── user.ts       # imports from auth + utils
│   │   ├── utils/
│   │   │   └── format.ts     # no imports (leaf node)
│   │   └── types/
│   │       └── index.d.ts    # type definitions
│   ├── tests/
│   │   └── auth.test.ts      # tests auth.ts
│   ├── package.json          # config file
│   └── tsconfig.json         # config file
│
├── go-service/               # ~8 files, Go module structure
│   ├── go.mod
│   ├── main.go
│   ├── internal/
│   │   ├── handler/
│   │   │   └── api.go
│   │   └── service/
│   │       └── auth.go
│   └── pkg/
│       └── utils/
│           └── helpers.go
│
├── python-package/           # ~8 files, package structure
│   ├── pyproject.toml
│   ├── src/
│   │   └── mypackage/
│   │       ├── __init__.py   # barrel imports
│   │       ├── api.py
│   │       ├── services/
│   │       │   ├── __init__.py
│   │       │   └── auth.py
│   │       └── utils.py
│   └── tests/
│       └── test_auth.py
│
├── mixed-project/            # ~6 files, multiple languages
│   ├── backend/
│   │   └── main.go
│   ├── frontend/
│   │   ├── app.ts
│   │   └── utils.ts
│   └── scripts/
│       └── deploy.py
│
├── rust-crate/               # Rust project with mod/use imports
├── csharp-project/           # C# project with using directives
├── java-project/             # Java project with import statements
├── workspace-project/        # npm workspace with 3 packages, cross-package imports
│
└── edge-cases/               # pathological inputs
    ├── empty-file.ts         # 0 imports, 0 exports
    ├── syntax-error.ts       # unparseable — should be skipped
    ├── circular-a.ts         # A imports B
    ├── circular-b.ts         # B imports A
    ├── deeply-nested/a/b/c/d/e/f.ts  # deep nesting
    └── unicode-path/файл.ts  # non-ASCII path
```

**What each fixture tests:**

| Fixture | Key behaviors verified |
|---------|----------------------|
| typescript-app | Full TS pipeline: imports, exports, barrel re-exports, type-only imports, test detection, layer inference, clustering |
| go-service | Go module resolution, internal/pkg convention, empty exports |
| python-package | `__init__.py` barrel, relative imports, TYPE_CHECKING, pyproject.toml as config |
| rust-crate | Rust mod/use resolution, pub exports, crate-internal imports |
| csharp-project | C# using directives, namespace resolution, public class exports |
| java-project | Java import statements, package-to-path resolution, public class/enum exports |
| mixed-project | Multi-language in one graph, correct language selection by extension |
| workspace-project | npm workspace cross-package imports, path resolution across package boundaries *(Phase 1b)* |
| edge-cases | Graceful degradation (syntax error), circular imports, empty files, deep nesting, unicode |

**Snapshot format:** Full `graph.json` and `clusters.json` output, snapshot-tested with `insta`. Any change to the graph = explicit review.

**Path normalization tests:** Path normalization, case sensitivity, and directory traversal prevention tests are covered under L2 fixture tests via the `workspace-project/` and `edge-cases/` fixtures *(Phase 1b)*, per `design/path-resolution.md`.

**Important:** Fixture files are committed and NEVER auto-generated. They represent known-good projects with hand-verified expected behavior.

### L3: Graph Invariant Tests

**Purpose:** Verify structural properties that must always hold, regardless of input.

**Phase 1a (basic):** Deterministic invariant checks on fixture graphs — INV-1 (edge referential integrity), INV-2 (no self-imports), INV-8 (counts match), INV-9 (no duplicates), INV-11 (byte-identical determinism).

**Phase 1b (full):** All 13 invariants + property-based testing via `proptest` crate.

**Technique:** Deterministic checks in Phase 1a, `proptest` added in Phase 1b.

**Invariants (must always hold):**

```
INV-1: Edge referential integrity
  ∀ edge ∈ graph.edges:
    edge.from ∈ graph.nodes AND edge.to ∈ graph.nodes

INV-2: No self-import edges
  ∀ edge ∈ graph.edges where edge.type = imports:
    edge.from ≠ edge.to

INV-3: Test edges connect test to source
  ∀ edge ∈ graph.edges where edge.type = tests:
    graph.nodes[edge.from].file_type = Test
    AND graph.nodes[edge.to].file_type ∈ {Source, TypeDef}

INV-4: Every node belongs to a cluster
  ∀ node ∈ graph.nodes:
    node.cluster ≠ "" AND node.cluster ∈ clusters.keys()

INV-5: Cluster file lists are complete
  ∀ cluster ∈ clusters:
    cluster.files = {n.path : n ∈ graph.nodes WHERE n.cluster = cluster.name}
    cluster.file_count = |cluster.files|

INV-6: Cluster edge counts are correct
  ∀ cluster ∈ clusters:
    cluster.internal_edges = |{e ∈ edges : nodes[e.from].cluster = cluster AND nodes[e.to].cluster = cluster}|
    cluster.external_edges = |{e ∈ edges : (nodes[e.from].cluster = cluster) XOR (nodes[e.to].cluster = cluster)}|

INV-7: Cohesion is correctly computed
  ∀ cluster ∈ clusters:
    IF cluster.internal_edges + cluster.external_edges > 0:
      cluster.cohesion = cluster.internal_edges / (cluster.internal_edges + cluster.external_edges)
    ELSE:
      cluster.cohesion = 1.0

INV-8: Node count and edge count match
  graph.node_count = |graph.nodes|
  graph.edge_count = |graph.edges|

INV-9: No duplicate edges
  ∀ (e1, e2) ∈ graph.edges where e1 ≠ e2:
    NOT (e1.from = e2.from AND e1.to = e2.to AND e1.edge_type = e2.edge_type)

INV-10: Content hashes are deterministic
  hash(file) at time T1 = hash(file) at time T2 IF file unchanged

INV-11: Graph build is deterministic
  build(project) at T1 = build(project) at T2 IF project files unchanged
  (byte-identical output)

INV-12: Type-only imports produce TypeImports edges
  ∀ edge ∈ graph.edges where edge.type = type_imports:
    the source import was marked is_type_only = true

INV-13: Re-export edges have source
  ∀ edge ∈ graph.edges where edge.type = re_exports:
    the source export was marked is_reexport = true
```

**Implementation:** Run all invariants after every L2 fixture graph build. Also run on any graph produced during property-based tests.

**Property-based tests (proptest):**

Generate random valid source files → build graph → verify invariants. This catches edge cases we didn't think of.

- Generate random TypeScript files with valid import statements → build → check INV-1..INV-13
- Generate random directory structures → build → check clustering invariants
- Generate files with random content → verify hashing determinism (INV-10)

### L4: Performance Tests *(Phase 1b)*

**Purpose:** Detect performance regressions early. Performance is a feature, not an afterthought.

**Technique:** `criterion` crate for statistical benchmarks. Separate binary (`cargo bench`).

**Benchmarks:**

| Benchmark | Input | Threshold | What it catches |
|-----------|-------|-----------|----------------|
| `bench_build_small` | 100 TS files, linear imports | <200ms | Basic overhead |
| `bench_build_medium` | 1000 TS files, tree imports | <3s | Scaling behavior |
| `bench_build_large` | 3000 mixed files, complex graph | <10s | Production-scale |
| `bench_parse_typescript` | 1 file, 50 imports | <5ms | Parser performance |
| `bench_parse_go` | 1 file, 30 imports | <3ms | Parser performance |
| `bench_parse_python` | 1 file, 40 imports | <3ms | Parser performance |
| `bench_hash_file` | 1MB file | <1ms | Hashing overhead |
| `bench_clustering` | 3000 nodes, 8000 edges | <100ms | Clustering performance |
| `bench_serialization` | 3000-node graph | <500ms | JSON write performance |

**Regression detection:**
- `criterion` tracks historical results and reports statistical significance
- CI stores benchmark results between runs
- Alert if any benchmark regresses by >20%

**Synthetic project generation:**
```rust
/// Generate a synthetic project for benchmarks.
/// Structure: N files in M directories with K imports each.
fn generate_synthetic_project(
    file_count: usize,
    dir_count: usize,
    imports_per_file: usize,
    language: Language,
) -> TempDir
```

This is a reusable function — new benchmarks just call it with different parameters.

## Test Infrastructure

### Dependencies

```toml
[dev-dependencies]
insta = { version = "1", features = ["yaml"] }   # snapshot testing
proptest = "1"                                     # property-based testing
criterion = { version = "0.5", features = ["html_reports"] }  # benchmarks
tempfile = "3"                                     # temp directories for generated projects
```

### Directory Structure

```
tests/
├── snapshots/              # insta snapshot files (committed to git)
│   ├── parser_*.snap       # L1 parser snapshots
│   └── fixture_*.snap      # L2 fixture graph snapshots
├── parsers/                # L1 parser tests
│   ├── mod.rs
│   ├── test_typescript.rs
│   ├── test_go.rs
│   ├── test_python.rs
│   ├── test_rust.rs
│   ├── test_csharp.rs
│   └── test_java.rs
├── fixtures/               # L2 fixture projects (committed)
│   ├── typescript-app/
│   ├── go-service/
│   ├── python-package/
│   ├── rust-crate/
│   ├── csharp-project/
│   ├── java-project/
│   ├── mixed-project/
│   ├── workspace-project/
│   └── edge-cases/
├── graph_tests.rs          # L2 fixture graph tests
├── invariants.rs           # L3 invariant checker (reusable module)
└── helpers.rs              # shared test utilities (synthetic project gen, etc.)

benches/
├── build_bench.rs          # L4 build benchmarks
├── parser_bench.rs         # L4 parser benchmarks
└── helpers.rs              # benchmark utilities (synthetic project gen)
```

### CI Integration

```yaml
# .github/workflows/ci.yml
jobs:
  test:
    steps:
      - cargo test                    # L1 + L2 + L3
      - cargo insta test --check      # fail if snapshots out of date

  bench:
    steps:
      - cargo bench                   # L4
      # optionally: compare against baseline, alert on regression
```

**Snapshot policy in CI:** `cargo insta test --check` fails if any snapshot doesn't match. Developers must explicitly review and accept snapshot changes via `cargo insta review` before pushing.

## Scaling the System

### Adding a New Language

When a new language parser is added, the following test artifacts are required:

1. **L1:** Snapshot tests in `tests/parsers/test_<language>.rs` — one test per import/export pattern
2. **L1:** Path resolution snapshot tests
3. **L2:** New fixture project in `tests/fixtures/<language>-project/` (or add files to `mixed-project/`)
4. **L3:** Invariant tests run automatically (no changes needed — invariants are language-agnostic)
5. **L4:** Parser benchmark in `benches/parser_bench.rs`

Checklist enforced by CI: if a new file exists in `src/parser/`, corresponding test files must exist.

### Adding a New Feature

Examples: new edge type, new file type, new algorithm (Phase 2).

1. **Update invariants** in `tests/invariants.rs` if the feature introduces new structural properties
2. **Add fixture coverage** — ensure at least one fixture exercises the new feature
3. **Update existing snapshots** via `cargo insta review` — the diff shows exactly what changed
4. **Add benchmarks** if the feature has performance implications

### Tracking Test Health

Metrics to monitor:
- **Snapshot count:** total number of `.snap` files. Should grow monotonically with features/languages.
- **Invariant count:** total number of INV-* checks. Should grow with data model complexity.
- **Fixture coverage:** % of import patterns exercised by fixture projects.
- **Benchmark baseline:** historical performance numbers.

## What We DON'T Test

- **Tree-sitter grammar correctness** — tree-sitter grammars are maintained by language communities. We trust them. We test that OUR extraction logic works on tree-sitter output.
- **File system edge cases beyond fixtures** — we test unicode paths, deep nesting, empty files, and syntax errors. We don't fuzz the file system.
- **Exact JSON formatting** — we test schema and values, not whitespace. `serde_json::to_string_pretty` handles formatting.
- **Cross-platform path resolution in CI** — we test on the CI platform (Linux). Windows path handling is deferred until Windows users report issues.
