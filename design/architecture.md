# Ariadne — Architecture

## Purpose

Ariadne is a structural topology map generator for software projects. It parses source code via tree-sitter and produces a dependency graph capturing file dependencies, architectural layers, and module clusters — enabling precise navigation and impact assessment.

**Ariadne produces deterministic structural data derived from code.** It does not learn, decay, or evolve through experience. It updates when code changes.

|              | Ariadne (Structural)                 | Knowledge (Semantic)                       |
| ------------ | ------------------------------------ | ------------------------------------------ |
| **Contains** | Files, imports, dependencies, layers | Patterns, decisions, conventions, failures |
| **Answers**  | "How is the code structured?"        | "What do we know about the project?"       |
| **Updates**  | When code changes (deterministic)    | When tasks complete (evidence-based)       |
| **Source**   | Static analysis (tree-sitter)        | External observations                      |

## Engine Architecture

Ariadne is a standalone Rust binary that parses source code via tree-sitter and produces a structural dependency graph.

**Why Rust:**

- Tree-sitter is written in Rust/C — native, first-class bindings
- Single binary, zero runtime dependencies
- Fastest option: 3000 files in under 10 seconds (see `design/performance.md`)
- No dependency on Node.js, Python, or any runtime

**Why tree-sitter:**

- Language-agnostic: 100+ grammar support
- Deterministic AST parsing — no LLM involvement, no token cost
- Incremental parsing support
- Battle-tested in editors (Neovim, Helix, Zed)

## Language Support

Each language implements two traits — parsing and import resolution are separate responsibilities (D-018):

```rust
/// Extracts imports/exports from AST (language syntax knowledge)
trait LanguageParser: Send + Sync {
    fn language(&self) -> &str;
    fn extensions(&self) -> &[&str];
    fn tree_sitter_language(&self) -> tree_sitter::Language;
    fn extract_imports(&self, tree: &Tree, source: &[u8]) -> Vec<RawImport>;
    fn extract_exports(&self, tree: &Tree, source: &[u8]) -> Vec<RawExport>;
}

/// Resolves raw import paths to canonical file paths (filesystem knowledge)
trait ImportResolver: Send + Sync {
    fn resolve(
        &self,
        import: &RawImport,
        from_file: &CanonicalPath,
        known_files: &FileSet,
    ) -> Option<CanonicalPath>;
}
```

A single struct can implement both traits. `LanguageParser` returns raw, unresolved import strings. `ImportResolver` maps those to canonical paths using filesystem knowledge. This separation enables swapping resolution strategies (e.g., workspace-aware resolution in Phase 1b) without touching parsers.

Parsers are registered in a `ParserRegistry` — adding a new language = implementing both traits + one `register()` call. Grammars are crate dependencies.

**Tier 1 (initial release):**

| Language                | Import forms                                                         | Complexity |
| ----------------------- | -------------------------------------------------------------------- | ---------- |
| TypeScript / JavaScript | `import`, `require`, `export`, dynamic `import()`, barrel re-exports | High       |
| Go                      | `import "path"`, `import (...)`                                      | Low        |
| Python                  | `import`, `from...import`, relative imports                          | Medium     |
| Rust                    | `use`, `mod`, `extern crate`                                         | Medium     |
| C#                      | `using`, `using static`                                              | Low        |
| Java                    | `import`, `import static`                                            | Low        |

**TypeScript/JavaScript resolution (Phase 1a):**
- Bare specifier (`react`, `lodash`) → skip (external package)
- Relative path (`./foo`, `../bar`) → join with source dir, probe extensions [`.ts`, `.tsx`, `.js`, `.jsx`, `.mjs`, `.cjs`], try index files [`index.ts`, `index.tsx`, `index.js`, `index.jsx`] for directory imports
- Scoped package (`@scope/name`) → skip (external) unless workspace-matched (Phase 1b)
- **Not in Phase 1a:** tsconfig `paths`/`baseUrl` (Phase 1b), package.json `exports` field (Phase 1b), CJS vs ESM distinction (treat uniformly)

**Phase 1a resolution limitations:**
- **Go:** Stdlib-only resolution. Module-qualified imports require `go.mod` reading (Phase 1b).
- **C#:** Namespace-to-path heuristic. C# namespaces don't map to filesystem paths; low accuracy.
- **Java:** Package-to-path heuristic with hardcoded `src/main/java/` prefix.

**Tier 2 (future):** Kotlin, Swift, C/C++, PHP, Ruby, Dart.

**Tier 3 (on demand):** Elixir, Scala, Haskell, Lua, Zig, etc.

## Graph Data Model

