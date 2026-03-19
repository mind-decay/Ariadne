# Phase 2a: Implementation Plan

**Spec:** `design/specs/2026-03-18-phase2-algorithms-queries-views.md`
**Scope:** Phase 2a only (D1, D2, D3, D4, D5, D7, D8, D10, D11)
**Date:** 2026-03-18

## Chunk Overview

```
Chunk 1: Foundation — deserialization, error codes, algo scaffold (no deps beyond Phase 1b)
Chunk 2: SCC + Topological Sort (depends on 1)
Chunk 3: Blast Radius + Subgraph (depends on 1)
Chunk 4: Brandes Centrality (depends on 1)
Chunk 5: Stats + Pipeline Integration (depends on 2, 4)
Chunk 6: Markdown Views (depends on 5)
Chunk 7: CLI Query Commands (depends on 3, 5, 6)
Chunk 8: Tests + Benchmarks (depends on all)
```

**Parallel:** Chunks 2, 3, 4 are fully independent — can run in parallel. Chunk 3 can also run in parallel with Chunks 5 and 6 (only needed before Chunk 7).

---

## Chunk 1: Foundation

### Task 1.1: Error codes + diagnostic extension (`src/diagnostic.rs`)

- **Source:** Spec §Error Codes, §DiagnosticCounts Extension
- **Key points:**
  - Add `#[non_exhaustive]` to `FatalError` and `WarningCode` enums
  - Add `E006: GraphNotFound` and `E007: StatsNotFound` to `FatalError`
  - Add `W010`-`W013` to `WarningCode` enum
  - Extend `DiagnosticCounts` with: `graph_load_warnings: u32`, `algorithm_failures: u32`, `stale_stats: u32`
  - Extend `warn()` match arms for new warning codes
- **Verify:** `cargo test` — existing tests still pass
- **Commit:** `ariadne(core): add Phase 2 error codes E006-E007, W010-W013`

### Task 1.2: Model extensions (`src/model/query.rs`, `src/model/mod.rs`)

- **Source:** Spec D1, D-033
- **Key points:**
  - New file `src/model/query.rs` with `SubgraphResult`:
    ```rust
    pub struct SubgraphResult {
        pub nodes: BTreeMap<CanonicalPath, Node>,
        pub edges: Vec<Edge>,
        pub center_files: Vec<CanonicalPath>,
        pub depth: u32,
    }
    ```
  - Add `pub mod query;` to `src/model/mod.rs`
  - `SubgraphResult` derives `Debug, Clone`
- **Commit:** `ariadne(core): add SubgraphResult to model`

### Task 1.3: GraphReader trait + deserialization (`src/serial/mod.rs`, `src/serial/json.rs`)

- **Source:** Spec D1, D-032
- **Key points:**
  - Add `Deserialize` derive to `GraphOutput`, `NodeOutput`, `ClusterEntryOutput`, `ClusterOutput`
  - Custom deserialization for compact tuple edges → `(String, String, String, Vec<String>)`
  - New `StatsOutput` and `StatsSummary` types with `Serialize + Deserialize` (includes `version: u32`)
  - New `FileQueryOutput` type with `Serialize` (for `ariadne query file --format json`)
  - New `GraphReader` trait (Send + Sync):
    - `read_graph(&self, dir: &Path) -> Result<GraphOutput, FatalError>`
    - `read_clusters(&self, dir: &Path) -> Result<ClusterOutput, FatalError>`
    - `read_stats(&self, dir: &Path) -> Result<Option<StatsOutput>, FatalError>`
  - Add `write_stats(&self, stats: &StatsOutput, dir: &Path) -> Result<(), FatalError>` to `GraphSerializer`
  - Implement `GraphReader` for `JsonSerializer`: read file → `serde_json::from_reader` → return. E006 on not-found.
  - Implement `write_stats` for `JsonSerializer`: same pattern as `write_graph` (atomic write via .tmp + rename)
- **Verify:** Unit test — write graph.json → read back → compare
- **Commit:** `ariadne(serial): add GraphReader trait and deserialization`

### Task 1.4: GraphOutput → ProjectGraph conversion (`src/serial/mod.rs` or `src/serial/convert.rs`)

