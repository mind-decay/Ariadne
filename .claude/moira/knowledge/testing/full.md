<!-- moira:deep-scan test-coverage 2026-03-21 -->

# Deep Test Coverage Assessment

Scan date: 2026-03-21
Source files scanned: 64
Test files scanned: 6 integration + 23 inline `#[cfg(test)]` modules
Total `#[test]` functions found: ~200 across 23 source files + 5 integration test files

---

## 1. Test File Mapping

### Integration Tests (`tests/`)

| Test File | Covers | Description |
|-----------|--------|-------------|
| `tests/graph_tests.rs` | `pipeline/*`, `parser/*`, `serial/*`, `cluster/*`, `detect/*` | End-to-end fixture builds for 6 fixtures: typescript-app, go-service, python-package, mixed-project, workspace-project, edge-cases |
| `tests/invariants.rs` | `serial/*`, `cluster/*`, `algo/*`, `model/*` | 18 structural invariants (INV-1 through INV-18) + determinism checks, run against all 6 fixtures |
| `tests/pipeline_tests.rs` | `pipeline/*`, `diagnostic.rs`, `serial/json.rs` | Pipeline error handling (E001, E004), walk config, binary detection, timestamp flags, raw imports round-trip, reparse_imports |
| `tests/properties.rs` | `model/types.rs`, `hash.rs`, `diagnostic.rs` | Property-based tests (proptest) for CanonicalPath normalization, hash determinism/format, WarningCode display |
| `tests/mcp_tests.rs` | `mcp/lock.rs`, `mcp/state.rs`, `mcp/server.rs`, `mcp/tools.rs` | Lock acquire/release/stale, FreshnessState confidence, GraphState indexing, MCP server initialize+tool-list (subprocess integration) |
| `tests/helpers.rs` | (test utility) | `fixture_path()`, `build_fixture()`, `build_and_read_graph_json()` shared helpers |

### Inline Unit Tests (`#[cfg(test)]` modules in `src/`)

| Source File | Test Count | What Is Tested |
|-------------|-----------|----------------|
| `src/algo/scc.rs` | 7 | Linear chain, simple cycle, two cycles, DAG, fully connected, empty graph, tests-edges exclusion |
| `src/algo/centrality.rs` | 5 | Star graph, linear chain, value range [0,1], <3 nodes, float determinism |
| `src/algo/topo_sort.rs` | 5 | Linear chain depths, DAG multi-path, cycle handling, single node, empty graph |
| `src/algo/subgraph.rs` | 2 | Depth-1 neighborhood, cluster inclusion expansion |
| `src/algo/blast_radius.rs` | 5 | Linear chain BFS, depth limit, disconnected node, nonexistent file, re-export propagation |
| `src/algo/delta.rs` | 8 | No changes, one changed, files added/removed, threshold triggers, empty old/current, sorted results |
| `src/algo/compress.rs` | 11 | L0 node count, edge weights, key files top-3, token estimate; L1 files, external edges, unknown cluster; L2 neighborhood, depth-1, unknown file; edge referential integrity |
| `src/algo/pagerank.rs` | 10 | Empty graph, chain rank ordering, star graph, sum-to-one, disconnected, self-loop, test-edge exclusion, determinism, combined importance balance/max |
| `src/algo/louvain.rs` | 8 | Empty graph, single file, no edges, two cliques, determinism, cohesion, naming plurality, disconnected components |
| `src/algo/spectral.rs` | 8 | Complete graph K5, path graph P5, complete vs path comparison, disconnected zero-lambda2, sign convention, determinism, single node, bridge separation |
| `src/analysis/smells.rs` | 10 | God file (detected + below threshold), circular dependency, layer violation, hub-and-spoke, unstable foundation, dead cluster (detected + top-level not detected), shotgun surgery, clean architecture no smells |
| `src/analysis/diff.rs` | 9 | Additive change, breaking change, refactor, migration, Louvain noise filtered, Louvain real change, cycle diff, magnitude calculation, empty diff |
| `src/analysis/metrics.rs` | 10 | Isolated cluster I=0, fully outgoing I=1, all-typedef A=1, no-abstract A=0, barrel file detection, zone of pain, zone of uselessness, main sequence, determinism, valid range |
| `src/cluster/mod.rs` | 8 | Cluster name extraction, basic clustering, empty graph, cohesion all-internal, cohesion no-edges, files sorted, root cluster, deterministic output |
| `src/detect/layer.rs` | 18 | API/Service/Data/Util/Component/Hook/Config layers, internal-not-util, SvelteKit, hexagonal, CQRS, event-driven, MVVM, Rails/Django, Angular/NestJS, unknown, case-insensitive, first-match-wins |
| `src/detect/filetype.rs` | 15 | Config exact/tsconfig/env variants, test TS/Go/Python/Rust/C#/Java, typedef, style, asset, source default, priority ordering (config>test, test>typedef) |
| `src/detect/workspace.rs` | 10 | npm/yarn/pnpm workspace detection, entry point preference, name collision W008, malformed JSON, no-workspace, empty directory, pnpm YAML parsing, stops-at-next-key |
| `src/detect/case_sensitivity.rs` | 6 | Platform detection, invalid root, exact match, different case, no match, empty fileset |
| `src/model/types.rs` | 21 | CanonicalPath normalization (14 cases: basic, backslash, dot-slash, double/triple slash, trailing, dot-dot resolve/escape/deep/only, empty, single dot, dot-slash-only, mixed), parent/extension/file_name, FileSet contains/len/order |
| `src/diagnostic.rs` | 9 | Human format with/without detail, JSON format with/without detail, summary with/without skipped, W006 filtered/shown with verbose, warning code display |
| `src/serial/convert.rs` | 3 | Round-trip graph output, version mismatch rejection, round-trip cluster output |
| `src/parser/typescript.rs` | 9 | Workspace direct/subpath/subpath-ext/subpath-index imports, non-workspace scoped, bare specifier, relative unchanged, workspace-none behavior |
| `src/mcp/watch.rs` | 3 | should_trigger_rebuild for extensions, .ariadne output dir exclusion, .ariadne component exclusion |

