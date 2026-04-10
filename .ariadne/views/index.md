# Project Index

## Architecture Summary

- **Files:** 378
- **Edges:** 442
- **Clusters:** 22
- **Max depth:** 11
- **Avg in-degree:** 1.1534
- **Avg out-degree:** 1.1534

## Clusters

| Cluster | Files | Key File | Cohesion |
|---------|------:|----------|--------:|
| .claude | 8 | `.claude/settings.local.json` | 1.0000 |
| .github | 2 | `.github/workflows/release.yml` | 1.0000 |
| .moira | 31 | `.moira/project/rules/stack.yaml` | 1.0000 |
| algo | 17 | `src/algo/mod.rs` | 0.4267 |
| analysis | 4 | `src/analysis/smells.rs` | 0.3333 |
| ariadne-cli | 1 | `ariadne-cli/src/main.rs` | 1.0000 |
| benches | 7 | `benches/symbol_bench.rs` | 1.0000 |
| cluster | 1 | `src/cluster/mod.rs` | 0.0000 |
| conventions | 6 | `src/conventions/types.rs` | 0.6429 |
| design | 40 | `design/testing.md` | 1.0000 |
| detect | 7 | `src/detect/mod.rs` | 0.3750 |
| mcp | 16 | `src/mcp/state.rs` | 0.4923 |
| model | 17 | `src/model/mod.rs` | 0.1316 |
| parser | 22 | `src/parser/mod.rs` | 0.5106 |
| pipeline | 5 | `src/pipeline/mod.rs` | 0.1250 |
| recommend | 7 | `src/recommend/mod.rs` | 0.3333 |
| root | 6 | `src/diagnostic.rs` | 0.0526 |
| semantic | 6 | `src/semantic/mod.rs` | 0.3750 |
| serial | 3 | `src/serial/mod.rs` | 0.1333 |
| temporal | 6 | `src/temporal/ownership.rs` | 0.2632 |
| tests | 162 | `tests/temporal_integration.rs` | 1.0000 |
| views | 4 | `src/views/mod.rs` | 0.3333 |

## Circular Dependencies

1. 12 files: src/algo/blast_radius.rs → src/algo/centrality.rs → src/algo/context.rs → src/algo/impact.rs → src/algo/mod.rs → src/algo/pagerank.rs → src/algo/reading_order.rs → src/algo/scc.rs → src/algo/spectral.rs → src/algo/stats.rs → src/algo/test_map.rs → src/algo/topo_sort.rs
2. 9 files: src/parser/config/mod.rs → src/parser/config/tsconfig.rs → src/parser/csharp.rs → src/parser/go.rs → src/parser/java.rs → src/parser/mod.rs → src/parser/python.rs → src/parser/registry.rs → src/parser/typescript.rs
3. 5 files: src/semantic/dotnet.rs → src/semantic/events.rs → src/semantic/http.rs → src/semantic/java.rs → src/semantic/mod.rs
4. 2 files: tests/fixtures/edge-cases/circular-a.ts → tests/fixtures/edge-cases/circular-b.ts

## Orphan Files