- **Source:** Spec D1, D-022
- **Key points:**
  - `impl TryFrom<GraphOutput> for ProjectGraph` (or standalone function)
  - String keys → `CanonicalPath::new()` (validates format)
  - Compact tuple edges → `Edge { from, to, edge_type, symbols }`
  - String enums → `FileType`, `EdgeType`, `ArchLayer` via `FromStr` or serde
  - Version check: if `graph_output.version != 1` → return error
  - `impl TryFrom<ClusterOutput> for ClusterMap` similarly
- **Verify:** Round-trip test — build fixture → serialize → deserialize → compare
- **Commit:** `ariadne(serial): implement graph deserialization conversion`

### Task 1.5: Algo module scaffold (`src/algo/mod.rs`, `src/lib.rs`)

- **Source:** Spec §Module Structure, D-033
- **Key points:**
  - Create `src/algo/mod.rs` with module declarations (all empty for now):
    ```rust
    pub mod scc;
    pub mod blast_radius;
    pub mod centrality;
    pub mod topo_sort;
    pub mod subgraph;
    ```
  - Create empty files: `src/algo/scc.rs`, `src/algo/blast_radius.rs`, `src/algo/centrality.rs`, `src/algo/topo_sort.rs`, `src/algo/subgraph.rs`
  - Add `pub mod algo;` to `src/lib.rs`
  - Helper: edge filtering function in `src/algo/mod.rs`:
    ```rust
    /// Filter edges to architectural types only (imports + re_exports + type_imports).
    /// Excludes tests edges per D-034.
    pub fn architectural_edges(edges: &[Edge]) -> Vec<&Edge>
    ```
  - Helper: build forward/reverse adjacency indices in `src/algo/mod.rs`:
    ```rust
    pub fn build_adjacency(edges: &[Edge], filter: fn(&Edge) -> bool)
        -> (BTreeMap<&CanonicalPath, Vec<&CanonicalPath>>,   // forward
            BTreeMap<&CanonicalPath, Vec<&CanonicalPath>>)   // reverse
    ```
- **Verify:** `cargo build` compiles
- **Commit:** `ariadne(graph): create algorithm module scaffold`

### Task 1.6: Verify foundation

- `cargo test` — all existing Phase 1 tests pass
- `cargo build` — clean compile with new modules

---

## Chunk 2: SCC + Topological Sort

### Task 2.1: Tarjan SCC (`src/algo/scc.rs`)

- **Source:** Spec D2, architecture.md §Algorithms §3
- **Key points:**
  - `pub fn find_sccs(graph: &ProjectGraph) -> Vec<Vec<CanonicalPath>>`
  - Use `architectural_edges` helper to filter to `imports + re_exports + type_imports`
  - Build forward adjacency index
  - Iterative Tarjan (avoid stack overflow on deep graphs): DFS with lowlink, stack-based
  - Filter to SCCs of size > 1
  - Sort: inner Vec lexicographically, outer Vec by first element
  - Edge case: empty graph → empty result
- **Verify:** Unit tests with hand-crafted graphs:
  - Linear chain (A→B→C): no SCCs
  - Simple cycle (A→B→A): one SCC [A, B]
  - Two separate cycles: two SCCs
  - DAG: no SCCs
  - Fully connected: one SCC with all nodes
- **Commit:** `ariadne(graph): implement Tarjan SCC`

### Task 2.2: Topological sort (`src/algo/topo_sort.rs`)

- **Source:** Spec D5, architecture.md §Algorithms §5, D-025
- **Key points:**
  - `pub fn topological_layers(graph: &ProjectGraph, sccs: &[Vec<CanonicalPath>]) -> BTreeMap<CanonicalPath, u32>`
  - Contract SCCs into supernodes: each SCC → single node, merge edges
  - Run Kahn's algorithm (BFS-based topo sort) on contracted DAG
  - Assign layer = longest path from sources (Layer 0 = no outgoing architectural deps)
  - All files within an SCC get the same layer
  - Degenerate: entire graph is one SCC → all get layer 0
  - Edge case: disconnected nodes → layer 0
