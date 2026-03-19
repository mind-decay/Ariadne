# Phase 3: MCP Server & Architectural Intelligence — Specification

**Note:** Phase 3 is split into three sub-phases: 3a (MCP Server), 3b (Architectural Intelligence), 3c (Advanced Graph Analytics). This spec covers all three.

## Goal

Ariadne becomes a long-running MCP server that provides instant, queryable access to structural dependency graphs — enabling any MCP-compatible consumer to get architectural insights without re-parsing the codebase.

## Dependencies

**Phase 2a + 2b must be complete.** Phase 3 builds on:

- `ProjectGraph` with `BTreeMap<CanonicalPath, Node>` + `Vec<Edge>` — full graph data model
- `ClusterMap` with directory-based + Louvain community clustering
- Algorithms: Tarjan SCC, Reverse BFS, Brandes centrality, topological sort, subgraph extraction
- `StatsOutput` with centrality, SCCs, layers, summary
- `stats.json` output (produced by every `ariadne build`)
- `GraphReader` trait + `JsonSerializer` reader implementation (deserialization)
- Markdown views (L0 index, L1 cluster, L2 impact)
- CLI query commands (`ariadne query *`, `ariadne views generate`)
- Delta computation (`ariadne update`) with full rebuild on changes, no-op fast path
- Louvain clustering (on by default, `--no-louvain` to disable)
- `ContentHash` on every node (for freshness checks)
- `SubgraphResult` in `model/query.rs`
- Full L1-L4 test suite

**Phase 3b depends on Phase 3a** (MCP tools expose analysis results; `analysis/` module callable from `mcp/tools.rs`).

**Phase 3c depends on Phase 3a only** (tools exposed via MCP; algorithms are pure functions on `ProjectGraph`). Phase 3c does NOT depend on Phase 3b.

## Phase Split

| Phase | Deliverables | Risk |
|-------|-------------|------|
| **3a** | D1 (MCP Server Core), D2 (MCP Tools), D3 (Freshness Engine), D4 (Auto-Update) | YELLOW |
| **3b** | D5 (Martin Metrics), D6 (Smell Detection), D7 (Structural Diff) | YELLOW |
| **3c** | D8 (PageRank), D9 (Hierarchical Compression), D10 (Spectral Analysis) | ORANGE |

Phase 3a delivers the server platform. Phase 3b adds architectural analysis. Phase 3c adds advanced graph analytics.

## Risk Classification

**Overall: YELLOW** (Phase 3a, 3b) / **ORANGE** (Phase 3c)

Phase 3a introduces a new runtime mode (long-running server) with fs watching and background state management — well-specified but has platform-specific complexity. Phase 3b is primarily pure computation on existing data. Phase 3c's spectral analysis (D10) is explicitly flagged as deferrable (D-043).

### Per-Deliverable Risk

