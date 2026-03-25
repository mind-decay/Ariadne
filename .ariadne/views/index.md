# Project Index

## Architecture Summary

- **Files:** 273
- **Edges:** 349
- **Clusters:** 17
- **Max depth:** 11
- **Avg in-degree:** 1.1795
- **Avg out-degree:** 1.1795

## Clusters

| Cluster | Files | Key File | Cohesion |
|---------|------:|----------|--------:|
| .claude | 39 | `.claude/settings.local.json` | 1.0000 |
| .github | 2 | `.github/workflows/release.yml` | 1.0000 |
| algo | 17 | `src/algo/mod.rs` | 0.4324 |
| analysis | 4 | `src/analysis/smells.rs` | 0.2857 |
| benches | 7 | `benches/symbol_bench.rs` | 0.0000 |
| cluster | 1 | `src/cluster/mod.rs` | 0.0000 |
| design | 34 | `design/testing.md` | 1.0000 |
| detect | 5 | `src/detect/mod.rs` | 0.2857 |
| docs | 1 | `docs/superpowers/plans/2026-03-19-architecture-review-fixes.md` | 1.0000 |
| mcp | 13 | `src/mcp/state.rs` | 0.4407 |
| model | 15 | `src/model/mod.rs` | 0.1552 |
| parser | 14 | `src/parser/mod.rs` | 0.4507 |
| pipeline | 5 | `src/pipeline/mod.rs` | 0.1212 |
| root | 7 | `src/diagnostic.rs` | 0.0769 |
| serial | 3 | `src/serial/mod.rs` | 0.1053 |
| tests | 102 | `tests/symbol_tests.rs` | 0.6182 |
| views | 4 | `src/views/mod.rs` | 0.3333 |

## Circular Dependencies

1. 12 files: src/algo/blast_radius.rs → src/algo/centrality.rs → src/algo/context.rs → src/algo/impact.rs → src/algo/mod.rs → src/algo/pagerank.rs → src/algo/reading_order.rs → src/algo/scc.rs → src/algo/spectral.rs → src/algo/stats.rs → src/algo/test_map.rs → src/algo/topo_sort.rs
2. 2 files: tests/fixtures/edge-cases/circular-a.ts → tests/fixtures/edge-cases/circular-b.ts

## Orphan Files

- `benches/helpers.rs`
- `tests/callgraph_tests.rs`
- `tests/fixtures/csharp-project/Data/UserRepository.cs`
- `tests/fixtures/csharp-project/Program.cs`
- `tests/fixtures/csharp-project/Services/AuthService.cs`
- `tests/fixtures/csharp-project/Tests/AuthTests.cs`
- `tests/fixtures/edge-cases/deeply/nested/a/b/c/d/file.ts`
- `tests/fixtures/edge-cases/empty.ts`
- `tests/fixtures/edge-cases/helper.ts`
- `tests/fixtures/go-service/cmd/server/main.go`
- `tests/fixtures/go-service/internal/handler/handler.go`
- `tests/fixtures/go-service/internal/repository/repo.go`
- `tests/fixtures/go-service/internal/service/service.go`
- `tests/fixtures/java-project/src/main/java/com/example/App.java`
- `tests/fixtures/java-project/src/main/java/com/example/data/UserRepo.java`
- `tests/fixtures/java-project/src/main/java/com/example/service/AuthService.java`
- `tests/fixtures/java-project/src/test/java/com/example/AppTest.java`
- `tests/fixtures/mixed-project/backend/main.go`
- `tests/fixtures/mixed-project/scripts/deploy.py`
- `tests/fixtures/python-package/src/__init__.py`
- `tests/fixtures/python-package/src/utils/__init__.py`
- `tests/fixtures/python-package/tests/conftest.py`
- `tests/fixtures/python-package/tests/test_auth.py`
- `tests/fixtures/rust-crate/tests/integration.rs`
- `tests/fixtures/typescript-app/src/__tests__/login.test.ts`
- `tests/graph_tests.rs`
- `tests/helpers.rs`
- `tests/invariants.rs`
- `tests/mcp_tests.rs`
- `tests/pipeline_tests.rs`
- `tests/properties.rs`
- `tests/symbol_tests.rs`

