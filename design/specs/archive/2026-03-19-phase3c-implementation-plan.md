# Phase 3c: Advanced Graph Analytics — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add PageRank-based file importance ranking, hierarchical graph compression for token-efficient consumption, and optional spectral analysis. Adds 2-3 new MCP tools.

**Architecture:** Pure computation in `algo/` module. Data types in `model/compress.rs`. PageRank runs on reversed import graph. Compression provides three zoom levels (project, cluster, file). Spectral analysis behind feature flag with decision gate.

**Tech Stack:** Existing + `sprs` (optional, for spectral analysis only). All algorithms are pure functions on `ProjectGraph`.

**Spec:** `design/specs/2026-03-19-phase3c-advanced-graph-analytics.md`

**Prerequisites:** Phase 3a must be complete (`src/mcp/` module must exist with `GraphState` and tool registration). Phase 3c does NOT depend on Phase 3b.

**Note:** Tasks 1-3 and 5 (algo + model types) can be implemented independently of Phase 3a. Task 4 (MCP tools + GraphState caching) requires Phase 3a's `src/mcp/` to exist.

---

## File Structure

### New Files

| File | Responsibility |
|------|---------------|
| `src/algo/pagerank.rs` | PageRank power iteration on reversed graph, combined importance |
| `src/algo/compress.rs` | Hierarchical compression: L0/L1/L2 |
| `src/model/compress.rs` | CompressedGraph, CompressedNode, CompressedEdge, CompressionLevel |
| `src/algo/spectral.rs` | (Conditional) Fiedler vector, algebraic connectivity |
| `tests/algo_pagerank_tests.rs` | PageRank and combined importance tests |
| `tests/algo_compress_tests.rs` | Compression tests |

### Modified Files

| File | Change |
|------|--------|
| `src/algo/mod.rs` | Re-export pagerank, compress, spectral (conditional) |
| `src/model/mod.rs` | Re-export compress module |
| `src/mcp/tools.rs` | Add ariadne_importance, ariadne_compressed, ariadne_spectral (conditional) |
| `src/mcp/state.rs` | Add pagerank, combined_importance, compressed_l0 fields to GraphState |
| `src/main.rs` | Add query importance, query compressed, query spectral CLI subcommands |
| `src/lib.rs` | Re-exports |
| `Cargo.toml` | Add sprs as optional dependency behind spectral feature flag |

---

## Task 1: Compression Data Types

**Files:**
- Create: `src/model/compress.rs`
- Modify: `src/model/mod.rs`

- [ ] **Step 1: Implement compression data types**

`src/model/compress.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum CompressionLevel { Project, Cluster, File }

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum CompressedNodeType { Cluster, File }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressedNode {
    pub name: String,
    pub node_type: CompressedNodeType,
    pub file_count: Option<u32>,
    pub cohesion: Option<f64>,
    pub key_files: Vec<String>,
    pub file_type: Option<String>,
    pub layer: Option<String>,
    pub centrality: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressedEdge {
    pub from: String,
    pub to: String,
    pub weight: u32,
    pub edge_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressedGraph {
    pub level: CompressionLevel,
    pub focus: Option<String>,
    pub nodes: Vec<CompressedNode>,
    pub edges: Vec<CompressedEdge>,
    pub token_estimate: u32,
}
```

- [ ] **Step 2: Update model/mod.rs**

```rust
pub mod compress;
pub use compress::*;
```

- [ ] **Step 3: Verify compilation**

Run: `cargo check`

- [ ] **Step 4: Commit**

```bash
git add src/model/compress.rs src/model/mod.rs
git commit -m "ariadne(model): add compression data types for hierarchical graph views"
```

---

## Task 2: PageRank

**Files:**
- Create: `src/algo/pagerank.rs`
- Modify: `src/algo/mod.rs`
- Test: `tests/algo_pagerank_tests.rs`

- [ ] **Step 1: Write PageRank tests**

`tests/algo_pagerank_tests.rs`:

```rust
mod pagerank_tests {
    use ariadne_graph::algo::pagerank::*;
    use ariadne_graph::model::*;

    #[test]
    fn test_chain_graph_foundation_highest() {
        // A imports B, B imports C → reversed: C→B→A
        // C has highest PageRank (most depended-on)
    }

    #[test]
    fn test_star_graph_center_highest() {
        // A,B,C,D all import E → E has highest PageRank
    }

    #[test]
    fn test_disconnected_graph_sums_to_one() {
        // Two components → all ranks sum to ≈1.0
    }

    #[test]
    fn test_empty_graph() {
        // Empty → empty result
    }

    #[test]
    fn test_self_loop_converges() {
        // A imports A → converges, no infinite loop
    }

    #[test]
    fn test_edge_filter_excludes_tests() {
        // Test edges excluded from PageRank
    }

    #[test]
    fn test_determinism() {
        // Same graph → same result across 10 runs
    }

    #[test]
    fn test_combined_importance() {
        // High centrality + low PageRank → moderate score
        // Low centrality + high PageRank → moderate score
        // Both high → top score
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

- [ ] **Step 3: Implement pagerank**

`src/algo/pagerank.rs`:

```rust
use crate::algo::round4;
use crate::model::*;
use std::collections::BTreeMap;

pub fn pagerank(
    graph: &ProjectGraph,
    damping: f64,       // 0.85
    max_iterations: u32, // 100
    tolerance: f64,      // 1e-6
) -> BTreeMap<CanonicalPath, f64> {
    let nodes: Vec<&CanonicalPath> = graph.nodes.keys().collect();
    let n = nodes.len();
    if n == 0 { return BTreeMap::new(); }
    let n_f64 = n as f64;

    // Build reversed graph: for original edge from→to, create reversed edge to→from.
    // This ranks files that are *imported by* many important files (authority ranking).
    // Only include imports + re_exports edges (D-063).
    //
    // rev_outgoing[v] = list of nodes v points to in the reversed graph
    //                 = list of nodes that point to v in the original graph
    //                 = files that import v
    // rev_incoming[v] = list of nodes pointing to v in the reversed graph
    //                 = list of nodes that v points to in the original graph
    //                 = files that v imports
    let mut rev_outgoing: BTreeMap<&CanonicalPath, Vec<&CanonicalPath>> = BTreeMap::new();
    let mut rev_incoming: BTreeMap<&CanonicalPath, Vec<&CanonicalPath>> = BTreeMap::new();
    for node in &nodes {
        rev_outgoing.insert(node, Vec::new());
        rev_incoming.insert(node, Vec::new());
    }
    for edge in &graph.edges {
        if !is_pagerank_edge(edge) { continue; }
        if !graph.nodes.contains_key(&edge.from) || !graph.nodes.contains_key(&edge.to) { continue; }
        // Original: from → to. Reversed: to → from.
        rev_outgoing.entry(&edge.to).or_default().push(&edge.from);
        rev_incoming.entry(&edge.from).or_default().push(&edge.to);
    }

    // Power iteration
    let mut ranks: BTreeMap<&CanonicalPath, f64> = nodes.iter()
        .map(|n| (*n, 1.0 / n_f64))
        .collect();

    for _ in 0..max_iterations {
        // Dangling nodes: no outgoing edges in reversed graph → redistribute rank equally
        let dangling_sum: f64 = nodes.iter()
            .filter(|n| rev_outgoing[*n].is_empty())
            .map(|n| ranks[n])
            .sum();

        let mut new_ranks: BTreeMap<&CanonicalPath, f64> = BTreeMap::new();
        let mut max_diff: f64 = 0.0;

        for node in &nodes {
            // rank(v) = (1-d)/N + d*(dangling/N + sum_{u→v in rev} rank(u)/out_deg(u))
            let incoming_sum: f64 = rev_incoming[node].iter()
                .map(|u| ranks[u] / rev_outgoing[u].len() as f64)
                .sum();
            let new_rank = (1.0 - damping) / n_f64
                + damping * (dangling_sum / n_f64 + incoming_sum);
            max_diff = max_diff.max((new_rank - ranks[node]).abs());
            new_ranks.insert(node, new_rank);
        }

        ranks = new_ranks;
        if max_diff < tolerance { break; }
    }

    ranks.into_iter().map(|(k, v)| (k.clone(), round4(v))).collect()
}