| # | Phase | Deliverable | Risk | Rationale |
|---|-------|------------|------|-----------|
| D1 | 3a | MCP Server Core | YELLOW | New runtime mode (long-running server vs one-shot CLI). GraphState with Arc<RwLock> swap. MCP protocol implementation — crate selection needed. `main.rs` dispatch for `serve` subcommand is straightforward (D-045). Lock file mechanism (D-046) needs careful crash recovery. |
| D2 | 3a | MCP Tools | GREEN | 11 tools, each a thin wrapper around existing `algo/` and `views/` functions. 1:1 mapping to existing `ariadne query` CLI commands. JSON-RPC dispatch + response formatting. Error semantics well-defined. |
| D3 | 3a | Freshness Engine | YELLOW | Hash comparison is straightforward (xxHash exists). Confidence scoring formula defined. "Structural confidence" (detecting import changes without re-parsing) has a design gap — needs resolution. |
| D4 | 3a | Auto-Update | ORANGE | `notify` crate for cross-platform fs watching. Debounce logic. Background thread delta rebuild with atomic GraphState swap. Platform-specific edge cases (kqueue vs inotify). Lock file management across crashes. Most moving parts in the phase. |
| D5 | 3b | Martin Metrics | GREEN | Pure computation. Instability and Abstractness are simple ratios from cluster edge counts and file type classification. All inputs exist in `ProjectGraph` and `ClusterMap`. |
| D6 | 3b | Smell Detection | YELLOW | 7 smell patterns with clear thresholds. Most are predicate checks on existing metrics. "Shotgun Surgery" requires per-file blast_radius (potentially expensive). <5% false positive target needs calibration. New `analysis/` module. |
| D7 | 3b | Structural Diff | ORANGE | `StructuralDiff` struct specified but computation has gaps: cycle diffing (old vs new SCC), `ChangeClassification` heuristic undefined, `new_smells`/`resolved_smells` requires running detection on both states. Louvain cluster instability adds noise to cluster change detection. |
| D8 | 3c | PageRank | GREEN | Well-known power iteration algorithm. Float determinism strategy established (D-049). Combined ranking formula defined (D-042). Only risk: convergence on disconnected graphs (standard dangling node handling). |
| D9 | 3c | Hierarchical Compression | YELLOW | Three zoom levels well-specified conceptually. L0 and L1 are aggregations. Token estimation is heuristic. `CompressedNode`/`CompressedEdge` struct definitions are incomplete. |
| D10 | 3c | Spectral Analysis | ORANGE | Deferrable per D-043 ("defer if determinism cost is too high"). Sparse eigensolver is algorithmically complex. Requires heavy crate dependency (`nalgebra-sparse` or `sprs`). Cross-platform float determinism for eigenvalues is hard. Eigenvector sign ambiguity. Decision gate during implementation. |

## Deliverables

### D1: MCP Server Core

**Files:** `src/mcp/mod.rs` (new), `src/mcp/server.rs` (new), `src/mcp/tools.rs` (new), `src/mcp/state.rs` (new), `src/main.rs` (modified — add `serve` subcommand)

Rust MCP server using JSON-RPC over stdio. On startup:

1. Load `graph.json`, `clusters.json`, `stats.json` into memory
2. Build derived indices (reverse adjacency, per-cluster file sets, layer index)
3. If no graph exists → run full build automatically, then load
4. Register MCP tools

**In-memory state (`GraphState`):**

```rust
pub struct GraphState {
    graph: ProjectGraph,
    stats: StatsOutput,
    clusters: ClusterMap,
    reverse_index: BTreeMap<CanonicalPath, Vec<Edge>>,
    layer_index: BTreeMap<u32, Vec<CanonicalPath>>,
    file_hashes: BTreeMap<CanonicalPath, ContentHash>,
    loaded_at: SystemTime,
    freshness: FreshnessState,
}
```

**Binary architecture (D-045):** Single `ariadne` binary with `serve` subcommand. `main.rs` remains sole Composition Root (D-020).

**Threading model (D-047):** No async runtime (no tokio). `notify` uses OS-native watchers. MCP JSON-RPC on main thread (stdio is sequential). Delta rebuild on background thread. `Arc<RwLock<GraphState>>` for state access.

**Memory budget:** Graph + indices for 10k-file project ≈ 50-100MB.

**Module dependency:** `mcp/` depends on `model/`, `algo/`, `analysis/`, `serial/`, `pipeline/` (traits only). Never depends on `parser/` directly. `main.rs` constructs `BuildPipeline` and passes it to the MCP server on startup — `mcp/` depends on pipeline traits, not concrete implementations (D-020 Composition Root preserved).

**CLI extension:**

```
ariadne serve [--project <path>] [--debounce <ms>] [--no-watch]
```

**Design source:** ROADMAP.md §Phase 3a D1, D-020, D-045, D-047

### D2: MCP Tools

**File:** `src/mcp/tools.rs`

Each tool maps 1:1 to an `ariadne query` CLI command, plus server-specific tools. All tools are generic, consumer-agnostic (D-044). Tools T1, T9, and T10 are MCP-only (no CLI equivalent); all others have CLI parity via `ariadne query`.

