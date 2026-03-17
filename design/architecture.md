# Ariadne — Architecture

## Purpose

Ariadne is a structural topology map generator for software projects. It parses source code via tree-sitter and produces a dependency graph capturing file dependencies, architectural layers, and module clusters — enabling precise navigation and impact assessment.

**Ariadne produces deterministic structural data derived from code.** It does not learn, decay, or evolve through experience. It updates when code changes.

|              | Ariadne (Structural)                 | Knowledge (Semantic)                       |
| ------------ | ------------------------------------ | ------------------------------------------ |
| **Contains** | Files, imports, dependencies, layers | Patterns, decisions, conventions, failures |
| **Answers**  | "How is the code structured?"        | "What do we know about the project?"       |
| **Updates**  | When code changes (deterministic)    | When tasks complete (evidence-based)       |
| **Source**   | Static analysis (tree-sitter)        | External observations                      |

## Engine Architecture

Ariadne is a standalone Rust binary that parses source code via tree-sitter and produces a structural dependency graph.

**Why Rust:**

- Tree-sitter is written in Rust/C — native, first-class bindings
- Single binary, zero runtime dependencies
- Fastest option: 3000 files in under 10 seconds (see `design/performance.md`)
- No dependency on Node.js, Python, or any runtime

**Why tree-sitter:**

- Language-agnostic: 100+ grammar support
- Deterministic AST parsing — no LLM involvement, no token cost
- Incremental parsing support
- Battle-tested in editors (Neovim, Helix, Zed)

## Language Support

Each language implements a `LanguageParser` trait:

```rust
trait LanguageParser {
    fn language(&self) -> &str;
    fn extensions(&self) -> &[&str];
    fn tree_sitter_language(&self) -> Language;
    fn extract_imports(&self, tree: &Tree, source: &[u8]) -> Vec<Import>;
    fn extract_exports(&self, tree: &Tree, source: &[u8]) -> Vec<Export>;
    fn resolve_import_path(&self, import: &Import, file: &Path, root: &Path) -> Option<PathBuf>;
}
```

Adding a new language = implementing one trait. Grammars are crate dependencies.

**Tier 1 (initial release):**

| Language                | Import forms                                                         | Complexity |
| ----------------------- | -------------------------------------------------------------------- | ---------- |
| TypeScript / JavaScript | `import`, `require`, `export`, dynamic `import()`, barrel re-exports | High       |
| Go                      | `import "path"`, `import (...)`                                      | Low        |
| Python                  | `import`, `from...import`, relative imports                          | Medium     |
| Rust                    | `use`, `mod`, `extern crate`                                         | Medium     |
| C#                      | `using`, `using static`                                              | Low        |
| Java                    | `import`, `import static`                                            | Low        |

**Tier 2 (future):** Kotlin, Swift, C/C++, PHP, Ruby, Dart.

**Tier 3 (on demand):** Elixir, Scala, Haskell, Lua, Zig, etc.

## Graph Data Model

**Nodes** — files with metadata:

``` rust
Node {
    path: String,          // relative to project root (unique ID)
    type: FileType,        // source | test | config | style | asset | type_def
    layer: ArchLayer,      // api | service | data | util | component | hook | config | unknown
    arch_depth: u32,       // topological depth (0 = no dependencies)
    lines: u32,            // line count
    hash: String,          // content hash for delta detection (xxHash)
    exports: Vec<String>,  // exported symbol names
    cluster: String,       // assigned cluster ID
}
```

**Edges** — directed, typed connections:

``` rust
Edge {
    from: String,          // source file path
    to: String,            // target file path
    edge_type: EdgeType,   // imports | tests | re_exports | type_imports
    symbols: Vec<String>,  // which symbols are used (optional)
}
```

`nodes` uses `BTreeMap<String, Node>` for deterministic output ordering (D-006).

This is a **directed multigraph** — multiple edges of different types can exist between two nodes.

**File types:**