fn is_pagerank_edge(edge: &Edge) -> bool {
    matches!(edge.edge_type, EdgeType::Imports | EdgeType::ReExports)
}

/// Combined importance: 0.5 * normalized_centrality + 0.5 * normalized_pagerank.
/// Note: centrality uses String keys (from StatsOutput), pagerank uses CanonicalPath.
pub fn combined_importance(
    centrality: &BTreeMap<String, f64>,  // from StatsOutput (String keys)
    pagerank: &BTreeMap<CanonicalPath, f64>,
) -> BTreeMap<CanonicalPath, f64> {
    let max_c = centrality.values().cloned().fold(0.0f64, f64::max);
    let max_p = pagerank.values().cloned().fold(0.0f64, f64::max);

    pagerank.keys().map(|path| {
        let c = centrality.get(path.as_str()).copied().unwrap_or(0.0);
        let p = pagerank.get(path).copied().unwrap_or(0.0);
        let norm_c = if max_c > 0.0 { c / max_c } else { 0.0 };
        let norm_p = if max_p > 0.0 { p / max_p } else { 0.0 };
        (path.clone(), round4(0.5 * norm_c + 0.5 * norm_p))
    }).collect()
}
```

- [ ] **Step 4: Update algo/mod.rs**

Add: `pub mod pagerank;`

- [ ] **Step 5: Run tests**

Run: `cargo test pagerank_tests`
Expected: all pass

- [ ] **Step 6: Commit**

```bash
git add src/algo/pagerank.rs src/algo/mod.rs tests/algo_pagerank_tests.rs
git commit -m "ariadne(algo): implement PageRank on reversed import graph with combined importance"
```

---

## Task 3: Hierarchical Compression

**Files:**
- Create: `src/algo/compress.rs`
- Modify: `src/algo/mod.rs`
- Test: `tests/algo_compress_tests.rs`

- [ ] **Step 1: Write compression tests**

```rust
mod compress_tests {
    #[test]
    fn test_l0_node_count_equals_cluster_count() {}
    #[test]
    fn test_l0_edge_weights_are_inter_cluster_counts() {}
    #[test]
    fn test_l0_key_files_top3_by_centrality() {}
    #[test]
    fn test_l1_contains_all_cluster_files() {}
    #[test]
    fn test_l1_external_edges_aggregated() {}
    #[test]
    fn test_l1_unknown_cluster_error() {}
    #[test]
    fn test_l2_correct_neighborhood() {}
    #[test]
    fn test_l2_depth_1_direct_only() {}
    #[test]
    fn test_l2_unknown_file_error() {}
    #[test]
    fn test_token_estimate_positive() {}
}
```

- [ ] **Step 2: Implement compress_l0, compress_l1, compress_l2**

`src/algo/compress.rs`:

```rust
use crate::model::*;
use crate::model::compress::*;
use std::collections::BTreeMap;

pub fn compress_l0(
    graph: &ProjectGraph,
    clusters: &ClusterMap,
    stats: &StatsOutput,
) -> CompressedGraph {
    // One node per cluster, edges = inter-cluster dependency counts
    // key_files = top-3 by centrality per cluster
    // token_estimate = serialized bytes / 4
}

pub fn compress_l1(
    graph: &ProjectGraph,
    clusters: &ClusterMap,
    stats: &StatsOutput,
    cluster_name: &ClusterId,
) -> Result<CompressedGraph, String> {
    // All files in cluster as nodes
    // Internal edges with full detail
    // External edges aggregated by target cluster
}

pub fn compress_l2(
    graph: &ProjectGraph,
    clusters: &ClusterMap,
    stats: &StatsOutput,
    file_path: &CanonicalPath,
    depth: u32,
) -> Result<CompressedGraph, String> {
    // BFS forward+reverse from file up to depth
    // Full edge detail in neighborhood
}