| # | Tool | Input | Output |
|---|------|-------|--------|
| T1 | `ariadne_overview` | — | Project summary: node/edge counts, language breakdown, layer distribution, critical files, cycles count, max depth. **MCP-only** — no single CLI equivalent (combines graph.json summary + stats.json) |
| T2 | `ariadne_file` | `path: string` | File detail: type, layer, arch_depth, exports, cluster, centrality, incoming/outgoing edges |
| T3 | `ariadne_blast_radius` | `path: string, depth?: number` | Reverse BFS: map of affected files with distances |
| T4 | `ariadne_subgraph` | `paths: string[], depth?: number` | Filtered graph: nodes + edges + clusters in neighborhood |
| T5 | `ariadne_centrality` | `min?: number` | Bottleneck files sorted by centrality score |
| T6 | `ariadne_cycles` | — | All SCCs (circular dependencies) |
| T7 | `ariadne_layers` | `layer?: number` | Topological layers: files per arch_depth level |
| T8 | `ariadne_cluster` | `name: string` | Cluster detail: files, internal/external deps, cohesion, tests |
| T9 | `ariadne_dependencies` | `path: string, direction: "in"\|"out"\|"both"` | Direct dependencies of a file (not transitive). **MCP-only** — `ariadne query file` shows edges but doesn't filter by direction |
| T10 | `ariadne_freshness` | — | Graph freshness: overall confidence, stale files list, last update time. **MCP-only** — server-specific, no CLI equivalent |
| T11 | `ariadne_views_export` | `level: "L0"\|"L1"\|"L2", cluster?: string` | Pre-generated markdown views from `.ariadne/views/` |

**Phase 3b adds:** `ariadne_metrics` (D5), `ariadne_smells` (D6), `ariadne_diff` (D7). CLI parity: `ariadne query metrics`, `ariadne query smells`. `ariadne_diff` is MCP-only (requires in-memory pre-update state; see DP-15).

**Phase 3c adds:** `ariadne_importance` (D8), `ariadne_compressed` (D9), `ariadne_spectral` (D10, if implemented). CLI parity: `ariadne query importance`, `ariadne query compressed`.

**Response format:** All tools return structured JSON. Token-efficient — no prose, just data.

**Error semantics:**
- File not in graph → `{ "error": "not_found", "path": "...", "suggestion": "File may be new. Graph freshness: 87%" }`
- Graph not built → auto-trigger build, return result after build completes
- Stale graph → return data + `"freshness": { "confidence": 0.73, "stale_files": [...] }` field

**Design source:** ROADMAP.md §Phase 3a D2, D-044

### D3: Freshness Engine

**File:** `src/mcp/state.rs`

```rust
pub struct FreshnessState {
    stale_files: BTreeSet<CanonicalPath>,
    new_files: Vec<PathBuf>,
    removed_files: Vec<CanonicalPath>,
    structural_confidence: f64,
    last_full_check: SystemTime,
}
```

**Algorithm:**
1. On query: compare in-memory hash vs current file hash for queried files
2. Confidence score: `confidence = 1 - (stale_files / total_files)`
3. Per-file staleness: track files with hash mismatch
4. Structural confidence: degrade based on stale count (pessimistic — any hash mismatch is potential structural change until delta rebuild confirms)

**Confidence thresholds (D-039):**
- ≥0.95 → graph is fresh, use as-is
- 0.80-0.95 → minor staleness, results reliable for structural queries
- 0.50-0.80 → noticeable drift, flag to user, auto-update recommended
- <0.50 → graph significantly outdated, auto-rebuild triggered

**Performance:** Freshness check (single file hash): <1ms.

**Design source:** ROADMAP.md §Phase 3a D3, D-039

### D4: Auto-Update Mechanism

**File:** `src/mcp/state.rs`

**Strategy:** File system watcher + debounced delta rebuild (D-038).

1. **fs watcher** (`notify` crate): watch project directory for source file changes
2. **Debounce:** Collect changes for 2 seconds after last modification (configurable via `--debounce`)
3. **Delta rebuild:** Run `ariadne update` logic (Phase 2b) on changed files
4. **Hot reload:** Swap `GraphState` atomically via `Arc<RwLock<GraphState>>`
5. **Fallback:** If watcher unavailable → poll-based check every 30 seconds

