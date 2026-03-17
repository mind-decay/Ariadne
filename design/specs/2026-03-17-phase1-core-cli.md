# Phase 1: Core CLI — Parse, Build, Output

## Goal

Implement `ariadne`, a standalone Rust CLI binary that parses a project's source code via tree-sitter and produces a structural dependency graph. After Phase 1: `ariadne build <path>` parses a multi-language project and outputs `graph.json` (files + typed edges) and `clusters.json` (directory-based grouping); `ariadne info` reports version and supported languages; 6 Tier 1 languages are supported (TypeScript/JavaScript, Go, Python, Rust, C#, Java); graph data model captures nodes with metadata (type, layer, hash, exports) and edges with types (imports, tests, re-exports, type-imports); content hashing enables future delta detection; the binary installs via `cargo install` with prebuilt binaries available via GitHub Releases CI.

## Risk Classification

**YELLOW (overall)** — New standalone project, additive. No modifications to existing files. Impact analysis and regression check needed for future phases, but Phase 1 itself is isolated.

**Per-deliverable:**

| Deliverable | Risk | Rationale |
|-------------|------|-----------|
| D1: Rust Project Structure | GREEN | New project scaffold, no existing files modified |
| D2: Core Data Model | GREEN | New Rust types, no existing files affected |
| D3: Tree-sitter Integration | YELLOW | External dependency, parser correctness critical |
| D4: LanguageParser Trait + Registry | GREEN | Extensibility contract, new code only |
| D5: TypeScript/JavaScript Parser | YELLOW | Highest complexity: import/require/export/dynamic import/barrel re-exports |
| D6: Go Parser | GREEN | Low complexity: `import "path"`, `import (...)` |
| D7: Python Parser | YELLOW | Medium: `import`, `from...import`, relative imports |
| D8: Rust Parser | YELLOW | Medium: `use`, `mod`, `extern crate` |
| D9: C# Parser | GREEN | Low complexity: `using`, `using static` |
| D10: Java Parser | GREEN | Low complexity: `import`, `import static` |
| D11: File Type Detection | GREEN | Pattern matching on paths, new code |
| D12: Architectural Layer Inference | YELLOW | Heuristic-based, needs calibration against real projects |
| D13: Content Hashing | GREEN | xxHash64, straightforward |
| D14: Graph Builder (orchestration) | YELLOW | Coordinates all components, correctness critical |
| D15: JSON Serialization | GREEN | Compact tuple format, straightforward |
| D16: Directory-based Clustering | GREEN | Simple directory grouping, no algorithms needed |
| D17: CLI Interface | GREEN | clap-based CLI, two subcommands |
| D18: GitHub Releases CI | GREEN | Standard cross-compilation workflow |
| D19: Tests | GREEN | Additive test files |

## Design Sources

| Deliverable | Primary Source | Supporting Sources |
|-------------|---------------|-------------------|
| D1: Project Structure | `architecture.md` (Architecture) | D-001 (Rust CLI decision) |
| D2: Data Model | `architecture.md` (Graph Data Model) | D-001 |
| D3: Tree-sitter | `architecture.md` (Why tree-sitter) | D-001, D-002 |
| D4: LanguageParser Trait | `architecture.md` (Language Support) | D-002 (trait-based extension) |
| D5-D10: Language Parsers | `architecture.md` (Language Support table) | D-002 (Tier 1 languages) |
| D11: File Type Detection | `architecture.md` (File types enum) | — |
| D12: Layer Inference | `architecture.md` (Architectural Layers) | — |
| D13: Content Hashing | `architecture.md` (Delta Computation, Content hash) | — |
| D14: Graph Builder | `architecture.md` (Storage Format, CLI) | D-001 |
| D15: JSON Serialization | `architecture.md` (graph.json format, clusters.json format) | — |
| D16: Clustering | `architecture.md` (Clustering — Level 1) | — |
| D17: CLI | `architecture.md` (CLI Interface) | — |
| D18: GitHub Releases CI | `architecture.md` (Installation) | D-003 (graceful degradation) |
| D19: Tests | `ROADMAP.md` (Phase 1 Testing) | — |

## Deliverables

