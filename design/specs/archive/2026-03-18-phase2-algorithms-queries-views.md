# Phase 2: Algorithms, Queries & Views — Specification

**Note:** Phase 2 is split into Phase 2a and Phase 2b. This spec covers both. Phase 2a delivers the core query value (algorithms, stats, views, CLI). Phase 2b adds Louvain clustering and delta computation.

## Goal

Graph becomes queryable — blast radius, centrality, cycles, clusters, layers, markdown views.

## Dependencies

**Phase 1b must be complete.** Phase 2 builds on:

- `ProjectGraph` with `BTreeMap<CanonicalPath, Node>` + `Vec<Edge>` — full graph data model
- `ClusterMap` with directory-based clustering and cohesion metrics
- `ContentHash` on every node (xxHash64) — ready for delta computation
- `arch_depth: 0` placeholder on all nodes (D-025) — ready for topological sort
- `BuildPipeline` with injectable traits (`FileWalker`, `FileReader`, `GraphSerializer`)
- Full CLI infrastructure (clap) with `build` and `info` subcommands
- JSON serialization (`GraphSerializer` trait + `JsonSerializer`) with deterministic output
- Structured diagnostics (`DiagnosticCollector`, `Warning`, `FatalError`)
- All Phase 1b flags (--verbose, --warnings, --strict, --timestamp, --max-file-size, --max-files)
- L1-L4 test suite (snapshots, fixtures, invariants, benchmarks)

**Phase 2b depends on Phase 2a** (Louvain needs algorithms infrastructure; delta needs deserialization).

## Phase Split

| Phase | Deliverables | Risk |
|-------|-------------|------|
| **2a** | D1 (deserialization), D2 (SCC), D3 (blast radius), D4 (centrality), D5 (topo sort), D7 (subgraph), D8 (stats.json), D10 (views), D11 (CLI queries + `views generate`) | YELLOW |
| **2b** | D6 (Louvain), D9 (delta/`ariadne update`) | ORANGE |

Phase 2a delivers 90% of user value: the graph becomes queryable. Phase 2b adds optimization (incremental builds) and refinement (community-based clustering).

## Risk Classification

**Overall: YELLOW** (Phase 2a) / **ORANGE** (Phase 2b)

Phase 2a adds `algo/` and `views/` modules — both additive, pure functions on existing data model. Phase 2b modifies core pipeline (delta) and overrides existing cluster behavior (Louvain).

### Per-Deliverable Risk

| # | Phase | Deliverable | Risk | Rationale |
|---|-------|------------|------|-----------|
| D1 | 2a | Graph deserialization (load graph.json → ProjectGraph) | YELLOW | No deserialization path exists yet. Touches `model/` types (add Deserialize derives) and `serial/` (add read methods). Mechanical but foundational |
| D2 | 2a | Tarjan SCC | GREEN | Well-known O(V+E) algorithm. Clear pseudocode in architecture.md. Pure function, no mutation of existing code |
| D3 | 2a | Reverse BFS (blast radius) | GREEN | Simple BFS variant on reversed edge list. O(V+E). Architecture.md has pseudocode |
| D4 | 2a | Brandes centrality | YELLOW | O(VE), well-documented. Risk: floating-point normalization must be deterministic (4 decimal places) |
| D5 | 2a | Topological sort | GREEN | Standard algorithm on DAG after SCC contraction. Populates `Node.arch_depth` (D-025 placeholder) |
| D6 | **2b** | Louvain clustering | ORANGE | Most complex algorithm. Overrides directory-based clusters. Iterative convergence with f64 determinism. Cluster ID naming. Directed→undirected conversion |
| D7 | 2a | Subgraph extraction | GREEN | Bidirectional BFS + cluster inclusion. Combines existing primitives |
| D8 | 2a | stats.json output | YELLOW | New output type + serialization. Deterministic sort orders for all collections. Float rounding |
| D9 | **2b** | Delta computation (`ariadne update`) | ORANGE | Modifies core pipeline. Needs deserialization (D1). Threshold logic. Incremental correctness |
| D10 | 2a | Markdown views (L0/L1/L2) | YELLOW | Template-based, no algorithmic risk. Volume of work: 3 view levels. New `views/` module |
| D11 | 2a | CLI commands (`query *`, `views generate`) | YELLOW | 8+ subcommands with `--format json|md`. Graph loading (D1). Volume |