**Lock file (D-046):**
- `.ariadne/graph/.lock` created on server startup (contains PID + timestamp)
- CLI `build`/`update` check for lock; refuse if server is running
- Stale lock detection: check if PID is alive; auto-remove if dead
- Lock released on server shutdown (SIGINT/SIGTERM handler)

**Graceful degradation:**
- fs watcher fails → fall back to poll every 30s
- Delta fails → fall back to full rebuild
- Full rebuild fails → serve stale graph with freshness warning
- Graph files missing → auto-run initial build
- Build in progress → serve stale data with `"rebuilding": true` flag

**Performance targets:**
- Delta rebuild (10 changed files / 3k project): <2s
- Full rebuild (3k files): <10s
- MCP tool response (in-memory query): <10ms
- Freshness check (single file hash): <1ms

**New dependency:** `notify` crate (v6, OS-native file watching).

**Design source:** ROADMAP.md §Phase 3a D4, D-038, D-046, D-047

### D5: Martin Metrics (Instability & Abstractness)

**Files:** `src/analysis/mod.rs` (new), `src/analysis/metrics.rs` (new)

**Instability:** `I = Ce / (Ca + Ce)` per cluster
- `Ca` = afferent coupling (incoming edges from other clusters)
- `Ce` = efferent coupling (outgoing edges to other clusters)

**Abstractness:** `A = Na / Nc` per cluster
- `Na` = abstract files (`FileType::type_def` + files where all exports are re-exports)
- `Nc` = total files in cluster

**Distance from Main Sequence:** `D = |A + I - 1|`

**Zone classification (D-040):**
- `D < 0.3` → Main Sequence (good balance)
- High `D` with low `A`, low `I` → Zone of Pain (concrete and stable — hard to change)
- High `D` with high `A`, high `I` → Zone of Uselessness (abstract and unstable — no real dependents)

Computed at cluster level (D-040). Float results rounded to 4 decimal places (D-049).

**Module dependency (D-048):** `analysis/` depends on `model/`, `algo/`. Never depends on `serial/`, `pipeline/`, `parser/`, `mcp/`.

**MCP tool:** `ariadne_metrics` → per-cluster `{instability, abstractness, distance, zone}`

**Design source:** ROADMAP.md §Phase 3b D5, D-040, D-049

### D6: Architectural Smell Detection

**Files:** `src/analysis/smells.rs` (new), `src/model/smell.rs` (new)

**Data types (in `model/smell.rs`):**

```rust
pub struct ArchSmell {
    pub smell_type: SmellType,
    pub files: Vec<CanonicalPath>,
    pub severity: SmellSeverity,
    pub explanation: String,
}

pub enum SmellSeverity { High, Medium, Low }

pub enum SmellType {
    GodFile,
    CircularDependency,
    LayerViolation,
    HubAndSpoke,
    UnstableFoundation,
    DeadCluster,
    ShotgunSurgery,
}
```

`ArchSmell`, `SmellSeverity`, and `SmellType` live in `model/` so both `analysis/` and `mcp/` can reference them.

**Detection rules:**

| Smell | Detection | Severity |
|-------|----------|----------|
| God File | Centrality > 0.8 AND out-degree > 20 AND lines > 500 | HIGH |
| Circular Dependency | SCC size > 1 (from Phase 2 Tarjan) | HIGH |
| Layer Violation | Edge from lower `arch_depth` to higher | MEDIUM |
| Hub-and-Spoke | One file has >50% of cluster's external edges | MEDIUM |
| Unstable Foundation | Cluster with `I > 0.7` AND `Ca > 10` | HIGH |
| Dead Cluster | Cluster with 0 incoming external edges AND not top-level entry point | LOW |
| Shotgun Surgery | File with blast radius > 30% of project | HIGH |

**Success criterion:** <5% false positive rate on known-good architectures.

**MCP tool:** `ariadne_smells` → detected smells with file paths, severity, explanation.

**Design source:** ROADMAP.md §Phase 3b D6, D-048

### D7: Structural Diff

**Files:** `src/analysis/diff.rs` (new), `src/model/diff.rs` (new)

**Data types (in `model/diff.rs`):**