fn estimate_tokens(graph: &CompressedGraph) -> u32 {
    let json = serde_json::to_string(graph).unwrap_or_default();
    (json.len() / 4).max(1) as u32
}
```

- [ ] **Step 3: Update algo/mod.rs**

Add: `pub mod compress;`

- [ ] **Step 4: Run tests**

Run: `cargo test compress_tests`
Expected: all pass

- [ ] **Step 5: Commit**

```bash
git add src/algo/compress.rs src/algo/mod.rs tests/algo_compress_tests.rs
git commit -m "ariadne(algo): implement hierarchical graph compression L0/L1/L2"
```

---

## Task 4: MCP Tools and CLI Integration

**PREREQUISITE:** Phase 3a must be complete. `src/mcp/` module must exist with `GraphState` and rmcp tool registration.

**Files:**
- Modify: `src/mcp/tools.rs`
- Modify: `src/mcp/state.rs`
- Modify: `src/main.rs`
- Test: `tests/mcp_analytics_tests.rs`

- [ ] **Step 1: Add cached fields to GraphState**

In `src/mcp/state.rs`:

```rust
pub pagerank: BTreeMap<CanonicalPath, f64>,
pub combined_importance: BTreeMap<CanonicalPath, f64>,
pub compressed_l0: CompressedGraph,
```

Compute during `GraphState::from_loaded_data()`:

```rust
let pr = pagerank::pagerank(&graph, 0.85, 100, 1e-6);
let combined = pagerank::combined_importance(&stats.centrality, &pr);
let l0 = compress::compress_l0(&graph, &clusters, &stats);
```

- [ ] **Step 2: Add ariadne_importance tool**

```rust
#[tool(description = "Files ranked by combined importance (centrality + PageRank)")]
async fn ariadne_importance(&self, top: Option<u32>) -> ToolResult {
    let state = self.state.load();
    let top = top.unwrap_or(20) as usize;
    let mut ranked: Vec<_> = state.combined_importance.iter().collect();
    ranked.sort_by(|a, b| b.1.partial_cmp(a.1).unwrap());
    ranked.truncate(top);
    // serialize with centrality and pagerank scores per file
}
```

- [ ] **Step 3: Add ariadne_compressed tool**

```rust
#[tool(description = "Hierarchical graph compression at project/cluster/file level")]
async fn ariadne_compressed(&self, level: u32, focus: Option<String>, depth: Option<u32>) -> ToolResult {
    let state = self.state.load();
    match level {
        0 => { /* return cached L0 */ },
        1 => { /* compute L1 for focus cluster */ },
        2 => { /* compute L2 for focus file with depth */ },
        _ => { /* error */ },
    }
}
```

- [ ] **Step 4: Add CLI query subcommands**

Add `Importance` and `Compressed` to `QueryCommands` enum in main.rs.

- [ ] **Step 5: Write CLI integration tests**

In `tests/mcp_analytics_tests.rs`:

```rust
#[test]
fn test_query_importance_json() {
    let output = build_fixture("typescript_simple");
    // Run: ariadne query importance --top 5 --format json
    // Assert: valid JSON with <= 5 entries, each has combined_score/centrality/pagerank
}

#[test]
fn test_query_compressed_l0_json() {
    let output = build_fixture("typescript_simple");
    // Run: ariadne query compressed --level 0 --format json
    // Assert: node count equals cluster count
}

#[test]
fn test_query_compressed_l1_requires_focus() {
    // Run: ariadne query compressed --level 1 --format json (no --focus)
    // Assert: error message about missing --focus
}
```

- [ ] **Step 6: Run tests**

Run: `cargo test`
Expected: all pass

- [ ] **Step 7: Commit**

```bash
git add src/mcp/tools.rs src/mcp/state.rs src/main.rs tests/mcp_analytics_tests.rs
git commit -m "ariadne(mcp): add ariadne_importance and ariadne_compressed MCP tools and CLI commands"
```

---

## Task 5: Spectral Analysis (Conditional)

**Files:**
- Modify: `Cargo.toml`
- Create: `src/algo/spectral.rs`
- Modify: `src/algo/mod.rs`
- Modify: `src/mcp/tools.rs`
- Modify: `src/main.rs`

**NOTE:** This task has a decision gate. Implement steps 1-3, then evaluate feasibility before proceeding.

- [ ] **Step 1: Add spectral feature flag**

In `Cargo.toml`:

```toml
[features]
spectral = ["sprs"]

