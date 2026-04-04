# Dependency Analysis — 2026-04-04

## 1. Package Versions (Cargo.toml)

### Runtime Dependencies

| Crate | Version | Purpose |
|-------|---------|---------|
| clap | 4 (features: derive) | CLI argument parsing |
| tree-sitter | 0.24 | Parser infrastructure |
| tree-sitter-typescript | 0.23 | TS/TSX grammar |
| tree-sitter-javascript | 0.23 | JS grammar |
| tree-sitter-go | 0.23 | Go grammar |
| tree-sitter-python | 0.23 | Python grammar |
| tree-sitter-rust | 0.23 | Rust grammar |
| tree-sitter-c-sharp | 0.23 | C# grammar |
| tree-sitter-java | 0.23 | Java grammar |
| tree-sitter-md | 0.3 | Markdown grammar |
| tree-sitter-json | 0.24 | JSON grammar |
| tree-sitter-yaml | 0.7 | YAML grammar |
| serde | 1 (features: derive) | Serialization framework |
| serde_json | 1 | JSON serialization |
| xxhash-rust | 0.8 (features: xxh64) | Content hashing |
| ignore | 0.4 | .gitignore-aware file walking |
| rayon | 1 | Data parallelism |
| thiserror | 2 | Error derive macros |
| dunce | 1 | Path canonicalization (Windows UNC) |
| time | 0.3 (features: formatting) | Timestamp generation |
| glob | 0.3 | Glob pattern matching |

### Optional Dependencies (behind `serve` feature, default ON)

| Crate | Version | Purpose |
|-------|---------|---------|
| rmcp | 1.2 (features: server, transport-io) | MCP protocol server |
| schemars | 1 | JSON Schema generation for MCP tools |
| tokio | 1 (features: rt-multi-thread, macros, signal, sync, time) | Async runtime |
| tokio-util | 0.7 | Async utilities |
| arc-swap | 1 | Lock-free atomic pointer swaps |
| notify | 8 | Filesystem watcher |
| notify-debouncer-full | 0.7 | Debounced filesystem events |

### Dev Dependencies

| Crate | Version | Purpose |
|-------|---------|---------|
| insta | 1 (features: yaml) | Snapshot testing |
| tempfile | 3 | Temporary directories for tests |
| proptest | 1 | Property-based testing |
| criterion | 0.5 (features: html_reports) | Benchmarking |
| serde_json | 1 | JSON in tests |

### Cargo.lock

Present at `/Users/minddecay/Documents/Projects/Ariadne/Cargo.lock`.

---

## 2. Internal Module Dependency Map

Source: `use crate::` statements across all files in `src/`.

### Leaf Modules (no internal imports outside self)

- **model** — Only self-references (`crate::model::*` within submodules). Zero external crate-level imports. This is the leaf data layer as designed (D-017).
  - Evidence: `src/model/symbol_index.rs` imports `crate::model::edge`, `crate::model::node`, `crate::model::symbol`, `crate::model::types` — all within model itself.

### Near-Leaf Modules

- **hash** — Imports only `model::ContentHash`. (`src/hash.rs:1`)
- **diagnostic** — Imports only `model::CanonicalPath`. (`src/diagnostic.rs:5`)

### Core Infrastructure Consumers

- **detect** — Imports from: `model`, `diagnostic`.
  - `detect/filetype.rs` → `model::{CanonicalPath, FileType}`
  - `detect/layer.rs` → `model::{ArchLayer, CanonicalPath, FsdLayer}`
  - `detect/case_sensitivity.rs` → `model::types::{CanonicalPath, FileSet}`
  - `detect/workspace.rs` → `diagnostic`, `model::workspace`, `model::CanonicalPath`

- **cluster** — Imports from: `model` only.
  - `cluster/mod.rs` → `model::{CanonicalPath, Cluster, ClusterId, ClusterMap, ProjectGraph}`

- **semantic** — Imports from: `model` only.
  - `semantic/mod.rs` → `model::semantic`, `model::types`
  - `semantic/http.rs` → `model::semantic`, `model::types`, `semantic::BoundaryExtractor`
  - `semantic/events.rs` → `model::semantic`, `model::types`, `semantic::BoundaryExtractor`
  - `semantic/edges.rs` → `model::semantic`, `model::types`

### Mid-Layer Modules

- **parser** — Imports from: `model`, `diagnostic`, `semantic`.
  - All language parsers (typescript, rust_lang, go, python, java, csharp, markdown, json_lang, yaml) → `model`, `parser::traits`, `parser::helpers`, `parser::symbols`
  - `parser/registry.rs` → `model::semantic`, `model::symbol`, `model::types`, **`semantic::BoundaryExtractor`**, `parser::config`
  - `parser/config/mod.rs` → `diagnostic`, `model::FileSet`
  - `parser/config/tsconfig.rs` → `diagnostic`, `model::CanonicalPath`
  - Notable cross-module: **parser depends on semantic** (for `BoundaryExtractor` trait in registry).