```rust
pub struct StructuralDiff {
    pub added_nodes: Vec<CanonicalPath>,
    pub removed_nodes: Vec<CanonicalPath>,
    pub added_edges: Vec<Edge>,
    pub removed_edges: Vec<Edge>,
    pub changed_layers: Vec<(CanonicalPath, u32, u32)>,
    pub changed_clusters: Vec<(CanonicalPath, ClusterId, ClusterId)>,
    pub new_cycles: Vec<Vec<CanonicalPath>>,
    pub resolved_cycles: Vec<Vec<CanonicalPath>>,
    pub new_smells: Vec<ArchSmell>,
    pub resolved_smells: Vec<ArchSmell>,
    pub summary: DiffSummary,
}

pub struct DiffSummary {
    pub structural_change_magnitude: f64,
    pub change_type: ChangeClassification,
}

pub enum ChangeClassification { Additive, Refactor, Migration, Breaking }
```

**Change magnitude:**
`magnitude = (|added_edges| + |removed_edges| + |added_nodes| + |removed_nodes|) / (2 * (|edges| + |nodes|))`

Computed in `analysis/diff.rs`. `StructuralDiff` lives in `model/diff.rs` (pure data type).

**MCP tool:** `ariadne_diff` → structural diff since last update.

**Design source:** ROADMAP.md §Phase 3b D7

### D8: PageRank for File Importance

**Files:** `src/algo/pagerank.rs` (new)

```rust
pub fn pagerank(
    graph: &ProjectGraph,
    damping: f64,          // 0.85
    max_iterations: u32,   // 100
    tolerance: f64,        // 1e-6
) -> BTreeMap<CanonicalPath, f64>
```

Power iteration on the transition matrix. O(V + E) per iteration, converges in 20-50 iterations.

**Combined ranking (D-042):** `combined_score = 0.5 * normalized_centrality + 0.5 * normalized_pagerank`

**Float determinism (D-049):** Round to 4 decimal places, BTreeMap iteration order, fixed parameters.

**Module:** `algo/` — depends on `model/` only.

**MCP tool:** `ariadne_importance` → files ranked by combined score.

**Performance:** `bench_pagerank` on 3k-node graph: <100ms.

**Design source:** ROADMAP.md §Phase 3c D8, D-042, D-049

### D9: Hierarchical Graph Compression

**Files:** `src/algo/compress.rs` (new — computation), `src/model/compress.rs` (new — data types)

**Data types (in `model/compress.rs`):**

```rust
pub enum CompressionLevel { Project, Cluster, File }

/// L0: one node per cluster
pub struct CompressedNode {
    pub name: String,            // cluster name (L0), file path (L1/L2)
    pub file_count: Option<u32>, // L0 only
    pub cohesion: Option<f64>,   // L0 only
    pub key_files: Vec<String>,  // L0: highest centrality files; L1/L2: empty
    pub node_type: Option<String>, // L1/L2: file type
    pub layer: Option<String>,   // L1/L2: arch layer
}

pub struct CompressedEdge {
    pub from: String,
    pub to: String,
    pub weight: u32,             // L0: number of inter-cluster edges; L1/L2: 1
}

pub struct CompressedGraph {
    pub level: CompressionLevel,
    pub nodes: Vec<CompressedNode>,
    pub edges: Vec<CompressedEdge>,
    pub token_estimate: u32,     // heuristic: serialized_json_bytes / 4
}
```

**Compression levels:**
- **L0 (Project):** ~10-30 nodes (clusters). Token budget: ~200-500.
- **L1 (Cluster):** ~50-200 nodes (files). Token budget: ~500-2000 per cluster.
- **L2 (File):** Full detail for file + N-hop neighborhood. Token budget: ~200-1000.

**MCP tool:** `ariadne_compressed(level: 0|1|2, focus?: string)`

**Performance:** `bench_compression_l0` on 10k-node graph: <50ms.

**Design source:** ROADMAP.md §Phase 3c D9, D-041

### D10: Spectral Analysis (Fiedler Vector) — CONDITIONAL

**Files:** `src/algo/spectral.rs` (new, if implemented)

