<!-- moira:freshness init 2026-04-04 -->
<!-- moira:knowledge project-model L2 -->

---
layout_pattern: single-app
source_root: src
entry_points:
  - src/main.rs
test_pattern: separate
test_roots:
  - tests
  - benches
test_naming: "*_tests.rs, *_bench.rs"
do_not_modify:
  - target/
  - .ariadne/graph/
  - tests/fixtures/
modify_with_caution:
  - Cargo.toml
  - Cargo.lock
  - .github/workflows/ci.yml
  - .github/workflows/release.yml
dir_types: src/model/
dir_utils: src/hash.rs
dir_algorithms: src/algo/
dir_parsers: src/parser/
dir_pipeline: src/pipeline/
dir_serialization: src/serial/
dir_detection: src/detect/
dir_analysis: src/analysis/
dir_views: src/views/
dir_mcp: src/mcp/
dir_temporal: src/temporal/
dir_semantic: src/semantic/
dir_recommend: src/recommend/
dir_cluster: src/cluster/
---

## Project Root

Top-level contents of `/Users/minddecay/Documents/Projects/Ariadne/`:

| Entry | Type | Purpose |
|-------|------|---------|
| `src/` | directory | Rust source code (112 files) |
| `tests/` | directory | Integration tests and fixtures (136 files) |
| `benches/` | directory | Performance benchmarks (7 files) |
| `design/` | directory | Design documents, specs, decisions, reports (38 files) |
| `docs/` | directory | Additional documentation (1 file) |
| `.github/` | directory | CI/CD workflows (2 files) |
| `.claude/` | directory | Claude Code commands and settings (7 files) |
| `.moira/` | directory | Moira orchestration config and state (22 files) |
| `.ariadne/` | directory | Ariadne's own graph output (25 files) |
| `.vscode/` | directory | VS Code settings (2 files) |
| `target/` | directory | Rust build output (generated) |
| `Cargo.toml` | file | Rust project manifest (77 lines) |
| `Cargo.lock` | file | Dependency lock file |
| `CLAUDE.md` | file | Project instructions for Claude Code |
| `README.md` | file | Project readme |
| `install.sh` | file | Local installation script |
| `remote-install.sh` | file | Remote installation script |
| `.mcp.json` | file | MCP server configuration |
| `.gitignore` | file | Git ignore rules |
| `LICENSE-APACHE` | file | Apache 2.0 license |
| `LICENSE-MIT` | file | MIT license |
| `.DS_Store` | file | macOS metadata (untracked) |

## Source Layout

### `src/` (112 files)

Top-level files:
- `main.rs` — application entry point (composition root, CLI via clap)
- `lib.rs` — public API re-exports
- `diagnostic.rs` — FatalError, Warning, DiagnosticCollector
- `hash.rs` — xxHash64 content hashing

Subdirectories (16 modules):

| Module | Files | Contents |
|--------|-------|----------|
| `algo/` | 17 | Graph algorithms: SCC, BFS, centrality, topo sort, subgraph, PageRank, spectral, blast radius, call graph, compression, delta, context, impact, reading order, Louvain, test map, stats |
| `model/` | 16 | Data types: annotation, bookmark, compress, diff, edge, graph, node, query, semantic, smell, stats, symbol, symbol_index, temporal, types, workspace |
| `mcp/` | 16 | MCP server: annotations, bookmarks, lock, persist, prompts, resources, server, state, tools, tools_context, tools_recommend, tools_semantic, tools_temporal, user_state, watch |
| `parser/` | 14 | Language parsers: TypeScript, Python, Go, Rust, Java, C#, JSON, YAML, Markdown + config/ subdirectory (tsconfig, gomod, pyproject, jsonc), helpers, symbols, traits, registry |
| `recommend/` | 7 | Recommendation engine: min_cut, pareto, placement, refactor, split, types |
| `pipeline/` | 5 | Build pipeline: build, walk, read, resolve |
| `temporal/` | 6 | Git temporal analysis: churn, coupling, git, hotspot, ownership |
| `detect/` | 5 | File detection: case_sensitivity, filetype, layer, workspace |
| `analysis/` | 4 | Architectural analysis: diff, metrics, smells |
| `serial/` | 3 | Serialization: convert, json |
| `semantic/` | 4 | Boundary extraction: edges, events, http |
| `views/` | 4 | Markdown views: cluster, impact, index |
| `cluster/` | 1 | Directory-based clustering |

### `src/parser/config/` (nested subdirectory)

- `mod.rs` — config module root
- `tsconfig.rs` — tsconfig.json path resolution
- `gomod.rs` — go.mod module resolution
- `pyproject.rs` — pyproject.toml resolution
- `jsonc.rs` — JSONC parser utility

