# Phase 3a: MCP Server — Specification

## Goal

Ariadne runs as a long-lived MCP server (`ariadne serve`), loads the dependency graph into memory, answers queries instantly via 11 MCP tools, and keeps the graph fresh automatically through fs watching and incremental rebuild.

## Dependencies

**Phase 2a + 2b must be complete.** Phase 3a builds on:

- `ProjectGraph` with `BTreeMap<CanonicalPath, Node>` + `Vec<Edge>`
- `ClusterMap` with directory-based + Louvain community clustering
- Algorithms: Tarjan SCC, Reverse BFS, Brandes centrality, topological sort, subgraph extraction
- `StatsOutput` in `model/stats.rs` with centrality, SCCs, layers, summary
- `stats.json` output (produced by every `ariadne build`)
- `GraphReader` trait + `JsonSerializer` reader implementation (deserialization)
- Markdown views (L0 index, L1 cluster, L2 impact)
- CLI query commands (`ariadne query *`, `ariadne views generate`)
- Delta computation (`ariadne update`) with full rebuild on changes, no-op fast path
- Louvain clustering (on by default, `--no-louvain` to disable)
- `ContentHash` on every node (for freshness checks)
- `SubgraphResult` in `model/query.rs`
- `algo/delta.rs` scaffolding (changed/added/removed sets, 5% threshold)
- Full L1-L4 test suite

## Risk Classification

**Overall: YELLOW**

Phase 3a introduces a new runtime mode (long-running server) with fs watching, background state management, and MCP protocol handling. All subsystems are well-specified but have platform-specific complexity (fs events, signal handling, lock files).

### Per-Deliverable Risk

| # | Deliverable | Risk | Rationale |
|---|------------|------|-----------|
| D1 | MCP Server Core | YELLOW | New runtime mode. `rmcp` + tokio isolated to `serve`. `arc-swap` for lock-free reads. Lock file crash recovery. |
| D2 | MCP Tools | GREEN | 11 tools, each a thin wrapper around existing `algo/` and `views/` functions. Well-defined inputs/outputs. |
| D3 | Freshness Engine | YELLOW | Hash comparison straightforward. Lightweight import re-parse adds complexity but is bounded (~50-100 LOC). `raw_imports.json` persistence is new serialization surface. |
| D4 | Auto-Update | ORANGE | `notify` crate for cross-platform fs watching. Debounce logic. Incremental rebuild on background thread with atomic state swap. Platform-specific edge cases (kqueue vs inotify). Most moving parts in the phase. |

## Deliverables

### D1: MCP Server Core

**New files:** `src/mcp/mod.rs`, `src/mcp/server.rs`, `src/mcp/lock.rs`
**Modified files:** `src/main.rs`, `src/lib.rs`, `Cargo.toml`

Rust MCP server using `rmcp` (official Anthropic SDK) over stdio. Tokio runtime is isolated to the `serve` subcommand — all CLI commands remain fully synchronous (D-051).

**McpServer struct:**

```rust
pub struct McpServer {
    state: Arc<ArcSwap<GraphState>>,  // lock-free read, atomic swap (D-052)
    rebuilding: AtomicBool,           // separate from GraphState — mutable without full swap
    pipeline: BuildPipeline,
    config: ServeConfig,
}

pub struct ServeConfig {
    project_root: PathBuf,
    output_dir: PathBuf,
    debounce_ms: u64,      // default: 2000
    watch_enabled: bool,   // default: true (--no-watch to disable)
}
```

**Startup flow:**
1. Check/create lock file (`.ariadne/graph/.lock`) — refuse if held by live process (E011)
2. Load `graph.json`, `clusters.json`, `stats.json`, `raw_imports.json` into `GraphState`
3. If no graph exists → run full build via `BuildPipeline`, then load
4. Build derived indices (reverse adjacency, layer index)
5. Start fs watcher (unless `--no-watch`)
6. Start MCP JSON-RPC server via `rmcp` on stdin/stdout
7. Register signal handlers (SIGINT/SIGTERM → cleanup lock, exit)

**Binary architecture (D-045, D-051):** Single `ariadne` binary. `main.rs` adds `Serve` variant to clap enum, dispatches to `mcp::server::run()`. `main.rs` remains sole Composition Root (D-020) — constructs `BuildPipeline` and `ParserRegistry`, passes to `McpServer`.