## Deliverables

### D1: Graph Deserialization

**Files:** `src/model/*.rs` (add `Deserialize`), `src/model/query.rs` (new — `SubgraphResult`), `src/serial/mod.rs` + `src/serial/json.rs` (add reader trait + impl)

Add `serde::Deserialize` derives to output types (`GraphOutput`, `NodeOutput`, `ClusterOutput`, `StatsOutput`). Implement `GraphOutput → ProjectGraph` conversion (reverse of existing `From<ProjectGraph> for GraphOutput`):

- String keys → `CanonicalPath` newtypes (validate during deserialization)
- Compact tuple edges → `Edge` structs
- String enums → `FileType`, `EdgeType`, `ArchLayer` enums

Introduce a new **`GraphReader` trait** (D-032), separate from `GraphSerializer` (which remains write-only per D-019):

```rust
/// Read-side counterpart to GraphSerializer. Separate trait because
/// read and write have different error semantics (missing file may be
/// acceptable for reads, never for writes). See D-032.
trait GraphReader: Send + Sync {
    fn read_graph(&self, dir: &Path) -> Result<GraphOutput, FatalError>;
    fn read_clusters(&self, dir: &Path) -> Result<ClusterOutput, FatalError>;
    fn read_stats(&self, dir: &Path) -> Result<Option<StatsOutput>, FatalError>;
}
```

`JsonSerializer` implements both `GraphSerializer` and `GraphReader`. Test mocks implement each independently.

Add `write_stats` to the existing `GraphSerializer` trait:

```rust
fn write_stats(&self, stats: &StatsOutput, dir: &Path) -> Result<(), FatalError>;
```

Add `SubgraphResult` to `src/model/query.rs` (pure data type in `model/`, per D-022 separation):

```rust
pub struct SubgraphResult {
    pub nodes: BTreeMap<CanonicalPath, Node>,
    pub edges: Vec<Edge>,
    pub center_files: Vec<CanonicalPath>,  // the query input files
    pub depth: u32,
}
```

**Error handling:**
- File not found → `E006: GraphNotFound` (callers decide: fall back to full build, or show error)
- Version mismatch → `W010: GraphVersionMismatch`, fall back to full rebuild
- Corrupted/unparseable → `W011: GraphCorrupted`, fall back to full rebuild

**Design source:** architecture.md §Output Model (D-022), D-019, D-032

### D2: Tarjan SCC

**Files:** `src/algo/mod.rs` (new), `src/algo/scc.rs` (new)

```rust
pub fn find_sccs(graph: &ProjectGraph) -> Vec<Vec<CanonicalPath>>
```

- DFS with lowlink tracking. O(V+E).
- Returns only SCCs of size > 1 (circular dependencies).
- Inner Vec sorted lexicographically. Outer Vec sorted by first element.
- Edge types traversed: `imports`, `re_exports`, `type_imports`. Excludes `tests` edges.
- Must run BEFORE topological sort (D5).

**Design source:** architecture.md §Algorithms §3

### D3: Reverse BFS (Blast Radius)

**Files:** `src/algo/blast_radius.rs` (new)

```rust
pub fn blast_radius(
    graph: &ProjectGraph,
    file: &CanonicalPath,
    max_depth: Option<u32>,
) -> BTreeMap<CanonicalPath, u32>
```

- Build reverse adjacency index from `ProjectGraph.edges`.
- Edge types traversed: `imports`, `re_exports`, `type_imports`. Excludes `tests`.
- Depth semantics: 1=direct dependents, 2=transitive, 3+=distant. `None`=unbounded.
- O(V+E). Returns `BTreeMap` for determinism.
- Re-exports propagate correctly: if B re-exports from C, and C changes, reverse BFS from C reaches B, then B's dependents.

**Design source:** architecture.md §Algorithms §1

### D4: Brandes Centrality

**Files:** `src/algo/centrality.rs` (new)

```rust
pub fn betweenness_centrality(graph: &ProjectGraph) -> BTreeMap<CanonicalPath, f64>
```

- Brandes algorithm. O(VE).
- **Normalization:** Divide raw BC by `(V-1)(V-2)` for directed graphs → values in [0.0, 1.0].
- **Float determinism:** Round to 4 decimal places before output (per `determinism.md` pattern).
- Edge types: `imports`, `re_exports`, `type_imports`. Excludes `tests`.
- Files with BC > 0.7 (normalized) are "bottlenecks" → `stats.json.summary.bottleneck_files`.
- Run once per build, cached in `stats.json`.

