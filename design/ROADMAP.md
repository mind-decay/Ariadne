# Ariadne — Implementation Roadmap

## Overview

Ariadne is a standalone Rust CLI that builds structural dependency graphs from source code via tree-sitter.

**Crate name:** `ariadne-graph` (binary: `ariadne`) — D-010.

---

## Phase 1a: MVP — Parse and Output [DONE]

**Goal:** `ariadne build <path>` works. Parses a multi-language project, outputs `graph.json` + `clusters.json`. Basic error handling (skip broken files, log to stderr). No frills.

**Deliverables:**

- Cargo project (`ariadne-graph` crate, `ariadne` binary)
- Core data model (BTreeMap for determinism — D-006)
- Tree-sitter integration with partial parse handling
- 6 Tier 1 language parsers (TS/JS, Go, Python, Rust, C#, Java)
- JSON and YAML data file parsers (no-dependency semantics, `FileType::Data`) (D-075, D-076)
- File type detection + architectural layer inference
- xxHash64 content hashing
- Directory-based clustering
- Graph builder pipeline (walk → read → parse → resolve → cluster → sort → output)
- JSON serialization (deterministic, sorted, atomic writes)
- CLI: `ariadne build <path> [--output <dir>]` and `ariadne info`
- Basic tests: parser snapshots (insta), fixture graph tests, invariant checks

**NOT in 1a (deferred to 1b):**

- Structured warning system (W001-W009 codes, JSON format)
- CLI flags: --verbose, --warnings, --strict, --timestamp, --max-file-size, --max-files
- Workspace/monorepo detection
- Case-insensitive FS handling
- Per-stage timing output
- Property-based tests, performance benchmarks
- CI/CD workflows, install.sh
- README.md

**Testing:** Parser snapshots (L1), fixture graph snapshots (L2), invariant checks (L3 basic). No benchmarks.

**Success criteria:**

1. `cargo build --release` compiles
2. `ariadne info` lists 6 languages
3. `ariadne build` on each fixture project produces correct graph.json
4. Output is byte-identical on repeated builds (determinism)
5. Broken files are skipped with stderr warning (not crash)
6. All `cargo test` pass

---

## Phase 1b: Hardening [DONE]

**Goal:** Production-quality error handling, full CLI, workspace support, comprehensive tests, CI/CD.

**Depends on:** Phase 1a.

**Deliverables:**

- Structured warning system (W001-W009, human + JSON format)
- All CLI flags (--verbose, --warnings, --strict, --timestamp, --max-file-size, --max-files)
- npm/yarn/pnpm workspace detection and workspace-aware import resolution (D-008)
- Path normalization with case-insensitive FS detection (D-007)
- Per-stage --verbose timing output
- Property-based tests (proptest)
- Performance benchmarks (criterion)
- GitHub Actions CI + release workflows
- install.sh script
- README.md

**Testing:** Full L1-L4 suite. Workspace fixture. Path normalization + traversal + case sensitivity tests.

---

## Phase 2a: Algorithms, Queries & Views [DONE]

**Goal:** Graph becomes queryable — blast radius, centrality, cycles, layers, markdown views. (D-036)

**Depends on:** Phase 1b.

**Deliverables:**

- Graph deserialization (GraphReader trait, GraphOutput → ProjectGraph conversion)
- Algorithms: Tarjan SCC, Reverse BFS (blast radius), Brandes centrality, topological sort
- Subgraph extraction
- Output: stats.json (centrality, SCCs, layers, summary)
- Markdown views (L0 index, L1 per-cluster, L2 on-demand impact reports)
- CLI: `ariadne query *` (blast-radius, subgraph, stats, centrality, cluster, file, cycles, layers), `ariadne views generate`
- `ariadne build` now always produces stats.json (algorithms run on every build)

**Testing:** Algorithm unit tests, INV-14 through INV-18, stats/views snapshots, deserialization round-trips, CLI integration tests, performance benchmarks (SCC <10ms, BFS <10ms, Brandes <500ms, topo sort <10ms).

---

## Phase 2b: Louvain Clustering & Delta Computation [DONE]

**Goal:** Community-based clustering refinement and incremental graph updates. (D-036)

**Depends on:** Phase 2a.

**Deliverables:**

- Louvain community detection (refines directory-based clusters, on by default, `--no-louvain` to disable, `--resolution <gamma>` for tuning). Guard (D-073): if Louvain reduces clusters below 50% of directory count, directory-based clusters are retained.
- Delta computation (`ariadne update` — detects changes via content hash; no-op fast path when nothing changed, full rebuild otherwise. True incremental re-parsing deferred to Phase 3 — see D-050). **Deviation:** D-050 documents that `ariadne update` always does a full rebuild when changes are detected; true incremental re-parsing is deferred to Phase 3.

**Testing:** Louvain correctness tests, delta round-trip tests, performance benchmarks (Louvain <200ms, delta <1s).

---

## Phase 3: MCP Server & Architectural Intelligence [DONE]

**Goal:** Ariadne becomes a long-running MCP server that provides instant, queryable access to structural dependency graphs — enabling any MCP-compatible consumer (AI orchestrators, IDEs, CI tools) to get architectural insights without re-parsing the codebase.

**Depends on:** Phase 2a + 2b (algorithms, queries, views, delta computation).

**Design principle:** Ariadne provides **generic, consumer-agnostic MCP tools**. Consumer-specific adapters (Moira knowledge bridge, IDE plugins, CI integrations) live in the consumer's codebase, not in Ariadne. See D-004 (updated), D-044.

### Problem Statement

AI coding agents currently re-read dozens of files every task to understand project architecture. This is:

- **Expensive:** Up to 140k tokens per exploration session
- **Slow:** Minutes of file reading before any real work
- **Non-deterministic:** Different sessions produce different understanding
- **Shallow:** Manual exploration misses cyclic dependencies, bottleneck files, blast radius, architectural layer violations

Ariadne solves this by providing a pre-computed, queryable structural graph that any MCP consumer can access instantly.

### Architecture Overview

```
┌─────────────────────────────────────────────────────────────┐
│  MCP Consumers (orchestrators, IDEs, CI tools)              │
│                                                             │
│  Consumer-specific adapters live here, not in Ariadne.      │
│  E.g., Moira wraps ariadne_overview into knowledge L0.      │
│                                                             │
│                          │ MCP tool calls                   │
└──────────────────────────┼──────────────────────────────────┘
                           │
              ┌────────────▼────────────┐
              │   Ariadne MCP Server    │
              │   (ariadne serve)       │
              │                         │
              │  ┌───────────────────┐  │
              │  │  In-Memory Graph  │  │
              │  │  (ProjectGraph +  │  │
              │  │   StatsOutput +   │  │
              │  │   ClusterMap +    │  │
              │  │   Reverse Index)  │  │
              │  └───────┬───────────┘  │
              │          │              │
              │  ┌───────▼───────────┐  │
              │  │  Freshness Engine │  │
              │  │  (hash comparison │  │
              │  │   + confidence)   │  │
              │  └───────────────────┘  │
              │                         │
              │  ┌───────────────────┐  │
              │  │  Auto-Update      │  │
              │  │  (fs watcher +    │  │
              │  │   delta rebuild)  │  │
              │  └───────────────────┘  │
              └────────────┬────────────┘
                           │
              ┌────────────▼────────────┐
              │  .ariadne/graph/        │
              │  ├── graph.json         │
              │  ├── clusters.json      │
              │  ├── stats.json         │
              │  └── views/             │
              └─────────────────────────┘
```

### Phase Split

| Phase | Deliverables | Risk |
|-------|-------------|------|
| **3a** | MCP Server: in-memory graph, MCP tools, freshness engine, auto-rebuild | YELLOW |
| **3b** | Architectural Intelligence: Martin metrics, smell detection, structural diff | YELLOW |
| **3c** | Advanced Graph Analytics: PageRank, hierarchical compression, spectral analysis | ORANGE |

**Note:** Phase 3b from the original plan (Moira Knowledge Bridge) is moved to the Moira project. Ariadne provides generic `ariadne_views_export` tool; Moira adapts the output into its knowledge schema on its side. See D-044.

---

### Phase 3a: MCP Server [DONE]

**Goal:** Ariadne runs as an MCP server, loads the graph into memory, answers queries instantly, and keeps the graph fresh automatically.

**Deliverables:**

#### D1: MCP Server Core

**Files:** `src/mcp/mod.rs` (new), `src/mcp/server.rs`, `src/mcp/tools.rs`, `src/mcp/state.rs`

Rust MCP server using JSON-RPC over stdio (standard MCP transport). On startup:

1. Load `graph.json`, `clusters.json`, `stats.json` into memory
2. Build derived indices (reverse adjacency, per-cluster file sets, layer index)
3. If no graph exists → run full build automatically, then load
4. Register MCP tools

**Binary architecture:** Single `ariadne` binary with `serve` subcommand (D-045). `main.rs` remains the sole Composition Root — it dispatches to either one-shot CLI (build/query) or long-running MCP server based on the subcommand.

**In-memory state (`GraphState`):**

```rust
pub struct GraphState {
    graph: ProjectGraph,                              // nodes + edges
    stats: StatsOutput,                               // centrality, SCCs, layers
    clusters: ClusterMap,                              // cluster assignments + cohesion
    reverse_index: BTreeMap<CanonicalPath, Vec<Edge>>, // precomputed for O(1) reverse lookups
    layer_index: BTreeMap<u32, Vec<CanonicalPath>>,    // arch_depth → files
    file_hashes: BTreeMap<CanonicalPath, ContentHash>, // for freshness checks
    loaded_at: SystemTime,                             // when graph was loaded
    freshness: FreshnessState,                         // per-file staleness tracking
}
```

**Memory budget:** Graph + indices for 10k-file project ≈ 50-100MB. Acceptable for a dev tool.

**Module dependency:** `mcp/` depends on `model/`, `algo/`, `analysis/`, `serial/`, `pipeline/`. Never depends on `parser/` directly (pipeline handles parsing).

#### D2: MCP Tools

Each tool maps 1:1 to an `ariadne query` CLI command, plus server-specific tools. All tools are generic — no consumer-specific formatting or semantics (D-044).

| # | Tool | Input | Output |
|---|------|-------|--------|
| T1 | `ariadne_overview` | — | Project summary: node/edge counts, language breakdown, layer distribution, critical files, cycles count, max depth |
| T2 | `ariadne_file` | `path: string` | File detail: type, layer, arch_depth, exports, cluster, centrality, incoming/outgoing edges |
| T3 | `ariadne_blast_radius` | `path: string, depth?: number` | Reverse BFS: map of affected files with distances |
| T4 | `ariadne_subgraph` | `paths: string[], depth?: number` | Filtered graph: nodes + edges + clusters in neighborhood |
| T5 | `ariadne_centrality` | `min?: number` | Bottleneck files sorted by centrality score |
| T6 | `ariadne_cycles` | — | All SCCs (circular dependencies) |
| T7 | `ariadne_layers` | `layer?: number` | Topological layers: files per arch_depth level |
| T8 | `ariadne_cluster` | `name: string` | Cluster detail: files, internal/external deps, cohesion, tests |
| T9 | `ariadne_dependencies` | `path: string, direction: "in"\|"out"\|"both"` | Direct dependencies of a file (not transitive) |
| T10 | `ariadne_freshness` | — | Graph freshness: overall confidence, stale files list, last update time |
| T11 | `ariadne_views_export` | `level: "L0"\|"L1"\|"L2", cluster?: string` | Pre-generated markdown views from `.ariadne/views/` (Phase 2 D10 output). Generic markdown — consumers transform as needed |

**Response format:** All tools return structured JSON. Token-efficient — no prose, just data. Consumers interpret the data.

**Error semantics:**
- File not in graph → `{ "error": "not_found", "path": "...", "suggestion": "File may be new. Graph freshness: 87%" }`
- Graph not built → auto-trigger build, return result after build completes
- Stale graph → return data + `"freshness": { "confidence": 0.73, "stale_files": [...] }` field

#### D3: Freshness Engine

**Problem:** When agents modify files, the in-memory graph becomes stale. We need to know *how stale* and auto-update.

**Approach: Hash-based confidence scoring.**

Every node in `graph.json` has a `ContentHash` (xxHash64). The freshness engine:

1. **On query:** Compare in-memory hash vs current file hash for queried files
2. **Confidence score:** `confidence = 1 - (stale_files / total_files)`
3. **Per-file staleness:** Track which specific files are stale (hash mismatch)
4. **Structural confidence:** If stale files have no new/removed imports (same file, different content) → structure is still valid even if hashes differ. This is the common case (editing function bodies doesn't change the dependency graph).

```rust
pub struct FreshnessState {
    stale_files: BTreeSet<CanonicalPath>,    // files with hash mismatch
    new_files: Vec<PathBuf>,                  // files on disk not in graph
    removed_files: Vec<CanonicalPath>,        // files in graph not on disk
    structural_confidence: f64,               // 0.0-1.0
    last_full_check: SystemTime,
}
```

**Confidence thresholds:**
- ≥0.95 → graph is fresh, use as-is
- 0.80-0.95 → minor staleness, results reliable for structural queries
- 0.50-0.80 → noticeable drift, flag to user, auto-update recommended
- <0.50 → graph significantly outdated, auto-rebuild triggered

#### D4: Auto-Update Mechanism

**Strategy: File system watcher + debounced delta rebuild.**

1. **fs watcher** (notify crate): watch project directory for file changes
2. **Debounce:** Collect changes for 2 seconds after last modification (configurable)
3. **Delta rebuild:** Run `ariadne update` logic (Phase 2b D9) on changed files
4. **Hot reload:** Swap `GraphState` atomically (Arc<RwLock<GraphState>>)
5. **Fallback:** If watcher unavailable (unsupported FS, permission error) → poll-based check every 30 seconds

**File ownership:** When the MCP server is running, it acquires exclusive write access to `.ariadne/graph/` via a lock file (`.ariadne/graph/.lock`). CLI `ariadne build` and `ariadne update` check for this lock and refuse to run while the server is active, with a message directing to the MCP server (D-046). The server is the sole writer — this prevents race conditions and double-write corruption.

**Threading model:** `notify` crate uses OS-native file watching (kqueue on macOS, inotify on Linux) with a dedicated watcher thread. No async runtime (tokio) required. MCP JSON-RPC runs on the main thread (stdio is inherently sequential). Delta rebuild runs on a background thread, communicates via `Arc<RwLock<GraphState>>` swap. See D-047. **Deviation:** D-051 documents that tokio is used for the `serve` subcommand due to `rmcp` crate requirements; all other commands remain synchronous.

**CLI extension:**

```
ariadne serve [--project <path>] [--debounce <ms>] [--no-watch]
```

Starts the MCP server. `--no-watch` disables fs watcher (poll-only mode).

**Registration in Claude Code settings.json:**

```json
{
  "mcpServers": {
    "ariadne": {
      "command": "ariadne",
      "args": ["serve", "--project", "."],
      "env": {}
    }
  }
}
```

---

### Phase 3b: Architectural Intelligence [DONE]

**Goal:** Move beyond basic graph metrics into architectural analysis — detect problems, quantify design quality, track structural evolution.

**Depends on:** Phase 3a.

**Deliverables:**

#### D5: Martin Metrics (Instability & Abstractness)

Robert C. Martin's package metrics applied at cluster level:

**Instability** `I = Ce / (Ca + Ce)`:
- `Ca` = afferent coupling (incoming edges from other clusters)
- `Ce` = efferent coupling (outgoing edges to other clusters)
- `I = 0` → maximally stable (everyone depends on it, it depends on nothing)
- `I = 1` → maximally unstable (depends on everything, nothing depends on it)

**Abstractness** `A = Na / Nc`:
- `Na` = number of abstract files (type_def files, interfaces, re-export barrels)
- `Nc` = total files in cluster
- `A = 0` → fully concrete
- `A = 1` → fully abstract

**Main Sequence:** The ideal is `A + I ≈ 1`. Distance from main sequence: `D = |A + I - 1|`.

- `D ≈ 0` → good balance
- High `D` with low `A`, low `I` → "Zone of Pain" (concrete and stable — hard to change)
- High `D` with high `A`, high `I` → "Zone of Uselessness" (abstract and unstable — no real dependents)

**MCP tool:** `ariadne_metrics` → returns per-cluster `{instability, abstractness, distance, zone}`.

#### D6: Architectural Smell Detection

**Files:** `src/analysis/mod.rs` (new), `src/analysis/metrics.rs`, `src/analysis/smells.rs`

The `analysis/` module is a new top-level module distinct from `algo/`. See D-048 for rationale.

**Module dependency:** `analysis/` depends on `model/` and `algo/` (for calling algorithms like blast_radius). Unlike `algo/` (which is pure computation on `ProjectGraph`), `analysis/` composes multiple algorithm results + graph data + stats to produce higher-level insights. It never depends on `serial/`, `pipeline/`, or `parser/`.

Automated detection of common structural anti-patterns:

| Smell | Detection | Severity |
|-------|----------|----------|
| **God File** | Centrality > 0.8 AND out-degree > 20 AND lines > 500 | HIGH |
| **Circular Dependency** | SCC size > 1 (already computed in Phase 2) | HIGH |
| **Layer Violation** | Edge from lower `arch_depth` to higher (dependency on a higher layer) | MEDIUM |
| **Hub-and-Spoke** | One file has >50% of cluster's external edges | MEDIUM |
| **Unstable Foundation** | Cluster with `I > 0.7` AND `Ca > 10` (many depend on it, but it also depends on many) | HIGH |
| **Dead Cluster** | Cluster with 0 incoming external edges AND not a top-level entry point | LOW |
| **Shotgun Surgery** | File with blast radius > 30% of project | HIGH |

**MCP tool:** `ariadne_smells` → returns detected smells with file paths, severity, and explanation.

#### D7: Structural Diff

When `ariadne update` runs (delta computation), compute not just "which files changed" but "how the *structure* changed":

```rust
pub struct StructuralDiff {
    added_nodes: Vec<CanonicalPath>,
    removed_nodes: Vec<CanonicalPath>,
    added_edges: Vec<Edge>,
    removed_edges: Vec<Edge>,
    changed_layers: Vec<(CanonicalPath, u32, u32)>,  // file, old_depth, new_depth
    changed_clusters: Vec<(CanonicalPath, ClusterId, ClusterId)>,
    new_cycles: Vec<Vec<CanonicalPath>>,              // SCCs that didn't exist before
    resolved_cycles: Vec<Vec<CanonicalPath>>,          // SCCs that were broken
    new_smells: Vec<ArchSmell>,
    resolved_smells: Vec<ArchSmell>,
    summary: DiffSummary,
}

pub struct DiffSummary {
    structural_change_magnitude: f64,  // 0.0 (no structural change) to 1.0 (complete restructure)
    change_type: ChangeClassification, // Additive, Refactor, Migration, Breaking
}
```

**Change magnitude** computed as normalized graph edit distance:
`magnitude = (|added_edges| + |removed_edges| + |added_nodes| + |removed_nodes|) / (2 * (|edges| + |nodes|))`

**MCP tool:** `ariadne_diff` → returns structural diff since last update.

**`StructuralDiff` lives in `model/`** (pure data type), computed in `analysis/diff.rs`. The `ArchSmell` type also lives in `model/` so both `analysis/` and `mcp/` can reference it without circular dependencies.

---

### Phase 3c: Advanced Graph Analytics [DONE]

**Goal:** Techniques from spectral graph theory and information retrieval to handle large codebases and provide deeper ranking insights.

**Depends on:** Phase 3a.

**Deliverables:**

#### D8: PageRank for File Importance

Brandes centrality (Phase 2) measures *betweenness* — files that are on many shortest paths. PageRank measures *authority* — files that many important files depend on.

```rust
pub fn pagerank(
    graph: &ProjectGraph,
    damping: f64,          // typically 0.85
    max_iterations: u32,   // typically 100
    tolerance: f64,        // typically 1e-6
) -> BTreeMap<CanonicalPath, f64>
```

**Algorithm:** Power iteration on the transition matrix of the import graph. O(V + E) per iteration, typically converges in 20-50 iterations.

**Float determinism:** All iterative floating-point algorithms (Brandes, PageRank, Louvain, spectral) share a common determinism strategy (D-049): round final results to 4 decimal places, use deterministic iteration order (BTreeMap keys), and fix iteration/tolerance parameters. Convergence ordering is defined by lexicographic node order to avoid platform-dependent floating-point accumulation differences.

**Combined ranking:** Files ranked by `combined_score = 0.5 * normalized_centrality + 0.5 * normalized_pagerank`. This captures both "bridge" files (centrality) and "foundation" files (PageRank).

**MCP tool:** `ariadne_importance` → returns files ranked by combined score.

#### D9: Hierarchical Graph Compression

For large codebases (10k+ files), sending full graph data to consumers is too expensive. Hierarchical compression provides zoom levels:

**Level 0 (Project):** ~10-30 nodes. Each node = cluster. Edges = inter-cluster dependencies. Includes: cluster names, file counts, cohesion, key files.

**Level 1 (Cluster):** ~50-200 nodes per cluster. Each node = file. Full internal edges. Simplified external edges (just counts per target cluster).

**Level 2 (File):** Full detail for a specific file and its N-hop neighborhood.

```rust
pub struct CompressedGraph {
    level: CompressionLevel,
    nodes: Vec<CompressedNode>,
    edges: Vec<CompressedEdge>,
    token_estimate: u32,  // estimated tokens when serialized
}
```

**MCP tool:** `ariadne_compressed(level: 0|1|2, focus?: string)` → returns compressed view at specified level. `focus` is a cluster name (for L1) or file path (for L2).

**Token budget estimation:**
- L0: ~200-500 tokens (project overview)
- L1: ~500-2000 tokens per cluster
- L2: ~200-1000 tokens per file neighborhood

#### D10: Spectral Analysis (Fiedler Vector)

The algebraic connectivity (second-smallest eigenvalue of the graph Laplacian, λ₂) and its eigenvector (Fiedler vector) provide insights that other metrics miss:

- **λ₂ value:** Measures overall graph connectivity. Low λ₂ → graph is close to splitting into components (natural module boundaries). High λ₂ → tightly connected (monolith).
- **Fiedler vector:** Natural bisection of the graph. Sign of each component indicates which partition a file belongs to. This reveals the *natural* division of the codebase — where Louvain gives communities, Fiedler gives the fundamental split.

**Practical value:**
- Detect monolithic structure (λ₂ >> 0 with single cluster)
- Identify natural refactoring boundaries (Fiedler vector sign changes)
- Validate Louvain clusters against spectral partitioning

**Implementation:** Sparse Laplacian + Lanczos iteration for eigenvalue computation. O(V + E) per iteration, ~50-100 iterations for convergence. Use `nalgebra-sparse` or `sprs` crate.

**Risk: ORANGE.** Sparse eigensolver complexity + floating-point determinism across platforms. May require fixed-precision arithmetic or platform-specific tolerances. **Evaluate feasibility during implementation — defer if determinism cost is too high (D-043).**

**MCP tool:** `ariadne_spectral` → returns `{ algebraic_connectivity, natural_partitions, monolith_score }`.

---

### Graph Update Strategy

**Trigger:** File system watcher detects changes → debounced 2s → delta rebuild.

**Flow:**
1. fs watcher fires on file write/delete/rename
2. Debounce timer (2s) — collect all changes into a batch
3. Run delta computation (Phase 2b D9 logic) on background thread
4. Update in-memory `GraphState` atomically (RwLock swap)
5. Persist updated graph to disk (`.ariadne/graph/`)
6. If structural changes detected → compute `StructuralDiff`, store as last diff
7. Update freshness state

**Graceful degradation:**
- fs watcher fails → fall back to poll every 30s
- Delta fails → fall back to full rebuild
- Full rebuild fails → serve stale graph with freshness warning
- Graph files missing → auto-run initial build
- Build in progress → queue requests, serve stale data with `"rebuilding": true` flag

**Performance targets:**
- Delta rebuild (10 changed files / 3k project): <2s
- Full rebuild (3k files): <10s
- MCP tool response (in-memory query): <10ms
- Freshness check (single file hash): <1ms

### Module Structure (Phase 3)

```
src/
├── (existing Phase 1-2 modules unchanged)
├── analysis/                # NEW — depends on model/, algo/ (D-048)
│   ├── mod.rs               # Re-exports
│   ├── metrics.rs           # Martin metrics: instability, abstractness (D5)
│   ├── smells.rs            # Architectural smell detection (D6)
│   └── diff.rs              # Structural diff computation (D7)
├── mcp/                     # NEW — depends on model/, algo/, analysis/, serial/, pipeline/ (D-045)
│   ├── mod.rs               # Re-exports
│   ├── server.rs            # JSON-RPC server, MCP protocol handling
│   ├── tools.rs             # MCP tool implementations (dispatch to algo/analysis)
│   └── state.rs             # GraphState, FreshnessState, auto-update logic
└── model/
    ├── (existing files)
    ├── query.rs             # SubgraphResult (Phase 2)
    ├── diff.rs              # NEW — StructuralDiff, DiffSummary, ChangeClassification
    └── smell.rs             # NEW — ArchSmell, SmellSeverity (pure data types)
```

**Updated dependency rules (extends D-033):**

| Module | Depends on | Never depends on |
|--------|-----------|-----------------|
| `analysis/` | `model/`, `algo/` | `serial/`, `pipeline/`, `parser/`, `mcp/` |
| `mcp/` | `model/`, `algo/`, `analysis/`, `serial/`, `pipeline/` | `parser/` (pipeline handles parsing) |

### Decision Log Entries (Phase 3)

| # | Decision | Rationale |
|---|----------|-----------|
| D-037 | MCP server over CLI for integration | In-memory graph = instant queries. CLI = cold start + JSON parse per query. MCP is native Claude Code integration |
| D-038 | fs watcher + debounced delta for auto-update | Consumers don't need manual `ariadne update`. 2s debounce prevents thrashing during multi-file writes |
| D-039 | Hash-based freshness with confidence scoring | Binary fresh/stale is too coarse. Confidence score lets consumers decide how much to trust results |
| D-040 | Martin metrics at cluster level | File-level instability/abstractness is noisy. Cluster-level aligns with module-oriented thinking |
| D-041 | Hierarchical compression for large codebases | 10k+ file graphs are too large for agent context. 3-level compression keeps tokens manageable |
| D-042 | PageRank + Centrality combined ranking | Neither metric alone captures "importance." Combined score balances bridges (centrality) and foundations (PageRank) |
| D-043 | Spectral analysis as optional (ORANGE risk) | Sparse eigensolver determinism is hard. Defer if f64 cross-platform reproducibility cost is too high |
| D-044 | Consumer-agnostic MCP tools (updates D-004) | Ariadne provides generic graph queries. Consumer-specific adapters (Moira knowledge bridge, IDE plugins) live in the consumer project. Ariadne has zero knowledge of any specific consumer |
| D-045 | Single binary with `serve` subcommand | `main.rs` remains sole Composition Root (D-020). Dispatches to one-shot CLI or long-running MCP server. Avoids separate binary target complexity |
| D-046 | Lock file for `.ariadne/graph/` write exclusion | When MCP server runs, it owns `.ariadne/graph/` exclusively. CLI build/update refuses while lock is held. Prevents race conditions and double writes |
| D-047 | Thread-based architecture, no async runtime | `notify` uses OS threads for fs watching. MCP JSON-RPC is sequential (stdio). Background delta rebuild on thread pool. No tokio dependency — keeps binary small and build fast |
| D-048 | `analysis/` module separate from `algo/` | `algo/` is pure computation on `ProjectGraph` (D-033). `analysis/` composes algorithm results + stats + graph into higher-level insights (metrics, smells, diffs). Different dependency profile: `analysis/` depends on `algo/`, `algo/` never depends on `analysis/` |
| D-049 | Unified float determinism strategy | All iterative f64 algorithms share: 4 decimal rounding, BTreeMap iteration order, fixed iteration/tolerance params. Standardized in utility function. Applies to Brandes (Phase 2), PageRank, Louvain, spectral (Phase 3) |

### Success Criteria

#### Phase 3a
1. `ariadne serve` starts MCP server, loads graph, answers queries via stdio JSON-RPC
2. All MCP tools return correct results matching CLI `ariadne query` equivalents
3. fs watcher triggers delta rebuild within 2s of file change
4. Freshness confidence score accurately reflects graph staleness
5. Server handles missing/corrupted graph gracefully (auto-rebuild)
6. MCP tool response latency <10ms for in-memory queries
7. Server operates correctly as Claude Code MCP server (settings.json registration)
8. Lock file prevents concurrent CLI writes while server is running
9. `ariadne_views_export` returns generic markdown views (no consumer-specific formatting)

#### Phase 3b
10. Martin metrics computed for all clusters, detect Zone of Pain / Zone of Uselessness
11. Architectural smell detection identifies known anti-patterns with <5% false positive rate
12. Structural diff correctly captures added/removed edges, new/resolved cycles
13. All metrics deterministic (byte-identical output)

#### Phase 3c
14. PageRank converges within 100 iterations, results deterministic to 4 decimal places
15. Hierarchical compression produces valid graphs at all 3 levels
16. L0 compressed graph fits within 500 tokens for projects up to 10k files
17. Spectral analysis (if implemented) returns algebraic connectivity and natural partitions

### Testing Requirements

**MCP Integration Tests:**
- Start server → send tool request → verify response matches CLI output
- File change → verify auto-rebuild → verify tool returns updated data
- Missing graph → verify auto-build → verify tools work after build
- Corrupted graph → verify graceful fallback
- Lock file test: start server → CLI build/update → verify refusal with clear message
- Lock file cleanup: server exits → verify lock released → CLI works

**Freshness Tests:**
- Modify file → check confidence drops
- Modify file body (no import changes) → structural confidence stays high
- Add new file → confidence reflects new file not in graph
- Delete file → confidence reflects removed file

**Architectural Intelligence Tests:**
- Hand-crafted graphs with known Martin metrics
- Known architectural smells → verify detection
- Known clean architectures → verify no false positives
- Structural diff on controlled changes → verify diff accuracy

**Performance Benchmarks:**
- `bench_mcp_overview` on 3k-node graph: <5ms
- `bench_mcp_blast_radius` on 3k-node graph: <10ms
- `bench_pagerank` on 3k-node graph: <100ms
- `bench_compression_l0` on 10k-node graph: <50ms
- `bench_auto_update` (10 files changed, 3k project): <2s

---

## Evolution: Code Intelligence Platform

Ariadne evolves from a **file-level structural graph** into a **comprehensive code intelligence platform** for AI agents. Four development axes, seven new phases, with explicit Moira integration notes for each.

**Current state (post Phase 3c):** 17 MCP tools, file-level dependency graph, architectural intelligence (Martin metrics, smells, spectral analysis), hierarchical compression, auto-update via fs watcher.

**Target state:** Symbol-level graph, git-aware temporal analysis, semantic boundary detection, agent-optimized composite queries, MCP resources/prompts, annotation system, recommendation engine.

### Development Axes

| Axis | Current | Target | Impact |
|------|---------|--------|--------|
| **Granularity** | File-level imports | Symbol-level call graph | 10x precision for blast radius, dependencies |
| **Temporality** | Snapshot only | Git history integration | Co-change, churn, hotspots, ownership |
| **Semantics** | Static imports | API routes, events, DI, config-driven | Hidden dependency detection |
| **Delivery** | Raw data tools | Composite queries, resources, recommendations | 3-5x fewer tool calls per agent task |

### Implementation Priority Matrix

```
Phase │ Name                    │ Effort │ Agent Value │ Moira Effort │ Dependencies
──────┼─────────────────────────┼────────┼─────────────┼──────────────┼─────────────
  4   │ Symbol Graph            │ LARGE  │ ★★★★★       │ MEDIUM       │ Phase 3c
  5   │ Agent Context Engine    │ MEDIUM │ ★★★★★       │ LARGE        │ Phase 4 (partial)
  6   │ MCP Protocol Expansion  │ MEDIUM │ ★★★★☆       │ MEDIUM       │ Phase 4
  7   │ Git Temporal Analysis   │ MEDIUM │ ★★★★☆       │ MEDIUM       │ Phase 3c (parallel)
  8   │ Semantic Boundaries     │ LARGE  │ ★★★☆☆       │ MEDIUM       │ Phase 4
  9   │ Recommendation Engine   │ MEDIUM │ ★★★★☆       │ MEDIUM       │ Phases 4, 5, 7
  10  │ External Deps           │ SMALL  │ ★★★☆☆       │ SMALL        │ Phase 1b
```

### Recommended Build Order

```
                     Phase 4: Symbol Graph
                    /                      \
        Phase 5: Context Engine      Phase 7: Git Temporal (parallel)
            |                              |
        Phase 6: MCP Expansion       Phase 9: Recommendations
            |
        Phase 8: Semantic
            Boundaries
```

**Critical path:** Phase 4 → Phase 5 → Phase 6.

**Parallel track:** Phase 7 (git temporal) can start alongside Phase 4.

---

## Phase 4: Symbol Graph [DONE]

**Goal:** Parse and index symbols (functions, classes, types, constants) from AST. Build call graph. Transform Ariadne from "which files depend on which files" to "which symbols depend on which symbols."

**Depends on:** Phase 3c.

### D1: Symbol Extraction Layer

**Files:** `src/parser/symbols.rs` (new trait), per-language parser extensions

Extend `LanguageParser` trait with symbol extraction:

```rust
pub struct SymbolDef {
    pub name: String,
    pub kind: SymbolKind,           // Function, Method, Class, Struct, Interface, Type, Enum, Const, Variable
    pub visibility: Visibility,      // Public, Private, Internal
    pub span: LineSpan,             // start_line..end_line
    pub signature: Option<String>,  // fn foo(x: i32) -> bool
    pub parent: Option<String>,     // class/struct/impl this belongs to
}

pub struct LineSpan {
    pub start: u32,
    pub end: u32,
}

pub enum SymbolKind {
    Function, Method, Class, Struct, Interface, Trait,
    Type, Enum, Const, Variable, Module,
}

pub trait SymbolExtractor {
    fn extract_symbols(&self, tree: &Tree, source: &[u8]) -> Vec<SymbolDef>;
}
```

Tree-sitter already parses full AST — we currently only extract imports/exports. Symbol extraction reads more node types from the same parse tree. **No re-parsing needed.**

Per-language extraction patterns:

| Language | Functions | Classes/Structs | Types/Interfaces | Consts |
|----------|-----------|----------------|-----------------|--------|
| TypeScript/JS | `function_declaration`, `arrow_function` (named) | `class_declaration` | `interface_declaration`, `type_alias_declaration` | `const` with UPPER_CASE |
| Go | `function_declaration`, `method_declaration` | `type_spec` (struct) | `type_spec` (interface) | `const_spec` |
| Python | `function_definition` | `class_definition` | — (runtime typing) | `UPPER_CASE` assignments |
| Rust | `function_item` | `struct_item`, `enum_item` | `trait_item`, `type_item` | `const_item`, `static_item` |
| C# | `method_declaration` | `class_declaration`, `struct_declaration` | `interface_declaration` | `const` fields |
| Java | `method_declaration` | `class_declaration` | `interface_declaration` | `static final` fields |

### D2: Symbol Index

**Files:** `src/model/symbol.rs` (new), `src/mcp/state.rs` (extend GraphState)

In-memory index for O(1) symbol lookups:

```rust
pub struct SymbolIndex {
    /// symbol name → definitions (may have multiple: overloads, same name in different files)
    by_name: BTreeMap<String, Vec<SymbolLocation>>,
    /// file → symbols defined in that file
    by_file: BTreeMap<CanonicalPath, Vec<SymbolDef>>,
    /// symbol → files that import/use it
    usages: BTreeMap<SymbolLocation, Vec<SymbolUsage>>,
}

pub struct SymbolLocation {
    pub file: CanonicalPath,
    pub name: String,
    pub kind: SymbolKind,
    pub span: LineSpan,
}

pub struct SymbolUsage {
    pub file: CanonicalPath,
    pub line: u32,
    pub usage_kind: UsageKind,  // Import, Call, TypeReference, Inheritance
}
```

### D3: Call Graph (Intra-File + Cross-File)

**Files:** `src/algo/callgraph.rs` (new)

Build caller → callee edges from import symbols + symbol definitions:

- If `file_a` imports `{foo}` from `file_b`, and `file_b` exports `fn foo()` → edge `file_a:* → file_b:foo`
- If `file_a:bar()` body references `foo()` (from import) → edge `file_a:bar → file_b:foo`

**Scope:** Cross-file call resolution via imports (static analysis). NOT intra-expression data flow (too expensive, diminishing returns).

### D4: New MCP Tools

| Tool | Input | Output | Agent value |
|------|-------|--------|-------------|
| `ariadne_symbols` | `path: string` | All symbols in file with kinds, spans, visibility | Agent knows exact contents without reading file |
| `ariadne_symbol_search` | `query: string, kind?: SymbolKind` | Matching symbols across project | "Find all classes named *Service" |
| `ariadne_callers` | `path: string, symbol: string` | All call sites of this symbol | "Who calls this function?" |
| `ariadne_callees` | `path: string, symbol: string` | All symbols this function calls (cross-file) | "What does this function depend on?" |
| `ariadne_symbol_blast_radius` | `path: string, symbol: string, depth?: u32` | Transitive callers of a symbol | Precise impact of changing one function |

### D5: Enhanced Existing Tools

- `ariadne_file` → adds `symbols: [{ name, kind, span, visibility }]` field
- `ariadne_blast_radius` → optional `symbol` parameter for symbol-level precision
- `ariadne_dependencies` → adds `symbol_edges: [{ from_symbol, to_symbol }]`
- `ariadne_compressed` L2 → includes symbol-level edges in file neighborhood

### Persistence

Symbols stored in `graph.json` as part of Node:

```rust
pub struct Node {
    // ... existing fields ...
    pub symbols: Vec<SymbolDef>,  // NEW — all symbols defined in this file
}
```

Symbol index built at load time (like reverse_index, forward_index) — not persisted separately.

### Formal Methods & CS Foundations (Phase 4)

The following formal approaches should be **evaluated during implementation** to determine if they provide measurable improvements over simpler heuristics.

#### FM-4.1: Dominator Trees (Lengauer-Tarjan)

**What:** A node D dominates N if every path from entry to N passes through D. The immediate dominator tree is a compact representation of "gatekeeper" relationships.

**Why it matters:** High centrality ≠ dominator. A file can be a hub (many connections) but not a dominator (alternative paths exist). Dominators are **single points of failure** — if the dominator breaks, everything below it is unreachable.

**Algorithm:** Lengauer-Tarjan, O(E × α(V)) — practically linear. ~100 lines of code.

**Application:** New MCP tool `ariadne_dominators` → for each file show its immediate dominator and subtree. Enhances `ariadne_importance` with a "criticality" dimension distinct from centrality/PageRank.

**Evaluate:** Does dominator analysis reveal insights not captured by centrality + blast radius? Test on 3+ real codebases.

#### FM-4.2: Class Hierarchy Analysis (CHA) / Rapid Type Analysis (RTA)

**What:** Classic compiler techniques for sound call graph construction in OOP code.
- **CHA:** For `obj.method()` where `obj: Interface`, consider ALL implementing classes as potential targets.
- **RTA:** Narrow to only classes that are actually instantiated in the codebase.

**Why it matters:** Phase 4 D3 call graph is based on "imports this symbol" — this underestimates polymorphic calls. CHA/RTA give a more sound (no false negatives) call graph for OOP languages (Java, C#, TypeScript classes).

**Application:** Extend call graph construction for languages with class hierarchies. Track `implements`/`extends` relationships from AST.

**Evaluate:** How many additional call edges does CHA/RTA reveal vs import-based analysis? Is the precision difference meaningful for blast radius? Measure on TypeScript + Java fixtures.

#### FM-4.3: Dependency Structure Matrix (DSM)

**What:** N×N matrix where cell[i][j] = dependency strength from module i to module j. DSM partitioning algorithms (Thebeau, Idicula) cluster by minimizing off-diagonal elements.

**Why it matters:** Alternative to Louvain for clustering that is specifically designed for dependency graphs (Louvain optimizes modularity which is a general graph metric). DSM also reveals "bus modules" — modules that everything depends on.

**Application:** Compare DSM clustering results with Louvain. If DSM produces better (more cohesive, less coupled) clusters on test codebases, consider as alternative or ensemble method.

**Evaluate:** Run DSM partitioning + Louvain on same graphs, compare inter-cluster coupling and intra-cluster cohesion. DSM is only worth adding if it beats Louvain on >50% of test cases.

### Moira Integration Notes (Phase 4)

| Moira Component | Change Needed | Priority |
|----------------|--------------|----------|
| **Knowledge Access Matrix** | Add `symbols` column: Hermes L0 (names only), Hephaestus L2 (full with spans), Themis L1 (signatures) | HIGH |
| **graph.sh** | Add `moira_graph_symbols` wrapper for CLI `ariadne query symbols` | MEDIUM |
| **MCP Registry** | Register 5 new tools with purpose/cost/when_to_use | HIGH |
| **Hermes (Explorer)** | Use `ariadne_symbol_search` instead of grep for symbol discovery — faster, more precise | HIGH |
| **Hephaestus (Implementer)** | Use `ariadne_symbols` to verify exports/imports match before writing code | HIGH |
| **Themis (Reviewer)** | Use `ariadne_callers` to verify all call sites are updated after interface changes | HIGH |
| **Athena (Analyst)** | Use `ariadne_symbol_blast_radius` for precise impact in requirements | MEDIUM |
| **Daedalus (Planner)** | Include symbol-level context in instruction assembly (§ Project Graph) | MEDIUM |
| **Dispatch** | Pre-planning agents get symbol counts per file (L0), planning agents get full symbols (L1) | MEDIUM |

**Estimated Moira effort:** ~Phase 15 (8-12 files changed, new knowledge access column, MCP registry update, 3-4 agent role updates).

**Success criteria:**
1. All 6 Tier 1 language parsers extract symbols (functions, classes, types, constants)
2. `ariadne_symbols` returns complete symbol list with line spans for any file
3. `ariadne_symbol_search` finds symbols by name/kind across project in <10ms
4. `ariadne_callers`/`ariadne_callees` return correct cross-file call edges
5. `ariadne_file` includes symbols field, backward-compatible with existing consumers
6. Symbol index rebuilt on auto-update (fs watcher → delta rebuild includes new symbols)
7. All output deterministic (BTreeMap ordering, sorted symbols)

---

## Phase 5: Agent Context Engine [DONE]

**Goal:** Transform Ariadne from "answer individual queries" to "assemble optimal context for a task." Reduce agent tool calls from 3-5 to 1 for common workflows.

**Depends on:** Phase 4 (symbols make context richer), but can partially ship without symbols.

### D6: `ariadne_context` — Smart Context Assembly

```
Input: {
    files: ["src/model/user.rs", "src/api/users.rs"],
    task?: "add_field" | "refactor" | "fix_bug" | "add_feature" | "understand",
    budget_tokens?: 4000,
    include?: ["tests", "interfaces", "configs"],
    depth?: 2
}

Output: {
    target_files: [{
        path, file_type, layer, symbols, centrality,
        incoming_count, outgoing_count
    }],
    direct_dependencies: [{
        path, direction, edge_type, symbols,
        relevance: "high" | "medium" | "low"
    }],
    interfaces: [{                    // types/traits that targets implement
        path, symbol, kind
    }],
    tests: [{                         // test files covering these modules
        path, covers: ["src/model/user.rs"]
    }],
    related_configs: [{               // configs in same cluster
        path, file_type
    }],
    reading_order: ["types.rs", "traits.rs", "user.rs", "api/users.rs"],
    warnings: [{                      // smells/violations in work zone
        type, severity, explanation
    }],
    token_estimate: 3200,
    budget_used: 3200,
    budget_total: 4000,
    trimmed: ["excluded 3 low-relevance dependencies to fit budget"]
}
```

**Task-aware prioritization:**
- `add_field` → prioritize interfaces, type definitions, serialization
- `refactor` → prioritize callers, blast radius, tests
- `fix_bug` → prioritize call chain, error handling, test coverage
- `understand` → prioritize reading order, architecture overview

**Token budget algorithm:**
1. Always include target files (non-negotiable)
2. Add high-relevance deps (direct imports/importers with shared symbols)
3. Add interfaces/types that targets implement
4. Add tests
5. Add medium-relevance deps
6. Add configs
7. Add low-relevance deps
8. Trim from bottom until within budget

### D7: `ariadne_tests_for` — Test Mapping

```
Input:  { paths: ["src/model/user.rs"] }
Output: {
    tests: [
        { path: "tests/model/user_test.rs", confidence: "high", reason: "imports user.rs" },
        { path: "tests/integration/api_test.rs", confidence: "medium", reason: "transitive via api/users.rs" }
    ],
    untested: ["src/model/user.rs has no direct test imports"],
    suggested_test_path: "tests/model/user_test.rs"
}
```

Detection: reverse edges with `EdgeType::Tests` + heuristic name matching (`user.rs` → `user_test.rs`, `user.spec.ts`).

### D8: `ariadne_reading_order` — Optimal Understanding Path

```
Input:  { paths: ["src/pipeline/"], depth?: 2 }
Output: {
    order: [
        { path: "src/model/types.rs", reason: "leaf dependency — read first" },
        { path: "src/pipeline/stages.rs", reason: "defines Stage trait" },
        { path: "src/pipeline/walker.rs", reason: "implements FileWalker stage" },
        { path: "src/pipeline/pipeline.rs", reason: "orchestrates stages — read last" }
    ],
    entry_points: ["src/pipeline/mod.rs"],
    total_lines: 1200,
    token_estimate: 9600
}
```

Algorithm: topological sort within subgraph, leaf nodes first, entry points last.

### D9: `ariadne_plan_impact` — Pre-Change Impact Analysis

```
Input: {
    changes: [
        { path: "src/model/user.rs", type: "modify" },
        { path: "src/api/v2/orders.rs", type: "add" },
        { path: "src/migration/003.sql", type: "add" }
    ]
}

Output: {
    blast_radius: {
        total_affected: 14,
        by_distance: { "1": 5, "2": 6, "3": 3 },
        critical_files: [{ path, centrality, reason: "hub file" }]
    },
    affected_tests: [
        { path: "tests/model_test.rs", confidence: "high" },
        { path: "tests/api_test.rs", confidence: "medium" }
    ],
    layer_analysis: {
        layers_crossed: 2,
        direction: "data → api",
        violations: []
    },
    new_risks: [
        "src/api/v2/orders.rs is new — no test coverage yet",
        "src/model/user.rs has centrality 0.72 — high blast radius"
    ],
    structural_change_class: "Additive",
    suggested_review_files: [
        { path: "src/service/user_service.rs", reason: "high centrality, affected" }
    ]
}
```

### Implementation Deviations (Phase 5 Reconciliation, D-084)

The following simplifications were made during implementation. These are intentional and documented in the decision log:

1. **Context output schema simplified (D-084):** Spec defined nested arrays (`interfaces`, `tests`, `related_configs`, `warnings`). Implementation uses flat `ContextEntry` list with `tier`/`relevance`/`tokens` fields. Simpler, proven in production via MCP clients.

2. **`plan_impact` omits `suggested_review_files` (D-084):** Spec included `suggested_review_files` in `ariadne_plan_impact` output. Implementation omits this — blast radius (`ariadne_blast_radius`) already provides equivalent information without redundancy.

3. **`TaskType` weight applies unconditionally (D-084):** Spec implied conditional weight logic per tier. Implementation applies task-type weight multipliers uniformly across all candidates. Simpler logic, no loss of ranking quality.

### Formal Methods & CS Foundations (Phase 5)

#### FM-5.1: Submodular Maximization under Knapsack Constraint

**What:** Token budget context assembly is formally a **submodular knapsack problem**:
- Items: files/dependencies with weights (token estimates) and values (relevance scores)
- Capacity: `budget_tokens`
- Submodularity: including file A increases the marginal value of file B (if B depends on A) — diminishing returns property

**Why it matters:** The planned tier-based heuristic works but has no optimality guarantee. Greedy submodular maximization guarantees (1 - 1/e) ≈ 63% of optimal value — a provable bound.

**Algorithm:** Sort items by value/weight ratio, greedily add. For submodular case: at each step, add item with highest marginal gain per token. O(n² × cost_of_marginal_eval). For our scale (~100 candidate files) — instant.

**Application:** Replace tier-based priority in `ariadne_context` token budget algorithm with greedy submodular selection. Marginal value of file F given already-selected set S = number of new unique dependencies/symbols F brings that S doesn't already cover.

**Evaluate:** Compare greedy submodular vs tier heuristic on 10+ real tasks. Measure: does the submodular selection include files that agents actually needed? Score by "files in context that were read" / "files in context".

#### FM-5.2: Information Gain for File Prioritization

**What:** Rank files by how much "new information" they add to the context:
```
IG(file | context) = H(task) - H(task | context ∪ {file})
```

Approximation: file adds high information gain if it brings unique symbols/types not reachable via already-included files.

**Why it matters:** Formalizes "relevance" beyond heuristic labels (high/medium/low). Two files may both be direct dependencies, but one may be redundant (all its exports are re-exported by another included file).

**Application:** Use as the value function in FM-5.1's knapsack. IG becomes the marginal gain metric.

**Evaluate:** Does IG-based ranking differ meaningfully from simple "dependency distance" ranking? If >80% same → keep simpler heuristic.

#### FM-5.3: Conditional Entropy for Reading Order

**What:** Instead of topological sort, optimize reading order by minimizing cumulative surprise:
```
H(file_B | file_A) = "how much unexpected information in B after reading A"
```

**Algorithm:** Greedy — at each step, pick file with lowest conditional entropy given already-read set. Conditional entropy approximated by: "what fraction of B's imports/types are already defined in read set?"

**Application:** Replace or augment topological sort in `ariadne_reading_order`. Topological sort is valid but not unique — many valid orderings exist. Conditional entropy picks the one that minimizes cognitive load.

**Evaluate:** Compare conditional-entropy order vs simple topo-sort on developer comprehension tasks. If orders differ <10% → keep topo-sort (simpler).

### Moira Integration Notes (Phase 5)

| Moira Component | Change Needed | Priority |
|----------------|--------------|----------|
| **Daedalus (Planner)** | Replace manual graph section assembly with single `ariadne_context` call per implementation batch. Massive simplification of instruction assembly | CRITICAL |
| **Athena (Analyst)** | Use `ariadne_plan_impact` in requirements phase — auto-populate impact section | HIGH |
| **Aletheia (Tester)** | Use `ariadne_tests_for` to identify existing tests before writing new ones | HIGH |
| **Hermes (Explorer)** | Use `ariadne_reading_order` when exploring unfamiliar area — structured exploration instead of ad-hoc | HIGH |
| **Dispatch** | Pre-planning context injection could use `ariadne_context` with small budget (1000 tokens) instead of raw L0 view | MEDIUM |
| **Budget tracking** | `ariadne_context` returns `token_estimate` — Planner can use this for precise budget allocation | HIGH |
| **Analytical Pipeline** | `ariadne_context` + `ariadne_plan_impact` replace 6 baseline queries in gather phase | MEDIUM |

**Estimated Moira effort:** ~Phase 16 (significant — Daedalus instruction assembly rewrite, 6+ agent role updates, dispatch changes, budget tracking integration). This is the highest-value Moira integration.

**Success criteria:**
1. `ariadne_context` returns complete context for given files within token budget
2. Token budget algorithm correctly prioritizes by relevance
3. Task-type parameter changes output prioritization measurably
4. `ariadne_tests_for` correctly identifies test files via edge analysis + name heuristics
5. `ariadne_reading_order` produces valid topological order
6. `ariadne_plan_impact` returns accurate blast radius + affected tests for planned changes
7. One `ariadne_context` call replaces 3-5 individual tool calls in typical agent workflows

---

## Phase 6: MCP Protocol Expansion [DONE]

**Goal:** Leverage full MCP protocol (resources, prompts) beyond just tools. Zero-cost context injection, workflow templates, annotation persistence.

**Depends on:** Phase 4 (symbols) for resource richness; independent of Phase 5.

### D10: MCP Resources — Zero-Cost Context

Resources are data that MCP clients can subscribe to and inject into agent context **without a tool call**. This saves the tool-call budget and latency.

```
Resources:
  ariadne://overview              → project summary JSON (auto-refreshed)
  ariadne://file/{path}           → file metadata + symbols + dependencies
  ariadne://cluster/{name}        → cluster detail + metrics
  ariadne://smells                → current architectural issues
  ariadne://hotspots              → top-10 files by combined importance × churn
  ariadne://freshness             → graph staleness state
```

**Implementation:** Resources backed by same `GraphState` as tools. Refreshed on state swap (auto-update). Client subscribes once, gets notifications on change.

**Key difference from tools:** Resources are declarative (client pulls), tools are imperative (client calls). Resources can be attached to agent context automatically by the MCP client without explicit orchestration.

### D11: MCP Prompts — Workflow Templates

Prompts are pre-built templates that clients can offer to users/agents. Ariadne provides context-enriched prompts.

```
Prompts:
  ariadne://explore-area
    args: { path: string }
    returns: structured prompt + graph data for area exploration

  ariadne://review-impact
    args: { base?: string }
    returns: structured prompt + diff analysis for change review

  ariadne://find-refactoring
    args: { scope?: string }
    returns: prompt + smells + metrics for refactoring discovery

  ariadne://understand-module
    args: { module: string }
    returns: prompt + reading order + architecture for module understanding
```

### D12: Annotation System

**Files:** `src/mcp/annotations.rs` (new), persisted in `.ariadne/annotations.json`

```rust
pub struct Annotation {
    pub target: AnnotationTarget,    // File(path), Cluster(id), Symbol(path, name)
    pub tag: String,                 // "tech-debt", "do-not-touch", "needs-refactor", "legacy"
    pub text: String,                // free-form description
    pub author: String,              // who created (agent name, user)
    pub created_at: String,          // ISO 8601
    pub expires_at: Option<String>,  // auto-cleanup
}
```

**MCP tools:**
- `ariadne_annotate` — add/update annotation
- `ariadne_annotations` — list annotations, filter by tag/target
- `ariadne_remove_annotation` — remove annotation

**Integration with existing tools:** `ariadne_file`, `ariadne_cluster` include annotations in response when present.

### D13: Bookmarks — Named Subgraphs

```
Input:  { name: "auth-subsystem", paths: ["src/auth/", "src/middleware/auth.rs"] }
```

Stored in `.ariadne/bookmarks.json`. Referenced in `ariadne_subgraph({ bookmark: "auth-subsystem" })` and `ariadne_context({ bookmark: "auth-subsystem" })`.

### Implementation Deviations (Phase 6 Reconciliation, D-089 through D-094)

The following deviations from the original spec were made during implementation. These are intentional and documented in the decision log:

1. **Resource URIs simplified (D-089):** Spec defined `ariadne://file/{path}` URIs. Implementation uses `ariadne:///file/{path}` (triple slash) for correct rmcp URI handling. The triple-slash form follows RFC 3986 for URIs with an empty authority component.

2. **`list_changed` push notification deferred (D-093):** Spec called for client notifications on resource change. Implementation sets the `list_changed` capability flag but does not actively push `notifications/resources/list_changed` messages. Clients poll on the flag. Active push can be wired incrementally.

3. **AnnotationTarget includes `Edge` variant (D-090):** Spec defined `AnnotationTarget` with `File`, `Cluster`, and `Symbol` variants. Implementation adds an `Edge` variant with `from`/`to` fields, enabling annotations on specific dependency edges (e.g., marking a dependency as "tech-debt" or "do-not-touch").

4. **Bookmark tool naming (D-094):** Spec implied `ariadne_bookmark_create`/`ariadne_bookmark_list`/`ariadne_bookmark_remove` naming. Implementation uses shorter names: `ariadne_bookmark`, `ariadne_bookmarks`, `ariadne_remove_bookmark` — consistent with existing annotation tool naming convention (`ariadne_annotate`, `ariadne_annotations`, `ariadne_remove_annotation`).

### Formal Methods & CS Foundations (Phase 6)

#### FM-6.1: Architecture Conformance Checking (Constraint Satisfaction)

**What:** Formalize architectural rules as constraints and verify the graph satisfies them:
```rust
pub struct ArchRule {
    pub from: LayerPattern,   // "src/api/**"
    pub to: LayerPattern,     // "src/model/**"
    pub relation: Relation,   // MustNotDepend, MustOnlyDepend, CanDepend
}
```

**Why it matters:** Current layer violations are hardcoded (lower arch_depth → higher = violation). Real projects have nuanced rules: "API can depend on service, service on model, but API must NEVER depend on model directly." User-defined conformance rules catch project-specific violations.

**Application:** New MCP tool `ariadne_conformance` — accepts rules (from `.ariadne/rules.json` or tool parameter), returns violations. Integrates with annotation system (Phase 6 D12) — violations can be annotated as "accepted" or "tech-debt."

**Evaluate:** Do real projects benefit from custom rules beyond the built-in layer check? Survey 5+ projects. If most projects only need the built-in check → defer.

#### FM-6.2: Graph Entropy as Structural Health Metric

**What:** Shannon entropy of degree distribution:
```
H(G) = -Σ (d_i / 2m) × log(d_i / 2m)
```
where d_i = degree of vertex i, m = total edges.

**Why it matters:**
- High entropy = uniform degree distribution (healthy, no dominant hubs)
- Low entropy = few nodes with extreme degree (unhealthy centralization)
- Per-cluster entropy measures cohesion quality: high internal entropy = uniform connectivity (good); low = star topology (hub-and-spoke smell)
- Delta entropy after changes: entropy drop = change increased centralization

**Application:** Add to `ariadne_overview` as `structural_entropy` metric. Add to `ariadne_cluster` as `internal_entropy`. Add to `ariadne_diff` as `entropy_delta`. Expose via `ariadne://overview` resource.

**Cost:** O(V) — trivial to compute. No reason not to include.

**Evaluate:** Does entropy correlate with known architectural quality? Compute on 5+ projects with known good/bad areas.

### Moira Integration Notes (Phase 6)

| Moira Component | Change Needed | Priority |
|----------------|--------------|----------|
| **MCP Client Config** | Subscribe to `ariadne://overview` and `ariadne://smells` as default resources — zero-cost context in every session | CRITICAL |
| **Knowledge System** | Annotations bridge Moira knowledge and Ariadne graph. Mnemosyne (Reflector) can write annotations to mark tech-debt discovered during tasks | HIGH |
| **Dispatch** | Resources eliminate need for graph.sh L0 reads in bootstrap — data comes via MCP subscription | HIGH |
| **Bookmarks** | Daedalus creates bookmarks for task scope → Hephaestus uses bookmark in `ariadne_context` → no re-specifying files | MEDIUM |
| **Analytical Pipeline** | Prompts (`ariadne://review-impact`, `ariadne://find-refactoring`) as first-class analytical subtypes | MEDIUM |
| **Argus (Auditor)** | Read annotations in audit — "are there stale tech-debt annotations?" | LOW |

**Estimated Moira effort:** ~Phase 17 (MCP client config, dispatch rewrite for resources, Mnemosyne annotation bridge, bookmark lifecycle in Daedalus).

**Success criteria:**
1. MCP resources registered and accessible via `resources/list`
2. `ariadne://overview` auto-refreshes on graph state swap
3. `ariadne://file/{path}` returns correct file metadata for any path in graph
4. MCP prompts registered and return context-enriched templates
5. Annotations persist across server restarts (`.ariadne/annotations.json`)
6. Bookmarks work as aliases in `ariadne_subgraph` and `ariadne_context`
7. Existing tools (`ariadne_file`, `ariadne_cluster`) include annotations when present

---

## Phase 7: Git Temporal Analysis [DONE]

**Goal:** Add time dimension — co-change patterns, code churn, file ownership, hotspot detection. Transforms Ariadne from "static snapshot" to "structural + temporal intelligence."

**Depends on:** Phase 3c (base graph). Independent of Phases 4-6 (can run in parallel).

### D14: Git History Engine

**Files:** `src/temporal/mod.rs` (new), `src/temporal/history.rs`, `src/temporal/churn.rs`, `src/temporal/coupling.rs`

```rust
pub struct TemporalState {
    /// Per-file change frequency in time window
    pub churn: BTreeMap<CanonicalPath, ChurnMetrics>,
    /// File pairs that change together above threshold
    pub co_changes: Vec<CoChange>,
    /// Per-file last author, top contributors
    pub ownership: BTreeMap<CanonicalPath, OwnershipInfo>,
    /// Combined structural + temporal risk score
    pub hotspots: Vec<Hotspot>,
}

pub struct ChurnMetrics {
    pub commits_30d: u32,
    pub commits_90d: u32,
    pub lines_changed_30d: u32,
    pub authors_30d: u32,
}

pub struct CoChange {
    pub file_a: CanonicalPath,
    pub file_b: CanonicalPath,
    pub co_change_count: u32,
    pub confidence: f64,           // Jaccard index: co_changes / (changes_a + changes_b - co_changes)
    pub has_structural_link: bool, // true if also connected in import graph
}

pub struct Hotspot {
    pub path: CanonicalPath,
    pub score: f64,                // churn × complexity × blast_radius
    pub churn_rank: u32,
    pub complexity_rank: u32,
    pub blast_radius_rank: u32,
}
```

**Git integration:** Shell out to `git log --numstat --follow` with appropriate date ranges. Parse output. No git library dependency (keeps build simple, git is always available).

### D15: New MCP Tools

| Tool | Input | Output |
|------|-------|--------|
| `ariadne_churn` | `period?: "30d"\|"90d"\|"1y", top?: u32` | Files by change frequency |
| `ariadne_coupling` | `min_confidence?: f64` | Co-change pairs above threshold |
| `ariadne_hotspots` | `top?: u32` | Files ranked by churn × complexity × blast_radius |
| `ariadne_ownership` | `path?: string` | Authors/contributors per file or project-wide |
| `ariadne_hidden_deps` | — | Co-change pairs that have NO structural (import) link — hidden dependencies |

### D16: Enhanced Existing Tools

- `ariadne_file` → adds `churn: { commits_30d, commits_90d, last_changed, top_authors }`
- `ariadne_overview` → adds `temporal: { total_commits_30d, hotspot_count, hidden_dep_count }`
- `ariadne_context` → includes temporal data in relevance scoring (high-churn files get higher priority)
- `ariadne_importance` → combined score now includes churn factor: `structural_importance × (1 + log(churn))`
- `ariadne_smells` → new smell: **Temporal Coupling Without Import** (hidden dependency)

### Formal Methods & CS Foundations (Phase 7)

#### FM-7.1: Mutual Information for Co-Change Analysis

**What:** Replace or augment Jaccard index with mutual information:
```
MI(X, Y) = Σ p(x,y) × log(p(x,y) / (p(x) × p(y)))
```
where X, Y = binary variables "file changed in commit."

**Why it matters:** Jaccard overestimates coupling for files that both change frequently (high base rate). Two files that each change in 80% of commits will have high Jaccard even if changes are independent. MI corrects for base rate by comparing joint probability to product of marginals.

**Normalized MI (NMI)** allows comparing pairs with different change frequencies:
```
NMI(X, Y) = MI(X, Y) / sqrt(H(X) × H(Y))
```

**Application:** Use NMI as the confidence metric in `ariadne_coupling` and `ariadne_hidden_deps` instead of (or alongside) Jaccard. Report both metrics; let users calibrate.

**Evaluate:** Compare Jaccard vs NMI rankings on 3+ repos with known hidden dependencies. If rankings differ significantly → NMI likely more accurate. If >90% same → keep Jaccard (simpler to explain).

#### FM-7.2: Bayesian Confidence for Coupling

**What:** Instead of frequentist Jaccard/MI, use Bayesian posterior:
```
P(coupled | data) ∝ P(data | coupled) × P(coupled)
```
With prior belief that most file pairs are NOT coupled (sparse prior).

**Why it matters:** Fixes small-sample problem. If files A and B changed together 3 out of 3 times, Jaccard = 1.0. But 3 observations is too few to be confident. Bayesian posterior with Beta(1, 10) prior (skeptical about coupling) gives P(coupled) ≈ 0.31 — much more calibrated.

**Application:** Replace point-estimate confidence with Bayesian credible interval: `{ confidence: 0.72, credible_interval: [0.45, 0.91] }`. Wide interval = low confidence (need more data). Narrow = high confidence.

**Evaluate:** Does Bayesian coupling reduce false positive hidden deps? Compare on repos where we can verify ground truth (manually annotated coupling pairs).

#### FM-7.3: Change-Point Detection (PELT)

**What:** Instead of "commits per 30d", detect **when** a file's change rate shifted:
```
file_a: [2, 1, 3, 2, 1, 15, 12, 18, 14, ...]
                         ↑ change point detected
```

**Algorithm:** PELT (Pruned Exact Linear Time) — detects optimal change points in O(n) time with known penalty parameter. Bayesian Online Change-Point Detection for streaming variant.

**Why it matters:**
- "This file became a hotspot 2 weeks ago" is more actionable than "this file has high churn"
- Correlation of change-points with git events (merge, large refactor) → automatic explanations
- Recent change-points = active instability. Old change-points that stabilized = resolved issue

**Application:** Add `change_points: [{ date, before_rate, after_rate }]` to `ariadne_churn` output. Highlight in `ariadne_hotspots` if hotspot is recent vs long-standing.

**Evaluate:** Does PELT find meaningful change points in real repos? Run on 5+ repos, manually verify top-5 detected change points correspond to real events.

#### FM-7.4: Survival Analysis (Kaplan-Meier)

**What:** Estimate P(file survives unchanged until time t):
```
S(t) = P(no modification before time t)
```

**Why it matters:** Predicts file stability — low S(t) = file likely to change again soon. Useful for context assembly (prefer stable files) and risk assessment (unstable files need more review).

**Application:** Add `stability_score: f64` to `ariadne_file` temporal data. Use in `ariadne_context` relevance scoring — stable dependencies are more reliable context.

**Evaluate:** Does survival-based stability predict actual changes better than simple churn rate? If survival analysis and churn rate rank files identically → keep churn (simpler). Survival analysis is most useful when files have bursty change patterns.

### Moira Integration Notes (Phase 7)

| Moira Component | Change Needed | Priority |
|----------------|--------------|----------|
| **MCP Registry** | Register 5 new temporal tools | HIGH |
| **Apollo (Classifier)** | Use hotspot count to adjust complexity estimation — task touching hotspot = higher risk | HIGH |
| **Metis (Architect)** | Use `ariadne_coupling` + `ariadne_hidden_deps` for informed dependency decisions | HIGH |
| **Daedalus (Planner)** | Include churn data in batch prioritization — high-churn files need more review time | MEDIUM |
| **Themis (Reviewer)** | Flag changes to hotspot files — extra scrutiny | MEDIUM |
| **Mnemosyne (Reflector)** | Track whether task outcomes correlate with hotspot changes (meta-learning) | LOW |
| **Knowledge: Quality Map** | Incorporate churn into per-area quality assessment (high churn + low quality = critical) | MEDIUM |
| **Analytical Pipeline** | `ariadne_hotspots` and `ariadne_hidden_deps` as default baseline queries in gather phase | HIGH |

**Estimated Moira effort:** ~Phase 18 (MCP registry, 4 agent role updates, quality map integration, analytical baseline update).

**Success criteria:**
1. `ariadne_churn` returns correct commit counts for 30d/90d/1y periods
2. `ariadne_coupling` detects co-change pairs with Jaccard confidence >0.5
3. `ariadne_hotspots` combined score correlates with actual bug frequency (manual validation)
4. `ariadne_hidden_deps` correctly identifies co-change pairs without import edges
5. Git parsing handles repos with 10k+ commits in <5s
6. Temporal data refreshed on graph auto-update
7. All output deterministic

---

## Phase 8: Semantic Boundaries [DONE]

**Goal:** Detect implicit dependencies that static imports miss — API routes, event systems, DI containers, database tables, config-driven routing.

**Depends on:** Phase 4 (symbol extraction needed for pattern matching).

**Risk: ORANGE.** Each pattern requires language/framework-specific detection. Scope carefully — start with highest-value patterns.

### D17: Boundary Extraction Framework

**Files:** `src/semantic/mod.rs` (new), per-pattern extractors

```rust
pub trait BoundaryExtractor {
    /// Extract semantic boundaries from a parsed file
    fn extract(&self, tree: &Tree, source: &[u8], path: &CanonicalPath) -> Vec<Boundary>;
}

pub struct Boundary {
    pub kind: BoundaryKind,
    pub name: String,              // route path, event name, table name
    pub role: BoundaryRole,        // Producer, Consumer, Both
    pub file: CanonicalPath,
    pub line: u32,
    pub framework: Option<String>, // "express", "fastapi", "spring", etc.
}

pub enum BoundaryKind {
    HttpRoute,          // GET /api/users
    EventChannel,       // emit("order.created") / on("order.created")
    DatabaseTable,      // SELECT FROM users / CREATE TABLE users
    ConfigReference,    // reads from config key
    DiBinding,          // @Injectable, @Autowired, etc.
    GrpcService,        // protobuf service definition
    MessageQueue,       // publish/subscribe to queue/topic
}
```

### D18: Priority Patterns (Phase 8a)

**HTTP Routes** (highest value, most common):
- Express/Koa: `app.get("/path", handler)`, `router.post("/path", ...)`
- FastAPI: `@app.get("/path")`, `@router.post("/path")`
- Spring: `@GetMapping("/path")`, `@RequestMapping`
- Go: `http.HandleFunc("/path", handler)`, `gin.GET("/path", ...)`
- ASP.NET: `[HttpGet("/path")]`, `[Route("/path")]`

**Event Emitters** (second highest):
- Node.js: `emit("event")`, `on("event")`, `addEventListener("event")`
- Python: signal/slot patterns, pubsub
- Generic: string literal in emit/subscribe/publish/on/addEventListener call

### D19: Deferred Patterns (Phase 8b)

- DI containers (framework-specific, many variants)
- Database tables (requires SQL parsing or ORM detection)
- Config references (too many patterns)
- gRPC/protobuf (needs proto file parsing)
- Message queues (Kafka, RabbitMQ — framework-specific)

### D20: New MCP Tools

| Tool | Input | Output |
|------|-------|--------|
| `ariadne_boundaries` | `kind?: BoundaryKind` | All detected semantic boundaries grouped by kind |
| `ariadne_route_map` | — | HTTP routes: path → handler file/symbol, with consumers |
| `ariadne_event_map` | — | Event channels: event name → producers + consumers |
| `ariadne_boundary_for` | `path: string` | Semantic boundaries in this file (routes it defines, events it emits/handles) |

### D21: Enhanced Existing Tools

- `ariadne_file` → adds `boundaries: [{ kind, name, role }]`
- `ariadne_dependencies` → adds `semantic_deps: [{ via: "HTTP /api/users", target_file }]`
- `ariadne_smells` → new smell: **Orphan Route** (defined but no consumer), **Orphan Event** (emitted but no listener)
- `ariadne_blast_radius` → optional `include_semantic: true` to follow semantic edges too

### Formal Methods & CS Foundations (Phase 8)

#### FM-8.1: String Pattern Matching & Abstract Interpretation

**What:** Semantic boundary detection (routes, events) fundamentally requires matching string literals in AST to known patterns. Two formal approaches:

1. **Tree-sitter pattern queries** — declarative S-expression patterns over AST nodes. Already used for imports; extend to match `call_expression` where callee matches known API patterns and first argument is a string literal.

2. **Abstract interpretation of string values** — for cases where route is constructed from variables: `const prefix = "/api"; app.get(prefix + "/users", ...)`. Light abstract interpretation can track string concatenation to recover the full route.

**Why it matters:** Simple literal matching misses constructed routes/events. Abstract interpretation catches more cases at the cost of complexity. The question is whether the additional coverage justifies the implementation effort.

**Evaluate:** Sample 5 real Express/FastAPI projects. Count: how many routes are direct string literals vs constructed? If >90% literals → skip abstract interpretation. If <80% → consider it.

#### FM-8.2: Typed Graph Edges for Semantic Dependencies

**What:** Current `EdgeType` enum has `Imports`, `Tests`, `ReExports`, `TypeImports`, `References`. Semantic boundaries create a new edge category that should be formally distinct.

Formalize as a **typed multigraph**: files can be connected by multiple edge types simultaneously. The semantic edge carries the boundary kind:

```rust
pub struct SemanticEdge {
    pub from: CanonicalPath,
    pub to: CanonicalPath,
    pub boundary: BoundaryKind,   // HttpRoute, EventChannel, etc.
    pub name: String,             // "/api/users", "order.created"
    pub confidence: f64,          // how sure we are this is a real connection
}
```

**Why it matters:** Structural edges (imports) are certain — they exist in code. Semantic edges (route → consumer) are probabilistic — the connection is inferred. Algorithms should weight them differently. Blast radius with `include_semantic: true` should treat semantic edges with lower confidence.

**Evaluate:** Does treating semantic edges as probabilistic improve blast radius accuracy? Test: run blast radius with/without semantic edges on project with known API consumers, measure false positive rate.

### Moira Integration Notes (Phase 8)

| Moira Component | Change Needed | Priority |
|----------------|--------------|----------|
| **MCP Registry** | Register 4 new boundary tools | HIGH |
| **Hermes (Explorer)** | Use `ariadne_route_map` when exploring API layer — instant understanding of all endpoints | HIGH |
| **Metis (Architect)** | Use `ariadne_boundaries` for complete dependency picture including implicit deps | HIGH |
| **Athena (Analyst)** | Include semantic deps in impact analysis — "changing this route affects these consumers" | HIGH |
| **Hephaestus (Implementer)** | Use `ariadne_boundary_for` to verify route/event consistency when adding new endpoints | MEDIUM |
| **Knowledge: Project Model** | Auto-detect framework (Express, FastAPI, Spring, etc.) from boundary patterns → enrich project model | MEDIUM |
| **Analytical Pipeline** | `ariadne_boundaries` as baseline query for API-focused analysis subtypes | MEDIUM |

**Estimated Moira effort:** ~Phase 19 (MCP registry, 4 agent role updates, project model enhancement).

**Success criteria:**
1. HTTP route detection works for Express, FastAPI, Spring, Go net/http, ASP.NET
2. Event emitter detection works for Node.js EventEmitter and generic emit/on patterns
3. `ariadne_route_map` returns complete route → handler mapping
4. `ariadne_event_map` returns complete event → producer/consumer mapping
5. <5% false positive rate on route/event detection
6. Boundary data included in `ariadne_file` and `ariadne_dependencies` responses

---

## Phase 9: Recommendation Engine [DONE]

**Goal:** Move from "here is data" to "here is what you should do." Ariadne suggests concrete actions based on graph analysis.

**Depends on:** Phases 4 (symbols), 5 (context), 7 (temporal). Benefits from Phase 8 but not required.

### D22: `ariadne_suggest_split` — File Decomposition

```
Input:  { path: "src/pipeline/pipeline.rs" }
Output: {
    should_split: true,
    reason: "God file: 800 lines, centrality 0.85, 25 exports, 4 distinct responsibility clusters",
    suggested_splits: [
        {
            name: "pipeline_config.rs",
            symbols: ["PipelineConfig", "StageConfig", "BuildOptions"],
            estimated_lines: 120,
            rationale: "Configuration types used by 3 other modules"
        },
        {
            name: "pipeline_runner.rs",
            symbols: ["run_pipeline", "execute_stage", "collect_results"],
            estimated_lines: 250,
            rationale: "Core execution logic, tightly coupled internally"
        }
    ],
    impact: {
        current_blast_radius: 34,
        estimated_per_split: [12, 18],
        centrality_reduction: "0.85 → estimated 0.45 max"
    }
}
```

**Algorithm:** Cluster symbols by their call-graph connectivity within the file. Symbols that form tight internal clusters but are loosely coupled to other clusters = split candidates.

### D23: `ariadne_suggest_placement` — New File Location

```
Input: {
    description: "order validator service",
    depends_on: ["src/model/order.rs", "src/model/pricing.rs"],
    depended_by?: ["src/api/orders.rs"]
}
Output: {
    suggested_path: "src/service/order_validator.rs",
    cluster: "service",
    layer: "service",
    arch_depth: 2,
    reasoning: [
        "Depends on model/ (depth 0-1) → should be depth 2 (service layer)",
        "Similar files exist: src/service/payment_validator.rs, src/service/user_validator.rs",
        "Placing in src/service/ maintains layer ordering (no violations)"
    ],
    alternatives: [
        { path: "src/model/order_validator.rs", risk: "would increase model/ coupling" }
    ]
}
```

### D24: `ariadne_refactor_opportunities` — Proactive Analysis

```
Input:  { scope?: "src/api/", min_impact?: "medium" }
Output: [
    {
        type: "extract_interface",
        target: "src/api/handler.rs",
        symbols: ["UserHandler", "OrderHandler"],
        benefit: "Reduce coupling: 3 dependents reference concrete types",
        effort: "low",
        impact: "medium"
    },
    {
        type: "break_cycle",
        cycle: ["src/api/auth.rs", "src/service/session.rs", "src/api/middleware.rs"],
        suggested_cut: "src/api/auth.rs → src/service/session.rs",
        alternative: "Extract shared types into src/model/auth_types.rs",
        effort: "medium",
        impact: "high"
    },
    {
        type: "merge_modules",
        targets: ["src/util/string_helpers.rs", "src/util/text_utils.rs"],
        reason: "83% symbol overlap, always co-imported, 0 external distinction",
        effort: "low",
        impact: "low"
    }
]
```

### Formal Methods & CS Foundations (Phase 9)

#### FM-9.1: Min-Cut for Optimal File Splitting

**What:** Given a file's internal symbol call graph, the optimal split point is the **minimum cut** — the set of edges whose removal partitions the graph into two components with minimal inter-component connectivity.

**Algorithm:** For small graphs (typical file has 5-50 symbols), Stoer-Wagner algorithm finds global min-cut in O(V³). For bipartition, max-flow/min-cut (Dinic's) in O(V²E). Both trivially fast at symbol-within-file scale.

**Why it matters:** Phase 9 D22 (`ariadne_suggest_split`) plans to cluster symbols by call-graph connectivity. Min-cut formalizes "where to cut" with an optimality guarantee. The heuristic approach (connected components after removing weak edges) may miss the globally optimal partition.

**Extension:** For k-way splitting (file should become 3+ files), use recursive min-cut or spectral k-way partitioning on the symbol graph.

**Application:** Use min-cut as the primary partitioning algorithm in `ariadne_suggest_split`. Report cut weight as "coupling between suggested splits" — lower = cleaner split.

**Evaluate:** Compare min-cut suggestions vs simple Louvain on symbol graph. If they agree >80% → use Louvain (already implemented). If they differ → min-cut likely more precise for this use case.

#### FM-9.2: Pareto Frontier for Recommendation Ranking

**What:** Recommendations have two dimensions: effort (cost) and impact (benefit). A recommendation R **Pareto-dominates** R' if effort(R) ≤ effort(R') AND impact(R) ≥ impact(R'). The Pareto frontier = set of non-dominated recommendations.

**Why it matters:** Simple scoring (effort × impact) forces a single trade-off ratio. Pareto frontier presents ALL optimal trade-offs, letting the consumer choose their preference.

**Application:** `ariadne_refactor_opportunities` returns recommendations tagged with Pareto position:
- `pareto: true` = on the frontier (always worth considering)
- `pareto: false, dominated_by: "R3"` = strictly worse than another recommendation

**Cost:** O(n²) to compute Pareto frontier for n recommendations — trivial.

**Evaluate:** Does Pareto ranking change which recommendations surface vs simple impact sorting? If frontiers typically have 1-2 items → Pareto adds no value over simple sorting.

#### FM-9.3: Network Motifs for Pattern Classification

**What:** Count frequency of small subgraph patterns (motifs) of size 3-4:
- Triangle: A→B→C→A (cycle-3)
- Fan-out: A→B, A→C, A→D (distributor)
- Fan-in: B→A, C→A, D→A (collector)
- Chain: A→B→C→D (pipeline)
- Mutual: A↔B (tight coupling pair)

**Algorithm:** ESU (enumerate subgraphs), O(V × d^k) where d = avg degree, k = motif size. For k=3,4 on sparse dependency graphs — fast.

**Why it matters:** Motif frequency profile characterizes architectural style:
- High triangle count → cyclic architecture (problematic)
- High fan-out → centralized control (potential god modules)
- High chain count → layered pipeline (healthy)
- High mutual count → tight coupling zones

**Application:** Add motif profile to `ariadne_overview` and `ariadne_cluster`. Use in `ariadne_refactor_opportunities` to classify cluster architectural style and suggest appropriate refactoring patterns.

**Evaluate:** Do motif profiles meaningfully differentiate healthy vs unhealthy clusters? Compare motif frequencies in clusters with smells vs clean clusters. If no significant difference → motifs don't add value beyond existing metrics.

#### FM-9.4: Formal Concept Analysis (FCA) for Natural Module Discovery

**What:** Given a set of files F and a set of dependencies D, FCA computes the concept lattice — all maximal pairs (files, shared_deps) where the files share exactly those dependencies and the dependencies are shared by exactly those files.

**Why it matters:** FCA finds "natural modules" that are invisible to both directory structure and Louvain:
- A concept = group of files that have identical dependency signatures
- Concept lattice = hierarchy of these groups from most general to most specific
- Top concepts = major architectural modules. Bottom concepts = tightly coupled file pairs.

**Application:** Compare FCA concepts with Louvain clusters. Where they agree = high-confidence module boundary. Where they disagree = potential clustering error or transitional zone.

**Risk: HIGH.** FCA concept lattice can be exponentially large for dense graphs. May need pruning (iceberg concepts = concepts with support above threshold).

**Evaluate:** Run FCA on 3 test projects. If concept lattice is tractable (<1000 concepts for 3k files) and reveals useful groupings not found by Louvain → include. If lattice explodes or mostly duplicates Louvain → defer to research backlog.

### Moira Integration Notes (Phase 9)

| Moira Component | Change Needed | Priority |
|----------------|--------------|----------|
| **MCP Registry** | Register 3 recommendation tools | HIGH |
| **Metis (Architect)** | Use `ariadne_suggest_placement` when designing new components — data-driven placement | CRITICAL |
| **Metis (Architect)** | Use `ariadne_refactor_opportunities` for informed refactoring decisions | HIGH |
| **Daedalus (Planner)** | Use `ariadne_suggest_split` when plan includes modifying god files — auto-suggest decomposition step | HIGH |
| **Mnemosyne (Reflector)** | Compare task outcomes with refactoring opportunities — "we touched a file that should be split" | MEDIUM |
| **Analytical Pipeline** | `ariadne_refactor_opportunities` as primary tool for `weakness` subtype | HIGH |
| **Quality Gates Q2** | Metis checks `ariadne_suggest_placement` for new files — architectural soundness verification | MEDIUM |

**Estimated Moira effort:** ~Phase 20 (MCP registry, Metis role rewrite for placement/refactoring, Daedalus god-file handling, Q2 gate enhancement).

**Success criteria:**
1. `ariadne_suggest_split` identifies valid decomposition for god files using symbol clustering
2. `ariadne_suggest_placement` recommends correct layer/cluster based on dependency analysis
3. `ariadne_refactor_opportunities` finds cycles, coupling issues, merge candidates
4. All suggestions include effort/impact estimates
5. <10% false positive rate on split/placement suggestions

---

## Phase 10: Config-Aware Import Resolution [DONE]

**Goal:** Resolve imports using language-specific config files — TypeScript path aliases and baseUrl (tsconfig.json), Go module paths (go.mod), Python src-layout (pyproject.toml). Imports that were previously unresolved now produce correct edges.

**Depends on:** Phase 1b (basic resolution pipeline). Independent of Phases 4-9.

**Deliverables:**

- New module `src/parser/config/` with submodules:
  - `mod.rs` — `ProjectConfig`, `ConfigDiscovery` trait, warning codes W030-W033
  - `discovery.rs` — `FsConfigDiscovery` implementation, walks files for config filenames
  - `typescript.rs` — tsconfig.json parsing (JSONC via comment stripping), `paths`/`baseUrl`/`extends` chain resolution, nearest-ancestor lookup for monorepos
  - `go.rs` — go.mod parsing, module path extraction
  - `python.rs` — pyproject.toml parsing, src-layout detection
- ConfigDiscovery pipeline stage between file walking and parsing/resolution (D-120)
- Construction-time config injection via `with_config()` on resolvers (D-118)
- JSONC parsing without new crate dependency (D-119)
- Nearest-ancestor tsconfig lookup for monorepo multi-tsconfig support (D-121)
- Warning codes W030-W033 for config errors (D-122)
- `ProjectConfig` as concrete language-keyed struct (D-123)

**Languages supported:**

| Language | Config file | Features |
|----------|-----------|----------|
| TypeScript/JS | `tsconfig.json` | `paths` aliases, `baseUrl`, `extends` chain, JSONC comments |
| Go | `go.mod` | Module path → directory mapping for intra-module imports |
| Python | `pyproject.toml` | `src`-layout detection for `src/` prefix resolution |

**Decision log entries:** D-118 through D-123.

**Success criteria:**
1. TypeScript imports using `paths` aliases resolve to correct files
2. TypeScript imports using `baseUrl` resolve relative to configured base
3. tsconfig `extends` chains are followed (with circular detection, W031)
4. Monorepo with multiple tsconfigs uses nearest-ancestor for each file
5. Go module-qualified imports resolve within the module directory
6. Python src-layout imports resolve through `src/` prefix
7. Config parse failures produce warnings, not fatal errors (graceful degradation)
8. All existing tests continue to pass (backward compatible)

---

## Moira Integration Summary

### Total Estimated Moira Phases

| Ariadne Phase | Moira Phase | Key Changes | Effort |
|--------------|-------------|-------------|--------|
| Phase 4 (Symbols) | ~Phase 15 | Knowledge matrix + MCP registry + 3 agent roles | MEDIUM |
| Phase 5 (Context) | ~Phase 16 | Daedalus instruction rewrite + 6 agent roles + dispatch + budget | LARGE |
| Phase 6 (MCP) | ~Phase 17 | Resource subscriptions + dispatch rewrite + annotation bridge | MEDIUM |
| Phase 7 (Temporal) | ~Phase 18 | MCP registry + 4 agent roles + quality map + analytical baseline | MEDIUM |
| Phase 8 (Semantic) | ~Phase 19 | MCP registry + 4 agent roles + project model | MEDIUM |
| Phase 9 (Recommendations) | ~Phase 20 | MCP registry + Metis rewrite + Daedalus + Q2 gate | MEDIUM |

### Moira Constitutional Impact

None of the proposed changes violate Moira's 6 Constitution articles:
- Orchestrator still never touches project code (Art 1)
- Pipeline determinism preserved (Art 2)
- Ariadne remains read-only infrastructure, agents never write to `.ariadne/` (existing D-105)
- Exception: `ariadne_annotate` writes to `.ariadne/annotations.json` — Ariadne's own data, not project code. Moira agents call MCP tool, Ariadne writes. Boundary respected.

### Highest-Impact Single Change for Moira

**`ariadne_context`** (Phase 5, D6). Today Daedalus assembles graph context from 4-6 separate reads/queries. One `ariadne_context` call with token budget replaces all of this. Estimated savings: 15-20k tokens per task, 3-5 fewer tool calls, simpler Daedalus logic.

---

## Evolution Decision Log Entries (Planned)

| # | Decision | Phase |
|---|----------|-------|
| D-077 | Symbol extraction via extended LanguageParser trait | 4 |
| D-078 | Symbol index built at load time, not persisted separately | 4 |
| D-079 | Call graph scope: cross-file via imports only (no intra-expression flow) | 4 |
| D-080 | Token-budget-aware context assembly algorithm | 5 |
| D-081 | Task-type-aware relevance scoring in ariadne_context | 5 |
| D-082 | MCP Resources backed by GraphState with change notifications | 6 |
| D-083 | Annotations persisted in .ariadne/annotations.json | 6 |
| D-084 | Bookmarks as named subgraph references | 6 |
| D-085 | Git history via shell-out to git log (no library dependency) | 7 |
| D-086 | Hotspot scoring formula: churn × complexity × blast_radius | 7 |
| D-087 | Boundary extraction as trait-based plugin system | 8 |
| D-088 | HTTP route detection as first boundary pattern (Phase 8a) | 8 |
| D-089 | Recommendation engine uses symbol call-graph clustering for split suggestions | 9 |
| D-090 | External dep tracking via manifest parsing + import site matching | 10 |
| D-118 | Construction-time config injection for ImportResolver | 10 |
| D-119 | JSONC parsing via comment stripping (no new crate) | 10 |
| D-120 | ConfigDiscovery pipeline stage | 10 |
| D-121 | Nearest-ancestor tsconfig lookup for monorepos | 10 |
| D-122 | Warning codes W030-W033 for config errors | 10 |
| D-123 | ProjectConfig as language-keyed struct | 10 |

---

## Formal Methods Catalog

Cross-reference of all CS/mathematical approaches documented in phase-level FM sections. Each approach has an **Evaluate** clause — during implementation, run the evaluation before committing to the approach. If evaluation shows <10% improvement over simpler heuristic, use the heuristic.

### Summary Table

| ID | Approach | Phase | Effort | Expected Value | Status |
|----|----------|-------|--------|---------------|--------|
| FM-4.1 | Dominator Trees (Lengauer-Tarjan) | 4 | LOW | HIGH — single points of failure | EVALUATE |
| FM-4.2 | CHA / RTA for OOP call graphs | 4 | MEDIUM | MEDIUM — sound polymorphic calls | EVALUATE |
| FM-4.3 | DSM Partitioning (alternative clustering) | 4 | HIGH | MEDIUM — compare with Louvain | EVALUATE |
| FM-5.1 | Submodular Knapsack (context assembly) | 5 | LOW | HIGH — optimal token budgeting | EVALUATE |
| FM-5.2 | Information Gain (file prioritization) | 5 | MEDIUM | HIGH — principled relevance | EVALUATE |
| FM-5.3 | Conditional Entropy (reading order) | 5 | MEDIUM | MEDIUM — cognitive load minimization | EVALUATE |
| FM-6.1 | Architecture Conformance (constraint SAT) | 6 | MEDIUM | HIGH — user-defined rules | EVALUATE |
| FM-6.2 | Graph Entropy (structural health) | 6 | LOW | MEDIUM — new health metric | EVALUATE |
| FM-7.1 | Mutual Information (co-change) | 7 | LOW | MEDIUM — better than Jaccard | EVALUATE |
| FM-7.2 | Bayesian Confidence (coupling) | 7 | MEDIUM | MEDIUM — small sample correction | EVALUATE |
| FM-7.3 | Change-Point Detection (PELT) | 7 | MEDIUM | HIGH — when things broke | EVALUATE |
| FM-7.4 | Survival Analysis (Kaplan-Meier) | 7 | HIGH | LOW — stability prediction | EVALUATE |
| FM-8.1 | Abstract Interpretation (string values) | 8 | HIGH | MEDIUM — constructed routes | EVALUATE |
| FM-8.2 | Typed Multigraph (semantic edges) | 8 | MEDIUM | MEDIUM — probabilistic edges | EVALUATE |
| FM-9.1 | Min-Cut (optimal file splitting) | 9 | MEDIUM | HIGH — guaranteed optimal split | EVALUATE |
| FM-9.2 | Pareto Frontier (recommendation ranking) | 9 | LOW | MEDIUM — multi-objective ranking | EVALUATE |
| FM-9.3 | Network Motifs (pattern classification) | 9 | MEDIUM | MEDIUM — architectural style | EVALUATE |
| FM-9.4 | Formal Concept Analysis (natural modules) | 9 | HIGH | MEDIUM — lattice-based grouping | EVALUATE |
| FM-10.1 | Transitive Closure (supply chain depth) | 10 | LOW | LOW — without vuln DB | EVALUATE |

### Evaluation Protocol

For each FM approach during implementation:

1. **Implement** the approach on a branch alongside the simpler heuristic
2. **Benchmark** on 3+ real codebases (varying sizes: small <500 files, medium 1-3k, large 5k+)
3. **Compare** output quality: does the formal approach find things the heuristic misses?
4. **Measure** compute cost: does it fit within performance targets (<10ms for queries, <2s for rebuilds)?
5. **Decide:**
   - Measurably better + acceptable cost → **ADOPT**
   - Marginal improvement → **DEFER** (keep code, don't ship)
   - No improvement or too expensive → **REJECT** with rationale in decision log

### Tier Classification

**Tier A — High confidence, include in phase spec:**
- FM-4.1 (Dominator Trees): LOW effort, unique insight not captured by existing metrics
- FM-5.1 (Submodular Knapsack): LOW effort, provable optimality bound
- FM-6.2 (Graph Entropy): trivial cost (O(V)), no reason to skip
- FM-7.1 (Mutual Information): LOW effort, well-understood improvement over Jaccard

**Tier B — Medium confidence, evaluate during phase implementation:**
- FM-4.2 (CHA/RTA): depends on OOP prevalence in target codebases
- FM-5.2 (Information Gain): depends on whether it differs from simpler distance-based ranking
- FM-5.3 (Conditional Entropy): depends on whether it differs from topo-sort
- FM-6.1 (Architecture Conformance): depends on user demand for custom rules
- FM-7.2 (Bayesian Confidence): depends on small-sample frequency in real git histories
- FM-7.3 (Change-Point Detection): depends on whether detected points are meaningful
- FM-8.2 (Typed Multigraph): depends on Phase 8 design decisions
- FM-9.1 (Min-Cut): depends on whether Louvain on symbol graph is sufficient
- FM-9.2 (Pareto Frontier): depends on recommendation count

**Tier C — Research backlog, evaluate only if simpler approaches fail:**
- FM-4.3 (DSM): only if Louvain proves inadequate for dependency graphs
- FM-7.4 (Survival Analysis): only if churn rate proves insufficient for stability
- FM-8.1 (Abstract Interpretation): only if >20% of routes are constructed
- FM-9.3 (Network Motifs): only if existing metrics fail to classify architectural style
- FM-9.4 (FCA): only if Louvain + directory clustering miss obvious natural modules
- FM-10.1 (Transitive Closure): only if paired with vulnerability data source

---

## Phase 11: Deep Language Support — C# / .NET [DONE]

**Goal:** Full C# and .NET project support: `.csproj` project references, namespace-to-file resolution, NuGet package detection, and framework-aware patterns.

**Depends on:** Phase 10 (config-aware resolution infrastructure).

**Scope:**

- Parse `.csproj` (MSBuild XML) for `<ProjectReference>`, `<PackageReference>`, output paths
- Parse `.sln` for multi-project structure
- Namespace-to-file mapping heuristics (convention-based: namespace segments → directory path)
- Distinguish internal project references vs NuGet packages vs framework assemblies
- Framework-aware patterns:
  - **ASP.NET Core** — controller/service/middleware discovery, DI registration patterns, route detection
  - **Entity Framework** — DbContext relationships, migration detection
  - **Blazor** — component dependency graph (`.razor` files), `@inject` directives
  - **MAUI / Xamarin** — platform-specific project structure
  - **MinimalAPI** — endpoint mapping patterns

**Deliverables:**
- `src/parser/config/csproj.rs` — .csproj/.sln parsing (roxmltree for XML, line-based for .sln)
- Enhanced `src/parser/csharp.rs` — config-aware namespace resolution with project context, .razor support
- `src/detect/framework.rs` — .NET framework detection (ASP.NET, EF, Blazor, MAUI, MinimalAPI, DI, Middleware)
- `src/semantic/dotnet.rs` — .NET boundary extractors (EF DbContext, DI registration)
- `ImportKind::ProjectReference` + `EdgeType::ProjectRef` for cross-project edges
- Warning codes W034-W037 for .NET config errors
- Test fixtures: dotnet-webapi, dotnet-blazor, dotnet-efcore, dotnet-maui
- Decisions D-124 through D-133

---

## Phase 12: Deep Language Support — Java [DONE]

**Goal:** Full Java project support: Gradle/Maven build systems, classpath resolution, and framework-aware patterns.

**Depends on:** Phase 10 (config-aware resolution infrastructure).

**Scope:**

- Parse `build.gradle` / `build.gradle.kts` for source sets, dependencies, multi-module structure
- Parse `pom.xml` for dependencies, modules, parent POM inheritance
- Classpath-aware resolution: map package imports to source directories via build config
- Distinguish internal modules vs Maven Central/local dependencies
- Framework-aware patterns:
  - **Spring Boot** — `@Component`/`@Service`/`@Repository`/`@Controller` detection, `@Autowired` DI graph, `@RequestMapping` route extraction
  - **Jakarta EE (ex-Java EE)** — CDI injection, JAX-RS endpoints, JPA entity relationships
  - **Android** — activity/fragment lifecycle, manifest component registration, resource references
  - **Micronaut** — compile-time DI, `@Controller` endpoints
  - **Quarkus** — CDI extensions, REST endpoints

**Deliverables:**
- `src/parser/config/gradle.rs` — build.gradle parsing (Groovy/Kotlin DSL subset)
- `src/parser/config/maven.rs` — pom.xml parsing
- Enhanced `src/parser/java.rs` — classpath-aware resolution
- Framework detection for Spring, Jakarta, Android patterns
- Test fixtures for each build system and framework

---

## Phase 13: Deep Language Support — TypeScript/JavaScript Frameworks

**Goal:** Framework-aware resolution and dependency extraction for the JS/TS ecosystem beyond basic tsconfig support.

**Depends on:** Phase 10 (tsconfig resolution already done).

**Scope:**

- Bundler alias resolution:
  - **Vite** — `vite.config.ts` `resolve.alias`
  - **Webpack** — `webpack.config.js` `resolve.alias`, `resolve.modules`
  - **Next.js** — `next.config.js` rewrites, automatic `app/` and `pages/` routing
  - **Turbopack** — turbo.json pipeline dependencies
- Framework-aware patterns:
  - **React** — component tree extraction, hook dependency tracking, context provider/consumer graph
  - **Next.js** — page routes from filesystem, server/client component boundaries (`"use client"`), API routes, middleware chain
  - **Vue** — SFC (`.vue`) component parsing, composables, Pinia store dependencies
  - **Angular** — module/component/service DI graph, `NgModule` declarations, lazy-loaded routes
  - **Svelte/SvelteKit** — `.svelte` component parsing, load functions, route structure
  - **Remix** — loader/action dependency graph, route modules
  - **Astro** — `.astro` component parsing, island architecture boundaries

**Deliverables:**
- `src/parser/config/bundler.rs` — Vite/Webpack/Next.js config parsing
- Enhanced `src/parser/typescript.rs` — bundler alias integration
- Framework-specific parsers for Vue SFC, Angular modules, Svelte components
- Test fixtures for each framework

---

## Future (Beyond Phase 13)

- Tier 2/3 language parsers (Kotlin, Swift, C/C++, PHP, Ruby, Dart)
- Config file (.ariadne.toml)
- Plugin system for external parsers
- `ariadne self-update`
- Package manager distribution (brew, nix, AUR)
- Multi-project graph federation (monorepo cross-project dependencies)
- IDE integration (LSP-based real-time graph updates)
- Web dashboard for graph visualization
- Lattice-based layer inference (Galois connection on dependency partial order)
- Node embeddings (node2vec / GNN) for similarity-based recommendations
- Anomaly detection for architectural drift (ML-based, needs training data)