- **serial** — Imports from: `model`, `diagnostic`.
  - `serial/mod.rs` → `diagnostic::FatalError`, `model::semantic`, `model::symbol`, `model::types`, `model::StatsOutput`
  - `serial/json.rs` → `diagnostic::FatalError`, `model::StatsOutput`
  - `serial/convert.rs` → `model::*`

- **algo** — Imports from: `model` only.
  - `algo/mod.rs` → `model::{CanonicalPath, Edge}`
  - All sub-algorithms → `model::*`, `algo::*` (internal)
  - `algo/reading_order.rs` → `algo::scc`, `algo::topo_sort`, `algo::{is_architectural, AdjacencyIndex}`

- **views** — Imports from: `model`, `diagnostic`.
  - `views/mod.rs` → `diagnostic::FatalError`, `model::{ClusterMap, ProjectGraph, StatsOutput}`
  - `views/cluster.rs` → `model::{EdgeType, ProjectGraph, StatsOutput}`
  - `views/impact.rs` → `model::{CanonicalPath, ProjectGraph, SubgraphResult}`

- **analysis** — Imports from: `model`, `algo`.
  - `analysis/metrics.rs` → `algo::round4`, `model::*`
  - `analysis/smells.rs` → `algo`, `analysis::metrics`, `model::*`
  - `analysis/diff.rs` → `algo::round4`, `analysis::metrics`, `analysis::smells`, `model::*`

- **temporal** — Imports from: `model`, `algo`, `diagnostic`.
  - `temporal/mod.rs` → `algo`, `diagnostic`, `model::{CanonicalPath, ProjectGraph, TemporalState}`
  - `temporal/git.rs` → `diagnostic`, `model::CanonicalPath`
  - Submodules (churn, coupling, hotspot, ownership) → `model::temporal`, `model::CanonicalPath`

- **recommend** — Imports from: `model`, `algo`.
  - `recommend/split.rs` → `algo::{blast_radius, callgraph, round4, AdjacencyIndex}`, `model::*`, `recommend::min_cut`, `recommend::types`
  - `recommend/refactor.rs` → `algo::{blast_radius, callgraph, scc, round4, AdjacencyIndex}`, `model::*`, `recommend::pareto`, `recommend::split`, `recommend::types`
  - `recommend/placement.rs` → `model::{ArchLayer, CanonicalPath, Cluster, ClusterId, Node}`

### High-Layer Modules

- **pipeline** — Imports from: `model`, `diagnostic`, `parser`, `detect`, `cluster`, `algo`, `serial`, `semantic`, `parser::config`.
  - `pipeline/mod.rs` → `algo`, `cluster::assign_clusters`, `detect::*`, `diagnostic::*`, `model::*`, `parser::*`, `parser::config`, `semantic`, `serial::*`
  - `pipeline/build.rs` → `detect`, `diagnostic`, `model`, `parser`
  - `pipeline/resolve.rs` → `detect`, `diagnostic`, `model`, `parser`
  - `pipeline/read.rs` → `hash`, `model`
  - `pipeline/walk.rs` → `diagnostic`, `model`

- **mcp** (feature-gated behind `serve`) — Imports from: nearly every module.
  - `mcp/state.rs` → `algo::{callgraph, compress, pagerank, spectral}`, `analysis::metrics`, `diagnostic`, `model::*`, `serial`
  - `mcp/tools.rs` → `algo`, `analysis::smells`, `recommend`, `mcp::state`, `mcp::tools_*`, `model`
  - `mcp/server.rs` → `diagnostic`, `mcp::{lock, state, tools, user_state, watch}`, `pipeline`, `serial::json`
  - `mcp/watch.rs` → `analysis::diff`, `diagnostic`, `mcp::state`, `pipeline`, `serial::json`
  - `mcp/resources.rs` → `analysis::smells`, `mcp::state`
  - `mcp/prompts.rs` → `algo::reading_order`, `analysis::smells`, `mcp::state`, `model`
  - `mcp/bookmarks.rs` → `mcp::user_state`, `model`
  - `mcp/annotations.rs` → `mcp::user_state`, `model`
  - `mcp/user_state.rs` → `mcp::persist`, `model`
  - `mcp/lock.rs` → `diagnostic`

- **main.rs** — Composition root. Imports from: `algo`, `diagnostic`, `model`, `parser`, `pipeline`, `semantic::{events, http}`, `serial::{json, BoundaryEntry, BoundaryOutput, GraphReader}`.

### Dependency Summary (module -> imports from)

