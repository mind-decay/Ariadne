# Phase 3b: Architectural Intelligence — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Detect structural problems, quantify design quality via Martin metrics, and track structural evolution. Adds `analysis/` module and 3 new MCP tools (`ariadne_metrics`, `ariadne_smells`, `ariadne_diff`).

**Architecture:** Pure computation module `analysis/` depends on `model/` and `algo/` only. Data types (`ArchSmell`, `StructuralDiff`) live in `model/` for cross-module access. MCP tools are thin wrappers. Structural diff is MCP-only (needs pre-update state in memory).

**Tech Stack:** Existing — no new dependencies. Pure Rust computation on existing data structures.

**Spec:** `design/specs/2026-03-19-phase3b-architectural-intelligence.md`

**Prerequisites:** Phase 3a must be complete (`src/mcp/` module must exist with `GraphState` and tool registration).

---

## File Structure

### New Files

| File | Responsibility |
|------|---------------|
| `src/model/smell.rs` | ArchSmell, SmellType, SmellSeverity, SmellMetrics data types |
| `src/model/diff.rs` | StructuralDiff, DiffSummary, ChangeClassification, LayerChange, ClusterChange |
| `src/analysis/mod.rs` | Module re-exports |
| `src/analysis/metrics.rs` | Martin metrics: compute_martin_metrics(), ClusterMetrics, MetricZone |
| `src/analysis/smells.rs` | detect_smells() — 7 architectural smell detectors |
| `src/analysis/diff.rs` | compute_structural_diff() — structural change analysis |
| `tests/analysis_tests.rs` | Unit tests for analysis module |

### Modified Files

| File | Change |
|------|--------|
| `src/model/mod.rs` | Re-export smell and diff modules |
| `src/lib.rs` | Re-export analysis module |
| `src/mcp/tools.rs` | Add ariadne_metrics, ariadne_smells, ariadne_diff tool handlers |
| `src/mcp/state.rs` | Add last_diff field to GraphState; add ClusterMetrics cache |
| `src/mcp/watch.rs` | Compute structural diff before state swap during auto-update |
| `src/main.rs` | Add `query metrics` and `query smells` CLI subcommands |
| `src/diagnostic.rs` | Add W017, W018 warning codes |

---

## Task 1: Data Types in model/

**Files:**
- Create: `src/model/smell.rs`
- Create: `src/model/diff.rs`
- Modify: `src/model/mod.rs`

- [ ] **Step 1: Implement smell data types**

`src/model/smell.rs`:

```rust
use crate::model::CanonicalPath;
use serde::{Deserialize, Serialize};

// Note: No Deserialize — these are output-only types. CanonicalPath and Edge
// do not implement Deserialize, so any struct containing them cannot derive it.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ArchSmell {
    pub smell_type: SmellType,
    pub files: Vec<CanonicalPath>,
    pub severity: SmellSeverity,
    pub explanation: String,
    pub metrics: SmellMetrics,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct SmellMetrics {
    pub primary_value: f64,
    pub threshold: f64,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, Hash)]
pub enum SmellSeverity { High, Medium, Low }

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, Hash)]
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

- [ ] **Step 2: Implement diff data types**

`src/model/diff.rs`:

```rust
use crate::model::{CanonicalPath, ClusterId, Edge};
use crate::model::smell::ArchSmell;
use serde::Serialize;

// Note: No Deserialize — contains CanonicalPath, ClusterId, Edge which lack Deserialize.
// These types are constructed in-memory and serialized to JSON output only.
#[derive(Debug, Clone, Serialize)]
pub struct StructuralDiff {
    pub added_nodes: Vec<CanonicalPath>,
    pub removed_nodes: Vec<CanonicalPath>,
    pub added_edges: Vec<Edge>,
    pub removed_edges: Vec<Edge>,
    pub changed_layers: Vec<LayerChange>,
    pub changed_clusters: Vec<ClusterChange>,
    pub new_cycles: Vec<Vec<CanonicalPath>>,
    pub resolved_cycles: Vec<Vec<CanonicalPath>>,
    pub new_smells: Vec<ArchSmell>,
    pub resolved_smells: Vec<ArchSmell>,
    pub summary: DiffSummary,
}