**Threading model (D-051 — supersedes D-047):** Tokio runtime for `ariadne serve` only. `rmcp` handles MCP JSON-RPC on async tasks. Reads from `ArcSwap<GraphState>` are lock-free (D-052). Delta rebuild runs on `tokio::spawn_blocking` (pipeline code is sync). CLI commands (`build`, `query`, `update`) remain fully synchronous — no tokio. Import re-parse for freshness checks is performed through `BuildPipeline` (which already depends on `parser/`), preserving the dependency rule that `mcp/` never depends on `parser/` directly.

**Memory budget:** Graph + indices + raw_imports for 10k-file project: ~60-120MB (increased from ROADMAP's 50-100MB to account for `raw_imports` and derived indices). Acceptable for a dev tool.

**Lock file (`src/mcp/lock.rs`):**

```rust
// .ariadne/graph/.lock contents:
{ "pid": 12345, "started_at": "2026-03-19T14:30:00Z" }
```

- Created in the configured output directory (default: `.ariadne/graph/.lock`)
- Created on server startup, removed on shutdown
- CLI `build`/`update` check for lock; refuse if server is running (E011)
- Stale detection: read PID, check if process alive (`kill(pid, 0)` on Unix); auto-remove if dead (W016)
- Signal handler (SIGINT/SIGTERM): remove lock, flush state, exit
- SIGKILL: lock remains, stale detection handles on next startup

**New dependencies (under `serve` feature flag — D-055):**

```toml
[features]
default = ["serve"]
serve = ["rmcp", "tokio", "arc-swap", "notify", "notify-debouncer-full"]

[dependencies]
rmcp = { version = "0.16", features = ["server", "transport-io"], optional = true }
tokio = { version = "1", features = ["rt-multi-thread", "macros", "signal"], optional = true }
arc-swap = { version = "1", optional = true }
notify = { version = "6", optional = true }
notify-debouncer-full = { version = "0.3", optional = true }
```

`cargo install ariadne-graph --no-default-features` builds pure CLI without tokio.

**Design source:** ROADMAP.md Phase 3a D1, D-020, D-045, D-051, D-052

### D2: MCP Tools

**New file:** `src/mcp/tools.rs`

11 MCP tools registered via `rmcp` `#[tool]` macro. Each is a thin wrapper around existing `algo/`, `views/`, and `GraphState` data. All tools are generic, consumer-agnostic (D-044).

| # | Tool | Input | Calls | Output |
|---|------|-------|-------|--------|
| T1 | `ariadne_overview` | — | `state.graph` + `state.stats` + `state.clusters` aggregation | Project summary: node/edge counts, language breakdown, layer distribution, critical files, cycles count, max depth |
| T2 | `ariadne_file` | `path: string` | `state.graph.nodes[path]` + `state.reverse_index` + `state.stats.centrality` | File detail: type, layer, arch_depth, exports, cluster, centrality, in/out edges |
| T3 | `ariadne_blast_radius` | `path: string, depth?: number` | `algo::blast_radius::blast_radius()` | Reverse BFS: map of affected files with distances |
| T4 | `ariadne_subgraph` | `paths: string[], depth?: number` | `algo::subgraph::extract_subgraph()` | Filtered graph: nodes + edges + clusters in neighborhood |
| T5 | `ariadne_centrality` | `min?: number` | `state.stats.centrality` filter | Bottleneck files sorted by centrality score |
| T6 | `ariadne_cycles` | — | `state.stats.sccs` | All SCCs (circular dependencies) |
| T7 | `ariadne_layers` | `layer?: number` | `state.layer_index` filter | Topological layers: files per arch_depth level |
| T8 | `ariadne_cluster` | `name: string` | `state.clusters[name]` + edge analysis | Cluster detail: files, internal/external deps, cohesion, tests |
| T9 | `ariadne_dependencies` | `path: string, direction: "in"\|"out"\|"both"` | `state.graph.edges` + `state.reverse_index` filter | Direct dependencies of a file (not transitive). MCP-only |
| T10 | `ariadne_freshness` | — | `state.freshness` | Graph freshness: hash/structural confidence, stale files, last update. MCP-only |
| T11 | `ariadne_views_export` | `level: "L0"\|"L1"\|"L2", cluster?: string` | Read `.ariadne/views/` files from disk | Pre-generated markdown views |

**Response format:** All tools return `serde_json::Value`. Structured JSON, no prose. If `structural_confidence < 0.95`, response includes freshness metadata:

```json
{
  "data": { ... },
  "freshness": {
    "hash_confidence": 0.92,
    "structural_confidence": 0.98,
    "stale_files": ["src/foo.ts"]
  }
}
```

**Error semantics:**
- File not in graph → `{ "error": "not_found", "path": "...", "suggestion": "File may be new.", "freshness": {...} }`
- Graph not built → auto-trigger build, return result after build completes
- Build in progress → return stale data + `"rebuilding": true`

**Snapshot consistency:** `arc-swap` `Guard` holds an `Arc<GraphState>` reference for the duration of a tool call. Rebuild thread cannot swap state while a query holds a Guard. This guarantees each query sees a consistent snapshot.

**Design source:** ROADMAP.md Phase 3a D2, D-044

### D3: Freshness Engine

**New file content in:** `src/mcp/state.rs`
**Modified files:** `src/pipeline/mod.rs`, `src/serial/mod.rs`, `src/serial/json.rs`

Two-level confidence scoring with lightweight import re-parse (D-053).

**GraphState (full definition):**

```rust
pub struct GraphState {
    // Core data (loaded from .ariadne/graph/ JSON files)
    pub graph: ProjectGraph,
    pub stats: StatsOutput,
    pub clusters: ClusterMap,

    // Derived indices (built on load)
    pub reverse_index: BTreeMap<CanonicalPath, Vec<Edge>>,
    pub layer_index: BTreeMap<u32, Vec<CanonicalPath>>,
    pub file_hashes: BTreeMap<CanonicalPath, ContentHash>,

    // Freshness — lightweight import re-parse (D-053)
    pub raw_imports: BTreeMap<CanonicalPath, Vec<RawImport>>,
    pub freshness: FreshnessState,

    // Metadata
    pub loaded_at: SystemTime,
}
```

**FreshnessState:**

```rust
pub struct FreshnessState {
    pub stale_files: BTreeSet<CanonicalPath>,          // hash mismatch
    pub structurally_changed: BTreeSet<CanonicalPath>, // imports actually differ
    pub new_files: Vec<PathBuf>,                        // on disk, not in graph
    pub removed_files: Vec<CanonicalPath>,              // in graph, not on disk
    pub hash_confidence: f64,                           // 1 - (stale / total)
    pub structural_confidence: f64,                     // 1 - (struct_changed / total)
    pub last_full_check: SystemTime,
}
```

**Freshness check flow** (triggered lazily on query for queried files, or periodically for all files):

1. Hash current file on disk → compare with `file_hashes[path]`
2. If hash matches → file is fresh, skip
3. If hash mismatch → add to `stale_files`
4. Re-parse imports via `pipeline.reparse_imports(path)` (delegates to `ParserRegistry` internally — `mcp/` never calls `parser/` directly)
5. Compare new `Vec<RawImport>` with `raw_imports[path]` (sorted comparison)
6. If imports differ → add to `structurally_changed`
7. Recompute `hash_confidence` and `structural_confidence`

**Confidence thresholds** (use `structural_confidence` for decisions):
- >= 0.95 → fresh, use as-is
- 0.80-0.95 → minor staleness, results reliable for structural queries
- 0.50-0.80 → noticeable drift, auto-update recommended
- < 0.50 → auto-rebuild triggered

**raw_imports.json persistence (D-054):**

New output file `.ariadne/graph/raw_imports.json`:

```json
{
  "src/auth/login.ts": [
    { "path": "./session", "symbols": ["getSession"], "is_type_only": false },
    { "path": "../utils/crypto", "symbols": ["hash"], "is_type_only": false }
  ]
}
```

Populated during `ariadne build` / `ariadne update` — the pipeline already has `Vec<ParsedFile>` with `Vec<RawImport>`. Serialized alongside `graph.json`.

**Changes to existing pipeline:**
- `pipeline/mod.rs`: After graph assembly, serialize `raw_imports.json` via `GraphSerializer`
- `serial/mod.rs`: Add `write_raw_imports()` / `read_raw_imports()` to `GraphSerializer` / `GraphReader` traits
- `serial/json.rs`: Implement JSON read/write for raw imports

**Design source:** ROADMAP.md Phase 3a D3, D-039, D-053, D-054

### D4: Auto-Update Mechanism

**New file:** `src/mcp/watch.rs`

File system watcher + debounced incremental rebuild (D-038).

**FileWatcher:**

```rust
pub struct FileWatcher {
    debounce_ms: u64,
    state: Arc<ArcSwap<GraphState>>,
    rebuilding: Arc<AtomicBool>,
    pipeline: Arc<BuildPipeline>,
    known_extensions: HashSet<String>,  // snapshot from ParserRegistry at startup
    project_root: PathBuf,
    output_dir: PathBuf,
}
```

**Watch flow:**

1. `notify::RecommendedWatcher` subscribes to project root (recursive)
2. `notify-debouncer-full` collects events for `debounce_ms` (default 2000ms)
3. After debounce → filter: keep only files with extensions from `ParserRegistry` + deleted files
4. Spawn rebuild on `tokio::spawn_blocking` (pipeline code is sync)
5. On completion → `state.store(Arc::new(new_state))` — atomic swap
6. Persist updated graph files to disk

**File pattern filtering:**

```rust
fn should_trigger_rebuild(path: &Path, known_extensions: &HashSet<String>) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|ext| known_extensions.contains(ext))
        .unwrap_or(false)
}
```

Changes to `README.md`, `.gitignore`, images, etc. are ignored.

**Incremental rebuild strategy:**

```
changed = files with hash mismatch
added = files on disk not in graph
removed = files in graph not on disk

if (changed + added + removed) > 5% of total_files:
    full rebuild via pipeline.run()
else:
    1. Remove edges FROM changed/removed files
    2. Remove nodes for removed files
    3. Read + parse changed/added files (tree-sitter via pipeline stages)
    4. Resolve imports for changed/added files
    5. Add new nodes + edges
    6. Recompute algorithms: SCC, centrality, topo_sort, clusters
    7. Update raw_imports for changed/added files
    8. Build new GraphState with fresh derived indices
```

5% threshold from existing `algo/delta.rs` scaffolding (which computes changed/added/removed sets). The incremental path (steps 1-8 above) is **new logic** to be implemented in Phase 3a, building on `algo/delta.rs` detection. Above 5%, incremental algorithm recompute may be inaccurate — full rebuild is safer and still fast (<10s for 3k files). `pipeline/` will expose a new `reparse_imports(&self, path) -> Vec<RawImport>` method for freshness checks and a `rebuild_incremental(&self, changed, added, removed, old_graph) -> ProjectGraph` method for the incremental path.

**Graceful degradation:**
- fs watcher fails → log W014, fall back to poll every 30s
- Incremental rebuild fails → log W015, fall back to full rebuild
- Full rebuild fails → serve stale graph with freshness warning
- Graph files missing → auto-run initial build
- Build in progress → queries return stale data + `"rebuilding": true`

**`.ariadne/` exclusion:** The watcher ignores all events under `.ariadne/` to prevent output writes from triggering recursive rebuilds.

**Design source:** ROADMAP.md Phase 3a D4, D-038, D-046, D-051

## New Error and Warning Codes

### Fatal Errors

| Code | Condition | Message |
|------|-----------|---------|
| E010 | MCP server startup failed | "Failed to start MCP server: {reason}" |
| E011 | Lock file held by live process | "Another ariadne server is running (PID {pid}, started {time}). Stop it first or remove .ariadne/graph/.lock" |
| E012 | MCP protocol error | "MCP protocol error: {reason}" (fallback for errors not handled by rmcp internally) |

### Warnings

| Code | Condition | Message |
|------|-----------|---------|
| W014 | fs watcher failed | "File watcher unavailable: {reason}. Falling back to polling (30s interval)" |
| W015 | Incremental rebuild failed | "Incremental rebuild failed: {reason}. Running full rebuild" |
| W016 | Stale lock file removed | "Removed stale lock file (PID {pid} is not running)" |

## New Decision Log Entries

| # | Decision | Rationale |
|---|----------|-----------|
| D-051 | `rmcp` (official MCP SDK) with tokio isolated to `serve` subcommand | MCP protocol evolves; official SDK tracks changes. tokio confined to `serve` — CLI stays sync. **Supersedes D-047** (original rationale — no tokio — no longer applies because all Rust MCP SDKs require async; tokio isolation to `serve` preserves the spirit of keeping CLI fast and dependency-light). |
| D-052 | `arc-swap` for lock-free GraphState reads | `Arc<RwLock>` blocks readers during write lock acquisition. `ArcSwap` gives zero-contention reads — rebuild thread builds new state, then atomic pointer swap. Queries always fast. |
| D-053 | Lightweight import re-parse for two-level structural confidence | Pessimistic approach (any hash change = structural change) causes false confidence drops. 90%+ of edits are body-only. Tree-sitter re-parse of one file: <1ms. Two confidence levels (hash vs structural) give consumers accurate signal. |
| D-054 | Persist `raw_imports.json` for fast server startup | Alternative (re-parse all files on startup) costs 1-3s. Persisted raw imports load instantly. Build pipeline already has `Vec<ParsedFile>` — serialization is trivial. |
| D-055 | Feature flag `serve` for MCP/tokio dependencies | Users who only need CLI can build without tokio: `--no-default-features`. Keeps binary small for CI/scripting use cases. |

## Module Structure

```
src/mcp/                    # NEW — depends on model/, algo/, serial/, pipeline/
├── mod.rs                  # Re-exports
├── server.rs               # McpServer struct, rmcp setup, startup/shutdown flow
├── tools.rs                # 11 MCP tool handlers (#[tool] macros)
├── state.rs                # GraphState, FreshnessState, freshness check logic
├── watch.rs                # FileWatcher, debounce, incremental rebuild, file filtering
└── lock.rs                 # Lock file create/check/remove/stale detection
```

**Modified existing files:**

| File | Change |
|------|--------|
| `src/main.rs` | Add `Serve` subcommand to clap, dispatch to `mcp::server::run()` |
| `src/lib.rs` | Re-export `mcp` module (behind `serve` feature) |
| `src/pipeline/mod.rs` | Serialize `raw_imports.json` during build; add `reparse_imports()` and `rebuild_incremental()` methods |
| `src/serial/mod.rs` | Add `write_raw_imports()` / `read_raw_imports()` to traits |
| `src/serial/json.rs` | Implement raw imports JSON read/write |
| `Cargo.toml` | New dependencies under `serve` feature flag |

**New output file:** `.ariadne/graph/raw_imports.json`

**Dependency rules:**

| Module | Depends on | Never depends on |
|--------|-----------|-----------------|
| `mcp/` | `model/`, `algo/`, `serial/`, `pipeline/` | `parser/` (freshness re-parse goes through `pipeline/` which already depends on `parser/`) |

## CLI Extension

```
ariadne serve [--project <path>] [--debounce <ms>] [--no-watch]
```

- `--project`: Project root to serve (default: current directory)
- `--debounce`: Milliseconds to wait after last file change before rebuild (default: 2000)
- `--no-watch`: Disable fs watcher (no auto-update, freshness checks only)

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

## Performance Targets

| Metric | Target |
|--------|--------|
| MCP tool response (in-memory query) | <10ms |
| Freshness check (single file hash + import re-parse) | <2ms |
| Delta rebuild (10 changed files / 3k project) | <2s |
| Full rebuild (3k files) | <10s |
| Server startup (load graph + build indices) | <1s |
| `bench_mcp_overview` on 3k-node graph | <5ms |
| `bench_mcp_blast_radius` on 3k-node graph | <10ms |

## Success Criteria

1. `ariadne serve` starts MCP server, loads graph, answers queries via stdio JSON-RPC
2. All 11 MCP tools return correct results matching CLI `ariadne query` equivalents
3. fs watcher triggers delta rebuild within debounce window + rebuild time of file change
4. Two-level freshness confidence accurately reflects graph staleness (hash vs structural)
5. Lightweight import re-parse correctly distinguishes body-only changes from structural changes
6. Server handles missing/corrupted graph gracefully (auto-rebuild)
7. MCP tool response latency <10ms for in-memory queries
8. Server operates correctly as Claude Code MCP server (settings.json registration)
9. Lock file prevents concurrent CLI writes while server is running
10. Lock file stale detection works (dead PID → auto-remove)
11. `ariadne_views_export` returns generic markdown views (no consumer-specific formatting)
12. `ariadne build` now also produces `raw_imports.json`
13. `--no-default-features` builds without tokio/rmcp dependencies

## Testing Requirements

### MCP Integration Tests
- Start server → send tool request → verify response matches CLI output
- All 11 tools: verify correct JSON response structure
- File change → verify auto-rebuild → verify tool returns updated data
- Missing graph → verify auto-build → verify tools work after build
- Corrupted graph → verify graceful fallback

### Freshness Tests
- Modify file body (no import changes) → `hash_confidence` drops, `structural_confidence` stays high
- Modify file imports → both confidences drop
- Add new file → `new_files` populated, confidences reflect
- Delete file → `removed_files` populated, confidences reflect
- Multiple stale files → confidence scores computed correctly

### Lock File Tests
- Start server → CLI `build` → verify refusal with E011 message
- Server exits normally → verify lock released → CLI works
- Kill server (SIGKILL) → verify stale detection → next startup removes lock (W016)

### Auto-Update Tests
- Change parseable file → rebuild triggered within debounce window
- Change non-parseable file (README.md) → no rebuild
- Change file under `.ariadne/` → no rebuild
- >5% files changed → full rebuild triggered (not incremental)
- fs watcher unavailable → poll fallback (W014)
- Incremental rebuild failure → full rebuild fallback (W015)

### raw_imports.json Tests
- `ariadne build` produces `raw_imports.json`
- Round-trip: build → load raw_imports → compare with parsed imports
- Missing `raw_imports.json` on server startup → re-parse all files (graceful fallback)

### Performance Benchmarks
- `bench_mcp_overview` on 3k-node graph: <5ms
- `bench_mcp_blast_radius` on 3k-node graph: <10ms
- `bench_freshness_check` (10 files): <20ms
- `bench_incremental_rebuild` (10 files changed, 3k project): <2s
- `bench_server_startup` (load 3k-node graph): <1s

### Invariant Extensions
- All tool responses are valid JSON
- Freshness confidences in [0.0, 1.0]
- `structural_confidence >= hash_confidence` always — a file can have hash mismatch but unchanged imports, so structural confidence is never worse than hash confidence
- Lock file PID matches current process
- `raw_imports.json` keys are subset of `graph.json` node keys

## Relationship to Parent Phase 3 Spec

This spec supersedes the D1-D4 sections of `2026-03-19-phase3-mcp-server-architectural-intelligence.md` for Phase 3a. Key differences from parent spec:

- **Concurrency model:** Parent spec uses `Arc<RwLock<GraphState>>` (D-047). This spec uses `ArcSwap` (D-052) with tokio (D-051, superseding D-047). Lock-free reads instead of RwLock.
- **`rebuilding` flag:** Parent spec implies it's part of GraphState. This spec uses a separate `AtomicBool` on McpServer (cannot mutate fields inside ArcSwap without full swap).
- **FreshnessState:** Extended with `structurally_changed`, `hash_confidence` fields (D-053 resolution of DP-3).
- **Parser dependency:** Parent spec says `mcp/` never depends on `parser/`. This spec routes freshness re-parse through `pipeline/` (which already depends on `parser/`), preserving the dependency rule.
- **New output:** `raw_imports.json` (D-054) not in parent spec.

Parent spec's D5-D10 (Phase 3b, 3c) remain unchanged and authoritative.

## Design Sources

| Deliverable | Authoritative Sources |
|-------------|----------------------|
| D1: MCP Server Core | ROADMAP.md Phase 3a D1, D-020, D-045, D-051, D-052 |
| D2: MCP Tools | ROADMAP.md Phase 3a D2, D-044 |
| D3: Freshness Engine | ROADMAP.md Phase 3a D3, D-039, D-053, D-054 |
| D4: Auto-Update | ROADMAP.md Phase 3a D4, D-038, D-046, D-051, D-055 |

## Discussion Points Resolved

| DP | Resolution |
|----|-----------|
| DP-1 | `rmcp` (official SDK). D-047 updated to D-051: tokio isolated to `serve` |
| DP-2 | Double-buffer via `arc-swap`. D-052. |
| DP-3 | Lightweight import re-parse with two-level confidence. D-053. |
| DP-4 | PageRank on reversed graph (Phase 3c, noted for future) |
| DP-5 | True incremental re-parsing for <5% changes, full rebuild above 5% |
| DP-6 | Lock file: JSON with PID + timestamp. Stale detection via `kill(pid, 0)`. |
| DP-7 | E010, E011, E012, W014, W015, W016 defined |
| DP-8 | Deferred to Phase 3b (Martin metrics) |
| DP-9 | Resolved: `StatsOutput` lives in `model/stats.rs` |
| DP-11 | File pattern filtering via `ParserRegistry.lookup(ext)` |
| DP-12 | Signal handling: SIGINT/SIGTERM → remove lock, flush, exit. SIGKILL → stale detection. |
| DP-13 | Deferred to Phase 3b (smell thresholds) |
| DP-14 | Deferred to Phase 3b (Louvain noise) |
| DP-15 | Deferred to Phase 3b (`ariadne_diff` is MCP-only) |
| DP-16 | Deferred to Phase 3c (token estimation) |
| DP-17 | Deferred to Phase 3b (ChangeClassification heuristic) |
| DP-18 | MCP integration tests: spawn subprocess, send JSON-RPC via stdin, read stdout |
| DP-19 | Deferred to Phase 3c (eigenvector sign) |
