# Testing Knowledge — Deep Scan (2026-04-04)

## Summary

- **Total source files**: 112 `.rs` files in `src/`
- **Total source lines**: ~117,227 lines
- **Files with inline tests** (`#[cfg(test)]`): 71 of 112 (63%)
- **Files without inline tests**: 41 of 112 (37%)
- **Total inline test functions**: 765 across 71 files
- **Total assertion calls in src/**: 1,615
- **Total assertion calls in tests/**: 496
- **Integration test files**: 10 in `tests/` (+ 1 helpers module)
- **Integration test functions**: ~181
- **Benchmark files**: 6 in `benches/` (+ 1 helpers module)
- **Property-based tests**: `tests/properties.rs` using `proptest` crate
- **Invariant tests**: `tests/invariants.rs` with 18 invariants (INV-1 through INV-18)
- **Test fixtures**: 17 fixture directories in `tests/fixtures/`

## Test File Mapping

### Integration Tests (tests/)

| Test file | Covers | Test count |
|-----------|--------|------------|
| `tests/graph_tests.rs` | Full pipeline build for each language fixture (TS, Go, Python, mixed, workspace, TSX, markdown, edge cases) | 8 |
| `tests/pipeline_tests.rs` | DiagnosticCollector, pipeline errors (E001, E004), walk config, binary detection, output files, timestamps, raw imports, reparse, watcher rebuild, L1 views | 20 |
| `tests/mcp_tests.rs` | MCP lock, freshness state, graph state indices, annotation/bookmark CRUD, MCP server subprocess (JSON-RPC initialize + tools/list) | 28 |
| `tests/symbol_tests.rs` | Symbol extraction for TypeScript, Rust, Go across many patterns | 60 |
| `tests/callgraph_tests.rs` | Call graph: callers_of, callees_of, symbol blast radius, circular handling | 5 |
| `tests/semantic_tests.rs` | HTTP route extraction (Express, Flask, Spring, Go, ASP.NET), event extraction (EventEmitter, Django signals, Spring events), semantic edge building | 25 |
| `tests/temporal_integration.rs` | Git temporal analysis end-to-end: churn, co-change coupling, hotspots, ownership against real temp git repos | 13 |
| `tests/config_resolution_tests.rs` | Config-aware import resolution: tsconfig paths, tsconfig extends, Go module, Python src layout | 9 |
| `tests/invariants.rs` | 18 structural invariants validated across 6 fixtures: edge integrity, no self-imports, test edges, cluster completeness, edge counts, cohesion, determinism, type imports, re-exports, arch depth, SCC depth, centrality range, layer coverage, bottleneck | 18+3 standalone |
| `tests/properties.rs` | Property-based tests: CanonicalPath normalization (4 props), hash determinism (2 props), warning code format, hash format | 8 |

### Inline Test Coverage by Module

**Well-tested modules (10+ tests)**:
- `src/parser/typescript.rs` — 50 tests (highest)
- `src/detect/layer.rs` — 47 tests
- `src/recommend/placement.rs` — 27 tests
- `src/recommend/refactor.rs` — 25 tests
- `src/parser/markdown.rs` — 24 tests
- `src/semantic/events.rs` — 22 tests
- `src/model/types.rs` — 21 tests
- `src/parser/python.rs` — 20 tests
- `src/parser/rust_lang.rs` — 19 tests
- `src/parser/config/tsconfig.rs` — 19 tests
- `src/recommend/split.rs` — 18 tests
- `src/detect/filetype.rs` — 18 tests
- `src/mcp/annotations.rs` — 17 tests
- `src/mcp/bookmarks.rs` — 16 tests
- `src/semantic/http.rs` — 15 tests
- `src/temporal/git.rs` — 14 tests
- `src/parser/go.rs` — 13 tests
- `src/temporal/coupling.rs` — 12 tests
- `src/parser/java.rs` — 12 tests
- `src/parser/csharp.rs` — 11 tests
- `src/mcp/resources.rs` — 11 tests
- `src/mcp/prompts.rs` — 11 tests
- `src/detect/workspace.rs` — 11 tests
- `src/algo/compress.rs` — 11 tests
- `src/temporal/churn.rs` — 10 tests
- `src/recommend/min_cut.rs` — 10 tests
- `src/analysis/smells.rs` — 10 tests
- `src/analysis/metrics.rs` — 10 tests
- `src/algo/pagerank.rs` — 10 tests
- `src/algo/context.rs` — 10 tests

**Moderately tested (5-9 tests)**:
- `src/temporal/hotspot.rs` — 9 tests
- `src/semantic/edges.rs` — 9 tests
- `src/recommend/pareto.rs` — 9 tests
- `src/parser/config/jsonc.rs` — 9 tests
- `src/diagnostic.rs` — 9 tests
- `src/analysis/diff.rs` — 9 tests
- `src/cluster/mod.rs` — 8 tests
- `src/algo/spectral.rs` — 8 tests
- `src/algo/louvain.rs` — 8 tests
- `src/algo/impact.rs` — 8 tests
- `src/algo/delta.rs` — 8 tests
- `src/algo/callgraph.rs` — 8 tests
- `src/views/mod.rs` — 7 tests
- `src/temporal/ownership.rs` — 7 tests
- `src/parser/helpers.rs` — 7 tests
- `src/parser/config/mod.rs` — 7 tests
- `src/mcp/persist.rs` — 7 tests
- `src/algo/scc.rs` — 7 tests
- `src/parser/config/pyproject.rs` — 6 tests
- `src/parser/config/gomod.rs` — 6 tests
- `src/model/annotation.rs` — 6 tests
- `src/detect/case_sensitivity.rs` — 6 tests
- `src/algo/test_map.rs` — 6 tests
- `src/views/index.rs` — 5 tests
- `src/views/impact.rs` — 5 tests
- `src/parser/json_lang.rs` — 5 tests
- `src/algo/topo_sort.rs` — 5 tests
- `src/algo/reading_order.rs` — 5 tests
- `src/algo/centrality.rs` — 5 tests
- `src/algo/blast_radius.rs` — 5 tests

**Lightly tested (1-4 tests)**:
- `src/recommend/types.rs` — 4 tests
- `src/parser/yaml.rs` — 4 tests
- `src/model/bookmark.rs` — 4 tests
- `src/views/cluster.rs` — 3 tests
- `src/serial/convert.rs` — 3 tests
- `src/model/symbol_index.rs` — 3 tests
- `src/mcp/watch.rs` — 3 tests
- `src/mcp/user_state.rs` — 3 tests
- `src/semantic/mod.rs` — 2 tests
- `src/algo/subgraph.rs` — 2 tests
- `src/parser/registry.rs` — 1 test

## Files Without Any Tests (41 files)

### Model layer (no inline tests — 10 files):
- `src/model/mod.rs` — re-export module
- `src/model/compress.rs` — compression model types
- `src/model/diff.rs` — diff model types
- `src/model/edge.rs` — Edge type definitions
- `src/model/graph.rs` — ProjectGraph type
- `src/model/node.rs` — Node type definitions
- `src/model/query.rs` — query model types
- `src/model/semantic.rs` — semantic model types (Boundary, BoundaryKind)
- `src/model/smell.rs` — smell model types
- `src/model/stats.rs` — stats model types
- `src/model/symbol.rs` — symbol model types (SymbolDef, SymbolKind)
- `src/model/temporal.rs` — temporal model types
- `src/model/workspace.rs` — workspace model types

Note: Model files are primarily struct/enum definitions with derives. They are exercised transitively through integration tests and inline tests in consuming modules.

### Pipeline layer (no inline tests — 5 files):
- `src/pipeline/mod.rs` — re-export module
- `src/pipeline/build.rs` — BuildPipeline orchestration
- `src/pipeline/read.rs` — FsReader implementation
- `src/pipeline/resolve.rs` — import resolution orchestration
- `src/pipeline/walk.rs` — FsWalker implementation

Note: Pipeline files are tested through `tests/pipeline_tests.rs` and `tests/graph_tests.rs` integration tests.

### MCP layer (no inline tests — 7 files):
- `src/mcp/mod.rs` — re-export module
- `src/mcp/lock.rs` — file locking (tested in `tests/mcp_tests.rs`)
- `src/mcp/server.rs` — MCP server handler
- `src/mcp/state.rs` — GraphState (tested in `tests/mcp_tests.rs`)
- `src/mcp/tools.rs` — AriadneTools registration
- `src/mcp/tools_context.rs` — context tool implementations
- `src/mcp/tools_recommend.rs` — recommendation tool implementations
- `src/mcp/tools_semantic.rs` — semantic tool implementations
- `src/mcp/tools_temporal.rs` — temporal tool implementations

### Other untested files:
- `src/main.rs` — CLI composition root (clap integration)
- `src/lib.rs` — public API re-exports
- `src/hash.rs` — xxHash64 wrapper (tested via `tests/properties.rs`)
- `src/algo/mod.rs` — re-export module
- `src/algo/stats.rs` — stats computation
- `src/analysis/mod.rs` — re-export module
- `src/detect/mod.rs` — re-export module
- `src/parser/mod.rs` — re-export module
- `src/parser/symbols.rs` — symbol extractor dispatch
- `src/parser/traits.rs` — trait definitions
- `src/recommend/mod.rs` — re-export module
- `src/serial/json.rs` — JSON serialization (tested in `tests/pipeline_tests.rs`)
- `src/serial/mod.rs` — re-export module
- `src/temporal/mod.rs` — re-export module

## Test Infrastructure

### Test Helpers
- `tests/helpers.rs` — `fixture_path()`, `build_fixture()`, `build_and_read_graph_json()` shared across all integration tests
- `benches/helpers.rs` — `generate_synthetic_project()` creates temp projects with configurable file count, directory count, imports per file, and language

### Test Fixtures (tests/fixtures/)
17 fixture directories covering language-specific and cross-cutting scenarios:
- **TypeScript**: `typescript-app`, `tsx-components`, `tsconfig_project`, `tsconfig_extends`
- **Go**: `go-service`, `gomod_project`
- **Python**: `python-package`, `python_src_layout`
- **Java**: `java-project`
- **C#**: `csharp-project`
- **Rust**: `rust-crate`
- **Data formats**: `data-files`, `markdown-docs`
- **Cross-cutting**: `mixed-project`, `edge-cases`, `workspace-project`, `semantic`

### Benchmarks (benches/)
6 benchmark files using Criterion:
- `build_bench.rs` — full pipeline build benchmarks
- `parser_bench.rs` — per-language parsing benchmarks
- `algo_bench.rs` — graph algorithm benchmarks (SCC, centrality, topo sort, etc.)
- `analysis_bench.rs` — Martin metrics and smell detection benchmarks
- `mcp_bench.rs` — MCP tool execution benchmarks
- `symbol_bench.rs` — symbol extraction benchmarks

### Test Patterns Observed

1. **Invariant testing**: `tests/invariants.rs` uses a macro `invariant_test!` to run 18 structural invariants across 6 fixtures. Each invariant verifies a fundamental property of graph output (referential integrity, no self-imports, cluster completeness, determinism, etc.).

2. **Property-based testing**: `tests/properties.rs` uses `proptest` for CanonicalPath normalization (no backslashes, no `./`, no `//`, no trailing slash) and hash format properties.

3. **Determinism testing**: INV-10 and INV-11 verify byte-identical output across repeated builds for both `graph.json` and `stats.json`.

4. **Assertion density**: ~2,111 total assertions (1,615 in src/ inline tests + 496 in tests/). Average ~2.8 assertions per test function. Integration tests tend to have higher assertion density (e.g., `graph_tests::tsx_components` has ~30 assertions in one test).

5. **Feature-gated tests**: MCP tests use `#[cfg(feature = "serve")]` to conditionally compile, matching the feature-gated `mcp/` module.

6. **Subprocess integration**: `tests/mcp_tests.rs::test_mcp_server_initialize_and_tool_list` spawns the ariadne binary as a subprocess, sends JSON-RPC messages over stdin/stdout, and validates the MCP protocol handshake.

7. **Temp directory isolation**: All integration tests use `tempfile::tempdir()` for output to avoid cross-test races.

## Coverage Gaps

### Significant gaps (non-trivial logic without direct tests):
1. **`src/mcp/server.rs`** — MCP server handler with JSON-RPC routing. Only tested indirectly via subprocess integration test.
2. **`src/mcp/tools.rs`** — Tool registration and routing. Only tested indirectly.
3. **`src/mcp/tools_context.rs`** — Context tool implementations (ariadne_context, ariadne_reading_order, etc.). No direct tests.
4. **`src/mcp/tools_recommend.rs`** — Recommendation tool implementations. No direct tests.
5. **`src/mcp/tools_semantic.rs`** — Semantic tool implementations. No direct tests.
6. **`src/mcp/tools_temporal.rs`** — Temporal tool implementations. No direct tests.
7. **`src/pipeline/build.rs`** — Build orchestration logic. Tested only through integration tests.
8. **`src/pipeline/resolve.rs`** — Import resolution orchestration. Tested only through integration tests.
9. **`src/algo/stats.rs`** — Stats computation. No inline tests; exercised transitively through invariant tests.
10. **`src/parser/symbols.rs`** — Symbol extractor dispatch. No inline tests; exercised through `tests/symbol_tests.rs`.
11. **`src/main.rs`** — CLI argument parsing and command dispatch. No tests.

### Acceptably untested (boilerplate/trivial):
- `mod.rs` files (13 files) — re-export modules with no logic
- `src/lib.rs` — public API re-exports
- Model struct/enum files (10 files) — primarily `#[derive]` definitions exercised transitively
- `src/parser/traits.rs` — trait definitions only