### D1: Rust Project Structure

**What:** Cargo project scaffold with workspace layout for the `ariadne` binary.

**Structure:**
```
├── Cargo.toml              # workspace root / binary crate
├── Cargo.lock
├── .gitignore
├── README.md               # what it is, how to install, how to use
├── src/
│   ├── main.rs             # CLI entry point (clap)
│   ├── lib.rs              # public API re-exports
│   ├── graph/
│   │   ├── mod.rs          # Graph struct, build orchestration
│   │   ├── model.rs        # Node, Edge, FileType, EdgeType, ArchLayer structs
│   │   ├── serialize.rs    # JSON serialization (graph.json, clusters.json)
│   │   └── cluster.rs      # Directory-based clustering
│   ├── parser/
│   │   ├── mod.rs          # LanguageParser trait, ParserRegistry
│   │   ├── typescript.rs   # TypeScript/JavaScript parser
│   │   ├── go.rs           # Go parser
│   │   ├── python.rs       # Python parser
│   │   ├── rust_lang.rs    # Rust parser (rust_lang to avoid keyword collision)
│   │   ├── csharp.rs       # C# parser
│   │   └── java.rs         # Java parser
│   ├── detect/
│   │   ├── mod.rs          # File type detection + layer inference
│   │   └── patterns.rs     # Path/naming patterns for detection
│   └── hash.rs             # xxHash64 content hashing
├── tests/
│   ├── fixtures/           # Multi-language sample project
│   │   ├── typescript/     # TS/JS test files
│   │   ├── go/             # Go test files
│   │   ├── python/         # Python test files
│   │   ├── rust_project/   # Rust test files
│   │   ├── csharp/         # C# test files
│   │   ├── java/           # Java test files
│   │   └── mixed/          # Multi-language project fixture
│   ├── parser_tests.rs     # Per-language parser unit tests
│   ├── integration_test.rs # Full build on mixed fixture
│   └── bench_test.rs       # Performance benchmark
└── .github/
    └── workflows/
        └── release.yml     # Cross-compilation + GitHub Releases
```

**Key dependencies (Cargo.toml):**
- `clap` — CLI argument parsing (derive API)
- `tree-sitter` — core parsing library
- `tree-sitter-typescript`, `tree-sitter-javascript`, `tree-sitter-go`, `tree-sitter-python`, `tree-sitter-rust`, `tree-sitter-c-sharp`, `tree-sitter-java` — grammar crates
- `serde`, `serde_json` — JSON serialization
- `xxhash-rust` — content hashing (xxHash64)
- `walkdir` — recursive directory traversal
- `ignore` — .gitignore-aware file walking (respects .gitignore, skips node_modules, .git, etc.)
- `rayon` — data parallelism for parallel file parsing
- `anyhow` — error handling for CLI (idiomatic for Rust CLI tools)

**Dev dependencies:**
- `insta` (with `yaml` feature) — snapshot testing for parser output and fixture graphs
- `proptest` — property-based testing for graph invariants
- `criterion` (with `html_reports` feature) — statistical benchmarks
- `tempfile` — temp directories for generated synthetic projects

### D2: Core Data Model (`src/graph/model.rs`)

**What:** Rust types for the graph data model per `architecture.md`.

**Types:**

```rust
pub struct Node {
    pub path: String,           // relative to project root (unique ID)
    pub file_type: FileType,
    pub layer: ArchLayer,
    pub arch_depth: u32,        // populated later (future phase, topological sort)
    pub lines: u32,
    pub hash: String,           // xxHash64 hex string
    pub exports: Vec<String>,
    pub cluster: String,
}

pub enum FileType {
    Source,
    Test,
    Config,
    Style,
    Asset,
    TypeDef,
}

pub enum ArchLayer {
    Api,
    Service,
    Data,
    Util,
    Component,
    Hook,
    Config,
    Unknown,
}

pub struct Edge {
    pub from: String,           // source file path
    pub to: String,             // target file path
    pub edge_type: EdgeType,
    pub symbols: Vec<String>,
}

pub enum EdgeType {
    Imports,
    Tests,
    ReExports,
    TypeImports,
}

pub struct ProjectGraph {
    pub version: u32,           // schema version (1)
    pub generated: String,      // ISO 8601 timestamp
    pub project_root: String,
    pub nodes: HashMap<String, Node>,
    pub edges: Vec<Edge>,
}

pub struct Cluster {
    pub files: Vec<String>,
    pub file_count: usize,
    pub internal_edges: usize,
    pub external_edges: usize,
    pub cohesion: f64,          // internal_edges / (internal_edges + external_edges)
}

pub struct ClusterMap {
    pub clusters: HashMap<String, Cluster>,
}
```