**Risk: ORANGE** (D-043). Sparse eigensolver determinism is hard. D-043 says "defer if determinism cost is too high."

Computes algebraic connectivity (λ₂) and Fiedler vector via sparse Laplacian + Lanczos iteration.

**Practical value:** Detect monolithic structure, identify natural refactoring boundaries, validate Louvain clusters.

**New dependency:** `nalgebra-sparse` or `sprs` crate.

**MCP tool:** `ariadne_spectral` → `{ algebraic_connectivity, natural_partitions, monolith_score }`

**Decision gate:** Evaluate feasibility during implementation. Defer if cross-platform float determinism cost is too high.

**Design source:** ROADMAP.md §Phase 3c D10, D-043

## Module Structure Changes

```
src/
├── (existing Phase 1-2 modules unchanged)
├── algo/
│   ├── (existing files)
│   ├── pagerank.rs          # NEW — PageRank power iteration (D8)
│   ├── compress.rs          # NEW — Hierarchical compression logic (D9)
│   └── spectral.rs          # NEW — Spectral analysis, if implemented (D10)
├── analysis/                # NEW — depends on model/, algo/ (D-048)
│   ├── mod.rs               # Re-exports
│   ├── metrics.rs           # Martin metrics: instability, abstractness (D5)
│   ├── smells.rs            # Architectural smell detection (D6)
│   └── diff.rs              # Structural diff computation (D7)
├── mcp/                     # NEW — depends on model/, algo/, analysis/, serial/, pipeline/ traits (D-045)
│   ├── mod.rs               # Re-exports
│   ├── server.rs            # JSON-RPC server, MCP protocol handling
│   ├── tools.rs             # MCP tool implementations (dispatch to algo/analysis)
│   └── state.rs             # GraphState, FreshnessState, auto-update logic
└── model/
    ├── (existing files)
    ├── diff.rs              # NEW — StructuralDiff, DiffSummary, ChangeClassification
    ├── smell.rs             # NEW — ArchSmell, SmellSeverity, SmellType (pure data types)
    └── compress.rs          # NEW — CompressedGraph, CompressedNode, CompressedEdge, CompressionLevel
```

**Updated dependency rules:**

| Module | Depends on | Never depends on |
|--------|-----------|-----------------|
| `analysis/` | `model/`, `algo/` | `serial/`, `pipeline/`, `parser/`, `mcp/` |
| `mcp/` | `model/`, `algo/`, `analysis/`, `serial/`, `pipeline/` | `parser/` (pipeline handles parsing) |

## New Error Codes

### Fatal Errors (Phase 3)

Phase 3 reuses existing E006-E009 (from Phase 2). New error conditions for MCP server to be defined during implementation planning.

### Warnings (Phase 3)

Phase 3 reuses existing W010-W013 (from Phase 2). New warning conditions for fs watcher failures, freshness issues, and analysis errors to be defined during implementation planning.

## Design Sources

| Deliverable | Authoritative Sources |
|-------------|----------------------|
| D1: MCP Server Core | ROADMAP.md §Phase 3a D1, D-020, D-045, D-046, D-047 |
| D2: MCP Tools | ROADMAP.md §Phase 3a D2, D-044, architecture.md §CLI Interface |
| D3: Freshness Engine | ROADMAP.md §Phase 3a D3, D-039 |
| D4: Auto-Update | ROADMAP.md §Phase 3a D4, D-038, D-046, D-047 |
| D5: Martin Metrics | ROADMAP.md §Phase 3b D5, D-040, D-048, D-049 |
| D6: Smell Detection | ROADMAP.md §Phase 3b D6, D-048 |
| D7: Structural Diff | ROADMAP.md §Phase 3b D7, D-048 |
| D8: PageRank | ROADMAP.md §Phase 3c D8, D-042, D-049 |
| D9: Compression | ROADMAP.md §Phase 3c D9, D-041 |
| D10: Spectral | ROADMAP.md §Phase 3c D10, D-043, D-049 |

## Success Criteria

### Phase 3a