- `source` — application code
- `test` — test files (detected by path pattern or naming convention)
- `config` — configuration files (tsconfig, webpack, etc.)
- `style` — CSS/SCSS/styled-components
- `asset` — static assets (images, fonts, JSON data)
- `type_def` — type definition files (.d.ts, .pyi)

**Edge types:**

- `imports` — runtime dependency (import/require/use)
- `tests` — test file covers source file (inferred from naming + imports)
- `re_exports` — barrel file re-exports (index.ts pattern)
- `type_imports` — compile-time only dependency (TypeScript `import type`, Python `TYPE_CHECKING`)

## Storage Format

Output goes under `.ariadne/` in the project root. The graph output directory is `.ariadne/graph/` (configurable via `--output`). The `.ariadne/` parent directory may contain other subdirectories in future phases.

``` rust
.ariadne/
├── graph/
│   ├── graph.json      # full graph — source of truth
│   ├── clusters.json   # cluster definitions with metadata
│   └── stats.json      # precomputed metrics (centrality, layers, SCCs) (Phase 2)
└── views/              # generated markdown views (Phase 2)
    ├── index.md        # L0: cluster list, critical files, cycles
    ├── clusters/       # L1: per-cluster detail
    │   ├── auth.md
    │   ├── api.md
    │   └── ...
    └── impact/         # L2: on-demand blast radius reports
        └── (generated per query)
```

### graph.json

Compact adjacency list format:

```json
{
  "version": 1,
  "project_root": "/path/to/project",
  "node_count": 847,
  "edge_count": 2341,
  "nodes": {
    "src/auth/login.ts": {
      "type": "source",
      "layer": "service",
      "arch_depth": 2,
      "lines": 142,
      "hash": "a1b2c3d4",
      "exports": ["login", "LoginParams"],
      "cluster": "auth"
    }
  },
  "edges": [
    ["src/api/auth.ts", "src/auth/login.ts", "imports", ["login"]],
    ["src/auth/__tests__/login.test.ts", "src/auth/login.ts", "tests", []]
  ]
}
```

Edges use compact tuple format — 60%+ space savings vs objects. `--timestamp` optionally includes a `"generated"` timestamp in the output.

### clusters.json

```json
{
  "clusters": {
    "auth": {
      "files": ["src/auth/login.ts", "src/auth/logout.ts", "..."],
      "file_count": 12,
      "internal_edges": 28,
      "external_edges": 15,
      "cohesion": 0.65
    }
  }
}
```

### stats.json

```json
{
  "centrality": {
    "src/utils/format.ts": 0.89,
    "src/auth/middleware.ts": 0.72
  },
  "sccs": [["src/billing/invoice.ts", "src/auth/permissions.ts"]],
  "layers": {
    "0": ["src/utils/constants.ts", "src/types/index.ts"],
    "1": ["src/services/auth.ts"],
    "2": ["src/api/routes.ts"]
  },
  "summary": {
    "max_depth": 7,
    "avg_in_degree": 2.8,
    "avg_out_degree": 2.8,
    "bottleneck_files": ["src/utils/format.ts"],
    "orphan_files": ["src/legacy/old-helper.ts"]
  }
}
```

## Algorithms

### 1. Blast Radius — Reverse BFS

Answers: "If I change file X, what else might break?"

``` rust
blast_radius(X, max_depth=inf) -> {file: distance}:
    visited = {}
    queue = [(X, 0)]
    while queue not empty:
        node, depth = dequeue
        if node in visited: skip
        visited[node] = depth
        for each dependent in reverse_edges[node]:
            if depth + 1 <= max_depth:
                enqueue (dependent, depth + 1)
    return visited
```

**Depth semantics:**

- depth=1: direct dependents — almost certainly affected
- depth=2: transitive dependents — probably affected
- depth=3+: distant dependents — possibly affected

**Complexity:** O(V + E), linear in graph size.

### 2. Betweenness Centrality — Brandes Algorithm

Identifies bottleneck files (files that many dependency paths pass through).

``` rust
BC(v) = sum_{s!=v!=t} (sigma_st(v) / sigma_st)
```

Where sigma_st = number of shortest paths from s to t, sigma_st(v) = those passing through v.