**Note:** `arch_depth` defaults to 0 in Phase 1. Topological sort that populates it is a future phase.

### D3: Tree-sitter Integration

**What:** Core tree-sitter setup — parsing source files into ASTs for import/export extraction.

**Responsibilities:**
- Initialize tree-sitter `Parser` instances per language
- Map file extensions to languages
- Parse file content into `Tree` AST
- Handle parse errors gracefully (skip unparseable files, log warning, continue)

**Error handling:** Per `design/error-handling.md`:
- Full parse failure (W001) → skip file, emit warning
- Partial parse with >50% ERROR nodes (W007) → extract from valid subtrees, emit warning
- Binary file detection (W004) → check for null bytes in first 8KB, skip
- Non-UTF-8 content (W009) → skip file, emit warning
- File too large (W003) → skip if >1MB (configurable via `--max-file-size`)
- Read permission error (W002) → skip, emit warning

Exit code remains 0 for recoverable errors. Exit code 1 only for fatal errors (E001-E005).

### D4: LanguageParser Trait + Registry (`src/parser/mod.rs`)

**What:** Trait definition and parser registry per D-002.

**Trait:**
```rust
pub trait LanguageParser: Send + Sync {
    fn language(&self) -> &str;
    fn extensions(&self) -> &[&str];
    fn tree_sitter_language(&self) -> tree_sitter::Language;
    fn extract_imports(&self, tree: &tree_sitter::Tree, source: &[u8]) -> Vec<Import>;
    fn extract_exports(&self, tree: &tree_sitter::Tree, source: &[u8]) -> Vec<Export>;
    fn resolve_import_path(&self, import: &Import, file: &Path, root: &Path) -> Option<PathBuf>;
}
```

**Supporting types:**
```rust
pub struct Import {
    pub module_path: String,    // raw import path as written in source
    pub symbols: Vec<String>,   // specific symbols imported (empty = whole module)
    pub is_type_only: bool,     // TypeScript `import type`, Python TYPE_CHECKING
    pub is_dynamic: bool,       // dynamic import()
}

pub struct Export {
    pub name: String,           // exported symbol name
    pub is_reexport: bool,      // barrel re-export (export { x } from './y')
    pub source: Option<String>, // re-export source path
}
```

**Registry (`ParserRegistry`):**
- `register(parser: Box<dyn LanguageParser>)` — register a language parser
- `parser_for_extension(ext: &str) -> Option<&dyn LanguageParser>` — lookup by file extension
- `supported_languages() -> Vec<&str>` — list registered language names
- `new_with_defaults() -> Self` — create registry with all Tier 1 parsers registered

### D5: TypeScript/JavaScript Parser (`src/parser/typescript.rs`)

**What:** Parser for TypeScript and JavaScript import/export patterns.

**Extensions:** `.ts`, `.tsx`, `.js`, `.jsx`, `.mjs`, `.cjs`

**Import patterns extracted:**
- `import { x, y } from './module'` — named imports
- `import x from './module'` — default import
- `import * as x from './module'` — namespace import
- `import './module'` — side-effect import (edge with empty symbols)
- `const x = require('./module')` — CommonJS require
- `import('./module')` — dynamic import (marked `is_dynamic: true`)
- `import type { X } from './module'` — type-only import (marked `is_type_only: true`)

**Export patterns extracted:**
- `export { x, y }` — named exports
- `export default x` — default export
- `export { x } from './module'` — re-export (marked `is_reexport: true`, `source` set)
- `export * from './module'` — barrel re-export
- `export function/class/const x` — declaration exports