1. `ariadne serve` starts MCP server, loads graph, answers queries via stdio JSON-RPC
2. All 11 MCP tools return correct results matching CLI `ariadne query` equivalents
3. fs watcher triggers delta rebuild within 2s of file change
4. Freshness confidence score accurately reflects graph staleness
5. Server handles missing/corrupted graph gracefully (auto-rebuild)
6. MCP tool response latency <10ms for in-memory queries
7. Server operates correctly as Claude Code MCP server (settings.json registration)
8. Lock file prevents concurrent CLI writes while server is running
9. `ariadne_views_export` returns generic markdown views (no consumer-specific formatting)

### Phase 3b

10. Martin metrics computed for all clusters, detect Zone of Pain / Zone of Uselessness
11. Architectural smell detection identifies known anti-patterns with <5% false positive rate
12. Structural diff correctly captures added/removed edges, new/resolved cycles
13. All metrics deterministic (byte-identical output)

### Phase 3c

14. PageRank converges within 100 iterations, results deterministic to 4 decimal places
15. Hierarchical compression produces valid graphs at all 3 levels
16. L0 compressed graph fits within 500 tokens for projects up to 10k files
17. Spectral analysis (if implemented) returns algebraic connectivity and natural partitions

## Testing Requirements

### MCP Integration Tests (Phase 3a)
- Start server → send tool request → verify response matches CLI output
- File change → verify auto-rebuild → verify tool returns updated data
- Missing graph → verify auto-build → verify tools work after build
- Corrupted graph → verify graceful fallback
- Lock file test: start server → CLI build/update → verify refusal with clear message
- Lock file cleanup: server exits → verify lock released → CLI works

### Freshness Tests (Phase 3a)
- Modify file → check confidence drops
- Modify file body (no import changes) → structural confidence stays high
- Add new file → confidence reflects new file not in graph
- Delete file → confidence reflects removed file

### Architectural Intelligence Tests (Phase 3b)
- Hand-crafted graphs with known Martin metrics
- Known architectural smells → verify detection
- Known clean architectures → verify no false positives
- Structural diff on controlled changes → verify diff accuracy

### Performance Benchmarks (Phase 3)

| Benchmark | Target |
|-----------|--------|
| `bench_mcp_overview` on 3k-node graph | <5ms |
| `bench_mcp_blast_radius` on 3k-node graph | <10ms |
| `bench_pagerank` on 3k-node graph | <100ms |
| `bench_compression_l0` on 10k-node graph | <50ms |
| `bench_auto_update` (10 files changed, 3k project) | <2s |

### Invariant Extensions
- Martin metrics: I and A in [0.0, 1.0], D in [0.0, 1.0]
- Smell detection: all referenced files must exist in graph
- StructuralDiff: added/removed edges must reference valid nodes
- PageRank: all values in [0.0, 1.0], sum ≈ 1.0
- CompressedGraph: L0 node count = cluster count

## Discussion Points

The following design gaps require resolution before implementation planning. They are prioritized by blocking impact.

### Blocking (must resolve before implementation)

**DP-1: MCP Rust Crate Selection**
The design specifies "JSON-RPC over stdio" but names no Rust crate. Options: (a) `rmcp` crate (Rust MCP SDK), (b) minimal hand-rolled JSON-RPC handler. D-047 explicitly avoids async runtime (tokio), so the selected solution must support sync/blocking mode. Which MCP protocol version (2024-11-05?)? Which capabilities (tools only, or also resources/prompts)?

**DP-2: Query-During-Rebuild Threading Semantics**
D-047 says `Arc<RwLock<GraphState>>`. ROADMAP says "serve stale data with `rebuilding: true` flag" during rebuild. But if the background thread holds a write lock for the swap, the main thread's read lock acquisition blocks. Options: (a) double-buffer — build new state in separate allocation, then quick Arc swap, (b) `RwLock::try_read` with stale snapshot fallback. Which approach?

**DP-3: Structural Confidence Computation**
D3 says "if stale files have no new/removed imports → structure is still valid." But detecting import changes requires re-parsing. Options: (a) pessimistic — any hash mismatch degrades confidence (simplest), (b) lightweight re-parse of import sections only, (c) defer structural confidence to post-delta-rebuild. Recommend option (a) for simplicity.