**Design source:** architecture.md §Algorithms §2, performance.md §Phase 2

### D5: Topological Sort

**Files:** `src/algo/topo_sort.rs` (new)

```rust
pub fn topological_layers(
    graph: &ProjectGraph,
    sccs: &[Vec<CanonicalPath>],
) -> BTreeMap<CanonicalPath, u32>
```

- Contract SCCs into supernodes → DAG. Run topological sort on DAG.
- Layer 0: no outgoing deps (utils, constants, types). Layer N: depends on layers 0..N-1.
- All files in an SCC get the same layer.
- Edge types: `imports`, `re_exports`, `type_imports`. Excludes `tests`.
- **Degenerate case:** Entire graph is one SCC → all files get `arch_depth = 0`, `max_depth = 0`.
- Updates `Node.arch_depth` field (currently 0, D-025).

**Design source:** architecture.md §Algorithms §5, D-025

### D6: Louvain Clustering

**Files:** `src/algo/louvain.rs` (new)

```rust
pub fn louvain_clustering(
    graph: &ProjectGraph,
    initial_clusters: &ClusterMap,
) -> ClusterMap
```

- Modularity maximization `Q = (1/2m) sum_{ij} [A_ij - k_i*k_j / 2m] * δ(c_i, c_j)`. O(n log n).
- **Input:** Directed graph → convert to undirected weights for modularity computation.
- **Initialization:** Start from directory-based clusters (Phase 1 output).
- **Cluster naming:** For each Louvain community, use the directory-based name of the plurality of files. If tied, use the name that comes first lexicographically.
- **Override policy:** If Louvain reassigns a file, it overrides the directory-based cluster. Louvain runs by default; `--no-louvain` flag to disable.
- **Float determinism:** Modularity values rounded to 4 decimal places.
- **Convergence:** Fixed iteration limit (e.g., 100) + convergence threshold (ΔQ < 1e-6). If convergence fails → `W012: AlgorithmFailed`, fall back to directory-based clusters.
- Returns new `ClusterMap` with reassigned files and recomputed cohesion.

**Design source:** architecture.md §Algorithms §4

### D7: Subgraph Extraction

**Files:** `src/algo/subgraph.rs` (new)

```rust
pub fn extract_subgraph(
    graph: &ProjectGraph,
    files: &[CanonicalPath],
    depth: u32,
) -> SubgraphResult  // defined in model/query.rs (D1)
```