**Path resolution:**
- Relative paths (`./`, `../`) → resolve against file directory
- Try extensions: `.ts`, `.tsx`, `.js`, `.jsx`, `.mjs`, `.cjs`
- Try `index.{ts,tsx,js,jsx}` for directory imports
- Bare specifiers (no `.` prefix) → skip (external package, not in graph)
- `@/` prefix → skip (alias resolution requires reading tsconfig.json `paths`, deferred to future enhancement; see Deferred item 8)

### D6: Go Parser (`src/parser/go.rs`)

**What:** Parser for Go import statements.

**Extensions:** `.go`

**Import patterns:**
- `import "path/to/pkg"` — single import
- `import ( "path/to/pkg1" \n "path/to/pkg2" )` — grouped import
- `import alias "path/to/pkg"` — aliased import
- `import . "path/to/pkg"` — dot import
- `import _ "path/to/pkg"` — blank import (side-effect)

**Export patterns:**
- Go uses capitalization for exports — not extracted at symbol level. All public functions/types are conceptually exported. `exports` will be empty for Go files (no explicit export statements to parse).

**Path resolution:**
- Module-relative paths → resolve against `go.mod` module root
- Standard library paths (`fmt`, `os`, etc.) → skip (external)
- External module paths → skip (external)
- Internal project paths → resolve against module root

### D7: Python Parser (`src/parser/python.rs`)

**What:** Parser for Python import statements.

**Extensions:** `.py`, `.pyi`

**Import patterns:**
- `import module` — module import
- `import module as alias` — aliased import
- `from module import name` — from-import
- `from module import name as alias` — aliased from-import
- `from . import name` — relative import (current package)
- `from ..module import name` — parent relative import
- `from __future__ import x` → skip (not a dependency)
- Imports inside `if TYPE_CHECKING:` block → marked `is_type_only: true`

**Export patterns:**
- `__all__ = ['x', 'y']` — explicit exports
- If no `__all__`, all top-level non-underscore names are conceptually exported (too expensive to extract fully — only extract `__all__` if present)

**Path resolution:**
- Relative imports → resolve against file's package directory
- Absolute imports → resolve against project root
- Try: `module.py`, `module/__init__.py`
- Standard library / external packages → skip

### D8: Rust Parser (`src/parser/rust_lang.rs`)

**What:** Parser for Rust use/mod statements.

**Extensions:** `.rs`

**Import patterns:**
- `use crate::module::Item` — crate-relative use
- `use super::Item` — parent module use
- `use self::Item` — current module use
- `use module::Item` — external crate use → skip
- `mod submodule;` — module declaration (implies dependency on `submodule.rs` or `submodule/mod.rs`)
- `extern crate name;` — external crate → skip
- `use std::*` / `use core::*` → skip (standard library)

**Export patterns:**
- `pub fn`, `pub struct`, `pub enum`, `pub trait`, `pub type`, `pub const`, `pub static` — public items
- `pub use` — re-export

**Path resolution:**
- `crate::` → resolve from crate root (src/lib.rs or src/main.rs)
- `super::` → resolve from parent module
- `self::` → resolve within current module
- `mod submodule;` → `submodule.rs` or `submodule/mod.rs` relative to current file
- External crates → skip

### D9: C# Parser (`src/parser/csharp.rs`)

**What:** Parser for C# using statements.

**Extensions:** `.cs`