- **Verify:** Unit tests:
  - Linear chain A→B→C: A=layer 2, B=layer 1, C=layer 0
  - DAG with multiple paths: correct max-path layer assignment
  - Graph with cycle: cycle members share layer
  - Single node: layer 0
  - Empty graph: empty result
- **Commit:** `ariadne(graph): implement topological sort`

---

## Chunk 3: Blast Radius + Subgraph

### Task 3.1: Reverse BFS / blast radius (`src/algo/blast_radius.rs`)

- **Source:** Spec D3, architecture.md §Algorithms §1
- **Key points:**
  - `pub fn blast_radius(graph: &ProjectGraph, file: &CanonicalPath, max_depth: Option<u32>) -> BTreeMap<CanonicalPath, u32>`
  - Build reverse adjacency from architectural edges
  - Standard BFS with visited set and depth tracking
  - `None` max_depth = unbounded
  - Returns `BTreeMap` (deterministic iteration)
  - File itself is NOT in the result (distance 0 is excluded, or include with distance 0 — match architecture.md pseudocode which includes the source)
  - Actually per architecture.md: source IS included with distance 0 in `visited`
- **Verify:** Unit tests:
  - Linear A→B→C, blast_radius(C) = {C:0, B:1, A:2}
  - With depth limit: blast_radius(C, depth=1) = {C:0, B:1}
  - Disconnected node: only itself
  - Nonexistent file: empty result
  - Re-export chain: correct propagation
- **Commit:** `ariadne(graph): implement blast radius`

### Task 3.2: Subgraph extraction (`src/algo/subgraph.rs`)

- **Source:** Spec D7, architecture.md §Algorithms §7, D-035
- **Key points:**
  - `pub fn extract_subgraph(graph: &ProjectGraph, files: &[CanonicalPath], depth: u32) -> SubgraphResult`
  - For each file: forward BFS (dependencies) + reverse BFS (dependents), both within `depth`
  - Uses ALL edge types (including `tests` — per D-034 exception for subgraph)
  - Cluster expansion: for each touched file, include all files in same cluster (from `Node.cluster`)
    - If cluster has >100 files → only include BFS-reachable files within that cluster (D-035)
  - Collect nodes + edges for the subgraph (only edges between included nodes)
  - Return `SubgraphResult` with center_files and depth
- **Verify:** Unit tests:
  - Small graph, depth=1: correct neighborhood
  - Cluster inclusion works
  - 100-file cluster limit triggers correctly
- **Commit:** `ariadne(graph): implement subgraph extraction`

---

## Chunk 4: Brandes Centrality

### Task 4.1: Betweenness centrality (`src/algo/centrality.rs`)

- **Source:** Spec D4, architecture.md §Algorithms §2, D-034
- **Key points:**
  - `pub fn betweenness_centrality(graph: &ProjectGraph) -> BTreeMap<CanonicalPath, f64>`
  - Brandes algorithm on architectural edges only
  - For each source s:
    1. BFS from s → compute σ (number of shortest paths) and d (distance)
    2. Back-propagation to accumulate δ values
    3. Add δ to centrality of each node
  - Normalize: divide by `(V-1)*(V-2)` for directed graphs (D-034)
  - Round to 4 decimal places: `(value * 10000.0).round() / 10000.0`
  - Return `BTreeMap` for deterministic output
  - Edge cases: V < 3 → all centrality = 0.0 (normalization divisor is 0)
  - Edge case: disconnected graph → nodes in isolated components get 0.0
- **Verify:** Unit tests:
  - Star graph (center has highest centrality)
  - Linear chain (middle nodes highest)
  - Complete graph (all equal, 0.0)
  - Known centrality values from textbook examples
  - Float determinism: run twice → identical results
- **Commit:** `ariadne(graph): implement Brandes centrality`

---

## Chunk 5: Stats + Pipeline Integration

### Task 5.1: Stats computation (`src/algo/mod.rs` or `src/algo/stats.rs`)