```
model         → (none — leaf)
hash          → model
diagnostic    → model
detect        → model, diagnostic
cluster       → model
semantic      → model
parser        → model, diagnostic, semantic
serial        → model, diagnostic
algo          → model
views         → model, diagnostic
analysis      → model, algo
temporal      → model, algo, diagnostic
recommend     → model, algo
pipeline      → model, diagnostic, parser, detect, cluster, algo, serial, semantic, hash (via read), parser::config
mcp           → model, diagnostic, algo, analysis, recommend, pipeline, serial, mcp (internal)
main.rs       → algo, diagnostic, model, parser, pipeline, semantic, serial
```

---

## 3. Circular Dependencies

### Module-Level Cycles: **None detected.**

The dependency flow is strictly layered:
1. `model` (leaf, no outward deps)
2. `hash`, `diagnostic` (depend only on model)
3. `detect`, `cluster`, `semantic`, `algo`, `serial`, `views` (depend on model + diagnostic)
4. `parser` (depends on model, diagnostic, semantic)
5. `analysis`, `temporal`, `recommend` (depend on model, algo)
6. `pipeline` (depends on most modules)
7. `mcp` (depends on everything)
8. `main.rs` (composition root)

### Intra-Module Cross-References

Within `algo/`: `reading_order` → `scc` + `topo_sort`. `impact` → `blast_radius` + `context` + `test_map`. These are internal and acyclic.

Within `analysis/`: `diff` → `metrics` + `smells`. `smells` → `metrics`. Acyclic chain.

Within `recommend/`: `refactor` → `pareto` + `split`. `split` → `min_cut` + `types`. Acyclic.

Within `mcp/`: `tools` → `tools_context` + `tools_temporal` + `tools_semantic` + `tools_recommend`. `server` → `tools` + `state` + `watch` + `lock` + `user_state`. `watch` → `state`. All acyclic.

### Notable Cross-Module Dependency

`parser::registry` → `semantic::BoundaryExtractor` (trait import). This creates a dependency from `parser` to `semantic`. While not circular (semantic does not import parser), it couples the parser registration to the semantic boundary extraction trait. This is by design (boundary extraction happens during parsing in the pipeline).

---

## 4. Duplicate / Overlapping Functionality

### Observation 1: `serde_json` appears twice

- Listed in `[dependencies]` (line 27)
- Listed in `[dev-dependencies]` (line 53)
- Impact: Cargo handles this correctly (dev-dependencies merge with dependencies for test builds). No actual duplication in the binary.
- Evidence: `Cargo.toml:27` and `Cargo.toml:53`

### Observation 2: Tree-sitter grammars at heterogeneous versions

- `tree-sitter` core is at 0.24
- `tree-sitter-json` is at 0.24
- `tree-sitter-typescript/javascript/go/python/rust/c-sharp/java` are all at 0.23
- `tree-sitter-md` is at 0.3
- `tree-sitter-yaml` is at 0.7
- The 0.23 grammars may pull in a different `tree-sitter` minor version transitively. This is managed by Cargo's semver resolution but could cause duplicate tree-sitter C library compilation if the grammars pin an incompatible minor.
- Evidence: `Cargo.toml:13-24`

### Observation 3: `notify` + `notify-debouncer-full`

- Both are optional, behind the `serve` feature.
- `notify-debouncer-full` depends on `notify` internally. Having both declared is correct practice (debouncer re-exports are not guaranteed), but it means `notify` is effectively declared twice (direct + transitive).
- Evidence: `Cargo.toml:41-42`

### Observation 4: No overlapping internal modules detected

- `algo/` and `analysis/` have complementary scopes (graph algorithms vs. metric computation/smell detection). `analysis` consumes `algo` outputs.
- `recommend/` is a distinct layer on top of `algo/` for actionable refactoring suggestions.
- `serial/` handles I/O serialization, `model/` handles types — no overlap.
- `detect/` (file classification) and `parser/` (AST extraction) have distinct responsibilities despite both operating on source files.

---

## 5. Feature Gate Analysis

The `serve` feature is **enabled by default** (`default = ["serve"]`). This means all MCP-related dependencies (rmcp, tokio, schemars, arc-swap, notify, notify-debouncer-full) are always compiled unless a consumer explicitly opts out with `--no-default-features`.

Evidence: `Cargo.toml:44-46`

The `mcp` module is conditionally compiled: `#[cfg(feature = "serve")] pub mod mcp;` in `src/lib.rs:8`.

---

## 6. Benchmark Targets

Six benchmark binaries are declared, each with `harness = false` (uses Criterion):
- `build_bench` — pipeline benchmarks
- `parser_bench` — parser benchmarks
- `algo_bench` — algorithm benchmarks
- `mcp_bench` — MCP server benchmarks
- `analysis_bench` — analysis benchmarks
- `symbol_bench` — symbol extraction benchmarks

Evidence: `Cargo.toml:57-77`