#[derive(Debug, Clone, Serialize)]
pub struct LayerChange {
    pub file: CanonicalPath,
    pub old_depth: u32,
    pub new_depth: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct ClusterChange {
    pub file: CanonicalPath,
    pub old_cluster: ClusterId,
    pub new_cluster: ClusterId,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiffSummary {
    pub structural_change_magnitude: f64,
    pub change_type: ChangeClassification,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq)]
pub enum ChangeClassification {
    Additive,
    Refactor,
    Migration,
    Breaking,
}
```

- [ ] **Step 3: Update model/mod.rs re-exports**

Add:

```rust
pub mod smell;
pub mod diff;
pub use smell::{ArchSmell, SmellType, SmellSeverity, SmellMetrics};
pub use diff::{StructuralDiff, DiffSummary, ChangeClassification, LayerChange, ClusterChange};
```

- [ ] **Step 4: Verify compilation**

Run: `cargo check`
Expected: compiles

- [ ] **Step 5: Commit**

```bash
git add src/model/smell.rs src/model/diff.rs src/model/mod.rs
git commit -m "ariadne(model): add ArchSmell and StructuralDiff data types for Phase 3b"
```

---

## Task 2: Martin Metrics

**Files:**
- Create: `src/analysis/mod.rs`
- Create: `src/analysis/metrics.rs`
- Modify: `src/lib.rs`
- Test: `tests/analysis_tests.rs`

- [ ] **Step 1: Write Martin metrics tests**

`tests/analysis_tests.rs`:

```rust
mod metrics_tests {
    use ariadne_graph::analysis::metrics::*;
    use ariadne_graph::model::*;
    // Build hand-crafted graphs with known Ca/Ce values

    #[test]
    fn test_isolated_cluster_instability_zero() { /* Ca=0, Ce=0 → I=0.0 */ }

    #[test]
    fn test_fully_outgoing_cluster_instability_one() { /* Ca=0, Ce=5 → I=1.0 */ }

    #[test]
    fn test_all_typedef_cluster_abstractness_one() { /* all type_def → A=1.0 */ }

    #[test]
    fn test_no_abstract_cluster_abstractness_zero() { /* all source → A=0.0 */ }

    #[test]
    fn test_barrel_file_detection() { /* >80% re-exports → abstract */ }

    #[test]
    fn test_zone_of_pain() { /* D >= 0.3, low A, low I */ }

    #[test]
    fn test_zone_of_uselessness() { /* D >= 0.3, high A, high I */ }

    #[test]
    fn test_main_sequence() { /* A + I ≈ 1.0, D < 0.3 */ }

