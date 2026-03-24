<!-- moira:deep-scan architecture 2026-03-21 -->

# Ariadne — Deep Architecture Map

**Scan date:** 2026-03-21
**Scan scope:** 48 source files read across src/, tests/, Cargo.toml
**Binary:** `ariadne` (single Rust binary, crate name `ariadne-graph`)
**Edition:** Rust 2021

---

## 1. Service Boundaries — Modules, Responsibilities, Interfaces

Ariadne is a single-binary CLI application with 13 top-level modules declared in `src/lib.rs`. There are no microservices or separate processes. The optional MCP server runs in-process via tokio.

### 1.1 Module Catalog

| Module | Path | Responsibility | Public Interface |
|--------|------|---------------|------------------|
| **model** | `src/model/` | Core data types (leaf module, no internal deps) | `ProjectGraph`, `Node`, `Edge`, `Cluster`, `ClusterMap`, `CanonicalPath`, `ContentHash`, `ClusterId`, `Symbol`, `FileSet`, `FileType`, `ArchLayer`, `EdgeType`, `SubgraphResult`, `ArchSmell`, `SmellType`, `SmellSeverity`, `SmellMetrics`, `StatsOutput`, `StatsSummary`, `StructuralDiff`, `DiffSummary`, `ChangeClassification`, `LayerChange`, `ClusterChange`, `CompressedGraph`, `CompressedNode`, `CompressedEdge`, `CompressionLevel`, `WorkspaceInfo`, `WorkspaceKind`, `WorkspaceMember` |
| **parser** | `src/parser/` | Language-specific AST extraction via tree-sitter | Traits: `LanguageParser`, `ImportResolver`; Structs: `ParserRegistry`, `ParseOutcome`, `RawImport`, `RawExport`, `ImportKind` |
| **pipeline** | `src/pipeline/` | Build orchestration (walk→read→parse→resolve→cluster→serialize) | `BuildPipeline`, `BuildOutput`, `ParsedFile`, `WalkConfig`, `FileWalker` trait, `FsWalker`, `FileReader` trait, `FsReader`, `FileContent`, `FileEntry`, `WalkResult` |
| **detect** | `src/detect/` | File type classification, arch layer inference, workspace detection, case sensitivity | `detect_file_type`, `infer_arch_layer`, `detect_workspace`, `is_case_insensitive`, `find_case_insensitive` |
| **cluster** | `src/cluster/` | Directory-based file clustering with cohesion metrics | `assign_clusters(graph) -> ClusterMap` |
| **algo** | `src/algo/` | Graph algorithms (SCC, topo sort, centrality, blast radius, subgraph, Louvain, PageRank, spectral, compression, delta, stats) | 11 submodules, each exposing algorithm functions |
| **analysis** | `src/analysis/` | Higher-level analysis (arch smell detection, structural diff, Martin metrics) | `detect_smells`, `compute_structural_diff`, `compute_martin_metrics`, `ClusterMetrics`, `MetricZone` |
| **serial** | `src/serial/` | Serialization/deserialization of graph artifacts (JSON) | Traits: `GraphSerializer`, `GraphReader`; Structs: `GraphOutput`, `NodeOutput`, `ClusterOutput`, `ClusterEntryOutput`, `FileQueryOutput`, `RawImportOutput`, `JsonSerializer`; Conversions: `GraphOutput → ProjectGraph`, `ClusterOutput → ClusterMap` |
| **views** | `src/views/` | Markdown view generation (L0 index, L1 cluster, L2 impact) | `generate_all_views`, `generate_index`, `generate_cluster_view`, `generate_impact_view` |
| **diagnostic** | `src/diagnostic.rs` | Error taxonomy and warning collection | `FatalError` (E001-E013), `WarningCode` (W001-W018), `Warning`, `DiagnosticCollector`, `DiagnosticReport`, `DiagnosticCounts`, `format_warnings`, `format_summary` |
| **hash** | `src/hash.rs` | Content hashing | `hash_content(bytes) -> ContentHash` (xxHash64, 16-char hex) |
| **mcp** | `src/mcp/` | MCP server for real-time graph queries (feature-gated: `serve`) | `ServeConfig`, `run()`, `AriadneTools` (ServerHandler), `GraphState`, `FreshnessState`, `FileWatcher`, `acquire_lock`/`release_lock` |
| **main** | `src/main.rs` | CLI composition root (clap), wires concrete types | Not a library module; binary entry point |

