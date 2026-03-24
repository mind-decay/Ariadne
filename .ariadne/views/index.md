# Project Index

## Architecture Summary

- **Files:** 177
- **Edges:** 253
- **Clusters:** 15
- **Max depth:** 10
- **Avg in-degree:** 1.3220
- **Avg out-degree:** 1.3220

## Clusters

| Cluster | Files | Key File | Cohesion |
|---------|------:|----------|--------:|
| algo | 12 | `src/algo/mod.rs` | 0.4091 |
| analysis | 4 | `src/analysis/diff.rs` | 0.3158 |
| benches | 6 | `benches/parser_bench.rs` | 0.0000 |
| cluster | 1 | `src/cluster/mod.rs` | 0.0000 |
| design | 32 | `design/testing.md` | 1.0000 |
| detect | 5 | `src/detect/mod.rs` | 0.2857 |
| docs | 1 | `docs/superpowers/plans/2026-03-19-architecture-review-fixes.md` | 1.0000 |
| mcp | 6 | `src/mcp/state.rs` | 0.3438 |
| model | 11 | `src/model/mod.rs` | 0.1370 |
| parser | 11 | `src/parser/mod.rs` | 0.4348 |
| pipeline | 5 | `src/pipeline/mod.rs` | 0.1250 |
| root | 6 | `src/diagnostic.rs` | 0.0769 |
| serial | 3 | `src/serial/mod.rs` | 0.1111 |
| tests | 70 | `tests/properties.rs` | 0.7234 |
| views | 4 | `src/views/mod.rs` | 0.3333 |

## Circular Dependencies

1. 8 files: src/algo/blast_radius.rs → src/algo/centrality.rs → src/algo/mod.rs → src/algo/pagerank.rs → src/algo/scc.rs → src/algo/spectral.rs → src/algo/stats.rs → src/algo/topo_sort.rs
2. 2 files: tests/fixtures/edge-cases/circular-a.ts → tests/fixtures/edge-cases/circular-b.ts

## Orphan Files

- `benches/helpers.rs`
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

