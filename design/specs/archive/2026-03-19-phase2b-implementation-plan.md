# Phase 2b: Implementation Plan

**Spec:** `design/specs/2026-03-18-phase2-algorithms-queries-views.md`
**Scope:** Phase 2b only (D6: Louvain clustering, D9: Delta computation)
**Date:** 2026-03-19

## Chunk Overview

```
Chunk 1: Louvain Clustering (no deps beyond Phase 2a)
Chunk 2: Delta Computation (depends on 1 — Louvain is part of recompute path)
Chunk 3: CLI Integration — `ariadne update` command (depends on 2)
Chunk 4: Tests + Benchmarks (depends on all)
```

**Parallel:** Chunks 1 and 2 are largely independent in implementation, but Chunk 2's full recompute path calls Louvain. In practice, build sequentially: Louvain first, then delta.

---

## Chunk 1: Louvain Clustering

### Task 1.1: Louvain algorithm (`src/algo/louvain.rs`)

- **Source:** Spec D6, architecture.md §Algorithms §4, D-034, D-049
- **Key points:**
  - New file `src/algo/louvain.rs`
  - Add `pub mod louvain;` to `src/algo/mod.rs`
  - Signature:
    ```rust
    pub fn louvain_clustering(
        graph: &ProjectGraph,
        initial_clusters: &ClusterMap,
    ) -> (ClusterMap, Vec<Warning>)
    ```
  - Returns a tuple: the (possibly refined) `ClusterMap` and a list of warnings. On success the warnings vec is empty. On convergence failure it contains `W012: AlgorithmFailed` and the returned `ClusterMap` is `initial_clusters` unchanged.
  - **Directed → undirected conversion:** For modularity computation, treat each directed edge as an undirected edge. If A→B and B→A both exist, weight = 2. If only A→B, weight = 1. Use architectural edges only (imports + re_exports + type_imports, exclude tests — per D-034).
  - **Initialization:** Start from directory-based clusters (the `initial_clusters` parameter). Each file starts in its directory-based community.
  - **Phase 1 — Local moves:** For each node (in BTreeMap key order for determinism per D-049), try moving it to each neighbor's community. Accept the move that gives the maximum ΔQ > 0. Repeat until no moves improve Q (or max 100 iterations).
  - **Phase 2 — Aggregation:** Collapse communities into supernodes. Rebuild graph with edges between supernodes. Repeat Phase 1 on the collapsed graph. Continue until no improvement.
  - **Convergence:** Fixed iteration limit = 100 per phase, convergence threshold ΔQ < 1e-6. If convergence fails → return `(initial_clusters.clone(), vec![W012 warning])`. The caller (pipeline) forwards warnings to `DiagnosticCollector`.
  - **Float determinism:** Modularity Q and ΔQ use f64. Node processing order is BTreeMap key order. Final cohesion values rounded to 4 decimal places (D-049).
  - **Cluster naming:** For each Louvain community, count directory-based cluster names of member files. Use the name with the highest count (plurality). If tied, use the name that comes first lexicographically.
  - **Return value:** New `ClusterMap` with:
    - Reassigned file→cluster mappings
    - Recomputed cohesion per cluster (internal_edges / total_edges, rounded to 4 decimals)
    - Cluster files sorted lexicographically
  - **Edge case — empty graph:** Return `initial_clusters` unchanged.
  - **Edge case — single file:** Return `initial_clusters` unchanged.
  - **Edge case — no edges:** Each file stays in its directory-based cluster (no modularity gain from moves).
- **Verify:** `cargo test` — new unit tests + existing tests still pass
- **Commit:** `ariadne(graph): implement Louvain community detection`

### Task 1.2: Wire Louvain into build pipeline (`src/pipeline/mod.rs`)