**DP-4: PageRank Graph Direction**
The import graph has edges A→B when A imports B. Standard PageRank on this graph ranks high-out-degree files highly. But D8's stated goal is to find files "many important files depend on" — which requires the reversed graph. Must explicitly state: PageRank runs on the reversed import graph (edges point from dependency to dependent).

**DP-5: True Incremental Re-Parsing vs Full Rebuild for <2s Target**
D-050 documents that `ariadne update` currently does full rebuild. Phase 3a D4 targets <2s for 10 changed files. Full rebuild of 3k files takes <10s. Either (a) Phase 3 must implement true incremental re-parsing (using `algo/delta.rs` scaffolding to selectively re-parse), or (b) revise the <2s target.

### High Priority

**DP-6: Lock File Format and Stale Detection**
D-046 says `.ariadne/graph/.lock` but doesn't specify: contents (PID? timestamp?), stale detection (check PID alive?), cleanup on SIGKILL, Windows PID checking differences.

**DP-7: New Error Codes for Phase 3**
`error-handling.md` covers E001-E009, W001-W013. Phase 3 introduces new conditions: MCP server startup failure, lock file held, watcher failure, MCP protocol errors. Need E010+ and W014+ definitions.

**DP-8: Abstract File Classification for Martin Metrics**
D5 says `Na = abstract files (type_def, interfaces, re-export barrels)`. `FileType` enum has `type_def` but not `interface` or `barrel`. How to classify barrel files (index.ts with only re-exports) — they have `FileType::source`. Need precise classification rules.

**DP-9: `StatsOutput` Module Location for `analysis/` Dependency** — LIKELY RESOLVED
D-048 says `analysis/` depends on `model/` and `algo/`, never on `serial/`. `StatsOutput` originally lived in `serial/mod.rs`. Phase 2a implementation moved it to `model/stats.rs` (confirmed by `src/model/stats.rs` in working tree). Verify during implementation planning that `analysis/` can access centrality data through `model/` without depending on `serial/`.

### Medium Priority

**DP-11: Auto-Update File Pattern Filtering**
D4 says "watch project directory for file changes" but doesn't specify which files trigger rebuild. Should filter to recognized parser extensions + workspace config files. Changes to README.md should not trigger rebuild.

**DP-12: Graceful Shutdown Behavior**
The design describes startup but not shutdown. SIGINT/SIGTERM handling? Flush state to disk before exit? Lock file release in signal handler? Crash (SIGKILL) leaves orphaned lock.

**DP-13: Smell Detection Threshold Justification**
D6's thresholds (centrality > 0.8, out-degree > 20, lines > 500, etc.) have no empirical justification. Should these be documented as initial heuristics subject to calibration? The <5% false positive target has no defined evaluation methodology.

**DP-14: Structural Diff and Louvain Cluster Instability**
`changed_clusters` in StructuralDiff may contain noise from Louvain reassignments even when the graph hasn't changed structurally. Need a strategy to distinguish meaningful cluster changes from optimization noise.

**DP-15: StructuralDiff Availability — MCP-Only or Also CLI?**
`ariadne_diff` returns diff "since last update." In MCP, the pre-update state is in memory. For CLI `ariadne update`, where is the "before" snapshot stored? Is structural diff MCP-only?

**DP-16: Token Estimation Algorithm**
`CompressedGraph.token_estimate` — how computed? Simple heuristic: `serialized_json_bytes / 4`?

**DP-17: ChangeClassification Heuristic**
`DiffSummary.change_type` is `Additive | Refactor | Migration | Breaking` but no heuristic is defined for classification. What conditions map to each variant?

**DP-18: MCP Integration Test Harness**
`testing.md` has no section for Phase 3. How to test a long-running MCP server? Spawn subprocess, send JSON-RPC via stdin, read from stdout? Define L5 test level?

**DP-19: Spectral Analysis Eigenvector Sign Ambiguity**
Eigenvectors are unique up to sign. The Fiedler vector `v` and `-v` give opposite partitions. Need a sign convention (e.g., lexicographically first node's component is positive). D-049 doesn't address this.
