# Phase 3c: Advanced Graph Analytics — Specification

## Goal

Apply techniques from information retrieval and spectral graph theory to provide deeper ranking insights and efficient graph representations for large codebases. Adds PageRank-based file importance, hierarchical graph compression for token-efficient consumption, and optional spectral analysis.

## Dependencies

**Phase 3a must be complete.** Phase 3c builds on:

- MCP server (`ariadne serve`) with `ArcSwap<GraphState>`, tool registration via `rmcp`
- `GraphState` with `ProjectGraph`, `StatsOutput`, `ClusterMap`, derived indices
- Algorithms: Tarjan SCC, Reverse BFS, Brandes centrality, topological sort, subgraph extraction
- `ContentHash` on every node
- Auto-update with incremental rebuild and atomic state swap

**Phase 3c does NOT depend on Phase 3b.** They are independent extensions of Phase 3a.

## Risk Classification

**Overall: ORANGE**

PageRank and compression are well-understood algorithms with clear implementations. Spectral analysis (D10) is explicitly flagged as deferrable (D-043) due to sparse eigensolver complexity and cross-platform float determinism concerns.

### Per-Deliverable Risk

| # | Deliverable | Risk | Rationale |
|---|------------|------|-----------|
| D8 | PageRank | GREEN | Well-known power iteration. Float determinism strategy established (D-049). Only risk: dangling node handling in disconnected graphs. |
| D9 | Hierarchical Compression | YELLOW | Three zoom levels conceptually clear. L0 and L1 are aggregations. Token estimation is heuristic. |
| D10 | Spectral Analysis | ORANGE | Deferrable per D-043. Sparse eigensolver complexity. Requires external crate (`nalgebra-sparse` or `sprs`). Cross-platform float determinism for eigenvalues is hard. Eigenvector sign ambiguity (resolved via D-060). **Decision gate during implementation.** |

## Deliverables

### D8: PageRank for File Importance

**New file:** `src/algo/pagerank.rs`
**Modified files:** `src/algo/mod.rs`, `src/mcp/tools.rs`, `src/main.rs`

PageRank measures *authority* — files that many important files depend on. Complementary to Brandes centrality (which measures *betweenness* — files on many shortest paths).

**Graph direction (resolves DP-4):**

PageRank runs on the **reversed** import graph. The original import graph has edges A→B when A imports B. Reversed: B→A. This way PageRank ranks files that are imported by many important files — the foundations of the codebase.

**Algorithm: Power iteration on reversed transition matrix.**

```rust
pub fn pagerank(
    graph: &ProjectGraph,
    damping: f64,          // 0.85
    max_iterations: u32,   // 100
    tolerance: f64,        // 1e-6
) -> BTreeMap<CanonicalPath, f64>
```

**Implementation details:**

1. Build reversed adjacency from `graph.edges` (only `EdgeType::imports` and `EdgeType::re_exports` — skip `tests` and `type_imports` edges)
2. Initialize all ranks to `1.0 / N`
3. Per iteration:
   - For each node, compute `new_rank = (1 - damping) / N + damping * sum(rank[in_neighbor] / out_degree[in_neighbor])`
   - Dangling nodes (zero out-degree in reversed graph = files that import nothing): distribute their rank equally to all nodes (standard dangling node handling)
4. Convergence: `max(|new_rank - old_rank|) < tolerance` or `iterations >= max_iterations`
5. Iteration order: lexicographic by `CanonicalPath` (BTreeMap keys) for determinism

**Float determinism (D-049):** Round final results to 4 decimal places. Fixed parameters (damping=0.85, max_iter=100, tolerance=1e-6). BTreeMap iteration order. Deterministic across platforms.

**Combined ranking (D-042):**

```rust
pub fn combined_importance(
    centrality: &BTreeMap<CanonicalPath, f64>,
    pagerank: &BTreeMap<CanonicalPath, f64>,
) -> BTreeMap<CanonicalPath, f64>
```

`combined_score = 0.5 * normalized_centrality + 0.5 * normalized_pagerank`

Normalization: divide each score by the maximum value in its respective map (so max = 1.0).

**Module:** `algo/` — depends on `model/` only.

**MCP tool:** `ariadne_importance [top?: number]` → files ranked by combined score. Default: top 20. Returns `{ path, combined_score, centrality, pagerank }` per file.

**CLI parity:** `ariadne query importance [--top N] [--format json|md]`

**Performance:** `bench_pagerank` on 3k-node graph: <100ms.