### 1.2 Submodule Breakdown

**model/** (10 files):
- `types.rs` — `CanonicalPath`, `ContentHash`, `ClusterId`, `Symbol`, `FileSet`
- `node.rs` — `Node`, `FileType`, `ArchLayer`
- `edge.rs` — `Edge`, `EdgeType`
- `graph.rs` — `ProjectGraph`, `Cluster`, `ClusterMap`
- `query.rs` — `SubgraphResult`
- `smell.rs` — `ArchSmell`, `SmellType`, `SmellSeverity`, `SmellMetrics`
- `stats.rs` — `StatsOutput`, `StatsSummary`
- `diff.rs` — `StructuralDiff`, `LayerChange`, `ClusterChange`, `DiffSummary`, `ChangeClassification`
- `compress.rs` — `CompressedGraph`, `CompressedNode`, `CompressedEdge`, `CompressionLevel`, `CompressedNodeType`
- `workspace.rs` — `WorkspaceInfo`, `WorkspaceMember`, `WorkspaceKind`

**parser/** (8 files):
- `traits.rs` — `LanguageParser` trait, `ImportResolver` trait, `RawImport`, `RawExport`, `ImportKind`
- `registry.rs` — `ParserRegistry`, `ParseOutcome`
- `typescript.rs` — `TypeScriptParser`, `TypeScriptResolver` (handles .ts, .tsx, .js, .jsx)
- `python.rs` — `PythonParser`, `PythonResolver`
- `rust_lang.rs` — `RustParser`, `RustResolver`
- `go.rs` — Go parser/resolver (factory functions)
- `csharp.rs` — C# parser/resolver (factory functions)
- `java.rs` — Java parser/resolver (factory functions)

**pipeline/** (4 files):
- `walk.rs` — `FileWalker` trait, `FsWalker`, `WalkConfig`, `FileEntry`, `WalkResult`
- `read.rs` — `FileReader` trait, `FsReader`, `FileContent`, `FileSkipReason`
- `resolve.rs` — `resolve_import()` function
- `build.rs` — `resolve_and_build()` function

**algo/** (11 files):
- `scc.rs` — Tarjan's SCC detection
- `topo_sort.rs` — Topological layering
- `centrality.rs` — Brandes betweenness centrality
- `blast_radius.rs` — Reverse BFS impact analysis
- `subgraph.rs` — N-hop subgraph extraction
- `louvain.rs` — Louvain community detection (refines directory clusters)
- `pagerank.rs` — PageRank + combined importance scoring
- `spectral.rs` — Spectral analysis (algebraic connectivity, Fiedler bisection, monolith score)
- `compress.rs` — Hierarchical graph compression (L0/L1/L2)
- `delta.rs` — Content-hash delta computation for incremental updates
- `stats.rs` — Graph statistics aggregation

**analysis/** (3 files):
- `smells.rs` — Architectural smell detection (7 types: GodFile, CircularDependency, LayerViolation, HubAndSpoke, UnstableFoundation, DeadCluster, ShotgunSurgery)
- `diff.rs` — Structural diff between graph snapshots
- `metrics.rs` — Martin metrics (Instability, Abstractness, Distance from Main Sequence)

**mcp/** (5 files):
- `server.rs` — MCP server lifecycle (`run()`)
- `tools.rs` — MCP tool definitions (`AriadneTools` with `ServerHandler`)
- `state.rs` — `GraphState` (in-memory state with precomputed indices), `FreshnessState`, `load_graph_state()`
- `watch.rs` — File system watcher (notify + debouncer), triggers rebuild on changes
- `lock.rs` — PID-based lock file management

**serial/** (3 files):
- `mod.rs` — Trait definitions (`GraphSerializer`, `GraphReader`), output models
- `json.rs` — `JsonSerializer` (implements both traits), atomic writes
- `convert.rs` — `TryFrom<GraphOutput> for ProjectGraph`, `TryFrom<ClusterOutput> for ClusterMap`

**views/** (3 files):
- `index.rs` — L0 project index markdown
- `cluster.rs` — L1 per-cluster view markdown
- `impact.rs` — L2 impact view markdown

**detect/** (4 files):
- `filetype.rs` — `detect_file_type(path) -> FileType`
- `layer.rs` — `infer_arch_layer(path) -> ArchLayer`
- `workspace.rs` — `detect_workspace(root) -> Option<WorkspaceInfo>` (npm/yarn/pnpm monorepo)
- `case_sensitivity.rs` — FS case sensitivity detection, case-insensitive path lookup

---

## 2. Dependency Graph — Internal Module Dependencies

Source of truth: `use` statements observed in module files.

### 2.1 Dependency Matrix (A → B means A imports from B)

```
main.rs → algo, diagnostic, model, parser, pipeline, serial, views (via ariadne_graph::*)
         + mcp (feature-gated)

pipeline/mod.rs → algo, cluster, detect, diagnostic, model, parser, serial
pipeline/build.rs → detect, diagnostic, model, parser, pipeline/read, pipeline/resolve
pipeline/resolve.rs → detect, diagnostic, model, parser
pipeline/read.rs → hash, model
pipeline/walk.rs → diagnostic, model

algo/mod.rs → model
algo/* → model (individual algorithms reference model types)

analysis/smells.rs → algo, analysis/metrics, model
analysis/diff.rs → algo, analysis/metrics, analysis/smells, model
analysis/metrics.rs → algo, model

cluster/mod.rs → model

detect/mod.rs → (re-exports submodules)

serial/mod.rs → diagnostic, model
serial/json.rs → diagnostic, model, serial/mod
serial/convert.rs → model, serial/mod

views/mod.rs → diagnostic, model

diagnostic.rs → model (CanonicalPath only)

hash.rs → model (ContentHash only)

mcp/server.rs → diagnostic, mcp/lock, mcp/state, mcp/tools, mcp/watch, parser, pipeline, serial
mcp/tools.rs → algo, analysis/smells, mcp/state, model
mcp/state.rs → algo (compress, pagerank, spectral), analysis/metrics, diagnostic, model, serial
mcp/watch.rs → analysis/diff, diagnostic, mcp/state, pipeline, serial
mcp/lock.rs → diagnostic
```

### 2.2 Dependency Layers (bottom-up)

```
Layer 0 (leaf):     model
Layer 1:            hash, diagnostic
Layer 2:            detect, serial, cluster, algo
Layer 3:            analysis, views, parser
Layer 4:            pipeline
Layer 5:            mcp
Layer 6:            main.rs (composition root)
```

### 2.3 Key Trait Boundaries

| Trait | Defined in | Implementations | Consumers |
|-------|-----------|----------------|-----------|
| `LanguageParser` | `parser/traits.rs` | `TypeScriptParser`, `PythonParser`, `RustParser`, Go/C#/Java parsers | `ParserRegistry` |
| `ImportResolver` | `parser/traits.rs` | `TypeScriptResolver`, `PythonResolver`, `RustResolver`, Go/C#/Java resolvers | `pipeline/resolve.rs` |
| `FileWalker` | `pipeline/walk.rs` | `FsWalker` | `BuildPipeline` |
| `FileReader` | `pipeline/read.rs` | `FsReader` | `BuildPipeline` |
| `GraphSerializer` | `serial/mod.rs` | `JsonSerializer` | `BuildPipeline` |
| `GraphReader` | `serial/mod.rs` | `JsonSerializer` | `BuildPipeline::update()`, `mcp/state.rs` |
| `ServerHandler` (rmcp) | rmcp crate | `AriadneTools` | MCP server runtime |

---

## 3. Data Flow Paths

### 3.1 Build Pipeline (primary flow)

Observed in `src/pipeline/mod.rs` lines 80-392:

```
[Filesystem]
    |
    v
Stage 1: Walk (FsWalker)
    → FileEntry[] (path, extension)
    |
    v
Stage 2: Read (FsReader)
    → FileContent[] (path, bytes, hash, lines)
    |  Skipped files → Warning (W001-W004, W009)
    |
    v
Stage 3: Parse (parallel via rayon, sorted input)
    → ParsedFile[] (path, imports[], exports[])
    |  ParseOutcome: Ok | Partial(W007) | Failed(W001)
    |
    v
Stage 4: Resolve + Build Graph
    → ProjectGraph { nodes: BTreeMap<CanonicalPath, Node>, edges: Vec<Edge> }
    |  Workspace detection (npm/yarn/pnpm)
    |  Case-insensitive fallback
    |  Unresolved imports → W006
    |
    v
Stage 5: Cluster (directory-based)
    → ClusterMap { clusters: BTreeMap<ClusterId, Cluster> }
    |
    v
Stage 5b: Louvain (optional refinement)
    → Refined ClusterMap (community detection)
    |
    v
Stage 6: Algorithms
    → SCC detection, topological layering (arch_depth), betweenness centrality
    → StatsOutput
    |
    v
Stage 7: Convert to output model
    → GraphOutput, ClusterOutput
    |
    v
Stage 8: Serialize (atomic JSON writes)
    → .ariadne/graph/graph.json
    → .ariadne/graph/clusters.json
    → .ariadne/graph/stats.json
    → .ariadne/graph/raw_imports.json
```

### 3.2 Incremental Update (delta path)

Observed in `src/pipeline/mod.rs` lines 394-533:

```
Load existing graph.json → ProjectGraph
    |
    v
Walk + Read current files → current content hashes
    |
    v
compute_delta(old_nodes, current_hashes)
    → Delta { changed, added, removed, requires_full_recompute }
    |
    ├─ No changes → return cached BuildOutput (short-circuit)
    ├─ >5% threshold → full rebuild
    └─ Changes detected → full rebuild (correctness over optimization)
```

### 3.3 MCP Server Data Flow

Observed in `src/mcp/server.rs` and `src/mcp/state.rs`:

```
ariadne serve
    |
    v
acquire_lock (.ariadne/graph/.lock)
    |
    v
Load or build graph → GraphState
    |  GraphState contains:
    |    - ProjectGraph + StatsOutput + ClusterMap
    |    - Precomputed indices (forward/reverse/layer)
    |    - file_hashes, raw_imports
    |    - Martin metrics, PageRank, combined importance
    |    - Compressed L0 graph, spectral analysis
    |    - FreshnessState
    |
    v
Arc<ArcSwap<GraphState>> (shared, atomically swappable)
    |
    ├─ FileWatcher (notify + debouncer)
    |    → On file change: rebuild → swap GraphState
    |    → Computes StructuralDiff on each update
    |
    └─ AriadneTools (MCP tool handler, stdin/stdout transport)
         → Reads from ArcSwap<GraphState> on each tool call
```

### 3.4 Query Data Flow (CLI)

```
ariadne query <subcommand>
    |
    v
JsonSerializer.read_graph/clusters/stats(.ariadne/graph/)
    |
    v
Convert to ProjectGraph + ClusterMap
    |
    v
Run requested algorithm (centrality, blast_radius, subgraph, etc.)
    |
    v
Format output (markdown or JSON) → stdout
```

### 3.5 Views Generation

```
ariadne views generate
    |
    v
Load graph + clusters + stats from .ariadne/graph/
    |
    v
generate_all_views()
    ├─ L0: index.md (project overview)
    ├─ L1: clusters/<name>.md (per-cluster detail)
    └─ Written to .ariadne/views/
```

---

## 4. External Integrations

### 4.1 Third-Party Crate Dependencies

Observed in `Cargo.toml`:

| Crate | Version | Purpose |
|-------|---------|---------|
| `clap` | 4 (derive) | CLI argument parsing |
| `tree-sitter` | 0.24 | Incremental parsing framework |
| `tree-sitter-typescript` | 0.23 | TS/JS grammar |
| `tree-sitter-javascript` | 0.23 | JS grammar |
| `tree-sitter-go` | 0.23 | Go grammar |
| `tree-sitter-python` | 0.23 | Python grammar |
| `tree-sitter-rust` | 0.23 | Rust grammar |
| `tree-sitter-c-sharp` | 0.23 | C# grammar |
| `tree-sitter-java` | 0.23 | Java grammar |
| `serde` / `serde_json` | 1 | JSON serialization |
| `xxhash-rust` | 0.8 (xxh64) | Content hashing |
| `ignore` | 0.4 | .gitignore-aware file walking |
| `rayon` | 1 | Parallel parsing |
| `thiserror` | 2 | Error derive macros |
| `dunce` | 1 | Path canonicalization (Windows UNC) |
| `time` | 0.3 | UTC timestamp formatting |
| `glob` | 0.3 | Glob pattern matching |
| `rmcp` | 1.2 (optional) | MCP protocol server |
| `schemars` | 1 (optional) | JSON Schema for MCP tools |
| `tokio` | 1 (optional) | Async runtime for MCP server |
| `tokio-util` | 0.7 (optional) | Cancellation tokens |
| `arc-swap` | 1 (optional) | Lock-free atomic pointer swap |
| `notify` | 8 (optional) | File system watching |
| `notify-debouncer-full` | 0.7 (optional) | Debounced FS events |

### 4.2 External Services

**None.** Ariadne is entirely offline. No network calls, no external APIs, no databases. The MCP server communicates via stdin/stdout (local transport only).

### 4.3 Feature Gates

| Feature | Default | Dependencies Enabled |
|---------|---------|---------------------|
| `serve` | Yes | `rmcp`, `tokio`, `tokio-util`, `arc-swap`, `notify`, `notify-debouncer-full`, `schemars` |

---

## 5. API Contracts

### 5.1 CLI Commands (clap-derived)

Observed in `src/main.rs`:

| Command | Arguments | Output |
|---------|-----------|--------|
| `ariadne build <path>` | `--output`, `--verbose`, `--warnings {human,json}`, `--strict`, `--timestamp`, `--max-file-size`, `--max-files`, `--no-louvain` | graph.json, clusters.json, stats.json, raw_imports.json → `.ariadne/graph/` |
| `ariadne update <path>` | `--output`, `--verbose`, `--warnings`, `--strict`, `--timestamp`, `--max-file-size`, `--max-files`, `--no-louvain` | Same as build (delta-aware) |
| `ariadne info` | (none) | Version + supported languages to stdout |
| `ariadne query blast-radius <file>` | `--depth`, `--format {md,json}`, `--graph-dir` | Affected files with distances |
| `ariadne query subgraph <files...>` | `--depth`, `--format`, `--graph-dir` | N-hop neighborhood graph |
| `ariadne query stats` | `--format`, `--graph-dir` | Project statistics |
| `ariadne query centrality` | `--min`, `--format`, `--graph-dir` | Betweenness centrality scores |
| `ariadne query cluster <name>` | `--format`, `--graph-dir` | Cluster details |
| `ariadne query file <path>` | `--format`, `--graph-dir` | File details + edges |
| `ariadne query cycles` | `--format`, `--graph-dir` | SCCs (circular deps) |
| `ariadne query layers` | `--format`, `--graph-dir` | Topological layers |
| `ariadne query metrics` | `--format`, `--graph-dir` | Martin metrics per cluster |
| `ariadne query smells` | `--min-severity`, `--format`, `--graph-dir` | Architectural anti-patterns |
| `ariadne query importance` | `--top`, `--format`, `--graph-dir` | Combined centrality+PageRank ranking |
| `ariadne query spectral` | `--format`, `--graph-dir` | Algebraic connectivity, monolith score, Fiedler bisection |
| `ariadne query compressed` | `--level {0,1,2}`, `--focus`, `--depth`, `--format`, `--graph-dir` | Hierarchical compressed graph |
| `ariadne views generate` | `--output`, `--graph-dir` | Markdown views → `.ariadne/views/` |
| `ariadne serve` | `--project`, `--output`, `--debounce`, `--no-watch` | MCP server on stdin/stdout |

### 5.2 MCP Tool Contracts (ariadne serve)

Observed in `src/mcp/tools.rs`:

| Tool Name | Parameters | Returns |
|-----------|-----------|---------|
| `ariadne_overview` | (none) | JSON: node/edge/cluster counts, language breakdown, layer distribution, max depth, bottleneck files, cycle count, freshness |
| `ariadne_file` | `path: String` | JSON: type, layer, arch_depth, lines, hash, exports, cluster, centrality, incoming/outgoing edges |
| `ariadne_blast_radius` | `path: String`, `depth: Option<u32>` | JSON: map of affected file paths to BFS distances |
| `ariadne_subgraph` | `paths: Vec<String>`, `depth: Option<u32>` | JSON: nodes, edges, center_files, depth |
| `ariadne_centrality` | `min: Option<f64>` | JSON: sorted array of {path, centrality} |
| `ariadne_cycles` | (none) | JSON: array of SCC arrays |
| `ariadne_layers` | `layer: Option<u32>` | JSON: layers map or filtered layer |
| `ariadne_cluster` | `name: String` | JSON: files, file_count, internal/external edges, cohesion |
| `ariadne_dependencies` | `path: String`, `direction: String` | JSON: incoming/outgoing edges |
| `ariadne_freshness` | (none) | JSON: confidence scores, stale/changed/new/removed files, rebuilding flag |
| `ariadne_metrics` | (none) | JSON: Martin metrics per cluster |
| `ariadne_smells` | `min_severity: Option<String>` | JSON: detected architectural smells |
| `ariadne_importance` | `top: Option<u32>` | JSON: ranked file importance |
| `ariadne_compressed` | `level: u32`, `focus: Option<String>`, `depth: Option<u32>` | JSON: compressed graph at requested level |
| `ariadne_spectral` | (none) | JSON: spectral analysis results |
| `ariadne_diff` | (none) | JSON: last structural diff |

### 5.3 Serialized Output Schema (graph.json)

Observed in `src/serial/mod.rs`:

```
GraphOutput {
    version: u32,              // Always 1
    project_root: String,
    node_count: usize,
    edge_count: usize,
    nodes: BTreeMap<String, NodeOutput>,
    edges: Vec<(from, to, edge_type, symbols)>,
    generated: Option<String>, // ISO 8601 UTC timestamp
}

NodeOutput {
    type: String,       // "source"|"test"|"config"|"style"|"asset"|"type_def"
    layer: String,      // "api"|"service"|"data"|"util"|"component"|"hook"|"config"|"unknown"
    arch_depth: u32,
    lines: u32,
    hash: String,       // 16-char hex (xxHash64)
    exports: Vec<String>,
    cluster: String,
}
```

### 5.4 Error Taxonomy

**Fatal Errors (exit code 1):** E001-E013
- E001: Project root not found
- E002: Not a directory
- E003: Cannot write output
- E004: No parseable files
- E005: Walk failed
- E006: Graph not found
- E007: Stats not found
- E008: Corrupted file
- E009: File not in graph
- E010: MCP server failed
- E011: Lock file held
- E012: MCP protocol error
- E013: Invalid argument

**Warnings (recoverable):** W001-W018
- W001: Parse failed, W002: Read failed, W003: File too large, W004: Binary file
- W005: Max files reached, W006: Import unresolved, W007: Partial parse
- W008: Config parse failed, W009: Encoding error
- W010: Graph version mismatch, W011: Graph corrupted, W012: Algorithm failed
- W013: Stale stats, W014: FS watcher failed, W015: Incremental rebuild failed
- W016: Stale lock removed, W017: Smell detection skipped, W018: Blast radius timeout

---

## 6. Supported Languages

Registered in `ParserRegistry::with_tier1()` (src/parser/registry.rs):

| Language | Extensions | Parser | Resolver |
|----------|-----------|--------|----------|
| TypeScript/JavaScript | .ts, .tsx, .js, .jsx | `TypeScriptParser` | `TypeScriptResolver` |
| Python | .py | `PythonParser` | `PythonResolver` |
| Rust | .rs | `RustParser` | `RustResolver` |
| Go | .go | Go parser (factory) | Go resolver (factory) |
| C# | .cs | C# parser (factory) | C# resolver (factory) |
| Java | .java | Java parser (factory) | Java resolver (factory) |

---

## 7. Concurrency Model

- **Parse stage:** Parallel via `rayon::par_iter()` over sorted file list (deterministic)
- **MCP server:** `tokio` async runtime (multi-thread), `Arc<ArcSwap<GraphState>>` for lock-free reads
- **File watcher:** `notify` + debouncer on separate thread, triggers rebuild which swaps `GraphState`
- **DiagnosticCollector:** `Mutex<(Vec<Warning>, DiagnosticCounts)>` for thread-safe warning collection
- **Determinism:** BTreeMap/BTreeSet used throughout; files sorted before parallel processing; floats rounded to 4 decimal places

---

## 8. Output Artifacts

All written to `.ariadne/graph/` by default:

| File | Written by | Read by |
|------|-----------|---------|
| `graph.json` | `JsonSerializer::write_graph` | `JsonSerializer::read_graph` |
| `clusters.json` | `JsonSerializer::write_clusters` | `JsonSerializer::read_clusters` |
| `stats.json` | `JsonSerializer::write_stats` | `JsonSerializer::read_stats` |
| `raw_imports.json` | `JsonSerializer::write_raw_imports` | `JsonSerializer::read_raw_imports` |
| `.lock` | `mcp/lock.rs` | `mcp/lock.rs` (PID-based, stale detection) |

Markdown views written to `.ariadne/views/`:
- `index.md` (L0)
- `clusters/<name>.md` (L1, one per cluster)

---

## 9. Test Infrastructure

Observed in `tests/`:

| File | Scope |
|------|-------|
| `tests/pipeline_tests.rs` | Integration tests for build pipeline |
| `tests/graph_tests.rs` | Graph algorithm integration tests |
| `tests/mcp_tests.rs` | MCP server integration tests |
| `tests/invariants.rs` | Structural invariant checks |
| `tests/properties.rs` | Property-based tests (proptest) |
| `tests/helpers.rs` | Shared test utilities |
| `tests/fixtures/` | 9 language fixture projects (typescript-app, python-package, rust-crate, go-service, java-project, csharp-project, mixed-project, edge-cases, workspace-project) |

Dev dependencies: `insta` (snapshot testing with YAML), `tempfile`, `proptest`, `criterion` (benchmarks).

Benchmarks in `benches/`: `build_bench`, `parser_bench`, `algo_bench`, `mcp_bench`, `analysis_bench`.