    #[test]
    fn test_determinism() { /* same graph → same metrics */ }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test metrics_tests`
Expected: FAIL

- [ ] **Step 3: Create analysis module skeleton**

`src/analysis/mod.rs` (start with only metrics — add smells and diff in their respective tasks to avoid compilation errors):

```rust
pub mod metrics;
// pub mod smells;  — added in Task 3
// pub mod diff;    — added in Task 4
```

Update `src/lib.rs`:

```rust
pub mod analysis;
```

- [ ] **Step 4: Implement compute_martin_metrics**

`src/analysis/metrics.rs`:

```rust
use crate::algo::round4;
use crate::model::*;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterMetrics {
    pub cluster_id: ClusterId,
    pub instability: f64,
    pub abstractness: f64,
    pub distance: f64,
    pub zone: MetricZone,
    pub afferent_coupling: u32,
    pub efferent_coupling: u32,
    pub abstract_files: u32,
    pub total_files: u32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum MetricZone {
    MainSequence,
    ZoneOfPain,
    ZoneOfUselessness,
    OffMainSequence,
}

pub fn compute_martin_metrics(
    graph: &ProjectGraph,
    clusters: &ClusterMap,
) -> BTreeMap<ClusterId, ClusterMetrics> {
    // For each cluster:
    // 1. Count Ca (afferent) and Ce (efferent) from inter-cluster edges
    // 2. Count abstract files (type_def + barrel files with >80% re-exports)
    // 3. Compute I, A, D
    // 4. Classify zone
    // All floats rounded via round4()
}

fn is_abstract_file(
    path: &CanonicalPath,
    node: &Node,
    edges: &[Edge],
) -> bool {
    if node.file_type == FileType::TypeDef {
        return true;
    }
    // Barrel file: >80% of exports are re-exports
    if node.exports.is_empty() {
        return false;
    }
    let re_export_count = edges.iter()
        .filter(|e| e.from == *path && e.edge_type == EdgeType::ReExports)
        .count();
    let ratio = re_export_count as f64 / node.exports.len() as f64;
    ratio > 0.8
}

fn classify_zone(distance: f64, abstractness: f64, instability: f64) -> MetricZone {
    if distance < 0.3 {
        MetricZone::MainSequence
    } else if abstractness < 0.5 && instability < 0.5 {
        MetricZone::ZoneOfPain
    } else if abstractness > 0.5 && instability > 0.5 {
        MetricZone::ZoneOfUselessness
    } else {
        MetricZone::OffMainSequence
    }
}
```

- [ ] **Step 5: Run tests**

Run: `cargo test metrics_tests`
Expected: all pass

- [ ] **Step 6: Commit**

```bash
git add src/analysis/ src/lib.rs tests/analysis_tests.rs
git commit -m "ariadne(analysis): implement Martin metrics with zone classification"
```

---

## Task 3: Smell Detection

**Files:**
- Create: `src/analysis/smells.rs`
- Modify: `src/diagnostic.rs`
- Test: `tests/analysis_tests.rs`

- [ ] **Step 1: Write smell detection tests**

```rust
mod smell_tests {
    #[test]
    fn test_god_file_detected() { /* centrality > 0.8, out-degree > 20, lines > 500 */ }
    #[test]
    fn test_god_file_below_threshold_not_detected() { /* centrality 0.75 */ }
    #[test]
    fn test_circular_dependency_from_scc() { /* SCC size 3 */ }
    #[test]
    fn test_layer_violation() { /* depth 1 imports depth 3 */ }
    #[test]
    fn test_hub_and_spoke() { /* one file >50% cluster external edges */ }
    #[test]
    fn test_unstable_foundation() { /* I > 0.7, Ca > 10 */ }
    #[test]
    fn test_dead_cluster() { /* 0 incoming, not top-level */ }
    #[test]
    fn test_shotgun_surgery() { /* blast radius > 30% */ }
    #[test]
    fn test_clean_architecture_no_smells() { /* well-structured → empty */ }
}
```

- [ ] **Step 2: Run tests to verify they fail**

- [ ] **Step 3: Add W017, W018 to diagnostic.rs**

- [ ] **Step 4: Implement detect_smells**

`src/analysis/smells.rs`:

```rust
use crate::algo;
use crate::analysis::metrics::ClusterMetrics;
use crate::model::*;
use crate::model::smell::*;
use std::collections::BTreeMap;

pub fn detect_smells(
    graph: &ProjectGraph,
    stats: &StatsOutput,
    clusters: &ClusterMap,
    metrics: &BTreeMap<ClusterId, ClusterMetrics>,
) -> Vec<ArchSmell> {
    let mut smells = Vec::new();
    detect_god_files(graph, stats, &mut smells);
    detect_circular_dependencies(stats, &mut smells);
    detect_layer_violations(graph, &mut smells);
    detect_hub_and_spoke(graph, clusters, &mut smells);
    detect_unstable_foundations(metrics, &mut smells);
    detect_dead_clusters(graph, clusters, &mut smells);
    detect_shotgun_surgery(graph, &mut smells);
    smells.sort_by(|a, b| a.files.cmp(&b.files));
    smells
}
```

Each sub-detector is a private function implementing the rules from the spec.

- [ ] **Step 5: Run tests**

Run: `cargo test smell_tests`
Expected: all pass

- [ ] **Step 6: Commit**

```bash
git add src/analysis/smells.rs src/diagnostic.rs tests/analysis_tests.rs
git commit -m "ariadne(analysis): implement 7 architectural smell detectors"
```

---

## Task 4: Structural Diff

**Files:**
- Create: `src/analysis/diff.rs`
- Test: `tests/analysis_tests.rs`

- [ ] **Step 1: Write structural diff tests**

```rust
mod diff_tests {
    #[test]
    fn test_additive_change() { /* add files, nothing removed → Additive */ }
    #[test]
    fn test_breaking_change() { /* removed edges + new cycles → Breaking */ }
    #[test]
    fn test_refactor() { /* roughly equal adds/removes, small magnitude → Refactor */ }
    #[test]
    fn test_migration() { /* more removed than added → Migration */ }
    #[test]
    fn test_louvain_noise_filtered() { /* cluster changed but no edge changes → filtered */ }
    #[test]
    fn test_cycle_diff() { /* old SCCs vs new SCCs → new_cycles, resolved_cycles */ }
    #[test]
    fn test_magnitude_calculation() { /* known diff → correct magnitude */ }
    #[test]
    fn test_empty_diff() { /* no changes → magnitude 0.0 */ }
}
```

- [ ] **Step 2: Implement compute_structural_diff**

`src/analysis/diff.rs`:

```rust
use crate::analysis::metrics::ClusterMetrics;
use crate::analysis::smells::detect_smells;
use crate::model::*;
use crate::model::diff::*;
use std::collections::{BTreeMap, BTreeSet};

pub fn compute_structural_diff(
    old_graph: &ProjectGraph,
    old_stats: &StatsOutput,
    old_clusters: &ClusterMap,
    old_metrics: &BTreeMap<ClusterId, ClusterMetrics>,
    new_graph: &ProjectGraph,
    new_stats: &StatsOutput,
    new_clusters: &ClusterMap,
    new_metrics: &BTreeMap<ClusterId, ClusterMetrics>,
) -> StructuralDiff {
    let added_nodes = find_added_nodes(old_graph, new_graph);
    let removed_nodes = find_removed_nodes(old_graph, new_graph);
    let (added_edges, removed_edges) = diff_edges(old_graph, new_graph);
    let changed_layers = diff_layers(old_graph, new_graph);
    let changed_clusters = diff_clusters_filtered(
        old_graph, new_graph, old_clusters, new_clusters,
        &added_edges, &removed_edges,
    );
    let (new_cycles, resolved_cycles) = diff_cycles(
        &old_stats.sccs, &new_stats.sccs,
    );
    let old_smells = detect_smells(old_graph, old_stats, old_clusters, old_metrics);
    let new_smells_all = detect_smells(new_graph, new_stats, new_clusters, new_metrics);
    let (new_smells, resolved_smells) = diff_smells(&old_smells, &new_smells_all);
    let magnitude = compute_magnitude(
        &added_edges, &removed_edges, &added_nodes, &removed_nodes, new_graph,
    );
    let change_type = classify_change(
        &added_nodes, &removed_nodes, &added_edges, &removed_edges,
        &new_cycles, magnitude,
    );

    StructuralDiff {
        added_nodes, removed_nodes, added_edges, removed_edges,
        changed_layers, changed_clusters, new_cycles, resolved_cycles,
        new_smells, resolved_smells,
        summary: DiffSummary { structural_change_magnitude: magnitude, change_type },
    }
}
```

Edge comparison by `(from, to, edge_type)` tuple — symbols ignored for diff purposes.

**Type conversion notes:**
- `StatsOutput.sccs` uses `Vec<Vec<String>>`, not `Vec<Vec<CanonicalPath>>`. The `diff_cycles` function must convert `String` → `CanonicalPath` for comparison and output.
- `StatsOutput.centrality` uses `String` keys. Use `CanonicalPath::new(key)` for lookups against `ProjectGraph` nodes.
- Add `pub mod smells;` to `src/analysis/mod.rs` at this point (was deferred from Task 2).

- [ ] **Step 3: Run tests**

Run: `cargo test diff_tests`
Expected: all pass

- [ ] **Step 4: Commit**

```bash
git add src/analysis/diff.rs tests/analysis_tests.rs
git commit -m "ariadne(analysis): implement structural diff with Louvain noise filtering"
```

---

## Task 5: MCP Tools and CLI Integration

**Files:**
- Modify: `src/mcp/tools.rs`
- Modify: `src/mcp/state.rs`
- Modify: `src/mcp/watch.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: Add ClusterMetrics cache and last_diff to GraphState**

In `src/mcp/state.rs`, add fields:

```rust
pub cluster_metrics: BTreeMap<ClusterId, ClusterMetrics>,
pub last_diff: Option<StructuralDiff>,
```

Compute `cluster_metrics` during `GraphState::from_loaded_data()`.

- [ ] **Step 2: Add ariadne_metrics tool handler**

In `src/mcp/tools.rs`:

```rust
#[tool(description = "Martin metrics per cluster: instability, abstractness, distance, zone")]
async fn ariadne_metrics(&self) -> ToolResult {
    let state = self.state.load();
    // serialize state.cluster_metrics to JSON
}
```

- [ ] **Step 3: Add ariadne_smells tool handler**

```rust
#[tool(description = "Detect architectural smells: god files, cycles, layer violations, etc.")]
async fn ariadne_smells(&self, min_severity: Option<String>) -> ToolResult {
    let state = self.state.load();
    let smells = detect_smells(&state.graph, &state.stats, &state.clusters, &state.cluster_metrics);
    // filter by min_severity if provided, serialize to JSON
}
```

- [ ] **Step 4: Add ariadne_diff tool handler**

```rust
#[tool(description = "Structural diff since last auto-update")]
async fn ariadne_diff(&self) -> ToolResult {
    let state = self.state.load();
    // return state.last_diff as JSON, or null if None
}
```

- [ ] **Step 5: Compute structural diff in watch.rs during auto-update**

In the rebuild path, before state swap:

```rust
let old_state = self.state.load();
// ... rebuild produces new graph, stats, clusters ...
let new_metrics = compute_martin_metrics(&new_graph, &new_clusters);
let diff = compute_structural_diff(
    &old_state.graph, &old_state.stats, &old_state.clusters, &old_state.cluster_metrics,
    &new_graph, &new_stats, &new_clusters, &new_metrics,
);
// store diff in new GraphState
new_state.last_diff = Some(diff);
```

- [ ] **Step 6: Add query metrics and query smells to main.rs**

Add `Metrics` and `Smells` variants to `QueryCommands` enum.

- [ ] **Step 7: Run tests**

Run: `cargo test`
Expected: all pass

- [ ] **Step 8: Commit**

```bash
git add src/mcp/tools.rs src/mcp/state.rs src/mcp/watch.rs src/main.rs
git commit -m "ariadne(mcp): add ariadne_metrics, ariadne_smells, ariadne_diff MCP tools and CLI commands"
```

---

## Task 6: Final Verification

- [ ] **Step 1: Run full test suite**

Run: `cargo test`
Expected: all tests pass

- [ ] **Step 2: Manual smoke test**

```bash
./target/release/ariadne build tests/fixtures/typescript_simple
./target/release/ariadne query metrics --format json
./target/release/ariadne query smells --format json
```

- [ ] **Step 3: Verify determinism**

Run metrics twice, diff output — should be identical.

- [ ] **Step 4: Update decision log**

Add D-056 through D-059 to `design/decisions/log.md`.

- [ ] **Step 5: Create performance benchmarks**

Add to `benches/analysis_bench.rs`:
- `bench_martin_metrics` on 3k-node graph: target <10ms
- `bench_smell_detection` on 3k-node graph: target <2s
- `bench_structural_diff` on 3k-node graph with 50 changes: target <100ms

- [ ] **Step 6: Add invariant assertions to tests**

Add to existing tests:
- Martin metrics: assert `I`, `A`, `D` all in `[0.0, 1.0]`
- Smell detection: assert all `ArchSmell.files` entries exist in the test graph
- StructuralDiff: assert all `added_edges`/`removed_edges` reference valid nodes

- [ ] **Step 7: Final commit**

```bash
git commit -m "ariadne(analysis): Phase 3b complete — Martin metrics, smell detection, structural diff"
```