**Design source:** ROADMAP.md Phase 3c D8, D-042, D-049

### D9: Hierarchical Graph Compression

**New files:** `src/algo/compress.rs`, `src/model/compress.rs`
**Modified files:** `src/model/mod.rs`, `src/algo/mod.rs`, `src/mcp/tools.rs`, `src/main.rs`

For large codebases (10k+ files), sending full graph data to MCP consumers is too expensive in tokens. Hierarchical compression provides zoom levels.

**Data types (in `model/compress.rs`):**

```rust
pub enum CompressionLevel {
    Project,   // L0: cluster-level view
    Cluster,   // L1: file-level within a cluster
    File,      // L2: single file + N-hop neighborhood
}

pub struct CompressedNode {
    pub name: String,                  // cluster name (L0), file path (L1/L2)
    pub node_type: CompressedNodeType,
    pub file_count: Option<u32>,       // L0 only
    pub cohesion: Option<f64>,         // L0 only
    pub key_files: Vec<String>,        // L0: top-3 by centrality; L1/L2: empty
    pub file_type: Option<String>,     // L1/L2: source/test/config/etc
    pub layer: Option<String>,         // L1/L2: arch layer name
    pub centrality: Option<f64>,       // L1/L2: betweenness centrality
}

pub enum CompressedNodeType {
    Cluster,  // L0
    File,     // L1/L2
}

pub struct CompressedEdge {
    pub from: String,
    pub to: String,
    pub weight: u32,    // L0: count of inter-cluster edges; L1/L2: 1
    pub edge_type: Option<String>,  // L1/L2: imports/tests/re_exports/type_imports
}

pub struct CompressedGraph {
    pub level: CompressionLevel,
    pub focus: Option<String>,         // cluster name (L1) or file path (L2)
    pub nodes: Vec<CompressedNode>,
    pub edges: Vec<CompressedEdge>,
    pub token_estimate: u32,
}
```

**Compression levels:**

**Level 0 (Project):** ~10-30 nodes. Each node = one cluster. Edges = aggregated inter-cluster dependencies with weight = edge count. Key files = top-3 by centrality per cluster.

```rust
pub fn compress_l0(
    graph: &ProjectGraph,
    clusters: &ClusterMap,
    stats: &StatsOutput,
) -> CompressedGraph
```

**Level 1 (Cluster):** ~50-200 nodes. All files in the specified cluster. Full internal edges. External edges simplified to just count per target cluster (not individual file edges).

```rust
pub fn compress_l1(
    graph: &ProjectGraph,
    clusters: &ClusterMap,
    stats: &StatsOutput,
    cluster_name: &ClusterId,
) -> Result<CompressedGraph, String>  // Err if cluster not found
```

**Level 2 (File):** Full detail for a specific file and its N-hop neighborhood (default N=2). All edges, all metadata.

```rust
pub fn compress_l2(
    graph: &ProjectGraph,
    clusters: &ClusterMap,
    stats: &StatsOutput,
    file_path: &CanonicalPath,
    depth: u32,  // default: 2
) -> Result<CompressedGraph, String>  // Err if file not found
```

**Token estimation (resolves DP-16):**

```rust
fn estimate_tokens(graph: &CompressedGraph) -> u32 {
    let json_bytes = serde_json::to_string(graph).unwrap().len();
    (json_bytes / 4) as u32  // 1 token ≈ 4 bytes of JSON
}
```

Simple heuristic. Provides order-of-magnitude guidance, not exact count.

**Token budgets:**
- L0: ~200-500 tokens (project overview)
- L1: ~500-2000 tokens per cluster
- L2: ~200-1000 tokens per file neighborhood

**MCP tool:** `ariadne_compressed(level: 0|1|2, focus?: string, depth?: number)` → compressed graph at specified level. `focus` required for L1 (cluster name) and L2 (file path). `depth` only for L2 (default: 2).

**CLI parity:** `ariadne query compressed --level 0|1|2 [--focus <name>] [--depth N] [--format json|md]`

**Performance:** `bench_compression_l0` on 10k-node graph: <50ms.

**Design source:** ROADMAP.md Phase 3c D9, D-041

### D10: Spectral Analysis (Fiedler Vector) — CONDITIONAL

**New file:** `src/algo/spectral.rs` (if implemented)
**Modified files:** `src/algo/mod.rs`, `src/mcp/tools.rs`, `src/main.rs`

**Risk: ORANGE** (D-043). This deliverable has an explicit **decision gate** during implementation. If cross-platform float determinism cost is too high, defer to future phase.

