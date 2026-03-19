# Phase 3b: Architectural Intelligence — Specification

## Goal

Move beyond basic graph metrics into architectural analysis — detect structural problems, quantify design quality via Martin metrics, and track structural evolution through diffs. Expose results via 3 new MCP tools.

## Dependencies

**Phase 3a must be complete.** Phase 3b builds on:

- MCP server (`ariadne serve`) with `ArcSwap<GraphState>`, tool registration via `rmcp`
- All 11 Phase 3a MCP tools operational
- `GraphState` with `ProjectGraph`, `StatsOutput`, `ClusterMap`, derived indices
- Freshness engine with two-level confidence (D-053)
- Auto-update with incremental rebuild and atomic state swap
- `raw_imports.json` persistence (D-054)
- Algorithms: Tarjan SCC, Reverse BFS, Brandes centrality, topological sort, subgraph extraction
- `ContentHash` on every node, `ClusterMap` with cohesion metrics
- `algo/delta.rs` changed/added/removed detection

**Phase 3b does NOT depend on Phase 3c.** They are independent extensions of Phase 3a.

## Risk Classification

**Overall: YELLOW**

Phase 3b is primarily pure computation on existing data structures. The main risks are smell detection calibration and structural diff correctness.

### Per-Deliverable Risk

| # | Deliverable | Risk | Rationale |
|---|------------|------|-----------|
| D5 | Martin Metrics | GREEN | Pure computation. Instability and Abstractness are simple ratios. All inputs exist. |
| D6 | Smell Detection | YELLOW | 7 smell patterns with clear thresholds. "Shotgun Surgery" requires per-file blast_radius (potentially expensive). <5% false positive target needs calibration. |
| D7 | Structural Diff | YELLOW | `StructuralDiff` struct well-specified. Cycle diffing (old vs new SCC) adds complexity. `ChangeClassification` heuristic now defined. Louvain noise filtering needed. |

## Deliverables

### D5: Martin Metrics (Instability & Abstractness)

**New files:** `src/analysis/mod.rs`, `src/analysis/metrics.rs`
**Modified files:** `src/mcp/tools.rs`, `src/main.rs` (CLI query), `src/lib.rs`

Robert C. Martin's package metrics applied at cluster level (D-040).

**Instability** `I = Ce / (Ca + Ce)` per cluster:
- `Ca` = afferent coupling (incoming edges from other clusters)
- `Ce` = efferent coupling (outgoing edges to other clusters)
- Edge case: if `Ca + Ce == 0` → `I = 0.0` (isolated cluster, maximally stable by convention)