- **Source:** Spec D8, D-034
- **Key points:**
  - Helper function to compute `StatsOutput` from algorithm results:
    ```rust
    pub fn compute_stats(
        graph: &ProjectGraph,
        centrality: &BTreeMap<CanonicalPath, f64>,
        sccs: &[Vec<CanonicalPath>],
        layers: &BTreeMap<CanonicalPath, u32>,
    ) -> StatsOutput
    ```
  - `version: 1`
  - `centrality`: convert CanonicalPath keys to String, BTreeMap for sort
  - `sccs`: convert to Vec<Vec<String>>, inner sorted, outer sorted by first
  - `layers`: invert layer map (file→layer to layer→files), BTreeMap<String, Vec<String>>
  - `summary.max_depth`: max of all layer values
  - `summary.avg_in_degree` / `avg_out_degree`: count architectural edges per node, average, round 4 dec
  - `summary.bottleneck_files`: files with centrality > 0.7, sorted by centrality desc then path
  - `summary.orphan_files`: source/test files with zero in-degree AND zero out-degree (architectural edges), sorted by path. Exclude config/style/asset.
- **Verify:** Unit test with known graph → verify all stats fields
- **Commit:** `ariadne(graph): implement stats computation`

### Task 5.2: Wire algorithms into build pipeline (`src/pipeline/mod.rs`)

- **Source:** Spec D11 (build scope change), D-034
- **Key points:**
  - **Restructure pipeline ordering:** algorithms must run BEFORE serialization so that `graph.json` contains computed `arch_depth` values (not placeholder 0). New flow:
    ```
    walk → read → parse → resolve_and_build → cluster
      → [NEW] run algorithms → apply arch_depth
      → serialize graph.json + clusters.json + stats.json
    ```
  - Algorithm steps inserted between cluster and serialize:
    1. `algo::scc::find_sccs(&graph)` → sccs
    2. `algo::topo_sort::topological_layers(&graph, &sccs)` → layers
    3. Apply layers to graph: `graph.nodes[path].arch_depth = layers[path]`
    4. `algo::centrality::betweenness_centrality(&graph)` → centrality
    5. `algo::compute_stats(&graph, &centrality, &sccs, &layers)` → stats
  - Serialize phase now writes all three files:
    1. `serializer.write_graph(&graph_output, output_dir)?`
    2. `serializer.write_clusters(&cluster_output, output_dir)?`
    3. `serializer.write_stats(&stats, output_dir)?`
  - **Note:** `GraphOutput` conversion (`From<ProjectGraph>`) must happen AFTER arch_depth is applied, so the serialized graph.json has correct layer values
  - Update `BuildOutput` struct: add `stats_path: PathBuf`
  - Update summary format to mention stats.json
  - `--verbose` timing: add `[algorithms]` and `[stats]` timing lines
- **Verify:** `cargo test` — build fixture → check stats.json exists and valid, check `arch_depth > 0` in graph.json for non-leaf nodes
- **Commit:** `ariadne(pipeline): integrate algorithms into build pipeline`

---

## Chunk 6: Markdown Views

### Task 6.1: Views module scaffold + L0 index (`src/views/mod.rs`, `src/views/index.rs`)

- **Source:** Spec D10, architecture.md §Views
- **Key points:**
  - Create `src/views/mod.rs`: `pub mod index; pub mod cluster; pub mod impact;`
  - Add `pub mod views;` to `src/lib.rs`
  - Views functions receive pre-computed data (no algo/ dependency, per D-033):
    ```rust
    pub fn generate_index(
        graph: &ProjectGraph,
        clusters: &ClusterMap,  // or ClusterOutput
        stats: &StatsOutput,
    ) -> String
    ```
  - L0 index.md content: cluster table, critical files, cycles, architecture summary
  - Dependent counts computed from reverse edge index (built locally from graph.edges)
  - Write to `.ariadne/views/index.md`
- **Commit:** `ariadne(graph): implement L0 index view`

### Task 6.2: L1 cluster detail (`src/views/cluster.rs`)

- **Source:** Spec D10, architecture.md §Views
- **Key points:**
  - `pub fn generate_cluster_view(cluster_name, graph, stats) -> String`
  - File table: path, type, layer (arch_depth), in-degree, out-degree, centrality
  - Internal dependencies section
  - External deps (outgoing) + external dependents (incoming)
  - Tests section (files with tests edges)
  - Write to `.ariadne/views/clusters/<name>.md`
- **Commit:** `ariadne(graph): implement L1 cluster views`

### Task 6.3: L2 impact report template (`src/views/impact.rs`)