Computes algebraic connectivity (second-smallest eigenvalue of the graph Laplacian, lambda_2) and Fiedler vector via sparse Laplacian + Lanczos iteration.

**Practical value:**
- **lambda_2 value:** Measures overall graph connectivity. Low lambda_2 → graph is close to splitting (natural module boundaries). High lambda_2 → tightly connected (monolith).
- **Fiedler vector:** Natural bisection. Sign of each component indicates partition membership. Reveals the *natural* division of the codebase.
- **Monolith score:** `monolith_score = lambda_2 / max_eigenvalue` — normalized connectivity. Higher = more monolithic.

**Sign convention (resolves DP-19):**

Eigenvectors are unique up to sign. Convention: the lexicographically first node (by `CanonicalPath`) always gets a **positive** component. If the raw Fiedler vector gives it a negative component, flip all signs. This is deterministic.

**Algorithm:**

```rust
pub fn spectral_analysis(
    graph: &ProjectGraph,
    max_iterations: u32,   // 200
    tolerance: f64,        // 1e-6
) -> SpectralResult

pub struct SpectralResult {
    pub algebraic_connectivity: f64,   // lambda_2, rounded to 4 decimals
    pub monolith_score: f64,           // lambda_2 / lambda_max, rounded to 4 decimals
    pub natural_partitions: Vec<SpectralPartition>,
}

pub struct SpectralPartition {
    pub partition_id: u32,             // 0 or 1
    pub files: Vec<CanonicalPath>,     // sorted
}
```

**Implementation approach:**
1. Build symmetric graph Laplacian `L = D - A` where `D` = degree matrix, `A` = adjacency (undirected — treat all edges as bidirectional)
2. Sparse representation (most entries are zero)
3. Lanczos iteration for second-smallest eigenvalue
4. Fiedler vector: eigenvector corresponding to lambda_2
5. Partition by sign: positive → partition 0, negative → partition 1

**New dependency:** `sprs` crate (sparse matrices) — lighter than `nalgebra-sparse`, sufficient for Lanczos.

**Float determinism (D-049):** Round lambda_2 and monolith_score to 4 decimal places. Fixed iteration order. Fixed parameters.

**Decision gate checklist:**
1. Can `sprs` + hand-rolled Lanczos produce deterministic results across macOS/Linux?
2. Does Lanczos converge within 200 iterations for typical dependency graphs?
3. Is the binary size increase from `sprs` acceptable?

If any answer is "no" → defer D10, document in decision log, proceed with D8+D9.

**MCP tool:** `ariadne_spectral` → `SpectralResult` as JSON.

**CLI parity:** `ariadne query spectral [--format json|md]` (if implemented).

**Performance:** `bench_spectral` on 3k-node graph: <500ms.

**Design source:** ROADMAP.md Phase 3c D10, D-043, D-049

## New Decision Log Entries

| # | Decision | Rationale |
|---|----------|-----------|
| D-060 | Fiedler vector sign convention: lexicographically first node is positive | Eigenvectors are unique up to sign. Fixing sign by first node in BTreeMap order is deterministic and requires zero additional computation. |
| D-061 | PageRank on reversed import graph | Standard PageRank on import graph A→B ranks high-out-degree files. Reversing gives authority ranking: files that important files depend on. Aligns with D8's stated goal. |
| D-062 | Token estimation: serialized JSON bytes / 4 | Simple heuristic. Exact tokenization depends on the model's tokenizer and is not worth computing precisely. Order-of-magnitude is sufficient for budget guidance. |
| D-063 | PageRank edge filter: imports + re_exports only | Test edges and type_imports don't represent runtime structural dependencies. Including them would inflate PageRank of test utility files and type definition files, distorting the ranking. |

## Module Structure

```
src/algo/
├── (existing files)
├── pagerank.rs              # NEW — PageRank power iteration, combined importance (D8)
├── compress.rs              # NEW — Hierarchical compression: L0/L1/L2 (D9)
└── spectral.rs              # NEW, CONDITIONAL — Spectral analysis, Lanczos (D10)

src/model/
├── (existing files)
└── compress.rs              # NEW — CompressedGraph, CompressedNode, CompressedEdge, CompressionLevel
```

**Modified existing files:**