## Directory Roles

| Directory | Role | Evidence |
|-----------|------|----------|
| `src/model/` | Data types (leaf module, no deps) | Contains only type definitions: node, edge, graph, types, symbol, etc. |
| `src/parser/` | Language parsing + import resolution | Per-language parsers (typescript.rs, python.rs, go.rs, etc.) + traits.rs + registry.rs |
| `src/pipeline/` | Build orchestration | walk.rs (file discovery), read.rs (file reading), build.rs (pipeline), resolve.rs (import resolution) |
| `src/algo/` | Graph algorithms | scc.rs, centrality.rs, topo_sort.rs, pagerank.rs, spectral.rs, etc. |
| `src/mcp/` | MCP server interface | server.rs, tools.rs, resources.rs, prompts.rs, state.rs, watch.rs |
| `src/serial/` | Graph serialization/deserialization | json.rs, convert.rs |
| `src/detect/` | File type + layer detection | filetype.rs, layer.rs, workspace.rs, case_sensitivity.rs |
| `src/analysis/` | Architectural metrics + smells | metrics.rs (Martin metrics), smells.rs (smell detection), diff.rs (structural diff) |
| `src/views/` | Markdown view generation | index.rs (L0), cluster.rs (L1), impact.rs (L2) |
| `src/temporal/` | Git history analysis | churn.rs, coupling.rs, hotspot.rs, ownership.rs, git.rs |
| `src/semantic/` | Boundary extraction | http.rs (HTTP routes), events.rs (event emitters), edges.rs |
| `src/recommend/` | Recommendation engine | refactor.rs, split.rs, placement.rs, min_cut.rs, pareto.rs |
| `src/cluster/` | Directory-based clustering | Single mod.rs |
| `design/` | Design documents (source of truth) | architecture.md, ROADMAP.md, decisions/log.md, specs/, reports/ |
| `.ariadne/` | Ariadne's own graph output | graph/ (JSON output), views/ (generated markdown), bookmarks.json |

## Generated (do not modify)

| Directory | Contents | Evidence |
|-----------|----------|----------|
| `target/` | Rust build artifacts | Listed in .gitignore; subdirs: debug/, release/, tmp/ |
| `.ariadne/graph/` | Generated graph JSON | .ariadne/graph/.lock and raw_imports.json listed in .gitignore as ephemeral |

## Vendored (do not modify)

No vendored or third-party directories detected. No `vendor/`, `third_party/`, or similar directories exist at any level.

## Configuration

| File | Purpose | Evidence |
|------|---------|----------|
| `Cargo.toml` | Rust project manifest — dependencies, build config, binary/lib targets | 77 lines, root-level |
| `Cargo.lock` | Pinned dependency versions | Root-level lock file |
| `.gitignore` | Git ignore rules — excludes target/, .vscode/, .worktrees/, ephemeral ariadne/moira state | Root-level |
| `.mcp.json` | MCP server configuration for Ariadne tool integration | Root-level |
| `.github/workflows/ci.yml` | CI workflow | GitHub Actions |
| `.github/workflows/release.yml` | Release workflow | GitHub Actions |

## Test Organization

**Pattern:** Separate test directory with integration tests + co-located mod tests within `src/`.

**Integration tests** (`tests/`, 12 test files):
- `pipeline_tests.rs` — pipeline integration
- `graph_tests.rs` — graph construction
- `mcp_tests.rs` — MCP server
- `invariants.rs` — structural invariants
- `properties.rs` — property-based tests
- `symbol_tests.rs` — symbol extraction
- `semantic_tests.rs` — semantic boundary extraction
- `temporal_integration.rs` — temporal/git analysis
- `callgraph_tests.rs` — call graph
- `config_resolution_tests.rs` — config-aware import resolution
- `helpers.rs` — shared test utilities

**Test fixtures** (`tests/fixtures/`, 17 fixture projects):
csharp-project, data-files, edge-cases, go-service, gomod_project, java-project, markdown-docs, mixed-project, python-package, python_src_layout, rust-crate, semantic, tsconfig_extends, tsconfig_project, tsx-components, typescript-app, workspace-project

**Benchmarks** (`benches/`, 7 files):
algo_bench.rs, analysis_bench.rs, build_bench.rs, mcp_bench.rs, parser_bench.rs, symbol_bench.rs, helpers.rs

**Naming conventions:**
- Integration tests: `*_tests.rs` or descriptive names (invariants.rs, properties.rs)
- Benchmarks: `*_bench.rs`
- Fixtures: project-like directory structures representing various language ecosystems
- Shared utilities: `helpers.rs` (present in both tests/ and benches/)