**Import patterns:**
- `using Namespace;` — namespace import
- `using static Namespace.Class;` — static import
- `using Alias = Namespace.Class;` — aliased using
- `global using Namespace;` — global using (C# 10+)

**Export patterns:**
- C# uses namespaces + access modifiers. `public class`, `public interface`, etc. → export symbol name.

**Path resolution:**
- C# uses namespace-based resolution, not file-path-based. Map namespace segments to directory structure as a heuristic (e.g., `MyApp.Services.Auth` → `Services/Auth/`). Match against known files in that directory.
- This is inherently approximate — C# doesn't enforce file-per-namespace. Accept some false negatives.

### D10: Java Parser (`src/parser/java.rs`)

**What:** Parser for Java import statements.

**Extensions:** `.java`

**Import patterns:**
- `import package.Class;` — class import
- `import package.*;` — wildcard import
- `import static package.Class.method;` — static import
- `import static package.Class.*;` — static wildcard import

**Export patterns:**
- `public class`, `public interface`, `public enum`, `public record` → export symbol name.

**Path resolution:**
- Java convention: `com.example.service.AuthService` → `com/example/service/AuthService.java`
- Resolve against `src/main/java/` or `src/` (common source roots)
- External packages → skip (not in project tree)

### D11: File Type Detection (`src/detect/mod.rs`)

**What:** Classify files into FileType enum based on path patterns and naming conventions.

**Detection rules (evaluated in order):**

1. **Test:** Path contains `test`, `tests`, `__tests__`, `spec`, `_test.go`, `.test.ts`, `.spec.ts`, `.test.js`, `.spec.js`, `test_*.py`, `*_test.py`, `*_test.rs`, `Tests/` (C#), `Test.java` suffix
2. **Config:** Extensions `.json`, `.yaml`, `.yml`, `.toml`, `.xml`, `.ini`, `.env`, `.config.ts`, `.config.js`. Also: `tsconfig*.json`, `webpack.config.*`, `package.json`, `Cargo.toml`, `go.mod`, `setup.py`, `pyproject.toml`, `*.csproj`, `*.sln`, `pom.xml`, `build.gradle`
3. **Style:** Extensions `.css`, `.scss`, `.sass`, `.less`, `.styl`. Also styled-components files (heuristic: `*.styles.ts`, `*.styled.ts`)
4. **Asset:** Extensions `.png`, `.jpg`, `.jpeg`, `.gif`, `.svg`, `.ico`, `.woff`, `.woff2`, `.ttf`, `.eot`
5. **TypeDef:** Extensions `.d.ts`, `.pyi`, files in `@types/` directory
6. **Source:** Everything else with a recognized source extension

### D12: Architectural Layer Inference (`src/detect/mod.rs`)

**What:** Infer ArchLayer from file path using directory naming conventions.

**Heuristic rules (directory-name based):**

| Directory pattern | Layer |
|-------------------|-------|
| `api/`, `routes/`, `controllers/`, `handlers/`, `endpoints/` | Api |
| `services/`, `service/`, `usecases/`, `usecase/` | Service |
| `data/`, `db/`, `database/`, `models/`, `entities/`, `repositories/`, `repo/` | Data |
| `utils/`, `util/`, `helpers/`, `lib/`, `common/`, `shared/` | Util |
| `components/`, `views/`, `pages/`, `screens/`, `widgets/`, `ui/` | Component |
| `hooks/`, `composables/` | Hook |
| `config/`, `configs/`, `configuration/`, `settings/` | Config |
| (no match) | Unknown |

**Resolution:** Check all path segments. If multiple match, use the deepest (most specific) segment. For example, `src/api/utils/format.ts` → `Util` (deepest match is `utils/`).

### D13: Content Hashing (`src/hash.rs`)

**What:** xxHash64-based content hashing for delta detection.

**Function:** `hash_file(path: &Path) -> Result<String>` — read file bytes, compute xxHash64, return lowercase hex string (16 chars).

**Why xxHash64:** Faster than SHA-256 (10x+), sufficient collision resistance for file identity within a single project (not a security hash). Used for delta detection in a future phase's incremental update — if hash matches, file hasn't changed.

### D14: Graph Builder (`src/graph/mod.rs`)

**What:** Orchestrates the full graph build pipeline.

**Build steps:**
1. Walk project directory (using `ignore` crate — respects `.gitignore`)
2. Filter to files with recognized extensions (from registered parsers)
3. For each file:
   a. Detect file type (D11)
   b. Infer architectural layer (D12)
   c. Count lines
   d. Compute content hash (D13)
   e. Parse with appropriate language parser (D3-D10):
      - Extract imports → resolve paths → create edges
      - Extract exports → store in node
   f. Create Node
4. Post-processing:
   a. Infer test edges: for each test file, find its subject (by naming convention or imports) → create `Tests` edge
   b. Detect re-export edges: for barrel files (index.ts, __init__.py) → convert import+export pairs to `ReExports` edges
5. Assign clusters (D16)
6. Compute edge counts per cluster (internal/external), cohesion metric

**Parallelism:** File parsing is embarrassingly parallel. Use `rayon` for parallel iteration over files. Tree-sitter parsing + import extraction per file is independent.

**Additional dependency:** `rayon` for data parallelism.

### D15: JSON Serialization (`src/graph/serialize.rs`)

**What:** Serialize ProjectGraph and ClusterMap to JSON per `architecture.md` format.

**graph.json:**
- Nodes as object map (path → node data)
- Edges as array of compact tuples: `[from, to, edge_type, symbols]`
- Includes: `version`, `generated`, `project_root`, `node_count`, `edge_count`

**Serialization format for enums:** Rust PascalCase enum variants serialize to snake_case JSON strings matching `architecture.md` format (e.g., `#[serde(rename_all = "snake_case")]`):
- EdgeType: `imports`, `tests`, `re_exports`, `type_imports`
- FileType: `source`, `test`, `config`, `style`, `asset`, `type_def`
- ArchLayer: `api`, `service`, `data`, `util`, `component`, `hook`, `config`, `unknown`

**clusters.json:**
- Clusters as object map (name → cluster data)
- Includes: `files`, `file_count`, `internal_edges`, `external_edges`, `cohesion`

**Output directory:** `--output` flag (default: `.ariadne/graph/`). Create directory if it doesn't exist.

### D16: Directory-based Clustering (`src/graph/cluster.rs`)

**What:** Level 1 clustering — group files by top-level source directory.

**Algorithm:**
1. For each node, extract the first meaningful directory segment under the source root
   - `src/auth/login.ts` → cluster "auth"
   - `src/api/routes/user.ts` → cluster "api"
   - `lib/utils/format.go` → cluster "utils"
   - `app/services/billing.py` → cluster "services"
2. Files directly in source root (no subdirectory) → cluster "root"
3. Detect common source root prefixes: `src/`, `lib/`, `app/`, `pkg/`, `internal/`, `cmd/` → strip for cluster naming

**Cohesion metric:** `internal_edges / (internal_edges + external_edges)`. A cluster with all dependencies internal has cohesion 1.0. A cluster with zero total edges has cohesion 1.0 (isolated cluster is perfectly cohesive by default).

### D17: CLI Interface (`src/main.rs`)

**What:** clap-based CLI with two subcommands per roadmap scope.

**Commands:**

```
ariadne build <project-root> [options]
    Parse project, build full graph
    Default output: <project-root>/.ariadne/graph/
    Outputs: graph.json, clusters.json (written atomically via .tmp + rename)
    Note: stats.json requires algorithms (Phase 2 scope). Phase 1 produces graph.json + clusters.json only.

    Options:
      --output <dir>              Output directory (default: <project-root>/.ariadne/graph/)
      --max-file-size <bytes>     Skip files larger than this (default: 1048576 = 1MB)
      --max-files <count>         Max files to include (default: 50000)
      --verbose                   Show all warnings including unresolved imports
      --warnings <format>         Warning format: human (default), json
      --strict                    Exit code 1 if any warnings occurred

    Summary to stdout: "Built graph: {N} files, {E} edges, {C} clusters in {T}ms"
    Warnings → stderr (human or JSON format per --warnings flag)
    See design/error-handling.md for full error taxonomy (E001-E005, W001-W009)

ariadne info
    Print version, supported languages with extensions
    Example output:
    ariadne v0.1.0
    Supported languages:
      TypeScript/JavaScript (.ts, .tsx, .js, .jsx, .mjs, .cjs)
      Go (.go)
      Python (.py, .pyi)
      Rust (.rs)
      C# (.cs)
      Java (.java)
```

**Exit codes:**
- 0: success (graph built, possibly with warnings about skipped files)
- 1: fatal error (project root doesn't exist, no parseable files found, output directory not writable)

### D18: GitHub Releases CI (`.github/workflows/release.yml`)

**What:** GitHub Actions workflow for cross-compilation and release publishing.

**Targets:**
- `x86_64-unknown-linux-gnu` (Linux x64)
- `aarch64-unknown-linux-gnu` (Linux ARM64)
- `x86_64-apple-darwin` (macOS x64)
- `aarch64-apple-darwin` (macOS ARM64)
- `x86_64-pc-windows-msvc` (Windows x64)

**Trigger:** On tag push `v*` (e.g., `v0.1.0`)

**Steps per target:**
1. Checkout
2. Install Rust toolchain + target
3. `cargo build --release --target <target>`
4. Rename binary to `ariadne-<os>-<arch>[.exe]`
5. Upload as release asset

**Also:** `cargo test` runs on every push/PR (standard CI).

### D19: Tests

**What:** Comprehensive 4-level test suite per `design/testing.md`.

#### D19a: L1 — Parser Snapshot Tests (`tests/parsers/`)

Snapshot testing via `insta` crate. Per-language test files, one test per import/export pattern. Each test provides source code → calls extract_imports/extract_exports → snapshots result.

**~50 parser snapshot tests + ~20 path resolution tests** covering the full coverage matrix from `testing.md`.

**Per-language test files:**
- `tests/parsers/test_typescript.rs` — 12 patterns (7 import + 5 export)
- `tests/parsers/test_go.rs` — 5 import patterns
- `tests/parsers/test_python.rs` — 9 patterns (7 import + TYPE_CHECKING + __all__)
- `tests/parsers/test_rust.rs` — 8 patterns (5 import + 3 export)
- `tests/parsers/test_csharp.rs` — 5 patterns (4 import + 1 export)
- `tests/parsers/test_java.rs` — 5 patterns (4 import + 1 export)

Snapshots committed in `tests/snapshots/`. CI runs `cargo insta test --check` — fails if snapshots are out of date.

#### D19b: L2 — Fixture Graph Tests (`tests/graph_tests.rs`)

Build graph on fixture projects, snapshot entire graph.json + clusters.json output.

**Fixture projects:**
- `tests/fixtures/typescript-app/` (~10 files, full TS pipeline)
- `tests/fixtures/go-service/` (~8 files, Go module structure)
- `tests/fixtures/python-package/` (~8 files, package with __init__.py)
- `tests/fixtures/mixed-project/` (~6 files, multi-language)
- `tests/fixtures/edge-cases/` (empty file, syntax error, circular imports, deep nesting, unicode path)

#### D19c: L3 — Graph Invariant Tests (`tests/invariants.rs`)

13 structural invariants verified on every fixture graph build (INV-1 through INV-13 from `testing.md`). Key invariants: edge referential integrity, no self-import, test edges connect test→source, cluster completeness, cohesion correctness, deterministic build.

Property-based tests via `proptest`: generate random valid source → build graph → verify all invariants hold.

#### D19d: L4 — Performance Benchmarks (`benches/`)

`criterion`-based statistical benchmarks:
- `bench_build_small` (100 files, <200ms)
- `bench_build_medium` (1000 files, <3s)
- `bench_build_large` (3000 files, <10s)
- Per-parser benchmarks
- Hash, clustering, serialization benchmarks

Synthetic project generation via reusable `generate_synthetic_project()` function. CI tracks regression (>20% = alert).

## Dependencies on Previous Phases

None — Phase 1 is the first phase.

## Files Created

| File | Type | Description |
|------|------|-------------|
| `Cargo.toml` | Config | Rust project manifest |
| `.gitignore` | Config | Rust-specific gitignore |
| `README.md` | Docs | Installation and usage |
| `src/main.rs` | Source | CLI entry point |
| `src/lib.rs` | Source | Public API |
| `src/graph/mod.rs` | Source | Graph builder orchestration |
| `src/graph/model.rs` | Source | Data model types |
| `src/graph/serialize.rs` | Source | JSON serialization |
| `src/graph/cluster.rs` | Source | Directory-based clustering |
| `src/parser/mod.rs` | Source | LanguageParser trait + registry |
| `src/parser/typescript.rs` | Source | TS/JS parser |
| `src/parser/go.rs` | Source | Go parser |
| `src/parser/python.rs` | Source | Python parser |
| `src/parser/rust_lang.rs` | Source | Rust parser |
| `src/parser/csharp.rs` | Source | C# parser |
| `src/parser/java.rs` | Source | Java parser |
| `src/detect/mod.rs` | Source | File type + layer detection |
| `src/detect/patterns.rs` | Source | Detection patterns |
| `src/hash.rs` | Source | xxHash64 content hashing |
| `tests/fixtures/...` | Test fixtures | Fixture projects (typescript-app, go-service, python-package, mixed-project, edge-cases) |
| `tests/parsers/mod.rs` | Test | Shared parser test utilities |
| `tests/parsers/test_typescript.rs` | Test | L1 TS/JS parser snapshots |
| `tests/parsers/test_go.rs` | Test | L1 Go parser snapshots |
| `tests/parsers/test_python.rs` | Test | L1 Python parser snapshots |
| `tests/parsers/test_rust.rs` | Test | L1 Rust parser snapshots |
| `tests/parsers/test_csharp.rs` | Test | L1 C# parser snapshots |
| `tests/parsers/test_java.rs` | Test | L1 Java parser snapshots |
| `tests/graph_tests.rs` | Test | L2 fixture graph snapshot tests |
| `tests/invariants.rs` | Test | L3 graph invariant checker |
| `tests/helpers.rs` | Test | Shared test utilities (synthetic project gen) |
| `tests/snapshots/*.snap` | Test | Committed snapshot files |
| `benches/build_bench.rs` | Bench | L4 build performance benchmarks |
| `benches/parser_bench.rs` | Bench | L4 parser performance benchmarks |
| `benches/helpers.rs` | Bench | Benchmark utilities |
| `.github/workflows/release.yml` | CI | Cross-compilation + release |
| `.github/workflows/ci.yml` | CI | Test, clippy, fmt on push/PR |

## Files Modified

None. Phase 1 creates a new standalone project. No existing files are modified.

## Success Criteria

1. `cargo build --release` compiles without errors
2. `ariadne info` lists all 6 Tier 1 languages
3. `ariadne build` on a TypeScript project produces valid graph.json with correct import edges
4. `ariadne build` on a Go project produces valid graph.json with correct import edges
5. `ariadne build` on a Python project produces valid graph.json with correct import/from-import edges
6. `ariadne build` on a Rust project produces valid graph.json with correct use/mod edges
7. `ariadne build` on a C# project produces valid graph.json with correct using edges
8. `ariadne build` on a Java project produces valid graph.json with correct import edges
9. `ariadne build` on a mixed-language project discovers files from all languages
10. graph.json matches the format specified in `architecture.md` (compact tuple edges, node metadata)
11. clusters.json groups files by directory with correct cohesion metrics
12. Unparseable files produce warnings but don't fail the build (graceful degradation, D-003)
13. Dynamic imports marked as `is_dynamic: true`
14. Type-only imports marked as `is_type_only: true` (→ `TypeImports` edge type)
15. Barrel re-exports produce `ReExports` edge type
16. Test files detected correctly and linked to subject files via `Tests` edge type
17. File type detection matches documented rules for all 6 types
18. Architectural layer inference produces reasonable results on standard project layouts
19. Performance: 1000+ file synthetic project builds in under 3 seconds
20. All `cargo test` pass
21. GitHub Actions workflow builds for all 5 targets

## Deferred / Out of Scope

1. **Algorithms (blast radius, centrality, cycles, Louvain clustering)** — Future phase. Phase 1 builds the data; future phases make it queryable.
2. **stats.json** — Future phase. Requires algorithms (centrality, SCCs, layers) not in Phase 1 scope.
3. **Markdown views generation** — Future phase. Views require stats and algorithm output.
4. **`ariadne update` (delta/incremental)** — Future phase. Content hashing infrastructure is built in Phase 1 (D13) but the delta logic is a future phase.
5. **`ariadne query *` subcommands** — Future phase.
6. **Tier 2/3 language parsers** — Future. 6 Tier 1 languages cover ~85% of projects.
7. **`arch_depth` population** — Future phase (requires topological sort algorithm).
8. **tsconfig.json `paths` / alias resolution** — Future enhancement. `@/` and other path aliases require reading tsconfig.json `paths` config; Phase 1 skips alias imports entirely.