| File | Change |
|------|--------|
| `src/algo/mod.rs` | Re-export `pagerank`, `compress`, `spectral` (conditional) |
| `src/model/mod.rs` | Re-export `compress` module |
| `src/mcp/tools.rs` | Add `ariadne_importance`, `ariadne_compressed`, `ariadne_spectral` (conditional) tool handlers |
| `src/main.rs` | Add `query importance`, `query compressed`, `query spectral` (conditional) CLI subcommands |
| `src/lib.rs` | Re-exports |
| `Cargo.toml` | Add `sprs` as optional dependency (behind feature flag, only for D10) |

**Feature flag for spectral analysis:**

```toml
[features]
default = ["serve"]
serve = ["rmcp", "tokio", "arc-swap", "notify", "notify-debouncer-full"]
spectral = ["sprs"]  # optional, disabled by default until decision gate passes
```

**Dependency rules:**

| Module | Depends on | Never depends on |
|--------|-----------|-----------------|
| `algo/pagerank.rs` | `model/` | everything else |
| `algo/compress.rs` | `model/` | everything else |
| `algo/spectral.rs` | `model/`, `sprs` (external) | everything else |

All three are pure computation modules within `algo/`, consistent with D-033.

## CLI Extension

```
ariadne query importance [--top N] [--format json|md]
ariadne query compressed --level 0|1|2 [--focus <name>] [--depth N] [--format json|md]
ariadne query spectral [--format json|md]                    # if spectral feature enabled
```

## Performance Targets

| Metric | Target |
|--------|--------|
| PageRank convergence (3k-node graph) | <100ms |
| PageRank iterations to convergence | <50 (typical) |
| Combined importance computation | <5ms (after PageRank) |
| L0 compression (10k-node graph) | <50ms |
| L1 compression (single cluster, 200 files) | <10ms |
| L2 compression (file + 2-hop, 3k graph) | <20ms |
| Spectral analysis (3k-node graph) | <500ms (if implemented) |
| `ariadne_importance` MCP response | <110ms (includes PageRank if not cached) |
| `ariadne_compressed` MCP response | <50ms |
| `ariadne_spectral` MCP response | <500ms (if implemented) |

**Caching note:** PageRank and spectral results should be computed once and stored in `GraphState`. Recomputed on state swap (auto-update). MCP tool responses then serve cached results in <5ms.

**New `GraphState` fields (added by Phase 3c):**

```rust
// In GraphState (extends Phase 3a definition)
pub pagerank: BTreeMap<CanonicalPath, f64>,
pub combined_importance: BTreeMap<CanonicalPath, f64>,
pub compressed_l0: CompressedGraph,                     // pre-computed on load
pub spectral: Option<SpectralResult>,                   // None if feature disabled or not computed
```

These are computed during graph load/rebuild and cached. `compressed_l0` is always pre-computed (cheap). L1/L2 are computed on-demand per request.

**Note:** `StatsOutput.centrality` uses `String` keys. `combined_importance` requires conversion to `CanonicalPath` keys when joining with PageRank results. This conversion happens once during graph load.

## Success Criteria

1. PageRank converges within 100 iterations for all test graphs
2. PageRank results deterministic to 4 decimal places across runs and platforms
3. PageRank correctly ranks foundation files higher than leaf files on test fixtures
4. Combined importance score balances centrality and PageRank (neither dominates)
5. L0 compressed graph node count equals cluster count
6. L0 compressed graph fits within 500 tokens for projects up to 10k files
7. L1 compressed graph contains all files from specified cluster
8. L2 compressed graph contains correct N-hop neighborhood
9. Token estimates are within 2x of actual token counts
10. Spectral analysis (if implemented): lambda_2 and Fiedler vector are deterministic
11. Spectral analysis (if implemented): monolith score is higher for tightly-connected graphs
12. `ariadne query importance` and `ariadne query compressed` CLI commands work

## Testing Requirements

### PageRank Tests
- Simple chain graph (A→B→C) → C has highest PageRank (most depended-on)
- Star graph (A,B,C,D all import E) → E has highest PageRank
- Disconnected graph → PageRank sums to ≈1.0, dangling nodes handled
- Self-loop → no infinite loop, converges
- Empty graph → empty result
- Determinism: same graph → same PageRank across 10 runs
- Edge filter: test edges and type_imports excluded from PageRank computation

### Combined Importance Tests
- File with high centrality, low PageRank → moderate combined score
- File with low centrality, high PageRank → moderate combined score
- File with both high → top combined score
- Normalization: max combined score ≈ 1.0