**Brandes algorithm:** O(VE), computes all centralities in one pass. For V=3000, E=~8000: milliseconds.

Files with BC > 0.7 are marked as bottlenecks in stats.json.

### 3. Cycle Detection — Tarjan's SCC

Finds circular dependencies (strongly connected components of size > 1).

```rust
Tarjan's algorithm: O(V + E)
    - DFS with lowlink tracking
    - Nodes on stack form SCCs
    - SCC size > 1 = circular dependency
```

SCCs are reported in stats.json and surfaced in views.

### 4. Clustering — Two-Level

**Level 1: Directory-based (free).** Files in `src/auth/` -> cluster "auth". Natural, intuitive, zero computation.

**Level 2: Louvain community detection (refinement).**

``` rust
Modularity Q = (1/2m) sum_{ij} [A_ij - k_i*k_j / 2m] * delta(c_i, c_j)
```

Louvain maximizes Q greedily in O(n\*log n). Detects real module boundaries that may not align with directories (e.g., a util file that belongs semantically to a specific domain).

**Cluster assignment:** Start with directory clusters, then run Louvain. If Louvain reassigns a file, it overrides the directory-based cluster. Cluster IDs use directory names where possible for readability.

### 5. Architectural Layers — Topological Sort

On DAG (after contracting SCCs into supernodes), topological sort produces dependency layers:

``` rust
Layer 0: files with no outgoing dependencies (utils, constants, types)
Layer 1: files depending only on Layer 0
Layer 2: files depending on Layer 0-1
...
```

Automatic architecture discovery. Layer information can be used to order implementation steps (bottom-up) and detect layer violations (e.g., service importing from API layer).

### 6. Incremental Updates — Delta Computation

Full rebuild on every refresh is wasteful at scale. Delta approach:

``` rust
update(old_graph, current_fs):
    // Phase 1: detect changes via content hash
    changed = {f : hash(f) != old_graph.nodes[f].hash}
    added = current_fs - old_graph.nodes
    removed = old_graph.nodes - current_fs

    // Phase 2: re-parse only affected files
    for f in (changed | added):
        parse imports/exports
        update edges from f

    // Phase 3: remove stale data
    remove all edges from/to removed files
    remove nodes for removed files

    // Phase 4: recompute derived data
    if |changed | added | removed| > 0.05 * |nodes|:
        full recompute (clusters, centrality, layers)
    else:
        incremental cluster update
        skip centrality recompute (use previous)
```

**Content hash:** xxHash64 — fast, collision-resistant, deterministic. O(1) per file check.

**Threshold:** If >5% of files changed, full recompute (derived data may have shifted significantly). Otherwise, incremental.

### 7. Subgraph Extraction

Extract relevant neighborhood around specified files:

``` rust
extract_subgraph(files, depth=2):
    result_nodes = {}
    for f in files:
        // BFS outward (dependencies)
        bfs(f, forward_edges, depth) -> add to result_nodes
        // BFS inward (dependents)
        bfs(f, reverse_edges, depth) -> add to result_nodes
    // Include full cluster for each touched file
    for f in files:
        add all files in f.cluster to result_nodes
    return subgraph(result_nodes) with metrics
```

This is what gets rendered into L2 markdown views.

## Views: Markdown Output

### L0: Index (`views/index.md`)

~200-500 tokens. Overview for quick orientation.

```markdown
# Project Graph — Index

## Clusters (12)

| Cluster | Files | Key file (highest centrality) |
| ------- | ----- | ----------------------------- |
| auth    | 12    | src/auth/middleware.ts (0.72) |
| api     | 23    | src/api/router.ts (0.65)      |

| ...

## Critical Files (centrality > 0.7)

- src/utils/format.ts (0.89) — 47 dependents
- src/auth/middleware.ts (0.72) — 28 dependents

## Circular Dependencies (2)

- auth <-> billing (via permissions.ts <-> invoice.ts)
- ...

## Architecture (7 layers, max depth: 7)

Layer 0 (foundations): 34 files
Layer 1-2 (services): 89 files
Layer 3+ (api/ui): 45 files
```