### Domain Types (D-017)

All domain-specific string values use newtype wrappers for compile-time safety:

```rust
CanonicalPath(String)  // relative to project root, normalized, forward slashes
ContentHash(String)    // xxHash64, lowercase hex (16 chars)
ClusterId(String)      // cluster identifier
Symbol(String)         // exported/imported symbol name
```

Constructors enforce invariants at creation time. The rest of the system works with validated types — no re-validation needed. See `design/path-resolution.md` for `CanonicalPath` normalization rules.

### Internal Model

Used by the pipeline and algorithms. Optimized for programmatic use with newtypes and enums.

**Nodes** — files with metadata (keyed by `CanonicalPath` in `BTreeMap`):

``` rust
Node {
    file_type: FileType,       // source | test | config | style | asset | type_def
    layer: ArchLayer,          // api | service | data | util | component | hook | config | unknown
    arch_depth: u32,           // topological depth (Phase 1a: always 0; computed via topo sort in Phase 2)
    lines: u32,                // line count
    hash: ContentHash,         // content hash for delta detection (D-013)
    exports: Vec<Symbol>,      // exported symbol names (sorted)
    cluster: ClusterId,        // assigned cluster ID
}
```

**Edges** — directed, typed connections:

``` rust
Edge {
    from: CanonicalPath,       // source file path
    to: CanonicalPath,         // target file path
    edge_type: EdgeType,       // imports | tests | re_exports | type_imports
    symbols: Vec<Symbol>,      // which symbols are used (sorted)
}
```

**Graph container:**

```rust
ProjectGraph {
    nodes: BTreeMap<CanonicalPath, Node>,  // sorted by path (D-006)
    edges: Vec<Edge>,                       // sorted before serialization
}
```

### Output Model (D-022)

Separate types optimized for JSON serialization. Internal newtypes convert to strings, edges become compact tuples (D-012). Conversion via `impl From<ProjectGraph> for GraphOutput`. All sort-point enforcement (D-006) happens during conversion.

```rust
GraphOutput {
    version: u32,
    project_root: String,
    node_count: usize,
    edge_count: usize,
    nodes: BTreeMap<String, NodeOutput>,     // sorted string keys
    edges: Vec<(String, String, String, Vec<String>)>,  // compact tuples
    generated: Option<String>,               // only with --timestamp
}
```

This is a **directed multigraph** — multiple edges of different types can exist between two nodes.

### Intermediate Types

Data flowing between pipeline stages uses typed intermediate structures:

```rust
RawImport  { path: String, symbols: Vec<String>, is_type_only: bool }
RawExport  { name: String, is_re_export: bool, source: Option<String> }
FileEntry  { path: PathBuf, extension: String }
FileContent { path: CanonicalPath, bytes: Vec<u8>, hash: ContentHash, lines: u32 }
ParsedFile { path: CanonicalPath, imports: Vec<RawImport>, exports: Vec<RawExport> }
```

### Pipeline Support Types

Types used by pipeline traits and orchestration:

```rust
/// Set of all successfully-read files — used for import resolution existence checks.
/// BTreeSet for deterministic iteration if ever needed. Populated after read stage,
/// before resolve stage. Contains only files that were successfully read (not walked-
/// but-failed-to-read files), preventing dangling edge targets.
/// Lives in model/types.rs (not pipeline/) so parser/traits.rs can reference it
/// without violating the dependency rule that parser/ depends on model/ only.
FileSet(BTreeSet<CanonicalPath>)  // defined in src/model/types.rs

/// Why a file was skipped during reading. Distinct from FatalError (which stops the
/// pipeline) and Warning (which is a reporting structure). The pipeline converts
/// FileSkipReason into the appropriate Warning via DiagnosticCollector.
enum FileSkipReason {
    ReadError { path: PathBuf, reason: String },   // → W002
    TooLarge { path: PathBuf, size: u64 },         // → W003
    BinaryFile { path: PathBuf },                   // → W004
    EncodingError { path: PathBuf },                // → W009
}

/// Configuration for the file walking stage.
WalkConfig {
    max_files: usize,          // default: 50_000
    max_file_size: u64,        // default: 1_048_576 (1MB)
    exclude_dirs: Vec<String>, // always includes ".ariadne"
}

/// Result of a successful pipeline run.
BuildOutput {
    graph_path: PathBuf,       // path to written graph.json
    clusters_path: PathBuf,    // path to written clusters.json
    file_count: usize,
    edge_count: usize,
    cluster_count: usize,
    warnings: Vec<Warning>,    // drained from DiagnosticCollector, sorted
}
```

**File types** (`FileType` enum, `Copy + Ord`):

