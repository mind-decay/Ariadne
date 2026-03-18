# Ariadne — Implementation Roadmap

## Overview

Ariadne is a standalone Rust CLI that builds structural dependency graphs from source code via tree-sitter.

**Crate name:** `ariadne-graph` (binary: `ariadne`) — D-010.

---

## Phase 1a: MVP — Parse and Output

**Goal:** `ariadne build <path>` works. Parses a multi-language project, outputs `graph.json` + `clusters.json`. Basic error handling (skip broken files, log to stderr). No frills.

**Deliverables:**

- Cargo project (`ariadne-graph` crate, `ariadne` binary)
- Core data model (BTreeMap for determinism — D-006)
- Tree-sitter integration with partial parse handling
- 6 Tier 1 language parsers (TS/JS, Go, Python, Rust, C#, Java)
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

## Phase 1b: Hardening

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

## Phase 2a: Algorithms, Queries & Views

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

## Phase 2b: Louvain Clustering & Delta Computation

**Goal:** Community-based clustering refinement and incremental graph updates. (D-036)

**Depends on:** Phase 2a.

**Deliverables:**

- Louvain community detection (overrides directory-based clusters, on by default, `--no-louvain` to disable)
- Delta computation (`ariadne update` — incremental via content hash, 5% threshold for full recompute)

**Testing:** Louvain correctness tests, delta round-trip tests, performance benchmarks (Louvain <200ms, delta <1s).

---

## Phase 3: MCP Server & Architectural Intelligence

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

### Phase 3a: MCP Server

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

**Threading model:** `notify` crate uses OS-native file watching (kqueue on macOS, inotify on Linux) with a dedicated watcher thread. No async runtime (tokio) required. MCP JSON-RPC runs on the main thread (stdio is inherently sequential). Delta rebuild runs on a background thread, communicates via `Arc<RwLock<GraphState>>` swap. See D-047.

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

### Phase 3b: Architectural Intelligence

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

### Phase 3c: Advanced Graph Analytics

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

## Future

- Tier 2/3 language parsers
- Config file (.ariadne.toml)
- Plugin system for external parsers
- Feature-Sliced Design (FSD) architecture support (D-031)
- `ariadne self-update`
- Package manager distribution (brew, nix, AUR)
- Git history integration (co-change analysis, temporal coupling)
- Multi-project graph federation (monorepo cross-project dependencies)
- IDE integration (LSP-based real-time graph updates)
- Web dashboard for graph visualization