- For each file: BFS outward (forward edges) + BFS inward (reverse edges) within depth.
- Include full cluster for each touched file (post-Louvain clusters from `Node.cluster`).
- **Cluster expansion limit:** If a cluster has >100 files, include only BFS-reachable files within that cluster, not all files (D-035).
- Edge types: all (including `tests` for subgraph — they're relevant for scoped views).
- Returns `SubgraphResult` (defined in `model/query.rs`) — a filtered subset of the graph.

**Design source:** architecture.md §Algorithms §7, D-035

### D8: stats.json Output

**Files:** `src/serial/mod.rs` (add types), `src/serial/json.rs` (add `write_stats`)

**Output path:** `.ariadne/graph/stats.json`

```rust
#[derive(Serialize, Deserialize)]
pub struct StatsOutput {
    pub version: u32,                            // schema version (1), for forward compatibility
    pub centrality: BTreeMap<String, f64>,       // path → BC, sorted by path
    pub sccs: Vec<Vec<String>>,                  // inner sorted, outer sorted by first element
    pub layers: BTreeMap<String, Vec<String>>,    // layer_number → sorted file paths
    pub summary: StatsSummary,
}

#[derive(Serialize, Deserialize)]
pub struct StatsSummary {
    pub max_depth: u32,
    pub avg_in_degree: f64,                      // 4 decimal places
    pub avg_out_degree: f64,                     // 4 decimal places
    pub bottleneck_files: Vec<String>,           // sorted by centrality desc, then path
    pub orphan_files: Vec<String>,               // sorted by path
}
```

**Definitions:**
- **Orphan:** `source` or `test` file with zero incoming edges AND zero outgoing `imports`/`type_imports`/`re_exports` edges. Config, style, asset files excluded.
- **Degree metrics:** Count `imports` + `re_exports` + `type_imports` edges. Exclude `tests` edges.
- **Bottleneck threshold:** BC > 0.7 (normalized).

**Determinism:** All sort orders explicitly defined above. Float values rounded to 4 decimal places. Git-tracked (same as graph.json).

Add `write_stats` to `GraphSerializer` trait.

**Design source:** architecture.md §Storage Format (stats.json), determinism.md

### D9: Delta Computation (`ariadne update`)

**Files:** `src/algo/delta.rs` (new — pure diff logic), `src/pipeline/mod.rs` (extend — orchestration)

**Responsibility split:** `algo/delta.rs` contains only the pure diff/merge logic (hash comparison, change classification). `pipeline/mod.rs` orchestrates the full update flow (load old graph, walk, read, re-parse, call delta, recompute). This preserves the `algo/` dependency rule: `algo/` depends on `model/` only, never on `pipeline/` or `serial/`.

`algo/delta.rs` — pure diff logic:
```rust
pub struct DeltaResult {
    pub changed: Vec<CanonicalPath>,
    pub added: Vec<CanonicalPath>,
    pub removed: Vec<CanonicalPath>,
    pub requires_full_recompute: bool,
}

/// Compare old graph against current file set. Pure function — no I/O.
pub fn compute_delta(
    old_nodes: &BTreeMap<CanonicalPath, Node>,
    current_files: &[(CanonicalPath, ContentHash)],
) -> DeltaResult
```

`pipeline/mod.rs` — orchestration:
1. Load existing `graph.json` via `GraphReader` (D1)
2. Walk + read current files (reuses existing pipeline stages)
3. Call `algo::delta::compute_delta(old_graph.nodes, current_hashes)`
4. Re-parse only `changed ∪ added` files
5. Remove edges from/to `removed` files, rebuild edges for `changed ∪ added`
6. If `requires_full_recompute` (>5% changed) → full recompute of derived data
7. Otherwise → incremental cluster update, reuse previous centrality
8. Always regenerate `stats.json` after update (algorithms are fast enough)
9. Do NOT regenerate views automatically

**Fallback behavior:**
- No `graph.json` → fall back to full `build`
- Version mismatch → `W010` + full rebuild
- Corrupted graph.json → `W011` + full rebuild

**CLI:** `ariadne update <project-root> [--output <dir>]` with same flags as `build`.

**Design source:** architecture.md §Algorithms §6

### D10: Markdown Views

**Files:** `src/views/mod.rs` (new), `src/views/index.rs`, `src/views/cluster.rs`, `src/views/impact.rs`

**Output directory:** `.ariadne/views/`

#### L0: Index (`views/index.md`)
- ~200-500 tokens (advisory, not enforced)
- Cluster list with file count + key file (highest centrality per cluster)
- Critical files (BC > 0.7) with dependent count
- Circular dependencies (SCCs)
- Architecture summary (layer count, max depth, file distribution)

#### L1: Cluster Detail (`views/clusters/<name>.md`)
- ~500-2000 tokens per cluster (advisory)
- File table: file, type, layer, in-degree, out-degree, centrality
- Internal dependencies
- External dependencies (outgoing) + external dependents (incoming)
- Tests

#### L2: Impact Reports (`views/impact/`)
- Generated on-demand by `ariadne query blast-radius --format md` and `ariadne query subgraph --format md`
- NOT generated by `ariadne views generate` (which generates L0 + L1 only)

**Dependent counts:** Computed during view generation from reverse adjacency index (not stored). Token budgets are advisory targets, not enforced limits.

**Module dependency:** `views/` depends on `model/` and output types from `serial/`.

**Git tracking:** All views committed to git (per architecture.md §Git Tracking Policy).

**Design source:** architecture.md §Views

### D11: CLI Commands

**Files:** `src/main.rs` (extend `Commands` enum)

#### Build scope change
`ariadne build` in Phase 2 always produces `graph.json` + `clusters.json` + `stats.json`. Algorithms (SCC, centrality, topo sort) run as part of every build. Louvain also runs by default (Phase 2b; `--no-louvain` to disable).

#### Phase 2a commands

```
ariadne query blast-radius <file> [--depth N] [--format json|md]
ariadne query subgraph <file...> [--depth N] [--format json|md]
ariadne query stats [--format json|md]
ariadne query centrality [--min N] [--format json|md]
ariadne query cluster <name> [--format json|md]
ariadne query file <path> [--format json|md]
ariadne query cycles [--format json|md]
ariadne query layers [--format json|md]

ariadne views generate [--output <dir>]
```

`ariadne query centrality` — dedicated command for bottleneck analysis. `--min` filters to files with centrality >= N (default: 0.0, show all). Maps 1:1 to Phase 3 MCP tool `ariadne_centrality`.

#### Phase 2b commands

```
ariadne update <project-root> [--output <dir>]
    Incremental update via delta computation (D9)
```

#### Common rules

- `query` is a clap subcommand group with sub-subcommands.
- Default `--format` is `md`. `json` for programmatic use.
- All query commands accept `--graph-dir` (default: `.ariadne/graph/`) to locate graph.json/stats.json.
- Query commands load previously-built graph from disk (D1). They do NOT re-parse source files.
- If graph.json doesn't exist → `E006: GraphNotFound` with message suggesting `ariadne build` first.
- If stats.json doesn't exist for commands that need it → `E007: StatsNotFound` with message suggesting `ariadne build` first.

#### JSON output schemas (`--format json`)

Each query command serializes a defined type when `--format json` is used:

| Command | JSON output type | Notes |
|---------|-----------------|-------|
| `blast-radius` | `BTreeMap<String, u32>` | path → distance |
| `subgraph` | `SubgraphResult` (from `model/query.rs`) serialized via serde | nodes + edges + center_files + depth |
| `stats` | `StatsOutput` (from `serial/mod.rs`) | full stats.json content |
| `centrality` | `BTreeMap<String, f64>` | path → BC score, filtered by `--min` |
| `cluster` | `ClusterEntryOutput` | single cluster detail |
| `file` | `FileQueryOutput { node, incoming, outgoing, centrality, cluster }` | new type in `serial/mod.rs` |
| `cycles` | `Vec<Vec<String>>` | SCC list |
| `layers` | `BTreeMap<String, Vec<String>>` | layer → file paths |

`FileQueryOutput` is a new output type defined in `src/serial/mod.rs`:
```rust
#[derive(Serialize)]
pub struct FileQueryOutput {
    pub path: String,
    pub node: NodeOutput,
    pub incoming_edges: Vec<(String, String, String, Vec<String>)>,  // compact tuple format
    pub outgoing_edges: Vec<(String, String, String, Vec<String>)>,
    pub centrality: Option<f64>,
    pub cluster: String,
}
```

**Design source:** architecture.md §CLI Interface

## DiagnosticCounts Extension

Add fields to `DiagnosticCounts` in `src/diagnostic.rs` for Phase 2 warnings:

```rust
pub struct DiagnosticCounts {
    // ... existing Phase 1 fields ...
    pub graph_load_warnings: u32,   // W010 + W011 combined
    pub algorithm_failures: u32,    // W012
    pub stale_stats: u32,           // W013
}
```

The `warn()` method in `DiagnosticCollector` is extended with new arms for `W010`-`W013`. `FatalError` enum gains `E006` and `E007` variants. Both `FatalError` and `WarningCode` should be marked `#[non_exhaustive]` for future extensibility.

## Error Codes (Phase 2)

### New Fatal Errors

| Error | Cause | Message |
|-------|-------|---------|
| `E006: GraphNotFound` | `ariadne query` or `ariadne update` when graph.json doesn't exist | `error: graph not found in {path}. Run 'ariadne build' first.` |
| `E007: StatsNotFound` | `ariadne query stats/layers/cycles` when stats.json doesn't exist | `error: stats not found in {path}. Run 'ariadne build' first.` |

### New Warnings

| Warning | Cause | Handling |
|---------|-------|----------|
| `W010: GraphVersionMismatch` | graph.json `version` field doesn't match current code | Fall back to full rebuild, emit warning |
| `W011: GraphCorrupted` | graph.json exists but can't be parsed | Fall back to full rebuild, emit warning |
| `W012: AlgorithmFailed` | An algorithm failed (e.g., Louvain didn't converge) | Skip that output, continue with remaining algorithms. Fall back to directory clusters if Louvain fails |
| `W013: StaleStats` | stats.json modification time older than graph.json | Recompute stats, emit warning |

## Module Structure Changes

```
src/
├── model/
│   └── query.rs             # NEW — SubgraphResult (pure data, used by algo/ and CLI)
├── algo/                    # NEW — depends on model/ only (D-033)
│   ├── mod.rs               # Re-exports
│   ├── scc.rs               # Tarjan SCC (D2)
│   ├── blast_radius.rs      # Reverse BFS (D3)
│   ├── centrality.rs        # Brandes betweenness centrality (D4)
│   ├── topo_sort.rs         # Topological sort after SCC contraction (D5)
│   ├── louvain.rs           # Louvain community detection (D6)
│   ├── subgraph.rs          # Subgraph extraction (D7)
│   └── delta.rs             # Delta diff logic only — no I/O (D9)
├── views/                   # NEW — depends on model/, serial/ (output types only)
│   ├── mod.rs               # Re-exports
│   ├── index.rs             # L0 index generation (D10)
│   ├── cluster.rs           # L1 per-cluster detail (D10)
│   └── impact.rs            # L2 on-demand reports (D10)
├── serial/
│   └── mod.rs               # EXTENDED — adds GraphReader trait (D-032), StatsOutput,
│                            #   FileQueryOutput, write_stats on GraphSerializer
└── (other existing modules unchanged)
```

**Updated dependency rules (D-033):**

| Module | Depends on | Never depends on |
|--------|-----------|-----------------|
| `algo/` | `model/` | `parser/`, `pipeline/`, `serial/`, `detect/`, `cluster/`, `views/` |
| `views/` | `model/`, `serial/` (output types only, never serialization methods) | `parser/`, `pipeline/`, `detect/`, `cluster/`, `algo/` |
| `pipeline/` | (existing) + `algo/` (for running algorithms after build) | concrete implementations |
| `serial/` | `model/`, `diagnostic.rs` (for `FatalError`) | (unchanged) |

## Design Sources

| Deliverable | Authoritative Sources |
|-------------|----------------------|
| D1: Deserialization | architecture.md §Output Model (D-022), D-032, D-033 |
| D2: Tarjan SCC | architecture.md §Algorithms §3, D-034 |
| D3: Blast radius | architecture.md §Algorithms §1, D-034 |
| D4: Centrality | architecture.md §Algorithms §2, performance.md §Phase 2, D-034 |
| D5: Topo sort | architecture.md §Algorithms §5, D-025 |
| D6: Louvain | architecture.md §Algorithms §4, D-034 |
| D7: Subgraph | architecture.md §Algorithms §7, D-035 |
| D8: stats.json | architecture.md §Storage Format, determinism.md |
| D9: Delta computation | architecture.md §Algorithms §6, D-033 |
| D10: Markdown views | architecture.md §Views, D-033 |
| D11: CLI commands | architecture.md §CLI Interface |

## Success Criteria

### Phase 2a

1. `ariadne build` on fixture project produces `graph.json` + `clusters.json` + `stats.json`
2. `stats.json` contains centrality, SCCs, layers, and summary with correct values
3. `Node.arch_depth` is computed correctly (no longer all-zeros)
4. `ariadne query blast-radius <file>` returns correct dependents with distances
5. `ariadne query cycles` lists all circular dependencies
6. `ariadne query layers` shows topological layer assignments
7. `ariadne query stats`, `ariadne query cluster`, `ariadne query file` produce correct output
8. `ariadne query subgraph <files>` extracts correct neighborhood
9. All query commands support `--format json` and `--format md`
10. `ariadne views generate` produces L0 index + L1 per-cluster markdown files
11. All output is byte-identical on repeated builds (determinism — D-006)
12. All `cargo test` pass (existing + new)
13. Algorithm performance within budgets (BFS <10ms, Brandes <500ms, Tarjan <10ms, topo sort <10ms)
14. `E006`/`E007` shown when querying without prior build

### Phase 2b

15. Louvain clustering enriches `clusters.json` (may reassign files from directory-based clusters)
16. `--no-louvain` flag disables Louvain and uses directory-only clusters
17. `ariadne update` performs incremental update (only re-parses changed files)
18. `ariadne update` with no prior graph.json falls back to full build
19. `ariadne update` with version mismatch or corrupted graph.json falls back to full build with warning
20. Louvain performance within budget (<200ms on 3000 nodes)
21. Delta computation within budget (<1s for 10 changed files out of 3000)

## Testing Requirements

### Algorithm Correctness Tests
- Hand-crafted graphs with known results for each algorithm (SCC, blast radius, centrality, topo sort, Louvain, subgraph)
- Edge cases: empty graph, single-node graph, fully connected graph, fully cyclic graph (one SCC), disconnected components

### Snapshot Tests
- `stats.json` snapshot on existing fixtures
- L0 and L1 markdown view snapshots on fixtures
- Query output snapshots (md and json format)

### Invariant Extensions
- INV-14: `arch_depth` consistent with topological ordering (no file depends on a higher-layer file, outside SCCs)
- INV-15: All SCC members have the same `arch_depth`
- INV-16: Centrality values in [0.0, 1.0]
- INV-17: Layers cover all nodes
- INV-18: `bottleneck_files` = exactly files with centrality > 0.7

### Performance Benchmarks (Phase 2a)
- `bench_blast_radius` on 3000-node graph: <10ms
- `bench_centrality` (Brandes) on 3000/8000: <500ms
- `bench_scc` (Tarjan) on 3000 nodes: <10ms
- `bench_topo_sort` on 3000 nodes: <10ms

### Performance Benchmarks (Phase 2b)
- `bench_louvain` on 3000 nodes: <200ms
- `bench_delta` on 3000-node graph with 10 changes: <1s

### Existing Tests
- All Phase 1a/1b tests continue to pass (no regressions)
- Existing fixture graphs gain `stats.json` snapshots
- Determinism test extended to cover `stats.json` byte-identity

## Resolved Design Decisions

| # | Question | Resolution | Decision Log |
|---|----------|------------|-------------|
| DP-1 | Edge type filtering in algorithms | `imports` + `re_exports` + `type_imports` for all algorithms, excluding `tests`. Tests are not architectural dependencies. Subgraph includes all types (tests relevant for scoped views). `--include-tests` flag deferred. | D-034 |
| DP-2 | `ariadne build` always running all algorithms | Always. ~720ms overhead acceptable for 4-10s builds. Avoids "forgot `--stats`" UX problem. One command = complete result. | D-034 |
| DP-3 | Louvain mandatory vs optional | On by default (no existing consumers, no breaking change). `--no-louvain` flag as escape hatch. | D-034 |
| DP-4 | Phase 2 error codes | E006, E007, W010-W013. `FatalError` and `WarningCode` enums marked `#[non_exhaustive]`. | See Error Codes section |
| DP-5 | Subgraph cluster expansion limit | 100 files (hardcoded). Large enough for real modules, prevents balloon. Configurable flag deferred. | D-035 |
| DP-6 | Phase 2a/2b split | Split confirmed. 2a = algorithms + stats + views + queries (YELLOW). 2b = Louvain + delta (ORANGE). | D-036 |
| DP-7 | `views/` dependency on `algo/` | Pre-computed data via `StatsOutput` + `ProjectGraph`. Views computes in/out degree from edges directly. No `algo/` dependency. | D-033 |
| DP-8 | `GraphSerializer` trait split | Separate `GraphReader` trait for read methods. `GraphSerializer` remains write-only per D-019. `JsonSerializer` implements both. | D-032 |
| DP-9 | Delta orchestration boundary | Pure diff logic in `algo/delta.rs` (depends on `model/` only). Walk/read/re-parse orchestration in `pipeline/`. | D-033 |
| DP-10 | Centrality normalization | Divide raw BC by `(V-1)(V-2)` for directed graphs → [0.0, 1.0]. Round to 4 decimal places. Threshold 0.7 on normalized values. | D-034 |
| DP-11 | Orphan definition | `source`/`test` file with zero incoming AND zero outgoing `imports`/`type_imports`/`re_exports` edges. Config/style/asset excluded. | D-034 |
| DP-12 | Louvain parameters | 100 iteration limit, ΔQ < 1e-6 convergence threshold. Directed→undirected weight conversion. Cluster naming: plurality of directory names, lexicographic tie-break. | D-034 |
| DP-13 | `arch_depth` transition | After Phase 2, `arch_depth > 0` no longer means "not computed." D-025 sentinel behavior ends. One-time diff is expected and intentional. | D-025 (existing) |