- `source` — application code
- `test` — test files (detected by path pattern or naming convention)
- `config` — configuration files (tsconfig, webpack, etc.)
- `style` — CSS/SCSS/styled-components
- `asset` — static assets (images, fonts, JSON data)
- `type_def` — type definition files (.d.ts, .pyi)

**Edge types** (`EdgeType` enum, `Copy + Ord`):

- `imports` — runtime dependency (import/require/use)
- `tests` — test file covers source file (inferred from naming + imports)
- `re_exports` — barrel file re-exports (index.ts pattern)
- `type_imports` — compile-time only dependency (TypeScript `import type`, Python `TYPE_CHECKING`)

### File Type Detection (`detect/filetype.rs`)

Detection uses **per-language pattern tables**, not a single generic list. Filename-specific rules take precedence over extension rules.

| Priority | Rule | Example | FileType |
|----------|------|---------|----------|
| 1 | Known config filenames | `tsconfig.json`, `package.json`, `go.mod`, `Cargo.toml`, `pom.xml`, `build.gradle` | `config` |
| 2 | Test file patterns (per-language) | See table below | `test` |
| 3 | Type definition extensions | `.d.ts`, `.d.mts`, `.pyi` | `type_def` |
| 4 | Style extensions | `.css`, `.scss`, `.sass`, `.less`, `.styled.*` | `style` |
| 5 | Asset extensions | `.png`, `.jpg`, `.svg`, `.woff`, `.json` (if not caught by rule 1) | `asset` |
| 6 | Default | Everything else with a recognized parser extension | `source` |

**Per-language test patterns:**

| Language | Test patterns |
|----------|--------------|
| TypeScript/JS | `*.test.ts`, `*.spec.ts`, `*.test.js`, `*.spec.js`, `__tests__/*`, `*.test.tsx`, `*.spec.tsx` |
| Go | `*_test.go` (language rule, not convention) |
| Python | `test_*.py`, `*_test.py`, `conftest.py`, `tests/*.py` |
| Rust | `tests/*.rs` (integration tests); unit tests are inline (not separate FileType) |
| C# | `*Tests.cs`, `*Test.cs`, `*.Tests/*.cs` |
| Java | `*Test.java`, `*Tests.java`, `*IT.java`, `src/test/**` |

### Architectural Layer Heuristics (`detect/layer.rs`)

Layer inference uses directory name pattern matching. First matching pattern wins. Both frontend and backend conventions are covered.

| Layer | Directory patterns (case-insensitive match) |
|-------|---------------------------------------------|
| `api` | `api/`, `routes/`, `endpoints/`, `controllers/`, `handlers/`, `rest/`, `graphql/` |
| `service` | `services/`, `service/`, `domain/`, `business/`, `usecases/`, `use-cases/`, `interactors/`, `middleware/` |
| `data` | `data/`, `db/`, `database/`, `repository/`, `repositories/`, `models/`, `dao/`, `store/`, `stores/`, `schema/`, `migration/`, `migrations/` |
| `util` | `utils/`, `util/`, `helpers/`, `lib/`, `shared/`, `common/`, `pkg/` |
| `component` | `components/`, `component/`, `ui/`, `views/`, `pages/`, `layouts/`, `widgets/` |
| `hook` | `hooks/`, `composables/` |
| `config` | `config/`, `configuration/`, `settings/`, `env/` |
| `unknown` | No matching pattern (default) |