- `ariadne-cli/src/main.rs`
- `benches/algo_bench.rs`
- `benches/analysis_bench.rs`
- `benches/build_bench.rs`
- `benches/helpers.rs`
- `benches/mcp_bench.rs`
- `benches/parser_bench.rs`
- `benches/symbol_bench.rs`
- `tests/callgraph_tests.rs`
- `tests/config_resolution_tests.rs`
- `tests/constraints.rs`
- `tests/dotnet_integration_tests.rs`
- `tests/fixtures/csharp-project/Data/UserRepository.cs`
- `tests/fixtures/csharp-project/Program.cs`
- `tests/fixtures/csharp-project/Services/AuthService.cs`
- `tests/fixtures/csharp-project/Tests/AuthTests.cs`
- `tests/fixtures/dotnet-blazor/BlazorApp/Data/WeatherService.cs`
- `tests/fixtures/dotnet-blazor/BlazorApp/Pages/Counter.razor`
- `tests/fixtures/dotnet-blazor/BlazorApp/Pages/Index.razor`
- `tests/fixtures/dotnet-blazor/BlazorApp/Shared/MainLayout.razor`
- `tests/fixtures/dotnet-efcore/DataApp/Data/AppDbContext.cs`
- `tests/fixtures/dotnet-efcore/DataApp/Migrations/20240101_Init.cs`
- `tests/fixtures/dotnet-efcore/DataApp/Models/Order.cs`
- `tests/fixtures/dotnet-efcore/DataApp/Models/User.cs`
- `tests/fixtures/dotnet-maui/MauiApp/Pages/MainPage.cs`
- `tests/fixtures/dotnet-maui/MauiApp/Pages/SettingsPage.cs`
- `tests/fixtures/dotnet-maui/MauiApp/Services/ISettingsService.cs`
- `tests/fixtures/dotnet-maui/MauiApp/ViewModels/MainViewModel.cs`
- `tests/fixtures/dotnet-webapi/MyApi/Controllers/WeatherController.cs`
- `tests/fixtures/dotnet-webapi/MyApi/Models/WeatherForecast.cs`
- `tests/fixtures/dotnet-webapi/MyApi/Program.cs`
- `tests/fixtures/dotnet-webapi/MyApi/Services/IWeatherService.cs`
- `tests/fixtures/dotnet-webapi/MyApi/Services/WeatherService.cs`
- `tests/fixtures/edge-cases/deeply/nested/a/b/c/d/file.ts`
- `tests/fixtures/edge-cases/empty.ts`
- `tests/fixtures/edge-cases/helper.ts`
- `tests/fixtures/go-service/cmd/server/main.go`
- `tests/fixtures/go-service/internal/handler/handler.go`
- `tests/fixtures/go-service/internal/repository/repo.go`
- `tests/fixtures/go-service/internal/service/service.go`
- `tests/fixtures/gomod_project/internal/auth/auth.go`
- `tests/fixtures/gomod_project/main.go`
- `tests/fixtures/jakarta-ee/src/main/java/com/example/UserService.java`
- `tests/fixtures/java-project/src/main/java/com/example/App.java`
- `tests/fixtures/java-project/src/main/java/com/example/data/UserRepo.java`
- `tests/fixtures/java-project/src/main/java/com/example/service/AuthService.java`
- `tests/fixtures/java-project/src/test/java/com/example/AppTest.java`
- `tests/fixtures/maven-project/src/main/java/com/example/App.java`
- `tests/fixtures/maven-project/src/main/java/com/example/data/UserRepo.java`
- `tests/fixtures/maven-project/src/main/java/com/example/service/UserService.java`
- `tests/fixtures/mixed-project/backend/main.go`
- `tests/fixtures/mixed-project/scripts/deploy.py`
- `tests/fixtures/python-package/src/__init__.py`
- `tests/fixtures/python-package/src/utils/__init__.py`
- `tests/fixtures/python-package/tests/conftest.py`
- `tests/fixtures/python-package/tests/test_auth.py`
- `tests/fixtures/python_src_layout/src/mypackage/__init__.py`
- `tests/fixtures/python_src_layout/src/mypackage/main.py`
- `tests/fixtures/python_src_layout/src/mypackage/utils.py`
- `tests/fixtures/rust-crate/tests/integration.rs`
- `tests/fixtures/semantic/aspnet_routes.cs`
- `tests/fixtures/semantic/dom_events.ts`
- `tests/fixtures/semantic/event_emitters.ts`
- `tests/fixtures/semantic/event_generic.py`
- `tests/fixtures/semantic/express_routes.ts`
- `tests/fixtures/semantic/fastapi_routes.py`
- `tests/fixtures/semantic/go_routes.go`
- `tests/fixtures/semantic/mixed_framework.ts`
- `tests/fixtures/semantic/no_boundaries.rs`
- `tests/fixtures/semantic/spring_routes.java`
- `tests/fixtures/typescript-app/src/__tests__/login.test.ts`
- `tests/graph_tests.rs`
- `tests/helpers.rs`
- `tests/integration_java_phase12.rs`
- `tests/invariants.rs`
- `tests/mcp_tests.rs`
- `tests/pipeline_tests.rs`
- `tests/properties.rs`
- `tests/semantic_tests.rs`
- `tests/symbol_tests.rs`
- `tests/temporal_integration.rs`

