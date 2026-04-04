# Ariadne Architecture — Deep Scan
Generated: 2026-04-04 (Hermes deep scan)

## 1. Project Identity

- **Crate name**: `ariadne-graph` (`Cargo.toml` line 2)
- **Binary name**: `ariadne` (`Cargo.toml` line 9)
- **Edition**: Rust 2021
- **License**: MIT OR Apache-2.0
- **Description**: Structural dependency graph engine for source code

## 2. Module Map & Responsibilities

All modules declared in `src/lib.rs`:

| Module | Responsibility | Feature-gated |
|--------|---------------|---------------|
| `model/` | Core data types: `ProjectGraph`, `Node`, `Edge`, `CanonicalPath`, `FileSet`, `ClusterId`, `ContentHash`, `Symbol`, `SymbolDef`, `SymbolIndex`, `Boundary`, `TemporalState`, `Annotation`, `Bookmark`, `CompressedGraph`, `StructuralDiff`, `ArchSmell`, `StatsOutput` | No |
| `parser/` | Tree-sitter parsing: `LanguageParser` + `ImportResolver` traits, `ParserRegistry`, per-language impls (TS, JS, Go, Python, Rust, C#, Java, Markdown, JSON, YAML), `SymbolExtractor`, `BoundaryExtractor` registration | No |
| `parser/config/` | Config-aware import resolution: `tsconfig.json`, `go.mod`, `pyproject.toml` parsing; `ProjectConfig` discovery (Phase 10) | No |
| `pipeline/` | Build orchestration: `BuildPipeline` (walk -> read -> parse -> resolve -> cluster -> serialize), `FileWalker`/`FsWalker`, `FileReader`/`FsReader`, `WalkConfig`, incremental `update()` via delta | No |
| `detect/` | File classification: `detect_file_type`, `infer_arch_layer`, `detect_fsd_project`, `detect_workspace`, `detect_rust_crate_name`, `is_case_insensitive` | No |
| `cluster/` | Directory-based clustering: `assign_clusters()` groups files by first path segment, computes cohesion metrics | No |
| `algo/` | Graph algorithms: SCC (`scc`), BFS blast radius (`blast_radius`), betweenness centrality (`centrality`), PageRank (`pagerank`), topological sort (`topo_sort`), subgraph extraction (`subgraph`), Louvain clustering (`louvain`), spectral analysis (`spectral`), graph compression (`compress`), delta computation (`delta`), call graph (`callgraph`), context assembly (`context`), impact analysis (`impact`), reading order (`reading_order`), test mapping (`test_map`), stats (`stats`) | No |
| `analysis/` | Architectural intelligence: Martin metrics (`metrics`), smell detection (`smells`), structural diff (`diff`) | No |
| `views/` | Markdown generation: L0 index, L1 cluster views, L2 impact views | No |
| `serial/` | Serialization: `GraphSerializer` + `GraphReader` traits, `GraphOutput`/`ClusterOutput`/`StatsOutput`/`BoundaryOutput` output models, JSON implementation (`json.rs`), `convert.rs` for `GraphOutput` <-> `ProjectGraph` | No |
| `semantic/` | Boundary extraction: HTTP routes, event channels; `BoundaryExtractor` trait, `ExtractorRegistry`, `SemanticState` analysis | No |
| `temporal/` | Git history analysis: churn metrics (`churn`), co-change coupling (`coupling`), file ownership (`ownership`), hotspot detection (`hotspot`), git log parsing (`git`) | No |
| `recommend/` | Recommendation engine (Phase 9): `suggest_placement`, `analyze_split` (Stoer-Wagner min-cut), `find_refactor_opportunities` (Pareto frontier), types (`SplitAnalysis`, `PlacementSuggestion`, `RefactorAnalysis`) | No |
| `mcp/` | MCP server: `rmcp`-based server with tools, resources, prompts; file watcher (`watch`), shared state (`state`), annotations/bookmarks (`user_state`), process locking (`lock`) | Yes (`serve` feature) |
| `diagnostic.rs` | Error/warning system: `FatalError` (E001-E014), `WarningCode` (W001-W033), `DiagnosticCollector` | No |
| `hash.rs` | Content hashing: xxHash64 -> `ContentHash` | No |

## 3. Dependency Layering (Observed from imports)

Layer structure (leaf to root, based on topological analysis from pre-context):

```
Layer 0  (leaf):     model types, case_sensitivity, symbol_index, parser/symbols, semantic/edges, temporal/hotspot
Layer 1:             model/mod, semantic/events, semantic/http
Layer 2-3:           algo sub-modules, cluster, detect/filetype, detect/layer, serial/mod
Layer 4:             algo/mod (re-exports all sub-algorithms)
Layer 5:             parser/mod, parser/config, detect/mod, analysis/metrics, serial/convert
Layer 6:             pipeline/build, pipeline/resolve, analysis/smells, mcp/state, recommend/refactor
Layer 7:             pipeline/mod, analysis/diff, mcp/prompts, mcp/resources, recommend/mod
Layer 8:             analysis/mod, mcp/tools, mcp/watch
Layer 9:             mcp/server
Layer 10:            mcp/mod
Layer 11:            lib.rs (top-level re-export)
```

**Key observation**: `model/mod.rs` has the highest centrality (0.0106) — it is the universal dependency for all other modules. `algo/mod.rs` is second (0.0037).

## 4. Service Boundaries & Interfaces

### 4.1 Core Trait Boundaries

**Parser Traits** (`src/parser/traits.rs`):
- `LanguageParser` — extracts imports/exports from AST via tree-sitter. Methods: `language()`, `extensions()`, `tree_sitter_language()`, `extract_imports()`, `extract_exports()`.
- `ImportResolver` — resolves raw import paths to canonical file paths. Method: `resolve()`.
- `SymbolExtractor` (from `src/parser/symbols.rs`) — extracts symbol definitions from parsed trees.
- `BoundaryExtractor` (`src/semantic/mod.rs`) — extracts HTTP routes, event channels from parsed trees.

**Serialization Traits** (`src/serial/mod.rs`):
- `GraphSerializer` — writes graph/clusters/stats/raw_imports/boundaries to disk. 5 methods.
- `GraphReader` — reads graph/clusters/stats/raw_imports/boundaries from disk. 5 methods.

**Pipeline Traits** (`src/pipeline/walk.rs`, `src/pipeline/read.rs`):
- `FileWalker` — walks project directory, returns `WalkResult`.
- `FileReader` — reads files, returns `FileContent` or `FileSkipReason`.

### 4.2 Composition Root (`src/main.rs`)

The `main()` function wires concrete implementations:
- `FsWalker` implements `FileWalker`
- `FsReader` implements `FileReader`
- `ParserRegistry` holds all language parsers/resolvers
- `JsonSerializer` implements both `GraphSerializer` and `GraphReader`
- `BuildPipeline::new()` accepts boxed trait objects (D-020 decision)

### 4.3 MCP Server Boundary

The MCP server (`src/mcp/server.rs`) is feature-gated behind `serve` (default-on). It:
- Takes an `Arc<BuildPipeline>` for rebuilds
- Holds shared state via `Arc<ArcSwap<GraphState>>`
- Uses `Arc<AtomicBool>` for rebuild coordination
- Starts a `FileWatcher` with debounced filesystem monitoring
- Exposes `AriadneTools` which implements `ServerHandler` from `rmcp`

## 5. Data Flow

### 5.1 Build Pipeline (main data path)

```
Walk (FsWalker)
  -> FileEntry[] (path, extension, size)
  -> Config Discovery (tsconfig.json, go.mod, pyproject.toml)
  -> ParserRegistry::with_project_config()

Read (FsReader)
  -> FileContent[] (path, bytes, hash, lines)

Parse (parallel via rayon)
  -> ParsedFile[] (path, imports, exports, symbols, boundaries)

Resolve (build::resolve_and_build)
  -> ProjectGraph { nodes: BTreeMap<CanonicalPath, Node>, edges: Vec<Edge> }

Cluster (cluster::assign_clusters)
  -> ClusterMap { clusters: BTreeMap<ClusterId, Cluster> }

Louvain (optional refinement)
  -> ClusterMap (refined)

Algorithms (SCC, topo sort, centrality)
  -> Layers applied to nodes (arch_depth), stats computed

Serialize (JsonSerializer)
  -> graph.json, clusters.json, stats.json, raw_imports.json, boundaries.json
```

### 5.2 Query Data Path (CLI)

```
CLI command (Query subcommand)
  -> JsonSerializer.read_graph() / read_clusters() / read_stats()
  -> GraphOutput -> ProjectGraph (via convert.rs)
  -> algo::* functions compute result
  -> Format as markdown or JSON
  -> stdout
```

### 5.3 MCP Server Data Path

```
Startup:
  load_graph_state() -> GraphState (precomputed indices, PageRank, spectral, call graph, symbol index)

Tool invocation:
  state.load() (ArcSwap read) -> &GraphState
  -> algo/analysis function
  -> format as text
  -> return via rmcp tool result

File change:
  FileWatcher detects change -> debounce -> pipeline.run_with_options()
  -> load_graph_state() -> state.store() (ArcSwap swap)
  -> compute_structural_diff() -> state.last_diff
```

### 5.4 Incremental Update Path

```
pipeline.update()
  -> Load existing graph (read_graph)
  -> Walk + Read current files for hashes
  -> compute_delta() -> Delta { changed, added, removed }
  -> If no changes: short-circuit (return existing BuildOutput)
  -> If any changes: full rebuild (correctness over optimization)
```

## 6. External Integrations

### 6.1 Tree-sitter Grammars (Cargo.toml)
- `tree-sitter` 0.24 (core)
- `tree-sitter-typescript` 0.23 (TS/TSX/JS/JSX)
- `tree-sitter-go` 0.23
- `tree-sitter-python` 0.23
- `tree-sitter-rust` 0.23
- `tree-sitter-c-sharp` 0.23
- `tree-sitter-java` 0.23
- `tree-sitter-md` 0.3 (Markdown)
- `tree-sitter-json` 0.24
- `tree-sitter-yaml` 0.7

### 6.2 MCP Protocol
- `rmcp` 1.2 (MCP server framework, stdio transport)
- `schemars` 1 (JSON Schema generation for tool parameters)

### 6.3 Async Runtime
- `tokio` 1 (rt-multi-thread, macros, signal, sync, time) — MCP server only

### 6.4 Filesystem
- `ignore` 0.4 (gitignore-aware file walking)
- `notify` 8 + `notify-debouncer-full` 0.7 (file system watching)
- `dunce` 1 (Windows path canonicalization)

### 6.5 Other
- `rayon` 1 (parallel parsing in build pipeline)
- `serde` + `serde_json` (all serialization)
- `xxhash-rust` 0.8 (content hashing)
- `thiserror` 2 (error derive)
- `time` 0.3 (UTC timestamp formatting)
- `glob` 0.3 (pattern matching)
- `arc-swap` 1 (lock-free state swapping in MCP server)

### 6.6 Git (Process Execution)
- `temporal/git.rs` shells out to `git log` for churn/coupling analysis
- No git library dependency; uses `std::process::Command`

## 7. API Contracts

### 7.1 CLI Commands (`src/main.rs`)

| Command | Purpose |
|---------|---------|
| `ariadne build <path>` | Parse project, build graph, write JSON artifacts |
| `ariadne update <path>` | Incremental update via delta detection |
| `ariadne info` | Show version and supported languages |
| `ariadne serve` | Start MCP server (stdio transport) |
| `ariadne restart` | Stop running MCP server (SIGTERM) |
| `ariadne query blast-radius <file>` | Show blast radius for a file |
| `ariadne query subgraph <files...>` | Extract subgraph around files |
| `ariadne query stats` | Show project statistics |
| `ariadne query centrality` | Show betweenness centrality scores |
| `ariadne query cluster <name>` | Show cluster details |
| `ariadne query file <path>` | Show file details |
| `ariadne query cycles` | Show circular dependencies |
| `ariadne query layers` | Show topological layers |
| `ariadne query metrics` | Martin metrics per cluster |
| `ariadne query smells` | Detect architectural smells |
| `ariadne query importance` | File importance ranking (centrality + PageRank) |
| `ariadne query spectral` | Spectral analysis (algebraic connectivity, monolith score) |
| `ariadne query churn` | Git churn statistics |
| `ariadne query coupling` | Co-change coupling pairs |
| `ariadne query hotspots` | High churn x size x blast radius |
| `ariadne query ownership` | File ownership from git |
| `ariadne query hidden-deps` | Co-changed but no structural link |
| `ariadne query compressed` | Compressed graph at project/cluster/file level |
| `ariadne query boundaries` | Semantic boundaries (HTTP routes, events) |
| `ariadne query routes` | HTTP routes with handlers/consumers |
| `ariadne query events` | Event channels with producers/consumers |
| `ariadne query boundary-for <path>` | Boundaries in a specific file |
| `ariadne views generate` | Generate L0 index + L1 cluster markdown views |

### 7.2 MCP Tools (from `src/mcp/tools.rs` and sub-files)

**Core graph tools**: `ariadne_overview`, `ariadne_file`, `ariadne_dependencies`, `ariadne_blast_radius`, `ariadne_subgraph`, `ariadne_centrality`, `ariadne_layers`, `ariadne_cluster`, `ariadne_cycles`, `ariadne_metrics`, `ariadne_smells`, `ariadne_importance`, `ariadne_spectral`, `ariadne_compressed`, `ariadne_diff`, `ariadne_freshness`, `ariadne_views_export`

**Symbol tools**: `ariadne_symbols`, `ariadne_symbol_search`, `ariadne_symbol_blast_radius`, `ariadne_callers`, `ariadne_callees`

**Context tools** (from `tools_context.rs`): `ariadne_context`, `ariadne_tests_for`, `ariadne_reading_order`, `ariadne_plan_impact`

**Temporal tools** (from `tools_temporal.rs`): `ariadne_churn`, `ariadne_coupling`, `ariadne_hotspots`, `ariadne_ownership`, `ariadne_hidden_deps`

**Semantic tools** (from `tools_semantic.rs`): `ariadne_boundaries`, `ariadne_boundary_for`, `ariadne_route_map`, `ariadne_event_map`

**Recommendation tools** (from `tools_recommend.rs`): `ariadne_suggest_split`, `ariadne_suggest_placement`, `ariadne_refactor_opportunities`

**User state tools**: `ariadne_annotate`, `ariadne_annotations`, `ariadne_remove_annotation`, `ariadne_bookmark`, `ariadne_bookmarks`, `ariadne_remove_bookmark`

### 7.3 MCP Resources (from `src/mcp/resources.rs`)

Static: `ariadne://overview`, `ariadne://smells`, `ariadne://hotspots`, `ariadne://freshness`
Dynamic: `ariadne://file/{path}` (per file), `ariadne://cluster/{name}` (per cluster)

### 7.4 MCP Prompts (from `src/mcp/prompts.rs`)

- `explore-area` — explore area with full graph context (requires `path`)
- `review-impact` — analyze impact of changes (requires `paths`)
- `find-refactoring` — find refactoring opportunities (optional `scope`)
- `understand-module` — understand module via reading order (requires `module`)

### 7.5 Serialized Artifacts

Written to `.ariadne/graph/` by default:
- `graph.json` — `GraphOutput { version, project_root, node_count, edge_count, nodes, edges, generated? }`
- `clusters.json` — `ClusterOutput { clusters: BTreeMap<String, ClusterEntryOutput> }`
- `stats.json` — `StatsOutput` (project-level statistics)
- `raw_imports.json` — `BTreeMap<String, Vec<RawImportOutput>>` (for freshness engine)
- `boundaries.json` — `BoundaryOutput { boundaries, edges, route_count, event_count, orphan_routes, orphan_events }`
- `.lock` — PID lock file for MCP server exclusion

## 8. Key Data Types

### 8.1 Core Model (`src/model/`)

- `CanonicalPath` — normalized relative file path (forward slashes, no `./`/`..`)
- `ContentHash` — xxHash64 lowercase hex (16 chars)
- `ClusterId`, `Symbol` — newtype wrappers over `String`
- `FileSet` — `BTreeSet<CanonicalPath>` for deterministic file lookups
- `Node` — `{ file_type, layer, fsd_layer?, arch_depth, lines, hash, exports, cluster, symbols }`
- `Edge` — `{ from, to, edge_type, symbols }`
- `FileType` — Source, Test, Config, Style, Asset, TypeDef, Doc, Data
- `ArchLayer` — Api, Service, Data, Util, Component, Hook, Config, Unknown
- `FsdLayer` — App, Processes, Pages, Widgets, Features, Entities, Shared
- `EdgeType` — Imports, Tests, ReExports, TypeImports, References

### 8.2 Algorithm Types (`src/algo/`)
- `AdjacencyIndex` — prebuilt forward/reverse adjacency maps with degree counts
- `CompressedGraph` / `CompressedNode` / `CompressedEdge` — multi-level graph compression
- `SpectralResult` — algebraic connectivity, Fiedler vector, monolith score

### 8.3 Analysis Types (`src/analysis/`)
- `ClusterMetrics` — Martin instability/abstractness/distance metrics
- `ArchSmell` — detected architectural anti-pattern with severity
- `StructuralDiff` — file-level change classification, cluster/layer changes

### 8.4 MCP State (`src/mcp/state.rs`)
- `GraphState` — full in-memory state with precomputed indices: graph, stats, clusters, forward/reverse index, layer index, file hashes, raw imports, cluster metrics, PageRank, combined importance, compressed L0, spectral result, symbol index, call graph, temporal state, semantic state, structural diff, freshness state
- `FreshnessState` — two-level staleness tracking: hash confidence + structural confidence

### 8.5 Recommend Types (`src/recommend/types.rs`)
- `SplitAnalysis` — file split recommendation with Stoer-Wagner min-cut
- `PlacementSuggestion` — where to place a new file
- `RefactorOpportunity` — refactoring suggestion with effort/impact/Pareto frontier
- `DataQuality` — Full, Structural, Minimal (confidence indicator)

## 9. Feature Flags

From `Cargo.toml`:
- `default = ["serve"]` — MCP server included by default
- `serve` — enables: `rmcp`, `tokio`, `tokio-util`, `arc-swap`, `notify`, `notify-debouncer-full`, `schemars`

The `mcp/` module is conditionally compiled: `#[cfg(feature = "serve")]` in `lib.rs`.

## 10. Supported Languages

From `src/parser/mod.rs` and registry construction:

| Language | Extensions | Parser | Resolver |
|----------|-----------|--------|----------|
| TypeScript/TSX | ts, tsx, js, jsx | `TypeScriptParser` | `TypeScriptResolver` |
| Go | go | `GoParser` | `GoResolver` |
| Python | py | `PythonParser` | `PythonResolver` |
| Rust | rs | `RustParser` | `RustResolver` |
| C# | cs | `CSharpParser` | `CSharpResolver` |
| Java | java | `JavaParser` | `JavaResolver` |
| Markdown | md | `MarkdownParser` | `MarkdownResolver` |
| JSON | json | `JsonParser` | (no resolver) |
| YAML | yaml, yml | `YamlParser` | (no resolver) |

## 11. Architectural Patterns

### 11.1 Determinism Strategy
- All maps use `BTreeMap`/`BTreeSet` (sorted iteration)
- File lists sorted before parallel processing
- Floats rounded to 4 decimal places (`algo::round4`)
- Content hashing via xxHash64 for reproducibility
- Evidence: `src/pipeline/mod.rs` lines 219, 128; `src/algo/mod.rs` line 29

### 11.2 Error Handling
- Fatal errors (`FatalError` enum): 14 variants (E001-E014), thiserror-derived
- Warnings (`WarningCode` enum): 33 variants (W001-W033)
- `DiagnosticCollector`: thread-safe (Mutex-wrapped) warning accumulator
- Two-tier: fatals abort pipeline, warnings are collected and reported at end

### 11.3 Parallelism
- `rayon` for parallel file parsing (Stage 3 of pipeline)
- `tokio` for async MCP server only
- `DiagnosticCollector` is `Sync` via interior `Mutex`

### 11.4 State Management (MCP)
- `ArcSwap<GraphState>` for lock-free reads, atomic state replacement on rebuild
- `AtomicBool` for rebuild-in-progress flag
- `UserStateManager` for persistent annotations/bookmarks
- `LockGuard` (PID-based) for single-server enforcement

### 11.5 Trait-Based Extensibility
- New languages: implement `LanguageParser` + `ImportResolver`, register in `ParserRegistry`
- New serialization formats: implement `GraphSerializer` + `GraphReader`
- New boundary extractors: implement `BoundaryExtractor`, register via `register_boundary_extractor()`
- New symbol extractors: implement `SymbolExtractor`, register via `register_symbol_extractor()`