## Structural Bottlenecks

| File | Centrality Score |
|------|-----------------|
| src/model/mod.rs | 0.0098 |
| src/algo/mod.rs | 0.0034 |
| src/parser/mod.rs | 0.0018 |
| src/pipeline/mod.rs | 0.0012 |
| src/parser/config/mod.rs | 0.0011 |
| src/parser/config/tsconfig.rs | 0.0007 |
| src/mcp/state.rs | 0.0006 |
| src/mcp/tools.rs | 0.0004 |
| src/semantic/mod.rs | 0.0004 |
| src/analysis/smells.rs | 0.0003 |
| src/detect/mod.rs | 0.0003 |
| src/parser/registry.rs | 0.0003 |
| src/analysis/diff.rs | 0.0002 |
| src/diagnostic.rs | 0.0002 |
| src/mcp/server.rs | 0.0002 |

## Architectural Layers

| Layer | Files |
|-------|-------|
| 00000 | .claude/CLAUDE.md, .claude/commands/audit-docs.md, .claude/commands/review-architecture.md, .claude/commands/review-p... |
| 00001 | src/detect/case_sensitivity.rs, src/model/symbol_index.rs, src/parser/symbols.rs, src/semantic/edges.rs, src/temporal... |
| 00002 | src/algo/callgraph.rs, src/model/mod.rs, src/semantic/events.rs, src/semantic/http.rs, src/semantic/mod.rs, tests/fix... |
| 00003 | src/algo/compress.rs, src/algo/delta.rs, src/algo/louvain.rs, src/algo/subgraph.rs, src/cluster/mod.rs, src/detect/fi... |
| 00004 | src/algo/blast_radius.rs, src/algo/centrality.rs, src/algo/context.rs, src/algo/impact.rs, src/algo/mod.rs, src/algo/... |
| 00005 | src/analysis/metrics.rs, src/detect/mod.rs, src/parser/config/mod.rs, src/parser/config/tsconfig.rs, src/parser/go.rs... |
| 00006 | src/analysis/smells.rs, src/mcp/state.rs, src/pipeline/build.rs, src/pipeline/resolve.rs, src/recommend/refactor.rs |
| 00007 | src/analysis/diff.rs, src/mcp/prompts.rs, src/mcp/resources.rs, src/pipeline/mod.rs, src/recommend/mod.rs |
| 00008 | src/analysis/mod.rs, src/mcp/tools.rs, src/mcp/watch.rs |
| 00009 | src/mcp/server.rs |
| 00010 | src/mcp/mod.rs |
| 00011 | src/lib.rs |

## Cluster Metrics

| Cluster | Instability | Abstractness | Distance | Zone |
|---------|-------------|-------------|----------|------|
| .claude | 0.0 | 0.0 | 1.0 | ZoneOfPain |
| .github | 0.0 | 0.0 | 1.0 | ZoneOfPain |
| .moira | 0.0 | 0.0 | 1.0 | ZoneOfPain |
| algo | 0.4651 | 0.0 | 0.5349 | ZoneOfPain |
| analysis | 0.5 | 0.0 | 0.5 | OffMainSequence |
| benches | 0.0 | 0.0 | 1.0 | ZoneOfPain |
| cluster | 0.3333 | 0.0 | 0.6667 | ZoneOfPain |
| design | 0.0 | 0.0 | 1.0 | ZoneOfPain |
| detect | 0.6 | 0.0 | 0.4 | OffMainSequence |
| mcp | 0.9697 | 0.0 | 0.0303 | MainSequence |
| model | 0.0 | 0.0 | 1.0 | ZoneOfPain |
| parser | 0.875 | 0.0526 | 0.0724 | MainSequence |
| pipeline | 0.8929 | 0.0 | 0.1071 | MainSequence |
| recommend | 0.9091 | 0.0 | 0.0909 | MainSequence |
| root | 0.4688 | 0.0 | 0.5312 | ZoneOfPain |
| semantic | 0.7273 | 0.0 | 0.2727 | MainSequence |
| serial | 0.6154 | 0.0 | 0.3846 | OffMainSequence |
| temporal | 0.9286 | 0.0 | 0.0714 | MainSequence |
| tests | 0.0 | 0.0538 | 0.9462 | ZoneOfPain |
| views | 0.8333 | 0.0 | 0.1667 | MainSequence |

## Architectural Boundaries

(no boundary data available)

## Graph Summary

- Nodes: 336
- Edges: 401
- Clusters: 20
- Cycles: 4
- Smells: 13
- Monolith score: 0
- Temporal: available