---

## 2. Untested Source Files

The following 29 source files have **zero** `#[test]` functions (no inline tests) and no dedicated integration test file:

### Completely Untested (no inline tests, no integration coverage)

| File | Lines | Risk Assessment |
|------|-------|----------------|
| `src/views/index.rs` | 101 | Markdown generation (L0 index view) -- no tests at all |
| `src/views/cluster.rs` | 173 | Markdown generation (L1 cluster view) -- no tests at all |
| `src/views/impact.rs` | 97 | Markdown generation (L2 impact view) -- no tests at all |
| `src/views/mod.rs` | 56 | View orchestration, sanitize_filename -- no tests |
| `src/algo/stats.rs` | 117 | Stats computation from algorithm results -- no tests |
| `src/algo/mod.rs` | 56 | Module re-exports + `is_architectural()` helper -- no tests |

### No Inline Tests, But Exercised Via Integration Tests

| File | Lines | Integration Coverage |
|------|-------|---------------------|
| `src/pipeline/build.rs` | 321 | Exercised by `tests/pipeline_tests.rs` and `tests/graph_tests.rs` (full pipeline runs) |
| `src/pipeline/walk.rs` | 194 | Exercised by `tests/pipeline_tests.rs` (`walk_excludes_all_configured_dirs`, `walk_config_respects_max_files`) |
| `src/pipeline/read.rs` | 109 | Exercised by `tests/pipeline_tests.rs` (`binary_file_detected_by_null_bytes`) |
| `src/pipeline/resolve.rs` | 43 | Exercised by pipeline integration tests via full build |
| `src/pipeline/mod.rs` | -- | Module re-exports only |
| `src/serial/json.rs` | 147 | Exercised by `tests/pipeline_tests.rs` (raw imports round-trip) and every fixture build |
| `src/parser/go.rs` | 219 | Exercised by `tests/graph_tests.rs` (`go_service` fixture) |
| `src/parser/python.rs` | 506 | Exercised by `tests/graph_tests.rs` (`python_package` fixture) |
| `src/parser/rust_lang.rs` | 597 | Exercised by `tests/graph_tests.rs` (`rust-crate` fixture exists but is not in `FIXTURES` list in invariants.rs) |
| `src/parser/csharp.rs` | 244 | Fixture `csharp-project` exists but is NOT used in any test file |
| `src/parser/java.rs` | 237 | Fixture `java-project` exists but is NOT used in any test file |
| `src/parser/traits.rs` | 48 | Trait definitions only |
| `src/parser/registry.rs` | 156 | `ParserRegistry::with_tier1()` used in every integration test |
| `src/parser/mod.rs` | -- | Module re-exports only |
| `src/model/graph.rs` | 28 | Struct definitions only |
| `src/model/node.rs` | 69 | Struct definitions only |
| `src/model/edge.rs` | 41 | Struct definitions only |
| `src/model/compress.rs` | 70 | Struct definitions used by `algo/compress.rs` tests |
| `src/model/diff.rs` | 53 | Struct definitions used by `analysis/diff.rs` tests |
| `src/model/smell.rs` | 60 | Struct definitions used by `analysis/smells.rs` tests |
| `src/model/stats.rs` | 23 | Struct definitions |
| `src/model/workspace.rs` | 51 | Struct definitions used by workspace tests |
| `src/model/query.rs` | 14 | Type alias only |
| `src/model/mod.rs` | -- | Module re-exports |
| `src/mcp/server.rs` | 209 | Exercised by `tests/mcp_tests.rs` subprocess integration test |
| `src/mcp/tools.rs` | 709 | 11+ MCP tool handlers -- tested only via subprocess integration (initialize + tool list). No individual tool handler tests |
| `src/mcp/lock.rs` | 146 | Exercised by `tests/mcp_tests.rs` lock_tests module |
| `src/main.rs` | -- | CLI entrypoint (clap), no tests |
| `src/lib.rs` | -- | Re-exports only |
| `src/hash.rs` | -- | Exercised via property tests in `tests/properties.rs` |