[dependencies]
sprs = { version = "0.11", optional = true }
```

- [ ] **Step 2: Write spectral tests**

```rust
#[cfg(feature = "spectral")]
mod spectral_tests {
    #[test]
    fn test_complete_graph_high_connectivity() {}
    #[test]
    fn test_path_graph_low_connectivity() {}
    #[test]
    fn test_disconnected_components_lambda2_zero() {}
    #[test]
    fn test_sign_convention_first_node_positive() {}
    #[test]
    fn test_determinism() {}
    #[test]
    fn test_bipartite_separation() { /* Fiedler vector separates the two groups */ }
    #[test]
    fn test_monolith_score_ordering() { /* complete graph > path graph */ }
}
```

- [ ] **Step 3: Define SpectralResult types and implement spectral analysis**

`src/algo/spectral.rs`:

```rust
use crate::algo::round4;
use crate::model::*;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct SpectralResult {
    pub algebraic_connectivity: f64,
    pub monolith_score: f64,
    pub natural_partitions: Vec<SpectralPartition>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SpectralPartition {
    pub partition_id: u32,
    pub files: Vec<CanonicalPath>,
}

#[cfg(feature = "spectral")]
pub fn spectral_analysis(
    graph: &ProjectGraph,
    max_iterations: u32,
    tolerance: f64,
) -> SpectralResult {
    // 1. Build symmetric Laplacian L = D - A
    // 2. Lanczos iteration for lambda_2
    // 3. Extract Fiedler vector
    // 4. Apply sign convention (first node positive)
    // 5. Partition by sign
}
```

- [ ] **Step 4: DECISION GATE — evaluate feasibility**

Check:
1. Deterministic results across macOS/Linux? Run test on both.
2. Convergence within 200 iterations on test graphs?
3. Binary size increase from `sprs` acceptable?

If any "no" → skip remaining steps, document in decision log, commit what exists with `#[cfg(feature = "spectral")]` guards.

- [ ] **Step 5: Add MCP tool and CLI (if gate passes)**

- [ ] **Step 6: Commit**

```bash
git add src/algo/spectral.rs src/algo/mod.rs Cargo.toml
git commit -m "ariadne(algo): implement spectral analysis with Fiedler vector (behind feature flag)"
```

---

## Task 6: Final Verification

- [ ] **Step 1: Run full test suite**

Run: `cargo test`
Expected: all pass

- [ ] **Step 2: Verify PageRank determinism**

Run pagerank on same graph twice, compare output — must be identical.

- [ ] **Step 3: Verify compression token budgets**

L0 on 3k-node graph → check token_estimate < 500.

- [ ] **Step 4: Update decision log**

Add D-060 through D-063 to `design/decisions/log.md`.

- [ ] **Step 5: Create performance benchmarks**

Add to `benches/algo_analytics_bench.rs`:
- `bench_pagerank` on 3k-node graph: target <100ms
- `bench_combined_importance` on 3k-node graph: target <5ms
- `bench_compression_l0` on 10k-node graph: target <50ms
- `bench_compression_l1` on 200-file cluster: target <10ms
- `bench_compression_l2` on 3k-node graph: target <20ms

- [ ] **Step 6: Manual smoke test**

```bash
./target/release/ariadne build tests/fixtures/typescript_simple
./target/release/ariadne query importance --top 5 --format json
./target/release/ariadne query compressed --level 0 --format json
```

- [ ] **Step 7: Final commit**

```bash
git commit -m "ariadne(algo): Phase 3c complete — PageRank, hierarchical compression, spectral analysis"
```