**Abstractness** `A = Na / Nc` per cluster:
- `Na` = abstract files in cluster
- `Nc` = total files in cluster
- Edge case: if `Nc == 0` → skip cluster (empty, shouldn't exist)

**Abstract file classification (resolves DP-8):**

A file is classified as abstract if ANY of:
1. `FileType::type_def` — type definition files (`.d.ts`, `.pyi`)
2. Barrel file — file where >80% of exports are re-exports. Determined by: `count(outgoing re_exports edges) / count(node.exports) > 0.8` AND `count(node.exports) > 0`

All other files are concrete.

**Distance from Main Sequence:** `D = |A + I - 1|`

**Zone classification:**
- `D < 0.3` → Main Sequence (good balance)
- `D >= 0.3` AND `A < 0.5` AND `I < 0.5` → Zone of Pain (concrete and stable — hard to change)
- `D >= 0.3` AND `A > 0.5` AND `I > 0.5` → Zone of Uselessness (abstract and unstable — no real dependents)
- Otherwise → Off Main Sequence (no specific zone)

**Output type (in `analysis/metrics.rs`):**

```rust
pub struct ClusterMetrics {
    pub cluster_id: ClusterId,
    pub instability: f64,       // [0.0, 1.0]
    pub abstractness: f64,      // [0.0, 1.0]
    pub distance: f64,          // [0.0, 1.0]
    pub zone: MetricZone,
    pub afferent_coupling: u32, // Ca
    pub efferent_coupling: u32, // Ce
    pub abstract_files: u32,    // Na
    pub total_files: u32,       // Nc
}

pub enum MetricZone {
    MainSequence,
    ZoneOfPain,
    ZoneOfUselessness,
    OffMainSequence,
}
```

Float results rounded to 4 decimal places (D-049).

**Function signature:**

```rust
pub fn compute_martin_metrics(
    graph: &ProjectGraph,
    clusters: &ClusterMap,
) -> BTreeMap<ClusterId, ClusterMetrics>
```

**Module dependency (D-048):** `analysis/` depends on `model/`, `algo/`. Never depends on `serial/`, `pipeline/`, `parser/`, `mcp/`.

**MCP tool:** `ariadne_metrics` → per-cluster `ClusterMetrics` as JSON.

**CLI parity:** `ariadne query metrics [--format json|md]`

**Design source:** ROADMAP.md Phase 3b D5, D-040, D-048, D-049

### D6: Architectural Smell Detection

**New files:** `src/analysis/smells.rs`, `src/model/smell.rs`
**Modified files:** `src/model/mod.rs`, `src/mcp/tools.rs`, `src/main.rs`

**Data types (in `model/smell.rs`):**

```rust
pub struct ArchSmell {
    pub smell_type: SmellType,
    pub files: Vec<CanonicalPath>,       // affected files (sorted)
    pub severity: SmellSeverity,
    pub explanation: String,             // human-readable explanation
    pub metrics: SmellMetrics,           // quantitative evidence
}

pub struct SmellMetrics {
    pub primary_value: f64,              // the main metric that triggered detection
    pub threshold: f64,                  // the threshold it exceeded
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

`ArchSmell`, `SmellSeverity`, `SmellType`, and `SmellMetrics` live in `model/` so both `analysis/` and `mcp/` can reference them without circular dependencies.

**Detection rules:**

| Smell | Detection | Severity | Metrics |
|-------|----------|----------|---------|
| God File | Centrality > 0.8 AND out-degree > 20 AND lines > 500 | HIGH | primary: centrality, threshold: 0.8 |
| Circular Dependency | SCC size > 1 (from Phase 2 Tarjan) | HIGH | primary: SCC size, threshold: 1 |
| Layer Violation | Edge from lower `arch_depth` to higher (dependency on a higher layer) | MEDIUM | primary: depth difference, threshold: 0 |
| Hub-and-Spoke | One file has >50% of cluster's external edges | MEDIUM | primary: edge share %, threshold: 0.5 |
| Unstable Foundation | Cluster with `I > 0.7` AND `Ca > 10` (many depend on it, but it also depends on many) | HIGH | primary: instability, threshold: 0.7 |
| Dead Cluster | Cluster with 0 incoming external edges AND not a top-level entry point (arch_depth != max_depth) | LOW | primary: incoming edges, threshold: 0 |
| Shotgun Surgery | File with blast radius > 30% of project file count | HIGH | primary: blast radius %, threshold: 0.3 |

**Threshold documentation:** All thresholds are initial heuristics subject to calibration. The `SmellMetrics` struct provides the actual values and thresholds so users can evaluate detection quality. The <5% false positive target will be evaluated against known-good architectures in test fixtures.

**Function signature:**

```rust
pub fn detect_smells(
    graph: &ProjectGraph,
    stats: &StatsOutput,
    clusters: &ClusterMap,
    metrics: &BTreeMap<ClusterId, ClusterMetrics>,  // from D5
) -> Vec<ArchSmell>
```

**Performance note:** "Shotgun Surgery" requires calling `blast_radius()` for every file. For 3k files this is 3k BFS traversals. Each BFS is O(V+E) ≈ O(11k). Total: ~33M operations — should complete in <1s but needs benchmarking. Optimization: only check files with out-degree > 10 (files with few dependents can't have 30% blast radius).

**MCP tool:** `ariadne_smells [--min-severity high|medium|low]` → detected smells as JSON.

**CLI parity:** `ariadne query smells [--min-severity high|medium|low] [--format json|md]`

**Design source:** ROADMAP.md Phase 3b D6, D-048

### D7: Structural Diff

**New files:** `src/analysis/diff.rs`, `src/model/diff.rs`
**Modified files:** `src/model/mod.rs`, `src/mcp/tools.rs`, `src/mcp/state.rs`

**Data types (in `model/diff.rs`):**

```rust
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

pub struct LayerChange {
    pub file: CanonicalPath,
    pub old_depth: u32,
    pub new_depth: u32,
}

pub struct ClusterChange {
    pub file: CanonicalPath,
    pub old_cluster: ClusterId,
    pub new_cluster: ClusterId,
}

pub struct DiffSummary {
    pub structural_change_magnitude: f64,   // [0.0, 1.0]
    pub change_type: ChangeClassification,
}

pub enum ChangeClassification {
    Additive,   // new nodes/edges, nothing removed
    Refactor,   // roughly equal add/remove, small magnitude
    Migration,  // more removed than added
    Breaking,   // edges removed AND new cycles introduced
}
```

**Change magnitude:**
`magnitude = (|added_edges| + |removed_edges| + |added_nodes| + |removed_nodes|) / (2 * (|edges| + |nodes|))`

Where `|edges|` and `|nodes|` are from the NEW graph (post-update).

**ChangeClassification heuristic (resolves DP-17):**

```
if removed_edges > 0 AND new_cycles.len() > 0:
    Breaking
else if added_nodes > 0 AND removed_nodes == 0 AND removed_edges == 0:
    Additive
else if |added_nodes - removed_nodes| <= max(1, 0.2 * max(added_nodes, removed_nodes))
        AND magnitude < 0.3:
    Refactor
else if removed_nodes > added_nodes AND magnitude > 0.1:
    Migration
else:
    Refactor  // default
```

**Louvain noise filtering (resolves DP-14):**

`changed_clusters` only includes files where:
1. The file's cluster assignment changed between old and new graph, AND
2. At least one edge of that file was also added or removed

Pure Louvain re-assignments (where all edges remain the same) are filtered out. This prevents noise from Louvain's non-deterministic community optimization.

**Cycle diffing:**

```rust
fn diff_cycles(old_sccs: &[Vec<CanonicalPath>], new_sccs: &[Vec<CanonicalPath>])
    -> (Vec<Vec<CanonicalPath>>, Vec<Vec<CanonicalPath>>)
    // returns (new_cycles, resolved_cycles)
```

SCCs are compared as sorted sets. An SCC in new but not in old → `new_cycles`. An SCC in old but not in new → `resolved_cycles`. Partial overlaps (SCC grew or shrank) are treated as: old version → resolved, new version → new.

**Smell diffing:**

Requires running `detect_smells()` on both old and new state. Smells are compared by `(smell_type, sorted files)` tuple. New smell not in old → `new_smells`. Old smell not in new → `resolved_smells`.

**Function signature:**

```rust
pub fn compute_structural_diff(
    old_graph: &ProjectGraph,
    old_stats: &StatsOutput,
    old_clusters: &ClusterMap,
    old_metrics: &BTreeMap<ClusterId, ClusterMetrics>,
    new_graph: &ProjectGraph,
    new_stats: &StatsOutput,
    new_clusters: &ClusterMap,
    new_metrics: &BTreeMap<ClusterId, ClusterMetrics>,
) -> StructuralDiff
```

**MCP integration (resolves DP-15 — MCP-only):**

`ariadne_diff` is MCP-only. The pre-update graph snapshot is held in `GraphState`. When auto-update triggers:

1. Load current `GraphState` via `state.load()` as `old_state` (returns `Arc`, cheap reference increment)
2. Run rebuild → produce new `GraphState`
3. Compute `StructuralDiff` from old vs new
4. Store diff in new `GraphState` as `last_diff: Option<StructuralDiff>`
5. Atomic swap

`GraphState` gains a new field:

```rust
pub last_diff: Option<StructuralDiff>,  // diff from last auto-update
```

No CLI equivalent — `ariadne update` doesn't hold previous state in memory.

**MCP tool:** `ariadne_diff` → last structural diff as JSON. Returns `null` if no update has occurred since server start.

**Design source:** ROADMAP.md Phase 3b D7, D-048

## New Error and Warning Codes

### Warnings

| Code | Condition | Message |
|------|-----------|---------|
| W017 | Smell detection skipped for a file | "Smell detection skipped for {path}: {reason}" |
| W018 | Blast radius computation timed out for a file | "Blast radius computation exceeded budget for {path}, skipping Shotgun Surgery check" |

## New Decision Log Entries

| # | Decision | Rationale |
|---|----------|-----------|
| D-056 | Abstract file classification: type_def OR >80% re-export ratio | Simple, deterministic. Covers `.d.ts`/`.pyi` and barrel files. Avoids AST-level abstract/interface detection which would require language-specific logic. |
| D-057 | Louvain noise filtering in structural diff via edge-change correlation | Pure Louvain re-assignments without edge changes are noise, not structural change. Filtering by edge correlation gives meaningful diffs. |
| D-058 | StructuralDiff is MCP-only (no CLI equivalent) | CLI `ariadne update` doesn't hold previous state. MCP server has pre-update snapshot in `Arc<GraphState>`. No value in persisting diffs to disk for CLI. |
| D-059 | ChangeClassification heuristic: Additive/Refactor/Migration/Breaking | Four categories cover common patterns. Breaking = removed edges + new cycles (worst case). Default to Refactor for ambiguous cases. Heuristic, not authoritative. |

## Module Structure

```
src/analysis/               # NEW — depends on model/, algo/ (D-048)
├── mod.rs                   # Re-exports
├── metrics.rs               # Martin metrics: ClusterMetrics, compute_martin_metrics()
├── smells.rs                # Smell detection: detect_smells()
└── diff.rs                  # Structural diff: compute_structural_diff()

src/model/
├── (existing files)
├── smell.rs                 # NEW — ArchSmell, SmellType, SmellSeverity, SmellMetrics
└── diff.rs                  # NEW — StructuralDiff, DiffSummary, ChangeClassification, LayerChange, ClusterChange
```

**Modified existing files:**

| File | Change |
|------|--------|
| `src/model/mod.rs` | Re-export `smell` and `diff` modules |
| `src/mcp/tools.rs` | Add `ariadne_metrics`, `ariadne_smells`, `ariadne_diff` tool handlers |
| `src/mcp/state.rs` | Add `last_diff: Option<StructuralDiff>` to `GraphState`; compute diff during auto-update |
| `src/mcp/watch.rs` | Call `compute_structural_diff()` before state swap |
| `src/main.rs` | Add `query metrics` and `query smells` CLI subcommands |
| `src/lib.rs` | Re-export `analysis` module |

**Dependency rules:**

| Module | Depends on | Never depends on |
|--------|-----------|-----------------|
| `analysis/` | `model/`, `algo/` | `serial/`, `pipeline/`, `parser/`, `mcp/` |

## CLI Extension

```
ariadne query metrics [--format json|md]
ariadne query smells [--min-severity high|medium|low] [--format json|md]
```

`ariadne_diff` has no CLI equivalent (MCP-only, D-058).

## Performance Targets

| Metric | Target |
|--------|--------|
| Martin metrics computation (all clusters, 3k project) | <10ms |
| Smell detection (all 7 smells, 3k project) | <2s |
| Structural diff computation | <100ms |
| `ariadne_metrics` MCP tool response | <10ms |
| `ariadne_smells` MCP tool response | <2s |
| `ariadne_diff` MCP tool response | <5ms (pre-computed) |

**Note:** Smell detection's <2s target is dominated by Shotgun Surgery's per-file blast radius. With the optimization (only check files with out-degree > 10), this should be achievable.

## Success Criteria

1. Martin metrics computed for all clusters, values in [0.0, 1.0], deterministic (D-049)
2. Zone classification correctly identifies Zone of Pain and Zone of Uselessness
3. All 7 architectural smells detected with correct severity
4. Smell detection <5% false positive rate on known-good test fixtures
5. Barrel file detection correctly classifies files with >80% re-exports as abstract
6. Structural diff correctly captures added/removed nodes and edges
7. Cycle diffing correctly identifies new and resolved SCCs
8. Louvain noise filtering prevents spurious cluster change reports
9. ChangeClassification heuristic produces reasonable classifications
10. `ariadne_diff` returns `null` before first auto-update, correct diff after
11. All metrics deterministic (byte-identical output across runs)
12. `ariadne query metrics` and `ariadne query smells` CLI commands work

## Testing Requirements

### Martin Metrics Tests
- Hand-crafted graph with known Ca/Ce values → verify I, A, D
- Cluster with 0 edges → verify I = 0.0
- Cluster with only outgoing edges → verify I = 1.0
- Cluster with all type_def files → verify A = 1.0
- Cluster with no abstract files → verify A = 0.0
- Barrel file detection: file with 4/5 re-export edges → abstract; file with 1/5 → concrete
- Zone classification: known Zone of Pain graph, known Zone of Uselessness graph
- Float determinism: same graph → same metrics across runs

### Smell Detection Tests
- God File: file with centrality 0.85, out-degree 25, 600 lines → detected
- God File: file with centrality 0.75 (below threshold) → not detected
- Circular Dependency: 3-node SCC → detected with correct files
- Layer Violation: edge from depth 1 to depth 3 → detected
- Hub-and-Spoke: file with 60% of cluster external edges → detected
- Unstable Foundation: cluster I=0.8, Ca=15 → detected
- Dead Cluster: cluster with 0 incoming, not max depth → detected
- Dead Cluster: top-level entry cluster with 0 incoming → NOT detected (not dead)
- Shotgun Surgery: file affecting 35% of project → detected
- Known clean architecture → no smells (false positive check)

### Structural Diff Tests
- Add 3 files → Additive classification, correct added_nodes
- Remove 2 files → correct removed_nodes and removed_edges
- Refactor (rename files) → Refactor classification
- Introduce cycle → Breaking classification, correct new_cycles
- Resolve cycle → correct resolved_cycles
- Louvain noise: change no edges, Louvain reassigns cluster → changed_clusters is empty
- Louvain real change: change edges AND cluster reassigned → changed_clusters populated
- Magnitude calculation on known diff → correct value
- Empty diff (no changes) → magnitude 0.0, all lists empty

### Performance Benchmarks
- `bench_martin_metrics` on 3k-node graph: <10ms
- `bench_smell_detection` on 3k-node graph: <2s
- `bench_structural_diff` on 3k-node graph with 50 changes: <100ms

### Invariant Extensions
- Martin metrics: I, A in [0.0, 1.0], D in [0.0, 1.0]
- Smell detection: all referenced files must exist in graph
- StructuralDiff: added/removed edges reference valid nodes (in old or new graph respectively)
- ChangeClassification is deterministic for same input

## Relationship to Parent Phase 3 Spec

This spec supersedes the D5-D7 sections of `2026-03-19-phase3-mcp-server-architectural-intelligence.md` for Phase 3b. Key refinements:

- **`StructuralDiff` struct:** Parent spec uses tuples for `changed_layers` and `changed_clusters`. This spec uses named structs (`LayerChange`, `ClusterChange`) for readability. Semantically identical.
- **`ArchSmell` struct:** Extended with `metrics: SmellMetrics` field (not in parent spec). Provides quantitative evidence for each smell, enabling threshold evaluation and calibration. Resolves DP-13.
- **`compute_structural_diff` signature:** Extended with `old_metrics`/`new_metrics` parameters (not in parent spec). Required because smell diffing calls `detect_smells()` which depends on Martin metrics (D5).
- **Edge comparison semantics:** Edges are compared by `(from, to, edge_type)` tuple for diff purposes. Symbol list changes within the same edge are not tracked in the structural diff.

Parent spec's D8-D10 (Phase 3c) remain unchanged.

## Design Sources

| Deliverable | Authoritative Sources |
|-------------|----------------------|
| D5: Martin Metrics | ROADMAP.md Phase 3b D5, D-040, D-048, D-049, D-056 |
| D6: Smell Detection | ROADMAP.md Phase 3b D6, D-048 |
| D7: Structural Diff | ROADMAP.md Phase 3b D7, D-048, D-057, D-058, D-059 |

## Discussion Points Resolved

| DP | Resolution |
|----|-----------|
| DP-8 | Abstract = type_def OR >80% re-export ratio. D-056. |
| DP-13 | Thresholds documented as initial heuristics. SmellMetrics provides values for evaluation. |
| DP-14 | Louvain noise filtered by edge-change correlation. D-057. |
| DP-15 | StructuralDiff is MCP-only. D-058. |
| DP-17 | ChangeClassification: Additive/Refactor/Migration/Breaking heuristic defined. D-059. |