- **Source:** Spec D10
- **Key points:**
  - `pub fn generate_blast_radius_view(file, blast_result, graph) -> String`
  - `pub fn generate_subgraph_view(subgraph_result, graph) -> String`
  - These are used by `ariadne query blast-radius --format md` and `ariadne query subgraph --format md`
  - NOT called by `ariadne views generate`
  - Write to `.ariadne/views/impact/` when generated
- **Commit:** `ariadne(graph): implement L2 impact views`

### Task 6.4: Views generate orchestrator

- **Source:** Spec D11
- **Key points:**
  - Function that generates L0 + all L1 views:
    ```rust
    pub fn generate_all_views(graph, clusters, stats, output_dir) -> Result<(), FatalError>
    ```
  - Creates `views/` and `views/clusters/` directories
  - Generates `index.md` + one `clusters/<name>.md` per cluster
  - Called by `ariadne views generate` CLI command
- **Commit:** `ariadne(graph): implement views generate orchestrator`

---

## Chunk 7: CLI Query Commands

### Task 7.1: Query subcommand structure (`src/main.rs`)

- **Source:** Spec D11
- **Key points:**
  - Add `Query` and `Views` to `Commands` enum
  - `Query` has `#[command(subcommand)] cmd: QueryCommands`
  - `QueryCommands` enum: `BlastRadius`, `Subgraph`, `Stats`, `Centrality`, `Cluster`, `File`, `Cycles`, `Layers`
  - All accept `--format` (default: `md`) and `--graph-dir` (default: `.ariadne/graph/`)
  - `BlastRadius` and `Subgraph` accept `--depth`
  - `Centrality` accepts `--min`
  - `Views` has `#[command(subcommand)] cmd: ViewsCommands` with `Generate { output }`
  - Add `Box<dyn GraphReader>` to composition root wiring
- **Commit:** `ariadne(cli): add query and views subcommand structure`

### Task 7.2: Graph loading helper

- **Source:** Spec D1, D11
- **Key points:**
  - Helper function used by all query commands:
    ```rust
    fn load_graph(reader: &dyn GraphReader, dir: &Path) -> Result<ProjectGraph, FatalError>
    fn load_stats(reader: &dyn GraphReader, dir: &Path) -> Result<StatsOutput, FatalError>
    ```
  - `load_graph`: read_graph → TryFrom → ProjectGraph. E006 on not found.
  - `load_stats`: read_stats → unwrap Option. E007 on not found.
- **Commit:** (combined with 7.1)

### Task 7.3: Implement query commands

- **Source:** Spec D11, §JSON output schemas
- **Key points:**
  - Each query command: load graph/stats → run algorithm or extract data → format (md or json) → print to stdout
  - `blast-radius`: load graph → `algo::blast_radius()` → format
  - `subgraph`: load graph → `algo::extract_subgraph()` → format
  - `stats`: load stats → format (json = print stats.json content, md = formatted summary)
  - `centrality`: load stats → filter by `--min` → format
  - `cluster`: load graph + clusters → find cluster → format
  - `file`: load graph + stats → find node → build `FileQueryOutput` → format
  - `cycles`: load stats → extract sccs → format
  - `layers`: load stats → extract layers → format
  - `views generate`: load graph + stats → `views::generate_all_views()`
  - JSON format: serialize the output type defined in spec §JSON output schemas
  - MD format: human-readable text rendering (for blast-radius and subgraph, use L2 view generators from Chunk 6)
- **Verify:** Run each command on fixture → check output makes sense
- **Commit:** `ariadne(cli): implement query commands`

### Task 7.4: Implement `views generate` command

- **Source:** Spec D11
- **Key points:**
  - Load graph + stats → call `views::generate_all_views()`
  - `--output` flag overrides default `.ariadne/views/` directory
  - Print summary: "Generated N cluster views + index"
- **Commit:** `ariadne(cli): implement views generate command`

---

## Chunk 8: Tests + Benchmarks

### Task 8.1: Algorithm unit tests (`tests/algo/`)