### L1: Cluster Detail (`views/clusters/<name>.md`)

~500-2000 tokens per cluster. Internal structure and external connections.

```markdown
# Cluster: auth (12 files)

## Files

| File          | Type   | Layer | In  | Out | Centrality |
| ------------- | ------ | ----- | --- | --- | ---------- |
| middleware.ts | source | 2     | 28  | 3   | 0.72       |
| login.ts      | source | 3     | 5   | 4   | 0.31       |

| ...

## Internal Dependencies

middleware.ts -> session.ts -> token.ts

## External Dependencies (outgoing)

auth/middleware.ts -> utils/crypto.ts
auth/session.ts -> database/redis.ts

## External Dependents (incoming)

api/routes.ts -> auth/middleware.ts
api/admin.ts -> auth/permissions.ts

## Tests

auth/**tests**/login.test.ts -> login.ts
auth/**tests**/middleware.test.ts -> middleware.ts
```

### L2: Subgraph / Impact Report (`views/impact/`)

Generated on-demand via `ariadne query subgraph` or `ariadne query blast-radius`. Contains full dependency tree for specific files with all metrics.

## CLI Interface

``` rust
ariadne build <project-root> [--output <dir>]              (Phase 1a)
    Parse project, build full graph -> graph.json, clusters.json

ariadne info                                               (Phase 1a)
    Version, supported languages, build info

ariadne update <project-root> [--output <dir>]             (Phase 2)
    Incremental update via delta computation

ariadne query blast-radius <file> [--depth N] [--format json|md]  (Phase 2)
    Reverse BFS from file, output dependents with distance

ariadne query subgraph <file...> [--depth N] [--format json|md]   (Phase 2)
    Extract neighborhood around specified files

ariadne query stats [--format json|md]                     (Phase 2)
    Output precomputed metrics (centrality, SCCs, layers)

ariadne query cluster <name> [--format json|md]            (Phase 2)
    Output cluster detail

ariadne query file <path> [--format json|md]               (Phase 2)
    All info about a specific file: deps, dependents, metrics

ariadne query cycles [--format json|md]                    (Phase 2)
    List all circular dependencies

ariadne query layers [--format json|md]                    (Phase 2)
    Show architectural layers

ariadne views generate [--output <dir>]                    (Phase 2)
    Generate/regenerate all markdown views
```

Default `--format` is `md` (human-readable). `json` for programmatic use. `--timestamp` includes a generation timestamp in output.

See `design/error-handling.md` for additional diagnostic flags (`--verbose`, `--warnings`, `--strict`) and resource-limit flags (`--max-file-size`, `--max-files`).

## Installation

### Requirements

- Rust toolchain (for building from source)
- Or: prebuilt binary from GitHub Releases

### Installation Methods

**Via cargo (recommended for developers):**

```bash
cargo install ariadne-graph
```

**Via GitHub Releases (no Rust needed):**

```bash
# macOS
curl -L https://github.com/<org>/ariadne/releases/latest/download/ariadne-darwin-arm64 -o /usr/local/bin/ariadne
chmod +x /usr/local/bin/ariadne

# Linux
curl -L https://github.com/<org>/ariadne/releases/latest/download/ariadne-linux-x64 -o /usr/local/bin/ariadne
chmod +x /usr/local/bin/ariadne
```

### Git Tracking Policy

- `.ariadne/graph/graph.json`, `clusters.json`, `stats.json`: **committed** (canonical, deterministic)
- `.ariadne/views/`: **committed** (generated but stable, useful for review)
- No gitignored state — everything is reproducible

## Integration

Ariadne is designed as a standalone tool. Integration points for external systems:

- **Build/update invocation:** `ariadne build` / `ariadne update` as CLI commands
- **Data consumption:** `graph.json`, `clusters.json`, `stats.json` as stable JSON formats
- **Query API:** `ariadne query *` commands with `--format json` for programmatic use
- **View generation:** Markdown views can be loaded into any LLM agent context

Ariadne has no dependency on any specific orchestration framework.