### Compression Tests
- L0: 5-cluster graph → 5 nodes in compressed output
- L0: edge weights = inter-cluster edge counts
- L0: key_files = top-3 by centrality per cluster
- L0 token estimate: <500 for 10k-file project
- L1: specified cluster → all files present, correct internal edges
- L1: external edges aggregated by target cluster (not individual files)
- L1: unknown cluster name → error
- L2: specified file → correct 2-hop neighborhood
- L2: depth=1 → only direct neighbors
- L2: unknown file → error
- Token estimate: within 2x of `serde_json::to_string().len() / 4`

### Spectral Tests (if implemented)
- Complete graph (K5) → high lambda_2 (well-connected)
- Path graph (P5) → low lambda_2 (barely connected)
- Two disconnected components → lambda_2 = 0
- Bipartite graph → Fiedler vector separates the two groups
- Sign convention: first node (lexicographic) always positive
- Determinism: same graph → same lambda_2 across runs
- Monolith score: higher for complete graph than path graph

### MCP Tool Integration Tests
- Start server → call `ariadne_importance` → verify response contains ranked files with combined_score, centrality, pagerank
- Start server → call `ariadne_compressed` with level 0 → verify response node count = cluster count
- Start server → call `ariadne_compressed` with level 1, focus = valid cluster → verify response contains cluster files
- Start server → call `ariadne_compressed` with level 2, focus = valid file → verify response contains neighborhood
- Start server → call `ariadne_compressed` with invalid focus → verify error response
- Start server → call `ariadne_spectral` (if enabled) → verify response contains algebraic_connectivity and partitions

### CLI Integration Tests
- `ariadne query importance --top 5 --format json` → valid JSON with 5 entries
- `ariadne query compressed --level 0 --format json` → valid compressed graph JSON
- `ariadne query compressed --level 1 --focus auth --format json` → valid cluster-level JSON
- `ariadne query compressed --level 1` without `--focus` → error message
- `ariadne query spectral --format json` (if enabled) → valid spectral result JSON

### Performance Benchmarks
- `bench_pagerank` on 3k-node graph: <100ms
- `bench_combined_importance` on 3k-node graph: <5ms
- `bench_compression_l0` on 10k-node graph: <50ms
- `bench_compression_l1` on 200-file cluster: <10ms
- `bench_compression_l2` on 3k-node graph: <20ms
- `bench_spectral` on 3k-node graph: <500ms (if implemented)

### Invariant Extensions
- PageRank: all values in [0.0, 1.0], sum ≈ 1.0 (within tolerance)
- Combined importance: all values in [0.0, 1.0]
- CompressedGraph L0: node count = cluster count
- CompressedGraph: all edges reference valid node names
- CompressedGraph: token_estimate > 0
- Spectral: lambda_2 >= 0 (Laplacian eigenvalues are non-negative)
- Spectral: partition count = 2 (Fiedler bisection)

## Relationship to Parent Phase 3 Spec

This spec supersedes the D8-D10 sections of `2026-03-19-phase3-mcp-server-architectural-intelligence.md` for Phase 3c. Key refinements:

- **`CompressedNode` struct:** Extended with `node_type: CompressedNodeType` enum (parent uses `Option<String>`), `centrality: Option<f64>`, and `file_type: Option<String>` fields for richer compressed views.
- **`CompressedEdge` struct:** Extended with `edge_type: Option<String>` for L1/L2 level edges.
- **PageRank graph direction:** Explicitly resolved as reversed import graph (D-061). Parent spec was ambiguous.
- **PageRank edge filtering:** Only `imports` + `re_exports` edges (D-063). Test and type_imports edges excluded.
- **`GraphState` extension:** New cached fields for PageRank, combined importance, L0 compression, and spectral results.
- **Decision numbering:** D-060 through D-063. D-056 through D-059 are defined in Phase 3b spec.

Parent spec's D1-D4 (Phase 3a) are superseded by the Phase 3a sub-spec. Parent spec's D5-D7 (Phase 3b) are superseded by the Phase 3b sub-spec.

## Design Sources

| Deliverable | Authoritative Sources |
|-------------|----------------------|
| D8: PageRank | ROADMAP.md Phase 3c D8, D-042, D-049, D-061, D-063 |
| D9: Compression | ROADMAP.md Phase 3c D9, D-041, D-062 |
| D10: Spectral | ROADMAP.md Phase 3c D10, D-043, D-049, D-060 |

## Discussion Points Resolved

| DP | Resolution |
|----|-----------|
| DP-4 | PageRank on reversed import graph. D-061. |
| DP-16 | Token estimation: JSON bytes / 4. D-062. |
| DP-19 | Fiedler vector sign: lexicographically first node positive. D-060. |