- **Source:** Spec D11 (build scope change), D-034
- **Key points:**
  - Add `no_louvain: bool` parameter to `run_with_output`. Update all existing call sites (Build command handler in `main.rs`, any test helpers that call `run_with_output` directly) to pass the new parameter.
  - In `run_with_output`, after Stage 5 (cluster) and before Stage 6 (algorithms):
    - If `!no_louvain`:
      - Insert Louvain call: `let (cluster_map, louvain_warnings) = algo::louvain::louvain_clustering(&graph, &cluster_map);`
      - Forward `louvain_warnings` to `DiagnosticCollector` via `diagnostics.warn(w)` for each
      - Re-apply cluster assignments to nodes after Louvain (same loop as existing Stage 5)
    - If `no_louvain`: skip Louvain entirely, keep directory-based `cluster_map`
  - This replaces directory-based `cluster_map` with Louvain-refined `cluster_map` (when enabled)
  - The rest of the pipeline (algorithms, serialization) operates on the resulting clusters
  - **Verbose output:** Add `[louvain]` timing line when verbose is enabled:
    ```
    [louvain]   12ms  14 clusters (was 18 directory-based)
    ```
- **Verify:** `cargo test` — build fixture → clusters.json may change (Louvain may reassign), arch_depth and stats still valid
- **Commit:** `ariadne(pipeline): integrate Louvain clustering into build pipeline`

### Task 1.3: `--no-louvain` CLI flag (`src/main.rs`)

- **Source:** Spec D6, D11
- **Key points:**
  - Add `--no-louvain` flag to `Build` command:
    ```rust
    /// Disable Louvain clustering (use directory-based clusters only)
    #[arg(long)]
    no_louvain: bool,
    ```
  - Pass the flag through to `BuildPipeline::run_with_output`
  - Default behavior (no flag) = Louvain enabled
- **Verify:** `ariadne build --no-louvain` produces directory-only clusters, `ariadne build` produces Louvain-refined clusters
- **Commit:** `ariadne(cli): add --no-louvain flag`

---

## Chunk 2: Delta Computation

### Task 2.1: Delta diff logic (`src/algo/delta.rs`)

- **Source:** Spec D9, architecture.md §Algorithms §6, D-033
- **Key points:**
  - New file `src/algo/delta.rs`
  - Add `pub mod delta;` to `src/algo/mod.rs`
  - Pure diff logic — no I/O, depends on `model/` only:
    ```rust
    pub struct DeltaResult {
        pub changed: Vec<CanonicalPath>,
        pub added: Vec<CanonicalPath>,
        pub removed: Vec<CanonicalPath>,
        pub requires_full_recompute: bool,
    }

    /// Compare old graph nodes against current file hashes.
    /// Pure function — no I/O.
    pub fn compute_delta(
        old_nodes: &BTreeMap<CanonicalPath, Node>,
        current_files: &[(CanonicalPath, ContentHash)],
    ) -> DeltaResult
    ```
  - **Logic:**
    1. Build `BTreeMap` from `current_files` for O(log n) lookup
    2. `changed`: files present in both `old_nodes` and `current_files` where `hash` differs
    3. `added`: files in `current_files` but not in `old_nodes`
    4. `removed`: files in `old_nodes` but not in `current_files`
    5. `requires_full_recompute`: `(changed.len() + added.len() + removed.len()) > (old_nodes.len() as f64 * 0.05) as usize`
  - All result vectors sorted lexicographically (BTreeMap gives this automatically)
  - **Edge case — empty old graph:** All files are `added`, `requires_full_recompute = true`
  - **Edge case — empty current files:** All files are `removed`, `requires_full_recompute = true`
  - **Edge case — no changes:** All vectors empty, `requires_full_recompute = false`
- **Verify:** Unit tests with various delta scenarios
- **Commit:** `ariadne(graph): implement delta computation logic`

### Task 2.2: Update pipeline orchestration (`src/pipeline/mod.rs`)