- **Source:** Spec §Testing Requirements
- **Key points:**
  - Create test directory structure: `tests/algo/mod.rs`, `tests/algo/test_scc.rs`, `tests/algo/test_blast_radius.rs`, `tests/algo/test_centrality.rs`, `tests/algo/test_topo_sort.rs`, `tests/algo/test_subgraph.rs`
  - Hand-crafted graphs with known expected results for each algorithm
  - Edge cases: empty graph, single node, fully connected, one big SCC, disconnected components
  - Float determinism test for centrality: run twice → identical
- **Commit:** `ariadne(test): add algorithm unit tests`

### Task 8.2: Invariant extensions (`tests/invariants.rs`)

- **Source:** Spec §Invariant Extensions
- **Key points:**
  - Add INV-14: `arch_depth` consistent (no file depends on higher layer, outside SCCs)
  - Add INV-15: SCC members share `arch_depth`
  - Add INV-16: centrality values in [0.0, 1.0]
  - Add INV-17: layers cover all nodes
  - Add INV-18: `bottleneck_files` = exactly files with centrality > 0.7
  - Run extended invariants on all fixture builds
- **Commit:** `ariadne(test): add Phase 2 invariants INV-14 through INV-18`

### Task 8.3: Snapshot tests for stats + views

- **Source:** Spec §Snapshot Tests
- **Key points:**
  - `stats.json` snapshot test on typescript-app fixture
  - L0 index.md snapshot test
  - L1 cluster view snapshot test (pick one cluster)
  - Query output snapshots: `blast-radius --format json`, `cycles --format md`, etc.
  - Extend determinism test: build twice → stats.json byte-identical
- **Commit:** `ariadne(test): add stats and views snapshot tests`

### Task 8.4: Deserialization round-trip tests

- **Source:** Spec D1
- **Key points:**
  - Build fixture → serialize → deserialize → re-serialize → byte-identical
  - Version mismatch test: tamper version → rebuild correctly
  - Corrupted JSON test → W011 error
- **Commit:** `ariadne(test): add deserialization round-trip tests`

### Task 8.5: CLI integration tests

- **Source:** Spec §Success Criteria
- **Key points:**
  - `ariadne build` on fixture → verify stats.json created
  - `ariadne query blast-radius <file>` → verify output
  - `ariadne query cycles --format json` → verify valid JSON
  - `ariadne query stats` → verify output
  - `ariadne views generate` → verify files created
  - E006 test: query without build → error message
  - E007 test: delete stats.json, query stats → error message
- **Commit:** `ariadne(test): add CLI integration tests`

### Task 8.6: Performance benchmarks (`benches/`)

- **Source:** Spec §Performance Benchmarks
- **Key points:**
  - `bench_scc`: Tarjan on 3000-node synthetic graph (<10ms)
  - `bench_blast_radius`: BFS on 3000 nodes (<10ms)
  - `bench_centrality`: Brandes on 3000 nodes / 8000 edges (<500ms)
  - `bench_topo_sort`: topo sort on 3000 nodes (<10ms)
  - Reuse existing `generate_synthetic_project` helper from Phase 1b benchmarks
  - Or build in-memory `ProjectGraph` directly (faster setup, tests algo performance only)
- **Commit:** `ariadne(test): add algorithm performance benchmarks`

---

## Dependency Graph

```
Chunk 1 ──┬── Chunk 2 (SCC + Topo) ───┐
           ├── Chunk 3 (BFS + Subgraph)┼──────────────┐
           └── Chunk 4 (Centrality) ───┤              │
                                       ▼              │
                                   Chunk 5 (Stats)    │
                                   [needs 2, 4]       │
                                       │              │
                                   Chunk 6 (Views)    │
                                       │              │
                                   Chunk 7 (CLI) ◄────┘
                                   [needs 3, 5, 6]
                                       │
                                   Chunk 8 (Tests)
```

**Parallel opportunities:**
- Chunks 2, 3, 4 are fully independent — can run in parallel
- Chunk 3 can also run in parallel with Chunks 5 and 6 — only needed before Chunk 7
- Chunk 8 tasks (8.1 algo tests) can start as soon as the corresponding algorithm chunk is done
- Chunk 6 tasks are mostly independent of each other (L0, L1, L2)

**Critical path:** C1 → C2|C4 → C5 → C6 → C7 → C8 (6 sequential steps)