---

## 3. Test Quality Observations

### Positive Patterns

- **Invariant tests are thorough**: `tests/invariants.rs` contains 18 structural invariants (INV-1 through INV-18) run against all 6 fixtures. These verify edge referential integrity, no self-imports, cluster completeness, cohesion formulas, centrality ranges, layer coverage, bottleneck correctness, and determinism.
- **Property-based testing present**: `tests/properties.rs` uses `proptest` for CanonicalPath normalization and hash format/determinism.
- **Determinism is explicitly tested**: INV-10 (hash determinism), INV-11 (byte-identical graph.json and stats.json), plus determinism tests in `centrality.rs`, `pagerank.rs`, `spectral.rs`, `louvain.rs`, `metrics.rs`.
- **Error paths are tested**: E001 (ProjectNotFound), E004 (NoParseableFiles), binary file detection, version mismatch rejection, unknown cluster/file errors.
- **Warning system tested**: W004 (BinaryFile), W009 (EncodingError), W006 filtering with/without verbose, W008 (ConfigParseFailed for name collision).

### Test Anti-Patterns and Gaps

1. **Duplicated `make_graph` helper**: The `make_graph()` helper function is independently defined in 8 separate test modules (`scc.rs`, `centrality.rs`, `topo_sort.rs`, `subgraph.rs`, `blast_radius.rs`, `cluster/mod.rs`, `louvain.rs`, `analysis/diff.rs`). Each has a slightly different signature but the same core pattern of building `ProjectGraph` from node names and edge pairs. There is no shared test utility module in `src/`.

2. **Duplicated `make_node` helper**: Similarly, `make_node()` is independently defined in `compress.rs`, `pagerank.rs`, `louvain.rs`, `spectral.rs`, `smells.rs`, `delta.rs`, `metrics.rs`, `cluster/mod.rs`, and `mcp_tests.rs` -- at least 9 separate definitions.