- **Source:** Spec D9, architecture.md §Algorithms §6
- **Key points:**
  - Add new method to `BuildPipeline`:
    ```rust
    pub fn update(
        &self,
        root: &Path,
        config: WalkConfig,
        reader: &dyn GraphReader,
        output_dir: Option<&Path>,
        timestamp: bool,
        verbose: bool,
        no_louvain: bool,
    ) -> Result<BuildOutput, FatalError>
    ```
  - **Flow:**
    1. Load existing graph via `reader.read_graph(output_dir)` → `GraphOutput`
    2. Convert `GraphOutput` → `ProjectGraph` via `TryFrom` (existing from Phase 2a)
    3. If load fails (E006, W010, W011) → fall back to full `run_with_output` build
    4. Walk + read current files (reuse existing Stage 1 + Stage 2)
    5. Collect `(CanonicalPath, ContentHash)` pairs from read files
    6. Call `algo::delta::compute_delta(old_graph.nodes, current_hashes)`
    7. If `delta.requires_full_recompute` → fall back to full `run_with_output`
    8. If no changes (`changed`, `added`, `removed` all empty) → short-circuit, return early with existing output paths
    9. **Incremental update:**
       - Re-parse only `changed ∪ added` files (reuse Stage 3 parse logic)
       - Remove edges from/to `removed` files from old graph
       - Remove edges from `changed` files from old graph (will be rebuilt)
       - Rebuild edges for `changed ∪ added` files (reuse Stage 4 resolve logic)
       - Add new nodes for `added` files, update nodes for `changed` files, remove `removed` nodes
    10. Re-cluster (directory-based → Louvain if enabled)
    11. Re-run algorithms (SCC, topo sort, centrality) — always, even for incremental updates (algorithms are fast)
    12. Serialize all three outputs (graph.json, clusters.json, stats.json)
  - **Note:** Views are NOT regenerated by `update` (per spec D9)
  - **Verbose output:** Show delta summary:
    ```
    [delta]     3ms  2 changed, 1 added, 0 removed (incremental)
    ```
    or:
    ```
    [delta]     3ms  150 changed, 20 added, 5 removed (full recompute — >5% threshold)
    ```
  - **Warning handling:**
    - `W010: GraphVersionMismatch` → log warning, fall back to full build
    - `W011: GraphCorrupted` → log warning, fall back to full build
- **Verify:** `cargo test` — update fixture with modified file → check incremental behavior
- **Commit:** `ariadne(pipeline): implement incremental update orchestration`

---

## Chunk 3: CLI — `ariadne update` Command

### Task 3.1: `ariadne update` command (`src/main.rs`)

- **Source:** Spec D9, D11
- **Key points:**
  - Add `Update` variant to `Commands` enum:
    ```rust
    /// Incremental update via delta computation
    Update {
        /// Path to the project root
        path: PathBuf,
        /// Output directory (default: .ariadne/graph/)
        #[arg(long, short)]
        output: Option<PathBuf>,
        /// Enable verbose output
        #[arg(long)]
        verbose: bool,
        /// Warning output format: "human" or "json"
        #[arg(long, default_value = "human", value_parser = ["human", "json"])]
        warnings: String,
        /// Exit with code 1 if any warnings occurred
        #[arg(long)]
        strict: bool,
        /// Include generation timestamp in output
        #[arg(long)]
        timestamp: bool,
        /// Maximum file size in bytes (default: 1048576 = 1MB)
        #[arg(long, default_value_t = 1_048_576)]
        max_file_size: u64,
        /// Maximum number of files to process (default: 50000)
        #[arg(long, default_value_t = 50_000)]
        max_files: usize,
        /// Disable Louvain clustering
        #[arg(long)]
        no_louvain: bool,
    },
    ```
  - Wire `Commands::Update` handler in `main()`:
    1. Build `WalkConfig` from flags (same as `Build`)
    2. Create `JsonSerializer` as `GraphReader`
    3. Call `pipeline.update(path, config, &reader, output, timestamp, verbose, no_louvain)`
    4. Handle result same as `Build` (summary, warnings, exit code)
  - If no previous graph exists → the `update` method falls back to full build internally (transparent to the user)
- **Verify:** `ariadne update <fixture>` works with and without prior build
- **Commit:** `ariadne(cli): add update command`

---

## Chunk 4: Tests + Benchmarks

### Task 4.1: Louvain unit tests

- **Source:** Spec §Testing Requirements (Algorithm Correctness Tests)
- **Key points:**
  - Hand-crafted graphs with known community structure:
    - Two well-separated cliques connected by a single edge → Louvain should detect 2 communities
    - Star graph → single community (all connected to center)
    - Disconnected components → each component = one community
    - Single file → unchanged
    - Empty graph → unchanged
    - Graph with no edges → each file stays in directory-based cluster
  - **Determinism test:** Run Louvain twice → identical ClusterMap
  - **Fallback test:** Graph that causes convergence failure (if possible to construct) → verify W012 emitted and directory-based clusters returned
  - **Cluster naming test:** Verify plurality rule + lexicographic tie-break