**Note:** `component` and `hook` are frontend-specific. Backend-only projects (Go, Java, C#) will typically produce `api`, `service`, `data`, `util`, and `unknown` layers. `unknown` is expected to be common for backend projects and is acceptable — the layer heuristic provides best-effort classification, not exhaustive coverage.

### Edge Type Inference

**`tests` edges** — inferred during graph assembly (`pipeline/build.rs`):

1. File has `FileType::test` (detected by `detect/filetype.rs` per-language patterns above)
2. For each import from the test file, if the target is `FileType::source` or `FileType::type_def`:
   - Create edge with `EdgeType::tests` instead of `EdgeType::imports`
3. Additionally, apply naming convention matching: if test file `foo.test.ts` exists and `foo.ts` exists in the same or parent directory, create a `tests` edge even without an explicit import

**`re_exports` edges** — inferred from `RawExport.is_re_export`:

A `re_exports` edge is created when:
1. The parser sets `RawExport.is_re_export = true` on an export (e.g., `export { foo } from './foo'` in TypeScript)
2. The `source` field of `RawExport` resolves to a known file
3. Edge created: `from` = re-exporting file, `to` = source file, `edge_type = re_exports`

Example graph shape for barrel re-export:
```
src/index.ts  --[re_exports]--> src/auth/login.ts     (export { login } from './auth/login')
src/index.ts  --[re_exports]--> src/auth/logout.ts    (export { logout } from './auth/logout')
src/api/router.ts --[imports]--> src/index.ts          (import { login } from '../index')
```

The barrel file (`index.ts`) has `re_exports` edges to its sources and `imports` edges from its consumers. Consumers import from the barrel, not from the sources directly.

## Pipeline Architecture (D-019, D-020)

The build pipeline uses injectable trait-based stages for testability and extensibility:

```rust
/// Directory traversal abstraction
trait FileWalker: Send + Sync {
    fn walk(&self, root: &Path, config: &WalkConfig) -> Result<Vec<FileEntry>, FatalError>;
}

/// File reading + filtering abstraction
trait FileReader: Send + Sync {
    fn read(&self, entry: &FileEntry) -> Result<FileContent, FileSkipReason>;
}

/// Output writing abstraction
trait GraphSerializer: Send + Sync {
    fn write_graph(&self, output: &GraphOutput, dir: &Path) -> Result<(), FatalError>;
    fn write_clusters(&self, clusters: &ClusterOutput, dir: &Path) -> Result<(), FatalError>;
}
```

**Pipeline struct** accepts traits as `Box<dyn Trait>` objects:

```rust
BuildPipeline {
    walker: Box<dyn FileWalker>,
    reader: Box<dyn FileReader>,
    registry: ParserRegistry,           // LanguageParser + ImportResolver pairs
    serializer: Box<dyn GraphSerializer>,
}
```

`DiagnosticCollector` is created per `run_with_output()` call, not stored as a struct field.

**Execution flow** — explicit stages with typed intermediates:

```
walk(root) → Vec<FileEntry>
  → read_files() → Vec<FileContent>           (parallel via rayon on sorted list)
    → parse_files() → Vec<ParsedFile>          (parallel via rayon, preserves order)
      → resolve_and_build() → ProjectGraph     (sequential: resolution + edge creation)
        → cluster() → ClusterMap               (sequential: directory-based grouping)
          → convert + serialize() → files       (GraphOutput via From<ProjectGraph>)
```

Each stage is independently testable. `--verbose` timing wraps each stage trivially.

**`resolve_and_build` sub-responsibilities** (`pipeline/build.rs`):

This is the most complex stage. Its responsibilities are:

1. Build `FileSet` from successfully-read files (for resolution existence checks)
2. For each `ParsedFile`, call `detect/filetype.rs` → `FileType`
3. For each `ParsedFile`, call `detect/layer.rs` → `ArchLayer`
4. For each `RawImport`, call `ImportResolver::resolve` → `Option<CanonicalPath>`
5. Classify edges: `tests` (if source is test file targeting source/typedef), `re_exports` (if from `RawExport.is_re_export`), `type_imports` (if `is_type_only`), else `imports`
6. Apply naming-convention test edge inference (see Edge Type Inference above)
7. Deduplicate edges: if multiple imports from A→B with same `EdgeType`, merge symbols (union, sorted)
8. Set `arch_depth = 0` for all nodes (placeholder; computed properly in Phase 2 via topological sort after SCC contraction)
9. Assemble `ProjectGraph` with sorted exports and symbols per node/edge

**`.ariadne/` exclusion:** The `FsWalker` always excludes the `.ariadne/` directory from walking, preventing output files from being parsed as source on subsequent builds. This is hardcoded, not configurable.

**Composition Root (D-020):** `main.rs` is the sole place where concrete types are wired:

```rust
// main.rs — only file that knows about concrete implementations
let pipeline = BuildPipeline::new(
    Box::new(FsWalker::new()),
    Box::new(FsReader::new()),
    ParserRegistry::with_tier1(),
    Box::new(JsonSerializer),
);
```

Tests use mock implementations (`MockWalker`, `MockReader`) — no filesystem needed.

## Module Structure (D-023)

```
src/
├── main.rs              # Composition Root: CLI (clap), wires concrete types
├── lib.rs               # Public API: re-exports pipeline, config, output types
├── model/               # Leaf module — depends on NOTHING
│   ├── mod.rs           # Re-exports
│   ├── types.rs         # Newtypes: CanonicalPath, ContentHash, ClusterId, Symbol, FileSet
│   ├── node.rs          # Node, FileType, ArchLayer
│   ├── edge.rs          # Edge, EdgeType
│   └── graph.rs         # ProjectGraph (BTreeMap<CanonicalPath, Node> + Vec<Edge>)
├── parser/              # Depends on model/ only
│   ├── mod.rs           # Re-exports
│   ├── traits.rs        # LanguageParser, ImportResolver, RawImport, RawExport
│   ├── registry.rs      # ParserRegistry (register, lookup by extension)
│   ├── typescript.rs    # TS/JS parser + resolver
│   ├── go.rs            # Go parser + resolver
│   ├── python.rs        # Python parser + resolver
│   ├── rust_lang.rs     # Rust parser + resolver
│   ├── csharp.rs        # C# parser + resolver
│   └── java.rs          # Java parser + resolver
├── pipeline/            # Depends on traits from parser/, serial/, model/
│   ├── mod.rs           # BuildPipeline struct, run()
│   ├── walk.rs          # FileWalker trait + FsWalker impl
│   ├── read.rs          # FileReader trait + FsReader impl
│   ├── resolve.rs       # Resolution orchestration (uses ImportResolver trait)
│   └── build.rs         # Graph assembly from ParsedFile → ProjectGraph
├── detect/              # Depends on model/ only
│   ├── mod.rs           # Re-exports
│   ├── filetype.rs      # FileType detection from path/extension
│   └── layer.rs         # ArchLayer inference from directory names
├── cluster/             # Depends on model/ only
│   └── mod.rs           # Directory-based clustering: assign_clusters() + compute_cohesion()
├── serial/              # Depends on model/ only
│   ├── mod.rs           # GraphSerializer trait, output types (GraphOutput, ClusterOutput)
│   └── json.rs          # JsonSerializer impl (atomic writes, BufWriter)
├── diagnostic.rs        # FatalError (thiserror), Warning, DiagnosticCollector (D-021)
└── hash.rs              # xxHash64 wrapper → ContentHash
```

**Dependency rules:**

| Module | Depends on | Never depends on |
|--------|-----------|-----------------|
| `model/` | nothing (leaf) | everything else |
| `parser/` | `model/` | `pipeline/`, `serial/`, `detect/`, `cluster/` |
| `pipeline/` | traits from `parser/`, `serial/`; types from `model/`, `detect/`; `diagnostic.rs` | concrete parser/serializer implementations |
| `detect/` | `model/` | `parser/`, `pipeline/`, `serial/` |
| `cluster/` | `model/` | `parser/`, `pipeline/`, `serial/` |
| `serial/` | `model/`, `diagnostic.rs` (for `FatalError`) | `parser/`, `pipeline/`, `detect/`, `cluster/` |
| `diagnostic.rs` | `model/` (for `CanonicalPath` in warnings) | everything else |
| `hash.rs` | `model/` (returns `ContentHash`) | everything else |
| `main.rs` | everything (Composition Root) | — |

Concrete parser implementations (e.g., `TypeScriptParser`) are **not** `pub` — accessed only through `ParserRegistry`.

### Cluster Interface (`cluster/mod.rs`)

```rust
/// Assigns files to directory-based clusters and computes cohesion metrics.
/// Returns a ClusterMap — does NOT mutate the ProjectGraph.
/// The pipeline (build.rs) applies cluster assignments to Node.cluster fields.
pub fn assign_clusters(graph: &ProjectGraph) -> ClusterMap;
```

**Cluster naming:** If a path starts with `src/`, the cluster name uses the next segment (e.g., `src/auth/login.ts` -> `auth`). Otherwise, the first segment is used. This works well for TypeScript/Rust but may produce less meaningful names for Go or Java projects.

```rust
ClusterMap {
    clusters: BTreeMap<ClusterId, Cluster>,
}

Cluster {
    files: Vec<CanonicalPath>,    // sorted
    file_count: usize,
    internal_edges: u32,          // edges where both endpoints are in this cluster
    external_edges: u32,          // edges where exactly one endpoint is in this cluster (in + out)
    cohesion: f64,                // internal / (internal + external), or 1.0 if both are 0
}
```

**Conversion path:** `ClusterMap → ClusterOutput` conversion happens in `pipeline/mod.rs` during the convert step, NOT in `serial/`. This preserves `serial/`'s dependency rule (depends on `model/` only). `ClusterOutput` is a serialization-ready type with string keys.

### Language-Specific Resolution Notes

**Rust module paths:** Rust `use` statements reference module paths (`crate::auth::login`), not filesystem paths. The Rust `ImportResolver` handles this by:
1. `extract_imports` converts `use crate::auth::login` to `RawImport { path: "src/auth/login", ... }` — the parser pre-maps module paths to filesystem paths using Rust's module conventions (`crate::` → `src/`, `mod.rs` / `<name>.rs` lookup)
2. The resolver then uses standard `FileSet` existence checks on the pre-mapped path
3. `mod` declarations are treated as imports: `mod auth;` → `RawImport { path: "src/auth/mod" or "src/auth", ... }` with extension probing

This keeps the `ImportResolver` trait interface uniform — all resolvers work with filesystem-path-like strings by the time `resolve()` is called.

**`ParserRegistry::lookup` on unknown extension:** Returns `Option<(&dyn LanguageParser, &dyn ImportResolver)>`. Files with no matching parser are silently skipped during parsing (no node created, no warning — they are not source files from Ariadne's perspective).

### WorkspaceInfo (Phase 1b)

`WorkspaceInfo` and `WorkspaceMember` structs live in `model/` (pure data, no behavior). This allows `ImportResolver` to accept `Option<&WorkspaceInfo>` without creating a backward dependency from `parser/` to `pipeline/`. Phase 1a resolvers receive `None`. See `design/path-resolution.md` for full workspace detection design.

## Storage Format

Output goes under `.ariadne/` in the project root. The graph output directory is `.ariadne/graph/` (configurable via `--output`). The `.ariadne/` parent directory may contain other subdirectories in future phases.

``` rust
.ariadne/
├── graph/
│   ├── graph.json      # full graph — source of truth
│   ├── clusters.json   # cluster definitions with metadata
│   └── stats.json      # precomputed metrics (centrality, layers, SCCs) (Phase 2)
└── views/              # generated markdown views (Phase 2)
    ├── index.md        # L0: cluster list, critical files, cycles
    ├── clusters/       # L1: per-cluster detail
    │   ├── auth.md
    │   ├── api.md
    │   └── ...
    └── impact/         # L2: on-demand blast radius reports
        └── (generated per query)
```

### graph.json

Compact adjacency list format:

```json
{
  "version": 1,
  "project_root": "/path/to/project",
  "node_count": 847,
  "edge_count": 2341,
  "nodes": {
    "src/auth/login.ts": {
      "type": "source",
      "layer": "service",
      "arch_depth": 0,
      "lines": 142,
      "hash": "a1b2c3d4e5f67890",
      "exports": ["login", "LoginParams"],
      "cluster": "auth"
    }
  },
  "edges": [
    ["src/api/auth.ts", "src/auth/login.ts", "imports", ["login"]],
    ["src/auth/__tests__/login.test.ts", "src/auth/login.ts", "tests", []]
  ]
}
```

Edges use compact tuple format — 60%+ space savings vs objects. `--timestamp` optionally includes a `"generated"` timestamp in the output.

### clusters.json

```json
{
  "clusters": {
    "auth": {
      "files": ["src/auth/login.ts", "src/auth/logout.ts", "..."],
      "file_count": 12,
      "internal_edges": 28,
      "external_edges": 15,
      "cohesion": 0.65
    }
  }
}
```

### stats.json

```json
{
  "centrality": {
    "src/utils/format.ts": 0.89,
    "src/auth/middleware.ts": 0.72
  },
  "sccs": [["src/billing/invoice.ts", "src/auth/permissions.ts"]],
  "layers": {
    "0": ["src/utils/constants.ts", "src/types/index.ts"],
    "1": ["src/services/auth.ts"],
    "2": ["src/api/routes.ts"]
  },
  "summary": {
    "max_depth": 7,
    "avg_in_degree": 2.8,
    "avg_out_degree": 2.8,
    "bottleneck_files": ["src/utils/format.ts"],
    "orphan_files": ["src/legacy/old-helper.ts"]
  }
}
```

## Algorithms

### 1. Blast Radius — Reverse BFS

Answers: "If I change file X, what else might break?"

``` rust
blast_radius(X, max_depth=inf) -> {file: distance}:
    visited = {}
    queue = [(X, 0)]
    while queue not empty:
        node, depth = dequeue
        if node in visited: skip
        visited[node] = depth
        for each dependent in reverse_edges[node]:
            if depth + 1 <= max_depth:
                enqueue (dependent, depth + 1)
    return visited
```

**Depth semantics:**

- depth=1: direct dependents — almost certainly affected
- depth=2: transitive dependents — probably affected
- depth=3+: distant dependents — possibly affected

**Complexity:** O(V + E), linear in graph size.

### 2. Betweenness Centrality — Brandes Algorithm

Identifies bottleneck files (files that many dependency paths pass through).

``` rust
BC(v) = sum_{s!=v!=t} (sigma_st(v) / sigma_st)
```

Where sigma_st = number of shortest paths from s to t, sigma_st(v) = those passing through v.

**Brandes algorithm:** O(VE), computes all centralities in one pass. For V=3000, E=~8000: milliseconds.

Files with BC > 0.7 are marked as bottlenecks in stats.json.

### 3. Cycle Detection — Tarjan's SCC

Finds circular dependencies (strongly connected components of size > 1).

```rust
Tarjan's algorithm: O(V + E)
    - DFS with lowlink tracking
    - Nodes on stack form SCCs
    - SCC size > 1 = circular dependency
```

SCCs are reported in stats.json and surfaced in views.

### 4. Clustering — Two-Level

**Level 1: Directory-based (free).** Files in `src/auth/` -> cluster "auth". Natural, intuitive, zero computation.

**Level 2: Louvain community detection (refinement).**

``` rust
Modularity Q = (1/2m) sum_{ij} [A_ij - k_i*k_j / 2m] * delta(c_i, c_j)
```

Louvain maximizes Q greedily in O(n\*log n). Detects real module boundaries that may not align with directories (e.g., a util file that belongs semantically to a specific domain).

**Cluster assignment:** Start with directory clusters, then run Louvain. If Louvain reassigns a file, it overrides the directory-based cluster. Cluster IDs use directory names where possible for readability.

### 5. Architectural Layers — Topological Sort

On DAG (after contracting SCCs into supernodes), topological sort produces dependency layers:

``` rust
Layer 0: files with no outgoing dependencies (utils, constants, types)
Layer 1: files depending only on Layer 0
Layer 2: files depending on Layer 0-1
...
```

Automatic architecture discovery. Layer information can be used to order implementation steps (bottom-up) and detect layer violations (e.g., service importing from API layer).

### 6. Incremental Updates — Delta Computation

Full rebuild on every refresh is wasteful at scale. Delta approach:

``` rust
update(old_graph, current_fs):
    // Phase 1: detect changes via content hash
    changed = {f : hash(f) != old_graph.nodes[f].hash}
    added = current_fs - old_graph.nodes
    removed = old_graph.nodes - current_fs

    // Phase 2: re-parse only affected files
    for f in (changed | added):
        parse imports/exports
        update edges from f

    // Phase 3: remove stale data
    remove all edges from/to removed files
    remove nodes for removed files

    // Phase 4: recompute derived data
    if |changed | added | removed| > 0.05 * |nodes|:
        full recompute (clusters, centrality, layers)
    else:
        incremental cluster update
        skip centrality recompute (use previous)
```

**Content hash:** xxHash64 — fast, collision-resistant, deterministic. O(1) per file check.

**Threshold:** If >5% of files changed, full recompute (derived data may have shifted significantly). Otherwise, incremental.

### 7. Subgraph Extraction

Extract relevant neighborhood around specified files:

``` rust
extract_subgraph(files, depth=2):
    result_nodes = {}
    for f in files:
        // BFS outward (dependencies)
        bfs(f, forward_edges, depth) -> add to result_nodes
        // BFS inward (dependents)
        bfs(f, reverse_edges, depth) -> add to result_nodes
    // Include full cluster for each touched file
    for f in files:
        add all files in f.cluster to result_nodes
    return subgraph(result_nodes) with metrics
```

This is what gets rendered into L2 markdown views.

## Views: Markdown Output

### L0: Index (`views/index.md`)

~200-500 tokens. Overview for quick orientation.

```markdown
# Project Graph — Index

## Clusters (12)

| Cluster | Files | Key file (highest centrality) |
| ------- | ----- | ----------------------------- |
| auth    | 12    | src/auth/middleware.ts (0.72) |
| api     | 23    | src/api/router.ts (0.65)      |

| ...

## Critical Files (centrality > 0.7)

- src/utils/format.ts (0.89) — 47 dependents
- src/auth/middleware.ts (0.72) — 28 dependents

## Circular Dependencies (2)

- auth <-> billing (via permissions.ts <-> invoice.ts)
- ...

## Architecture (7 layers, max depth: 7)

Layer 0 (foundations): 34 files
Layer 1-2 (services): 89 files
Layer 3+ (api/ui): 45 files
```

### L1: Cluster Detail (`views/clusters/<name>.md`)

~500-2000 tokens per cluster. Internal structure and external connections.

```markdown
# Cluster: auth (12 files)

## Files

| File          | Type   | Layer | In  | Out | Centrality |
| ------------- | ------ | ----- | --- | --- | ---------- |
| middleware.ts | source | 2     | 28  | 3   | 0.72       |
| login.ts      | source | 3     | 5   | 4   | 0.31       |

| ...

## Internal Dependencies

middleware.ts -> session.ts -> token.ts

## External Dependencies (outgoing)

auth/middleware.ts -> utils/crypto.ts
auth/session.ts -> database/redis.ts

## External Dependents (incoming)

api/routes.ts -> auth/middleware.ts
api/admin.ts -> auth/permissions.ts

## Tests

auth/**tests**/login.test.ts -> login.ts
auth/**tests**/middleware.test.ts -> middleware.ts
```

### L2: Subgraph / Impact Report (`views/impact/`)

Generated on-demand via `ariadne query subgraph` or `ariadne query blast-radius`. Contains full dependency tree for specific files with all metrics.

## CLI Interface

``` rust
ariadne build <project-root> [--output <dir>]              (Phase 1a)
    Parse project, build full graph -> graph.json, clusters.json

ariadne info                                               (Phase 1a)
    Version, supported languages, build info

ariadne update <project-root> [--output <dir>]             (Phase 2)
    Incremental update via delta computation

ariadne query blast-radius <file> [--depth N] [--format json|md]  (Phase 2)
    Reverse BFS from file, output dependents with distance

ariadne query subgraph <file...> [--depth N] [--format json|md]   (Phase 2)
    Extract neighborhood around specified files

ariadne query stats [--format json|md]                     (Phase 2)
    Output precomputed metrics (centrality, SCCs, layers)

ariadne query cluster <name> [--format json|md]            (Phase 2)
    Output cluster detail

ariadne query file <path> [--format json|md]               (Phase 2)
    All info about a specific file: deps, dependents, metrics

ariadne query cycles [--format json|md]                    (Phase 2)
    List all circular dependencies

ariadne query layers [--format json|md]                    (Phase 2)
    Show architectural layers

ariadne views generate [--output <dir>]                    (Phase 2)
    Generate/regenerate all markdown views
```

Default `--format` is `md` (human-readable). `json` for programmatic use. `--timestamp` includes a generation timestamp in output.

See `design/error-handling.md` for additional diagnostic flags (`--verbose`, `--warnings`, `--strict`) and resource-limit flags (`--max-file-size`, `--max-files`).

## Installation

### Requirements

- Rust toolchain (for building from source)
- Or: prebuilt binary from GitHub Releases

### Installation Methods

**Via cargo (recommended for developers):**

```bash
cargo install ariadne-graph
```

**Via GitHub Releases (no Rust needed):**

```bash
# macOS
curl -L https://github.com/<org>/ariadne/releases/latest/download/ariadne-darwin-arm64 -o /usr/local/bin/ariadne
chmod +x /usr/local/bin/ariadne

# Linux
curl -L https://github.com/<org>/ariadne/releases/latest/download/ariadne-linux-x64 -o /usr/local/bin/ariadne
chmod +x /usr/local/bin/ariadne
```

### Git Tracking Policy

- `.ariadne/graph/graph.json`, `clusters.json`, `stats.json`: **committed** (canonical, deterministic)
- `.ariadne/views/`: **committed** (generated but stable, useful for review)
- No gitignored state — everything is reproducible

## Integration

Ariadne is designed as a standalone tool. Integration points for external systems:

- **Build/update invocation:** `ariadne build` / `ariadne update` as CLI commands
- **Data consumption:** `graph.json`, `clusters.json`, `stats.json` as stable JSON formats
- **Query API:** `ariadne query *` commands with `--format json` for programmatic use
- **View generation:** Markdown views can be loaded into any LLM agent context

Ariadne has no dependency on any specific orchestration framework.

## Limitations

Ariadne captures **syntactic static imports only**. The following are NOT modeled:

- **Dynamic imports with computed paths** — `require(config.moduleName)`, `import(variable)` where the path is not a string literal. These produce no edge.
- **Dependency injection** — dependencies wired at runtime via DI containers are invisible to static analysis.
- **Build tool transforms** — Webpack aliases, Babel module resolution, custom path mapping beyond tsconfig. Only standard language import semantics are supported.
- **Cross-language imports** — Go does not import TypeScript, etc. Each language's imports resolve within its own ecosystem.
- **JSON/data file imports** — `import config from './config.json'` (common in TypeScript) produces no edge because no JSON parser exists. The JSON file is not a node.
- **Conditional/platform imports** — imports guarded by `#ifdef`, build flags, or runtime platform checks are all extracted regardless of the condition.
- **Macro-generated imports** — Rust `macro_rules!` or procedural macros that generate `use` statements are invisible to tree-sitter.

These limitations are inherent to the tree-sitter-only, syntactic approach. They are the cost of being fast, deterministic, and language-runtime-free. For the vast majority of codebases, syntactic imports capture 95%+ of the real dependency graph.