3. **Fixture coverage gaps**:
   - `tests/fixtures/rust-crate/` exists but is NOT in the `FIXTURES` list in `invariants.rs`. It is not used by any test.
   - `tests/fixtures/csharp-project/` exists but is NOT used by any test.
   - `tests/fixtures/java-project/` exists but is NOT used by any test.
   - The `FIXTURES` list in `invariants.rs` is: `["typescript-app", "go-service", "python-package", "mixed-project", "edge-cases", "workspace-project"]`.

4. **`mcp/tools.rs` is the largest untested file (709 lines)**: Contains all MCP tool handlers (ariadne_overview, ariadne_file, ariadne_blast_radius, ariadne_freshness, etc.). The only test coverage is a subprocess integration test that verifies tool listing, not individual tool execution or output correctness.

5. **Views module entirely untested (371 lines total)**: `views/index.rs` (101), `views/cluster.rs` (173), `views/impact.rs` (97), `views/mod.rs` (56) have zero test coverage. These generate Markdown output that could silently break.

6. **Parser implementations lack unit tests**: `go.rs` (219 lines), `python.rs` (506 lines), `rust_lang.rs` (597 lines), `csharp.rs` (244 lines), `java.rs` (237 lines) have no inline tests. Only `typescript.rs` has inline tests (9 tests for import resolution). Go and Python parsers get some integration coverage via fixtures; C#, Java, and Rust parsers have fixture directories that are not exercised.

7. **`algo/stats.rs` untested (117 lines)**: Computes `StatsOutput` from algorithm results. Tested indirectly through invariants (INV-8, INV-16, INV-17, INV-18 verify stats.json contents) but has no direct unit tests for edge cases in stats computation.

8. **Integration tests assert minimums, not specifics**: `graph_tests.rs` uses assertions like `output.file_count > 0`, `output.edge_count > 0` -- these verify the pipeline doesn't crash but don't verify correctness of specific parse results. The workspace test is an exception, verifying specific cross-package edges.

9. **No negative tests for parser correctness**: There are no tests for malformed input handling (e.g., syntactically invalid TypeScript/Go/Python), except for the binary file and bad-encoding cases in the edge-cases fixture.

10. **MCP feature-gated tests may not run in default CI**: Tests in `mcp_tests.rs` are gated with `#[cfg(feature = "serve")]`. If CI does not enable this feature, these tests are silently skipped.

---

## 4. Test Infrastructure

### Test Helpers

| Location | Purpose |
|----------|---------|
| `tests/helpers.rs` | Shared integration test utilities: `fixture_path()`, `build_fixture()`, `build_and_read_graph_json()`. Uses `tempfile::tempdir()` for output isolation. |
| `invariant_test!` macro in `tests/invariants.rs` | Generates tests that run a check function against all 6 fixtures |

### Fixtures

| Fixture | Location | Used By |
|---------|----------|---------|
| `typescript-app` | `tests/fixtures/typescript-app/` | `graph_tests.rs`, `invariants.rs`, `pipeline_tests.rs`, `mcp_tests.rs` |
| `go-service` | `tests/fixtures/go-service/` | `graph_tests.rs`, `invariants.rs` |
| `python-package` | `tests/fixtures/python-package/` | `graph_tests.rs`, `invariants.rs` |
| `mixed-project` | `tests/fixtures/mixed-project/` | `graph_tests.rs`, `invariants.rs` |
| `edge-cases` | `tests/fixtures/edge-cases/` | `graph_tests.rs`, `invariants.rs` |
| `workspace-project` | `tests/fixtures/workspace-project/` | `graph_tests.rs`, `invariants.rs` |
| `rust-crate` | `tests/fixtures/rust-crate/` | **UNUSED** |
| `csharp-project` | `tests/fixtures/csharp-project/` | **UNUSED** |
| `java-project` | `tests/fixtures/java-project/` | **UNUSED** |

### External Test Dependencies

| Crate | Used In |
|-------|---------|
| `tempfile` | `tests/helpers.rs`, `tests/pipeline_tests.rs`, `tests/mcp_tests.rs`, inline tests in `detect/workspace.rs`, `detect/case_sensitivity.rs` |
| `proptest` | `tests/properties.rs` |
| `serde_json` | `tests/graph_tests.rs`, `tests/invariants.rs`, `tests/pipeline_tests.rs`, `tests/mcp_tests.rs` |