- **Commit:** `ariadne(test): add Louvain unit tests`

### Task 4.2: Delta unit tests

- **Source:** Spec §Testing Requirements
- **Key points:**
  - No changes → empty DeltaResult, `requires_full_recompute = false`
  - One file changed → `changed = [file]`, `requires_full_recompute = false` (assuming >20 files total)
  - Files added → `added = [new_files]`
  - Files removed → `removed = [old_files]`
  - >5% threshold → `requires_full_recompute = true`
  - Empty old graph → all added, full recompute
  - Empty current files → all removed, full recompute
- **Commit:** `ariadne(test): add delta computation unit tests`

### Task 4.3: Integration tests

- **Source:** Spec §Success Criteria (Phase 2b)
- **Key points:**
  - `ariadne build` on fixture → verify Louvain runs (clusters may differ from directory-only)
  - `ariadne build --no-louvain` → verify directory-only clusters (same as Phase 2a behavior)
  - `ariadne update` with no prior build → falls back to full build, produces valid output
  - `ariadne update` with prior build + no changes → fast no-op (all output files unchanged)
  - `ariadne update` with prior build + modified file → incremental update, valid output
  - `ariadne update` with corrupted graph.json → W011 warning, falls back to full build
  - `ariadne update` with version-mismatched graph.json → W010 warning, falls back to full build
  - Determinism: `ariadne build` twice → byte-identical output (with Louvain)
- **Commit:** `ariadne(test): add Phase 2b integration tests`

### Task 4.4: Snapshot updates

- **Source:** Spec §Existing Tests
- **Key points:**
  - Louvain changes cluster assignments → existing `clusters.json` snapshots need updating
  - `stats.json` may change slightly (different cluster assignments affect orphan/degree computations)
  - `graph.json` should be unaffected (cluster field on nodes may change)
  - View snapshots may change (cluster views depend on cluster assignments)
  - Run `cargo test` with `INSTA_UPDATE=1` (or `cargo insta review`) to update snapshots
  - Review each snapshot diff to ensure changes are expected and correct
- **Commit:** `ariadne(test): update snapshots for Louvain clustering`

### Task 4.5: Performance benchmarks (`benches/`)

- **Source:** Spec §Performance Benchmarks (Phase 2b)
- **Key points:**
  - `bench_louvain` on 3000-node synthetic graph with ~8000 edges: <200ms
    - Reuse or extend existing `generate_synthetic_project` helper
    - Build `ProjectGraph` + directory-based `ClusterMap`, then run `louvain_clustering`
  - `bench_delta` on 3000-node graph with 10 changed files: <1s
    - Build old graph, modify 10 file hashes, call `compute_delta`
    - Note: this benchmarks only the diff logic, not the full update pipeline
  - Add to existing `benches/` directory alongside Phase 2a benchmarks
- **Commit:** `ariadne(test): add Louvain and delta benchmarks`

---

## Dependency Graph

```
Chunk 1 (Louvain + --no-louvain pipeline wiring + CLI flag)
    │
Chunk 2 (Delta — Task 2.2 calls Louvain in recompute path)
    │
Chunk 3 (ariadne update CLI — needs 2)
    │
Chunk 4 (Tests — needs all)
```

**Critical path:** C1 → C2 → C3 → C4 (4 sequential steps)

**Parallel opportunities:**
- Task 2.1 (delta diff logic) is independent of Chunk 1 and can start early — only Task 2.2 (pipeline orchestration) needs Louvain
- Task 4.1 (Louvain tests) can start as soon as Chunk 1 is done
- Task 4.2 (delta tests) can start as soon as Task 2.1 is done

**Note:** `--no-louvain` parameter threading and CLI flag are handled in Chunk 1 (Tasks 1.2 and 1.3), so `run_with_output` signature is stable before Chunk 2 starts. Task 2.2's fallback to `run_with_output` can pass `no_louvain` without issues.