### Test Patterns Used

- **Fixture-based integration testing**: Build real fixture projects through the full pipeline, assert on output JSON structure.
- **Invariant testing**: Structural properties verified across all fixtures via a custom macro.
- **Property-based testing**: Proptest for path normalization and hash format properties.
- **Subprocess integration testing**: MCP server tested by spawning a child process and sending JSON-RPC over stdin/stdout.
- **Inline unit tests**: Algorithm modules use `#[cfg(test)]` with hand-crafted graph structures.
- **No snapshot testing**: Despite the design doc mentioning snapshot tests, no `.snap` files or `insta` crate usage was found.
- **No benchmark tests**: No `benches/` directory files found, despite the design doc referencing Phase 1b benchmarks.
- **No mocking/faking**: No mock frameworks. The pipeline uses trait objects (`Box<dyn FileWalker>`, `Box<dyn FileReader>`) but tests use real implementations (`FsWalker`, `FsReader`).

---

## 5. Coverage Summary

| Module | Inline Tests | Integration Tests | Coverage Level |
|--------|-------------|-------------------|----------------|
| `algo/scc` | 7 | via invariants (INV-14, INV-15) | HIGH |
| `algo/centrality` | 5 | via invariants (INV-16, INV-18) | HIGH |
| `algo/topo_sort` | 5 | via invariants (INV-14) | HIGH |
| `algo/blast_radius` | 5 | -- | MEDIUM (unit only) |
| `algo/subgraph` | 2 | -- | LOW |
| `algo/delta` | 8 | -- | MEDIUM (unit only) |
| `algo/compress` | 11 | -- | HIGH (unit only) |
| `algo/pagerank` | 10 | -- | HIGH (unit only) |
| `algo/louvain` | 8 | -- | HIGH (unit only) |
| `algo/spectral` | 8 | -- | HIGH (unit only) |
| `algo/stats` | 0 | indirect via invariants | LOW |
| `analysis/smells` | 10 | -- | HIGH (unit only) |
| `analysis/diff` | 9 | -- | HIGH (unit only) |
| `analysis/metrics` | 10 | -- | HIGH (unit only) |
| `cluster/mod` | 8 | via invariants (INV-4 to INV-7) | HIGH |
| `detect/layer` | 18 | via fixtures | HIGH |
| `detect/filetype` | 15 | via fixtures | HIGH |
| `detect/workspace` | 10 | via workspace fixture | HIGH |
| `detect/case_sensitivity` | 6 | -- | MEDIUM |
| `model/types` | 21 | via properties | HIGH |
| `diagnostic` | 9 | via pipeline_tests | HIGH |
| `serial/convert` | 3 | -- | MEDIUM |
| `serial/json` | 0 | via pipeline_tests | LOW |
| `parser/typescript` | 9 | via fixtures | HIGH |
| `parser/go` | 0 | via go-service fixture | LOW |
| `parser/python` | 0 | via python-package fixture | LOW |
| `parser/rust_lang` | 0 | fixture exists but unused | NONE |
| `parser/csharp` | 0 | fixture exists but unused | NONE |
| `parser/java` | 0 | fixture exists but unused | NONE |
| `pipeline/build` | 0 | via all integration tests | MEDIUM |
| `pipeline/walk` | 0 | via pipeline_tests | LOW |
| `pipeline/read` | 0 | via pipeline_tests | LOW |
| `pipeline/resolve` | 0 | indirect via builds | LOW |
| `views/*` | 0 | -- | NONE |
| `mcp/tools` | 0 | subprocess only (tool list) | VERY LOW |
| `mcp/server` | 0 | subprocess integration | LOW |
| `mcp/state` | 0 | via mcp_tests (state_tests) | MEDIUM |
| `mcp/lock` | 0 | via mcp_tests (lock_tests) | HIGH |
| `mcp/watch` | 3 | -- | MEDIUM |
| `hash` | 0 | via properties | MEDIUM |
| `main.rs` | 0 | -- | NONE |

---

*End of deep scan.*
