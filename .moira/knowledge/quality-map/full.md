<!-- moira:freshness ariadne-init 2026-04-04 -->
<!-- moira:mode conform -->

# Quality Map

## Problematic

### CircularDependency: src/algo/blast_radius.rs
- **Category**: CircularDependency
- **Evidence**: ariadne structural analysis
- **File(s)**: src/algo/blast_radius.rs, src/algo/centrality.rs, src/algo/context.rs, src/algo/impact.rs, src/algo/mod.rs, src/algo/pagerank.rs, src/algo/reading_order.rs, src/algo/scc.rs, src/algo/spectral.rs, src/algo/stats.rs, src/algo/test_map.rs, src/algo/topo_sort.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### CircularDependency: src/parser/config/mod.rs
- **Category**: CircularDependency
- **Evidence**: ariadne structural analysis
- **File(s)**: src/parser/config/mod.rs, src/parser/config/tsconfig.rs, src/parser/go.rs, src/parser/mod.rs, src/parser/python.rs, src/parser/registry.rs, src/parser/typescript.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### CircularDependency: src/semantic/events.rs
- **Category**: CircularDependency
- **Evidence**: ariadne structural analysis
- **File(s)**: src/semantic/events.rs, src/semantic/http.rs, src/semantic/mod.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### CircularDependency: tests/fixtures/edge-cases/circular-a.ts
- **Category**: CircularDependency
- **Evidence**: ariadne structural analysis
- **File(s)**: tests/fixtures/edge-cases/circular-a.ts, tests/fixtures/edge-cases/circular-b.ts
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### DeadCluster: .claude/CLAUDE.md
- **Category**: DeadCluster
- **Evidence**: ariadne structural analysis
- **File(s)**: .claude/CLAUDE.md, .claude/commands/audit-docs.md, .claude/commands/review-architecture.md, .claude/commands/review-plan.md, .claude/commands/review-spec.md, .claude/commands/write-spec.md, .claude/settings.local.json
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### DeadCluster: .github/workflows/ci.yml
- **Category**: DeadCluster
- **Evidence**: ariadne structural analysis
- **File(s)**: .github/workflows/ci.yml, .github/workflows/release.yml
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### DeadCluster: .moira/config.yaml
- **Category**: DeadCluster
- **Evidence**: ariadne structural analysis
- **File(s)**: .moira/config.yaml, .moira/config/budgets.yaml, .moira/config/mcp-registry.yaml, .moira/knowledge/conventions/full.md, .moira/knowledge/conventions/index.md, .moira/knowledge/conventions/summary.md, .moira/knowledge/decisions/full.md, .moira/knowledge/decisions/index.md, .moira/knowledge/decisions/summary.md, .moira/knowledge/failures/full.md, .moira/knowledge/failures/index.md, .moira/knowledge/failures/summary.md, .moira/knowledge/libraries/index.md, .moira/knowledge/libraries/summary.md, .moira/knowledge/patterns/full.md, .moira/knowledge/patterns/index.md, .moira/knowledge/patterns/summary.md, .moira/knowledge/project-model/full.md, .moira/knowledge/project-model/index.md, .moira/knowledge/project-model/summary.md, .moira/knowledge/quality-map/full.md, .moira/knowledge/quality-map/index.md, .moira/knowledge/quality-map/summary.md, .moira/project/rules/boundaries.yaml, .moira/project/rules/conventions.yaml, .moira/project/rules/patterns.yaml, .moira/project/rules/stack.yaml, .moira/state/graph-snapshot.json, .moira/state/init/ariadne-context.md, .moira/state/init/convention-scan.md, .moira/state/init/mcp-scan.md, .moira/state/init/pattern-scan.md, .moira/state/init/raw-configs.md, .moira/state/init/raw-structure.md, .moira/state/init/rules-audit.md, .moira/state/init/structure-scan.md, .moira/state/init/tech-scan.md
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### DeadCluster: benches/algo_bench.rs
- **Category**: DeadCluster
- **Evidence**: ariadne structural analysis
- **File(s)**: benches/algo_bench.rs, benches/analysis_bench.rs, benches/build_bench.rs, benches/helpers.rs, benches/mcp_bench.rs, benches/parser_bench.rs, benches/symbol_bench.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### DeadCluster: design/ROADMAP.md
- **Category**: DeadCluster
- **Evidence**: ariadne structural analysis
- **File(s)**: design/ROADMAP.md, design/architecture.md, design/decisions/log.md, design/determinism.md, design/distribution.md, design/error-handling.md, design/path-resolution.md, design/performance.md, design/reports/archive/2026-03-17-doc-audit.md, design/reports/archive/2026-03-18-architecture-review.md, design/reports/archive/2026-03-18-v2-architecture-review.md, design/reports/archive/2026-03-19-architecture-review.md, design/reports/archive/2026-03-19-v2-architecture-review.md, design/reports/archive/2026-03-22-architectural-review.md, design/reports/archive/2026-03-23-moira-violations.md, design/reports/archive/2026-03-25-phase4d-formal-methods-evaluation.md, design/specs/2026-03-29-phase9-implementation-plan.md, design/specs/2026-03-29-phase9-recommendations.md, design/specs/archive/2026-03-17-phase1-core-cli.md, design/specs/archive/2026-03-17-phase1-implementation-plan.md, design/specs/archive/2026-03-17-phase1a-implementation-plan.md, design/specs/archive/2026-03-17-phase1a-mvp.md, design/specs/archive/2026-03-18-phase1b-hardening.md, design/specs/archive/2026-03-18-phase1b-implementation-plan.md, design/specs/archive/2026-03-18-phase2-algorithms-queries-views.md, design/specs/archive/2026-03-18-phase2a-implementation-plan.md, design/specs/archive/2026-03-18-review-fixes-plan.md, design/specs/archive/2026-03-19-phase2b-implementation-plan.md, design/specs/archive/2026-03-19-phase3-mcp-server-architectural-intelligence.md, design/specs/archive/2026-03-19-phase3a-implementation-plan.md, design/specs/archive/2026-03-19-phase3a-mcp-server.md, design/specs/archive/2026-03-19-phase3b-architectural-intelligence.md, design/specs/archive/2026-03-19-phase3b-implementation-plan.md, design/specs/archive/2026-03-19-phase3c-advanced-graph-analytics.md, design/specs/archive/2026-03-19-phase3c-implementation-plan.md, design/specs/archive/2026-03-25-phase7-git-temporal-analysis.md, design/specs/archive/2026-03-25-phase7-implementation-plan.md, design/testing.md
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### DeadCluster: tests/callgraph_tests.rs
- **Category**: DeadCluster
- **Evidence**: ariadne structural analysis
- **File(s)**: tests/callgraph_tests.rs, tests/config_resolution_tests.rs, tests/fixtures/csharp-project/Data/UserRepository.cs, tests/fixtures/csharp-project/Program.cs, tests/fixtures/csharp-project/Services/AuthService.cs, tests/fixtures/csharp-project/Tests/AuthTests.cs, tests/fixtures/data-files/malformed.json, tests/fixtures/data-files/malformed.yaml, tests/fixtures/data-files/sample.json, tests/fixtures/data-files/sample.yaml, tests/fixtures/data-files/sample.yml, tests/fixtures/edge-cases/.ariadne/graph/clusters.json, tests/fixtures/edge-cases/.ariadne/graph/graph.json, tests/fixtures/edge-cases/circular-a.ts, tests/fixtures/edge-cases/circular-b.ts, tests/fixtures/edge-cases/deeply/nested/a/b/c/d/file.ts, tests/fixtures/edge-cases/empty.ts, tests/fixtures/edge-cases/graph/clusters.json, tests/fixtures/edge-cases/graph/graph.json, tests/fixtures/edge-cases/helper.ts, tests/fixtures/edge-cases/syntax-error.ts, tests/fixtures/edge-cases/valid.ts, tests/fixtures/go-service/.ariadne/graph/clusters.json, tests/fixtures/go-service/.ariadne/graph/graph.json, tests/fixtures/go-service/cmd/server/main.go, tests/fixtures/go-service/internal/handler/handler.go, tests/fixtures/go-service/internal/repository/repo.go, tests/fixtures/go-service/internal/service/service.go, tests/fixtures/gomod_project/internal/auth/auth.go, tests/fixtures/gomod_project/main.go, tests/fixtures/java-project/src/main/java/com/example/App.java, tests/fixtures/java-project/src/main/java/com/example/data/UserRepo.java, tests/fixtures/java-project/src/main/java/com/example/service/AuthService.java, tests/fixtures/java-project/src/test/java/com/example/AppTest.java, tests/fixtures/markdown-docs/README.md, tests/fixtures/markdown-docs/docs/api.md, tests/fixtures/markdown-docs/docs/guide.md, tests/fixtures/mixed-project/.ariadne/graph/clusters.json, tests/fixtures/mixed-project/.ariadne/graph/graph.json, tests/fixtures/mixed-project/README.md, tests/fixtures/mixed-project/backend/main.go, tests/fixtures/mixed-project/frontend/package.json, tests/fixtures/mixed-project/frontend/src/App.tsx, tests/fixtures/mixed-project/frontend/src/components/Button.tsx, tests/fixtures/mixed-project/scripts/deploy.py, tests/fixtures/python-package/.ariadne/graph/clusters.json, tests/fixtures/python-package/.ariadne/graph/graph.json, tests/fixtures/python-package/src/__init__.py, tests/fixtures/python-package/src/auth/__init__.py, tests/fixtures/python-package/src/auth/login.py, tests/fixtures/python-package/src/main.py, tests/fixtures/python-package/src/utils/__init__.py, tests/fixtures/python-package/src/utils/helpers.py, tests/fixtures/python-package/tests/conftest.py, tests/fixtures/python-package/tests/test_auth.py, tests/fixtures/python_src_layout/src/mypackage/__init__.py, tests/fixtures/python_src_layout/src/mypackage/main.py, tests/fixtures/python_src_layout/src/mypackage/utils.py, tests/fixtures/rust-crate/.ariadne/graph/clusters.json, tests/fixtures/rust-crate/.ariadne/graph/graph.json, tests/fixtures/rust-crate/src/auth/login.rs, tests/fixtures/rust-crate/src/auth/mod.rs, tests/fixtures/rust-crate/src/lib.rs, tests/fixtures/rust-crate/src/utils/format.rs, tests/fixtures/rust-crate/src/utils/mod.rs, tests/fixtures/rust-crate/tests/integration.rs, tests/fixtures/semantic/aspnet_routes.cs, tests/fixtures/semantic/dom_events.ts, tests/fixtures/semantic/event_emitters.ts, tests/fixtures/semantic/event_generic.py, tests/fixtures/semantic/express_routes.ts, tests/fixtures/semantic/fastapi_routes.py, tests/fixtures/semantic/go_routes.go, tests/fixtures/semantic/mixed_framework.ts, tests/fixtures/semantic/no_boundaries.rs, tests/fixtures/semantic/spring_routes.java, tests/fixtures/tsconfig_extends/lib/core.ts, tests/fixtures/tsconfig_extends/src/main.ts, tests/fixtures/tsconfig_extends/src/service.ts, tests/fixtures/tsconfig_extends/tsconfig.base.json, tests/fixtures/tsconfig_extends/tsconfig.json, tests/fixtures/tsconfig_project/src/app.ts, tests/fixtures/tsconfig_project/src/components/Button.ts, tests/fixtures/tsconfig_project/src/index.ts, tests/fixtures/tsconfig_project/src/shared/lib/utils.ts, tests/fixtures/tsconfig_project/tsconfig.json, tests/fixtures/tsx-components/App.tsx, tests/fixtures/tsx-components/components/Callback.tsx, tests/fixtures/tsx-components/components/Card.tsx, tests/fixtures/tsx-components/components/Conditional.tsx, tests/fixtures/tsx-components/components/DefaultAnon.tsx, tests/fixtures/tsx-components/components/Fragment.tsx, tests/fixtures/tsx-components/components/GenericBox.tsx, tests/fixtures/tsx-components/components/Header.tsx, tests/fixtures/tsx-components/components/LegacyButton.jsx, tests/fixtures/tsx-components/components/SpreadProps.tsx, tests/fixtures/tsx-components/components/StyledBox.tsx, tests/fixtures/typescript-app/.ariadne/graph/clusters.json, tests/fixtures/typescript-app/.ariadne/graph/graph.json, tests/fixtures/typescript-app/.ariadne/graph/raw_imports.json, tests/fixtures/typescript-app/.ariadne/graph/stats.json, tests/fixtures/typescript-app/package.json, tests/fixtures/typescript-app/src/__tests__/login.test.ts, tests/fixtures/typescript-app/src/auth/login.ts, tests/fixtures/typescript-app/src/auth/register.ts, tests/fixtures/typescript-app/src/index.ts, tests/fixtures/typescript-app/src/types/index.d.ts, tests/fixtures/typescript-app/src/utils/format.ts, tests/fixtures/typescript-app/tsconfig.json, tests/fixtures/workspace-project/.ariadne/graph/clusters.json, tests/fixtures/workspace-project/.ariadne/graph/graph.json, tests/fixtures/workspace-project/package.json, tests/fixtures/workspace-project/packages/api/package.json, tests/fixtures/workspace-project/packages/api/src/index.ts, tests/fixtures/workspace-project/packages/api/src/router.ts, tests/fixtures/workspace-project/packages/auth/package.json, tests/fixtures/workspace-project/packages/auth/src/index.ts, tests/fixtures/workspace-project/packages/auth/src/login.ts, tests/fixtures/workspace-project/packages/shared/package.json, tests/fixtures/workspace-project/packages/shared/src/format.ts, tests/fixtures/workspace-project/packages/shared/src/index.ts, tests/graph_tests.rs, tests/helpers.rs, tests/invariants.rs, tests/mcp_tests.rs, tests/pipeline_tests.rs, tests/properties.rs, tests/semantic_tests.rs, tests/symbol_tests.rs, tests/temporal_integration.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### HubAndSpoke: src/diagnostic.rs
- **Category**: HubAndSpoke
- **Evidence**: ariadne structural analysis
- **File(s)**: src/diagnostic.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### HubAndSpoke: src/model/mod.rs
- **Category**: HubAndSpoke
- **Evidence**: ariadne structural analysis
- **File(s)**: src/model/mod.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### HubAndSpoke: src/serial/mod.rs
- **Category**: HubAndSpoke
- **Evidence**: ariadne structural analysis
- **File(s)**: src/serial/mod.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Circular dependency: src/algo/blast_radius.rs, src/algo/centrality.rs, src/algo/context.rs, src/algo/impact.rs, src/algo/mod.rs, src/algo/pagerank.rs, src/algo/reading_order.rs, src/algo/scc.rs, src/algo/spectral.rs, src/algo/stats.rs, src/algo/test_map.rs, src/algo/topo_sort.rs
- **Category**: circular dependency
- **Evidence**: ariadne structural analysis
- **File(s)**: src/algo/blast_radius.rs, src/algo/centrality.rs, src/algo/context.rs, src/algo/impact.rs, src/algo/mod.rs, src/algo/pagerank.rs, src/algo/reading_order.rs, src/algo/scc.rs, src/algo/spectral.rs, src/algo/stats.rs, src/algo/test_map.rs, src/algo/topo_sort.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Circular dependency: src/parser/config/mod.rs, src/parser/config/tsconfig.rs, src/parser/go.rs, src/parser/mod.rs, src/parser/python.rs, src/parser/registry.rs, src/parser/typescript.rs
- **Category**: circular dependency
- **Evidence**: ariadne structural analysis
- **File(s)**: src/parser/config/mod.rs, src/parser/config/tsconfig.rs, src/parser/go.rs, src/parser/mod.rs, src/parser/python.rs, src/parser/registry.rs, src/parser/typescript.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Circular dependency: src/semantic/events.rs, src/semantic/http.rs, src/semantic/mod.rs
- **Category**: circular dependency
- **Evidence**: ariadne structural analysis
- **File(s)**: src/semantic/events.rs, src/semantic/http.rs, src/semantic/mod.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Circular dependency: tests/fixtures/edge-cases/circular-a.ts, tests/fixtures/edge-cases/circular-b.ts
- **Category**: circular dependency
- **Evidence**: ariadne structural analysis
- **File(s)**: tests/fixtures/edge-cases/circular-a.ts, tests/fixtures/edge-cases/circular-b.ts
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Hotspot: src/parser/typescript.rs
- **Category**: churn hotspot
- **Evidence**: ariadne temporal analysis
- **File(s)**: src/parser/typescript.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Hotspot: src/parser/rust_lang.rs
- **Category**: churn hotspot
- **Evidence**: ariadne temporal analysis
- **File(s)**: src/parser/rust_lang.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Hotspot: src/diagnostic.rs
- **Category**: churn hotspot
- **Evidence**: ariadne temporal analysis
- **File(s)**: src/diagnostic.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Hotspot: src/algo/louvain.rs
- **Category**: churn hotspot
- **Evidence**: ariadne temporal analysis
- **File(s)**: src/algo/louvain.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Hotspot: src/parser/python.rs
- **Category**: churn hotspot
- **Evidence**: ariadne temporal analysis
- **File(s)**: src/parser/python.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Hotspot: src/analysis/smells.rs
- **Category**: churn hotspot
- **Evidence**: ariadne temporal analysis
- **File(s)**: src/analysis/smells.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Hotspot: src/mcp/tools.rs
- **Category**: churn hotspot
- **Evidence**: ariadne temporal analysis
- **File(s)**: src/mcp/tools.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Hotspot: src/parser/go.rs
- **Category**: churn hotspot
- **Evidence**: ariadne temporal analysis
- **File(s)**: src/parser/go.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Hotspot: src/pipeline/mod.rs
- **Category**: churn hotspot
- **Evidence**: ariadne temporal analysis
- **File(s)**: src/pipeline/mod.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Hotspot: src/parser/csharp.rs
- **Category**: churn hotspot
- **Evidence**: ariadne temporal analysis
- **File(s)**: src/parser/csharp.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Hotspot: src/parser/java.rs
- **Category**: churn hotspot
- **Evidence**: ariadne temporal analysis
- **File(s)**: src/parser/java.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Hotspot: src/parser/registry.rs
- **Category**: churn hotspot
- **Evidence**: ariadne temporal analysis
- **File(s)**: src/parser/registry.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Hotspot: src/model/types.rs
- **Category**: churn hotspot
- **Evidence**: ariadne temporal analysis
- **File(s)**: src/model/types.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Hotspot: src/algo/compress.rs
- **Category**: churn hotspot
- **Evidence**: ariadne temporal analysis
- **File(s)**: src/algo/compress.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Hotspot: src/detect/layer.rs
- **Category**: churn hotspot
- **Evidence**: ariadne temporal analysis
- **File(s)**: src/detect/layer.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Hotspot: src/analysis/diff.rs
- **Category**: churn hotspot
- **Evidence**: ariadne temporal analysis
- **File(s)**: src/analysis/diff.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Hotspot: src/algo/spectral.rs
- **Category**: churn hotspot
- **Evidence**: ariadne temporal analysis
- **File(s)**: src/algo/spectral.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Hotspot: src/analysis/metrics.rs
- **Category**: churn hotspot
- **Evidence**: ariadne temporal analysis
- **File(s)**: src/analysis/metrics.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Hotspot: src/pipeline/build.rs
- **Category**: churn hotspot
- **Evidence**: ariadne temporal analysis
- **File(s)**: src/pipeline/build.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Hotspot: src/algo/context.rs
- **Category**: churn hotspot
- **Evidence**: ariadne temporal analysis
- **File(s)**: src/algo/context.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

## Adequate

### Co-change coupling: .ariadne/graph/.lock <-> .ariadne/graph/raw_imports.json
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 1.0)
- **File(s)**: .ariadne/graph/.lock, .ariadne/graph/raw_imports.json
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: .ariadne/graph/clusters.json <-> .ariadne/graph/graph.json
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 1.0)
- **File(s)**: .ariadne/graph/clusters.json, .ariadne/graph/graph.json
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: .ariadne/views/clusters/algo.md <-> .ariadne/views/clusters/mcp.md
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 1.0)
- **File(s)**: .ariadne/views/clusters/algo.md, .ariadne/views/clusters/mcp.md
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: .ariadne/views/clusters/algo.md <-> .ariadne/views/clusters/model.md
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 1.0)
- **File(s)**: .ariadne/views/clusters/algo.md, .ariadne/views/clusters/model.md
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: .ariadne/views/clusters/algo.md <-> .ariadne/views/clusters/parser.md
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 1.0)
- **File(s)**: .ariadne/views/clusters/algo.md, .ariadne/views/clusters/parser.md
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: .ariadne/views/clusters/algo.md <-> .ariadne/views/clusters/root.md
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 1.0)
- **File(s)**: .ariadne/views/clusters/algo.md, .ariadne/views/clusters/root.md
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: .ariadne/views/clusters/algo.md <-> .ariadne/views/index.md
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 1.0)
- **File(s)**: .ariadne/views/clusters/algo.md, .ariadne/views/index.md
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: .ariadne/views/clusters/mcp.md <-> .ariadne/views/clusters/model.md
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 1.0)
- **File(s)**: .ariadne/views/clusters/mcp.md, .ariadne/views/clusters/model.md
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: .ariadne/views/clusters/mcp.md <-> .ariadne/views/clusters/parser.md
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 1.0)
- **File(s)**: .ariadne/views/clusters/mcp.md, .ariadne/views/clusters/parser.md
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: .ariadne/views/clusters/mcp.md <-> .ariadne/views/clusters/root.md
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 1.0)
- **File(s)**: .ariadne/views/clusters/mcp.md, .ariadne/views/clusters/root.md
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: .ariadne/views/clusters/mcp.md <-> .ariadne/views/index.md
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 1.0)
- **File(s)**: .ariadne/views/clusters/mcp.md, .ariadne/views/index.md
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: .ariadne/views/clusters/model.md <-> .ariadne/views/clusters/parser.md
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 1.0)
- **File(s)**: .ariadne/views/clusters/model.md, .ariadne/views/clusters/parser.md
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: .ariadne/views/clusters/model.md <-> .ariadne/views/clusters/root.md
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 1.0)
- **File(s)**: .ariadne/views/clusters/model.md, .ariadne/views/clusters/root.md
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: .ariadne/views/clusters/model.md <-> .ariadne/views/index.md
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 1.0)
- **File(s)**: .ariadne/views/clusters/model.md, .ariadne/views/index.md
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: .ariadne/views/clusters/parser.md <-> .ariadne/views/clusters/root.md
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 1.0)
- **File(s)**: .ariadne/views/clusters/parser.md, .ariadne/views/clusters/root.md
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: .ariadne/views/clusters/parser.md <-> .ariadne/views/index.md
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 1.0)
- **File(s)**: .ariadne/views/clusters/parser.md, .ariadne/views/index.md
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: .ariadne/views/clusters/root.md <-> .ariadne/views/index.md
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 1.0)
- **File(s)**: .ariadne/views/clusters/root.md, .ariadne/views/index.md
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: src/algo/blast_radius.rs <-> src/algo/topo_sort.rs
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 1.0)
- **File(s)**: src/algo/blast_radius.rs, src/algo/topo_sort.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: src/algo/centrality.rs <-> src/views/impact.rs
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 1.0)
- **File(s)**: src/algo/centrality.rs, src/views/impact.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: src/algo/compress.rs <-> src/algo/pagerank.rs
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 1.0)
- **File(s)**: src/algo/compress.rs, src/algo/pagerank.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: src/algo/compress.rs <-> src/algo/spectral.rs
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 1.0)
- **File(s)**: src/algo/compress.rs, src/algo/spectral.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: src/algo/pagerank.rs <-> src/algo/spectral.rs
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 1.0)
- **File(s)**: src/algo/pagerank.rs, src/algo/spectral.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: src/mcp/prompts.rs <-> src/mcp/resources.rs
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 1.0)
- **File(s)**: src/mcp/prompts.rs, src/mcp/resources.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: src/parser/csharp.rs <-> src/parser/java.rs
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 1.0)
- **File(s)**: src/parser/csharp.rs, src/parser/java.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: src/views/cluster.rs <-> src/views/index.rs
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 1.0)
- **File(s)**: src/views/cluster.rs, src/views/index.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: src/parser/csharp.rs <-> src/parser/go.rs
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.875)
- **File(s)**: src/parser/csharp.rs, src/parser/go.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: src/parser/go.rs <-> src/parser/java.rs
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.875)
- **File(s)**: src/parser/go.rs, src/parser/java.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: src/parser/go.rs <-> src/parser/python.rs
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.875)
- **File(s)**: src/parser/go.rs, src/parser/python.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: src/algo/centrality.rs <-> src/views/cluster.rs
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.8571)
- **File(s)**: src/algo/centrality.rs, src/views/cluster.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: src/algo/centrality.rs <-> src/views/index.rs
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.8571)
- **File(s)**: src/algo/centrality.rs, src/views/index.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: src/views/cluster.rs <-> src/views/impact.rs
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.8571)
- **File(s)**: src/views/cluster.rs, src/views/impact.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: src/views/impact.rs <-> src/views/index.rs
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.8571)
- **File(s)**: src/views/impact.rs, src/views/index.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: src/algo/centrality.rs <-> src/algo/scc.rs
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.8333)
- **File(s)**: src/algo/centrality.rs, src/algo/scc.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: src/algo/scc.rs <-> src/views/impact.rs
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.8333)
- **File(s)**: src/algo/scc.rs, src/views/impact.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: .ariadne/views/clusters/algo.md <-> .ariadne/views/clusters/analysis.md
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.8)
- **File(s)**: .ariadne/views/clusters/algo.md, .ariadne/views/clusters/analysis.md
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: .ariadne/views/clusters/algo.md <-> .ariadne/views/clusters/design.md
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.8)
- **File(s)**: .ariadne/views/clusters/algo.md, .ariadne/views/clusters/design.md
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: .ariadne/views/clusters/algo.md <-> .ariadne/views/clusters/pipeline.md
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.8)
- **File(s)**: .ariadne/views/clusters/algo.md, .ariadne/views/clusters/pipeline.md
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: .ariadne/views/clusters/analysis.md <-> .ariadne/views/clusters/mcp.md
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.8)
- **File(s)**: .ariadne/views/clusters/analysis.md, .ariadne/views/clusters/mcp.md
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: .ariadne/views/clusters/analysis.md <-> .ariadne/views/clusters/model.md
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.8)
- **File(s)**: .ariadne/views/clusters/analysis.md, .ariadne/views/clusters/model.md
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: .ariadne/views/clusters/analysis.md <-> .ariadne/views/clusters/parser.md
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.8)
- **File(s)**: .ariadne/views/clusters/analysis.md, .ariadne/views/clusters/parser.md
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: .ariadne/views/clusters/analysis.md <-> .ariadne/views/clusters/root.md
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.8)
- **File(s)**: .ariadne/views/clusters/analysis.md, .ariadne/views/clusters/root.md
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: .ariadne/views/clusters/analysis.md <-> .ariadne/views/index.md
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.8)
- **File(s)**: .ariadne/views/clusters/analysis.md, .ariadne/views/index.md
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: .ariadne/views/clusters/design.md <-> .ariadne/views/clusters/mcp.md
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.8)
- **File(s)**: .ariadne/views/clusters/design.md, .ariadne/views/clusters/mcp.md
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: .ariadne/views/clusters/design.md <-> .ariadne/views/clusters/model.md
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.8)
- **File(s)**: .ariadne/views/clusters/design.md, .ariadne/views/clusters/model.md
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: .ariadne/views/clusters/design.md <-> .ariadne/views/clusters/parser.md
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.8)
- **File(s)**: .ariadne/views/clusters/design.md, .ariadne/views/clusters/parser.md
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: .ariadne/views/clusters/design.md <-> .ariadne/views/clusters/root.md
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.8)
- **File(s)**: .ariadne/views/clusters/design.md, .ariadne/views/clusters/root.md
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: .ariadne/views/clusters/design.md <-> .ariadne/views/index.md
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.8)
- **File(s)**: .ariadne/views/clusters/design.md, .ariadne/views/index.md
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: .ariadne/views/clusters/mcp.md <-> .ariadne/views/clusters/pipeline.md
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.8)
- **File(s)**: .ariadne/views/clusters/mcp.md, .ariadne/views/clusters/pipeline.md
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: .ariadne/views/clusters/model.md <-> .ariadne/views/clusters/pipeline.md
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.8)
- **File(s)**: .ariadne/views/clusters/model.md, .ariadne/views/clusters/pipeline.md
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: .ariadne/views/clusters/parser.md <-> .ariadne/views/clusters/pipeline.md
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.8)
- **File(s)**: .ariadne/views/clusters/parser.md, .ariadne/views/clusters/pipeline.md
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: .ariadne/views/clusters/pipeline.md <-> .ariadne/views/clusters/root.md
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.8)
- **File(s)**: .ariadne/views/clusters/pipeline.md, .ariadne/views/clusters/root.md
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: .ariadne/views/clusters/pipeline.md <-> .ariadne/views/index.md
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.8)
- **File(s)**: .ariadne/views/clusters/pipeline.md, .ariadne/views/index.md
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: src/algo/blast_radius.rs <-> src/algo/scc.rs
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.8)
- **File(s)**: src/algo/blast_radius.rs, src/algo/scc.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: src/algo/scc.rs <-> src/algo/subgraph.rs
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.8)
- **File(s)**: src/algo/scc.rs, src/algo/subgraph.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: src/algo/scc.rs <-> src/algo/topo_sort.rs
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.8)
- **File(s)**: src/algo/scc.rs, src/algo/topo_sort.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: src/parser/python.rs <-> src/parser/typescript.rs
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.7778)
- **File(s)**: src/parser/python.rs, src/parser/typescript.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: .ariadne/views/clusters/analysis.md <-> .ariadne/views/clusters/serial.md
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.75)
- **File(s)**: .ariadne/views/clusters/analysis.md, .ariadne/views/clusters/serial.md
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: .ariadne/views/clusters/design.md <-> .ariadne/views/clusters/tests.md
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.75)
- **File(s)**: .ariadne/views/clusters/design.md, .ariadne/views/clusters/tests.md
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: .ariadne/views/clusters/pipeline.md <-> .ariadne/views/clusters/serial.md
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.75)
- **File(s)**: .ariadne/views/clusters/pipeline.md, .ariadne/views/clusters/serial.md
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: .ariadne/views/clusters/pipeline.md <-> .ariadne/views/clusters/tests.md
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.75)
- **File(s)**: .ariadne/views/clusters/pipeline.md, .ariadne/views/clusters/tests.md
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: src/parser/csharp.rs <-> src/parser/python.rs
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.75)
- **File(s)**: src/parser/csharp.rs, src/parser/python.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: src/parser/java.rs <-> src/parser/python.rs
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.75)
- **File(s)**: src/parser/java.rs, src/parser/python.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: src/algo/scc.rs <-> src/views/cluster.rs
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.7143)
- **File(s)**: src/algo/scc.rs, src/views/cluster.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: src/algo/scc.rs <-> src/views/index.rs
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.7143)
- **File(s)**: src/algo/scc.rs, src/views/index.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: src/parser/go.rs <-> src/parser/typescript.rs
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.7)
- **File(s)**: src/parser/go.rs, src/parser/typescript.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: benches/algo_bench.rs <-> src/algo/blast_radius.rs
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.6667)
- **File(s)**: benches/algo_bench.rs, src/algo/blast_radius.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: benches/algo_bench.rs <-> src/algo/topo_sort.rs
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.6667)
- **File(s)**: benches/algo_bench.rs, src/algo/topo_sort.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: src/algo/blast_radius.rs <-> src/algo/centrality.rs
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.6667)
- **File(s)**: src/algo/blast_radius.rs, src/algo/centrality.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: src/algo/blast_radius.rs <-> src/views/impact.rs
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.6667)
- **File(s)**: src/algo/blast_radius.rs, src/views/impact.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: src/algo/centrality.rs <-> src/algo/delta.rs
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.6667)
- **File(s)**: src/algo/centrality.rs, src/algo/delta.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: src/algo/centrality.rs <-> src/algo/subgraph.rs
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.6667)
- **File(s)**: src/algo/centrality.rs, src/algo/subgraph.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: src/algo/centrality.rs <-> src/algo/topo_sort.rs
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.6667)
- **File(s)**: src/algo/centrality.rs, src/algo/topo_sort.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: src/algo/centrality.rs <-> src/views/mod.rs
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.6667)
- **File(s)**: src/algo/centrality.rs, src/views/mod.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: src/algo/delta.rs <-> src/algo/louvain.rs
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.6667)
- **File(s)**: src/algo/delta.rs, src/algo/louvain.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: src/algo/delta.rs <-> src/views/impact.rs
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.6667)
- **File(s)**: src/algo/delta.rs, src/views/impact.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: src/algo/mod.rs <-> src/views/cluster.rs
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.6667)
- **File(s)**: src/algo/mod.rs, src/views/cluster.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: src/algo/mod.rs <-> src/views/index.rs
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.6667)
- **File(s)**: src/algo/mod.rs, src/views/index.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: src/algo/subgraph.rs <-> src/serial/convert.rs
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.6667)
- **File(s)**: src/algo/subgraph.rs, src/serial/convert.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: src/algo/subgraph.rs <-> src/views/impact.rs
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.6667)
- **File(s)**: src/algo/subgraph.rs, src/views/impact.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: src/algo/topo_sort.rs <-> src/views/impact.rs
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.6667)
- **File(s)**: src/algo/topo_sort.rs, src/views/impact.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: src/parser/csharp.rs <-> src/parser/rust_lang.rs
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.6667)
- **File(s)**: src/parser/csharp.rs, src/parser/rust_lang.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: src/parser/java.rs <-> src/parser/rust_lang.rs
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.6667)
- **File(s)**: src/parser/java.rs, src/parser/rust_lang.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: src/parser/python.rs <-> src/parser/rust_lang.rs
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.6667)
- **File(s)**: src/parser/python.rs, src/parser/rust_lang.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: src/views/impact.rs <-> src/views/mod.rs
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.6667)
- **File(s)**: src/views/impact.rs, src/views/mod.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: Cargo.lock <-> Cargo.toml
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.6429)
- **File(s)**: Cargo.lock, Cargo.toml
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: .ariadne/graph/stats.json <-> .ariadne/views/clusters/algo.md
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.625)
- **File(s)**: .ariadne/graph/stats.json, .ariadne/views/clusters/algo.md
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: .ariadne/graph/stats.json <-> .ariadne/views/clusters/mcp.md
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.625)
- **File(s)**: .ariadne/graph/stats.json, .ariadne/views/clusters/mcp.md
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: .ariadne/graph/stats.json <-> .ariadne/views/clusters/model.md
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.625)
- **File(s)**: .ariadne/graph/stats.json, .ariadne/views/clusters/model.md
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: .ariadne/graph/stats.json <-> .ariadne/views/clusters/parser.md
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.625)
- **File(s)**: .ariadne/graph/stats.json, .ariadne/views/clusters/parser.md
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: .ariadne/graph/stats.json <-> .ariadne/views/clusters/root.md
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.625)
- **File(s)**: .ariadne/graph/stats.json, .ariadne/views/clusters/root.md
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: .ariadne/graph/stats.json <-> .ariadne/views/index.md
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.625)
- **File(s)**: .ariadne/graph/stats.json, .ariadne/views/index.md
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: .ariadne/views/clusters/algo.md <-> .ariadne/views/clusters/serial.md
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.6)
- **File(s)**: .ariadne/views/clusters/algo.md, .ariadne/views/clusters/serial.md
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: .ariadne/views/clusters/algo.md <-> .ariadne/views/clusters/tests.md
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.6)
- **File(s)**: .ariadne/views/clusters/algo.md, .ariadne/views/clusters/tests.md
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: .ariadne/views/clusters/analysis.md <-> .ariadne/views/clusters/design.md
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.6)
- **File(s)**: .ariadne/views/clusters/analysis.md, .ariadne/views/clusters/design.md
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: .ariadne/views/clusters/analysis.md <-> .ariadne/views/clusters/pipeline.md
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.6)
- **File(s)**: .ariadne/views/clusters/analysis.md, .ariadne/views/clusters/pipeline.md
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: .ariadne/views/clusters/design.md <-> .ariadne/views/clusters/pipeline.md
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.6)
- **File(s)**: .ariadne/views/clusters/design.md, .ariadne/views/clusters/pipeline.md
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: .ariadne/views/clusters/mcp.md <-> .ariadne/views/clusters/serial.md
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.6)
- **File(s)**: .ariadne/views/clusters/mcp.md, .ariadne/views/clusters/serial.md
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: .ariadne/views/clusters/mcp.md <-> .ariadne/views/clusters/tests.md
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.6)
- **File(s)**: .ariadne/views/clusters/mcp.md, .ariadne/views/clusters/tests.md
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: .ariadne/views/clusters/model.md <-> .ariadne/views/clusters/serial.md
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.6)
- **File(s)**: .ariadne/views/clusters/model.md, .ariadne/views/clusters/serial.md
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: .ariadne/views/clusters/model.md <-> .ariadne/views/clusters/tests.md
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.6)
- **File(s)**: .ariadne/views/clusters/model.md, .ariadne/views/clusters/tests.md
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: .ariadne/views/clusters/parser.md <-> .ariadne/views/clusters/serial.md
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.6)
- **File(s)**: .ariadne/views/clusters/parser.md, .ariadne/views/clusters/serial.md
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: .ariadne/views/clusters/parser.md <-> .ariadne/views/clusters/tests.md
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.6)
- **File(s)**: .ariadne/views/clusters/parser.md, .ariadne/views/clusters/tests.md
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: .ariadne/views/clusters/root.md <-> .ariadne/views/clusters/serial.md
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.6)
- **File(s)**: .ariadne/views/clusters/root.md, .ariadne/views/clusters/serial.md
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: .ariadne/views/clusters/root.md <-> .ariadne/views/clusters/tests.md
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.6)
- **File(s)**: .ariadne/views/clusters/root.md, .ariadne/views/clusters/tests.md
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: .ariadne/views/clusters/serial.md <-> .ariadne/views/index.md
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.6)
- **File(s)**: .ariadne/views/clusters/serial.md, .ariadne/views/index.md
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: .ariadne/views/clusters/tests.md <-> .ariadne/views/index.md
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.6)
- **File(s)**: .ariadne/views/clusters/tests.md, .ariadne/views/index.md
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: .github/workflows/release.yml <-> install.sh
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.6)
- **File(s)**: .github/workflows/release.yml, install.sh
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: benches/mcp_bench.rs <-> src/analysis/smells.rs
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.6)
- **File(s)**: benches/mcp_bench.rs, src/analysis/smells.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: src/algo/blast_radius.rs <-> src/algo/subgraph.rs
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.6)
- **File(s)**: src/algo/blast_radius.rs, src/algo/subgraph.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: src/algo/compress.rs <-> src/algo/delta.rs
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.6)
- **File(s)**: src/algo/compress.rs, src/algo/delta.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: src/algo/compress.rs <-> src/algo/subgraph.rs
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.6)
- **File(s)**: src/algo/compress.rs, src/algo/subgraph.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: src/algo/delta.rs <-> src/algo/pagerank.rs
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.6)
- **File(s)**: src/algo/delta.rs, src/algo/pagerank.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: src/algo/delta.rs <-> src/algo/spectral.rs
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.6)
- **File(s)**: src/algo/delta.rs, src/algo/spectral.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: src/algo/delta.rs <-> src/algo/subgraph.rs
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.6)
- **File(s)**: src/algo/delta.rs, src/algo/subgraph.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: src/algo/pagerank.rs <-> src/algo/subgraph.rs
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.6)
- **File(s)**: src/algo/pagerank.rs, src/algo/subgraph.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: src/algo/spectral.rs <-> src/algo/subgraph.rs
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.6)
- **File(s)**: src/algo/spectral.rs, src/algo/subgraph.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: src/algo/subgraph.rs <-> src/algo/topo_sort.rs
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.6)
- **File(s)**: src/algo/subgraph.rs, src/algo/topo_sort.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: src/parser/csharp.rs <-> src/parser/typescript.rs
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.6)
- **File(s)**: src/parser/csharp.rs, src/parser/typescript.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: src/parser/go.rs <-> src/parser/rust_lang.rs
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.6)
- **File(s)**: src/parser/go.rs, src/parser/rust_lang.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: src/parser/java.rs <-> src/parser/typescript.rs
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.6)
- **File(s)**: src/parser/java.rs, src/parser/typescript.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: .ariadne/graph/clusters.json <-> .ariadne/graph/stats.json
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.5833)
- **File(s)**: .ariadne/graph/clusters.json, .ariadne/graph/stats.json
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: .ariadne/graph/graph.json <-> .ariadne/graph/stats.json
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.5833)
- **File(s)**: .ariadne/graph/graph.json, .ariadne/graph/stats.json
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: benches/algo_bench.rs <-> src/algo/scc.rs
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.5714)
- **File(s)**: benches/algo_bench.rs, src/algo/scc.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: src/algo/blast_radius.rs <-> src/views/cluster.rs
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.5714)
- **File(s)**: src/algo/blast_radius.rs, src/views/cluster.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: src/algo/blast_radius.rs <-> src/views/index.rs
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.5714)
- **File(s)**: src/algo/blast_radius.rs, src/views/index.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: src/algo/delta.rs <-> src/views/cluster.rs
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.5714)
- **File(s)**: src/algo/delta.rs, src/views/cluster.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: src/algo/delta.rs <-> src/views/index.rs
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.5714)
- **File(s)**: src/algo/delta.rs, src/views/index.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: src/algo/scc.rs <-> src/serial/convert.rs
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.5714)
- **File(s)**: src/algo/scc.rs, src/serial/convert.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: src/algo/subgraph.rs <-> src/views/cluster.rs
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.5714)
- **File(s)**: src/algo/subgraph.rs, src/views/cluster.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: src/algo/subgraph.rs <-> src/views/index.rs
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.5714)
- **File(s)**: src/algo/subgraph.rs, src/views/index.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: src/algo/topo_sort.rs <-> src/views/cluster.rs
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.5714)
- **File(s)**: src/algo/topo_sort.rs, src/views/cluster.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: src/algo/topo_sort.rs <-> src/views/index.rs
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.5714)
- **File(s)**: src/algo/topo_sort.rs, src/views/index.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: src/views/cluster.rs <-> src/views/mod.rs
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.5714)
- **File(s)**: src/views/cluster.rs, src/views/mod.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: src/views/index.rs <-> src/views/mod.rs
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.5714)
- **File(s)**: src/views/index.rs, src/views/mod.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: benches/analysis_bench.rs <-> benches/mcp_bench.rs
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.5556)
- **File(s)**: benches/analysis_bench.rs, benches/mcp_bench.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: benches/analysis_bench.rs <-> src/analysis/smells.rs
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.5556)
- **File(s)**: benches/analysis_bench.rs, src/analysis/smells.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: src/algo/centrality.rs <-> src/algo/mod.rs
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.5556)
- **File(s)**: src/algo/centrality.rs, src/algo/mod.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: src/algo/mod.rs <-> src/views/impact.rs
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.5556)
- **File(s)**: src/algo/mod.rs, src/views/impact.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: src/parser/rust_lang.rs <-> src/parser/typescript.rs
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.5455)
- **File(s)**: src/parser/rust_lang.rs, src/parser/typescript.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: src/mcp/server.rs <-> src/mcp/watch.rs
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.5333)
- **File(s)**: src/mcp/server.rs, src/mcp/watch.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: src/mcp/state.rs <-> src/mcp/tools.rs
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.5294)
- **File(s)**: src/mcp/state.rs, src/mcp/tools.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: .ariadne/graph/stats.json <-> .ariadne/views/clusters/analysis.md
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.5)
- **File(s)**: .ariadne/graph/stats.json, .ariadne/views/clusters/analysis.md
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: .ariadne/graph/stats.json <-> .ariadne/views/clusters/design.md
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.5)
- **File(s)**: .ariadne/graph/stats.json, .ariadne/views/clusters/design.md
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: .ariadne/graph/stats.json <-> .ariadne/views/clusters/pipeline.md
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.5)
- **File(s)**: .ariadne/graph/stats.json, .ariadne/views/clusters/pipeline.md
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: README.md <-> install.sh
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.5)
- **File(s)**: README.md, install.sh
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: benches/algo_bench.rs <-> src/algo/centrality.rs
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.5)
- **File(s)**: benches/algo_bench.rs, src/algo/centrality.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: benches/algo_bench.rs <-> src/views/impact.rs
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.5)
- **File(s)**: benches/algo_bench.rs, src/views/impact.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: benches/mcp_bench.rs <-> src/analysis/diff.rs
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.5)
- **File(s)**: benches/mcp_bench.rs, src/analysis/diff.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: src/algo/centrality.rs <-> src/algo/louvain.rs
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.5)
- **File(s)**: src/algo/centrality.rs, src/algo/louvain.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: src/algo/centrality.rs <-> src/serial/convert.rs
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.5)
- **File(s)**: src/algo/centrality.rs, src/serial/convert.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: src/algo/compress.rs <-> src/algo/scc.rs
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.5)
- **File(s)**: src/algo/compress.rs, src/algo/scc.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: src/algo/compress.rs <-> src/cluster/mod.rs
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.5)
- **File(s)**: src/algo/compress.rs, src/cluster/mod.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: src/algo/delta.rs <-> src/algo/scc.rs
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.5)
- **File(s)**: src/algo/delta.rs, src/algo/scc.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: src/algo/delta.rs <-> src/cluster/mod.rs
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.5)
- **File(s)**: src/algo/delta.rs, src/cluster/mod.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: src/algo/louvain.rs <-> src/views/impact.rs
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.5)
- **File(s)**: src/algo/louvain.rs, src/views/impact.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: src/algo/mod.rs <-> src/views/mod.rs
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.5)
- **File(s)**: src/algo/mod.rs, src/views/mod.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: src/algo/pagerank.rs <-> src/algo/scc.rs
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.5)
- **File(s)**: src/algo/pagerank.rs, src/algo/scc.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: src/algo/pagerank.rs <-> src/cluster/mod.rs
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.5)
- **File(s)**: src/algo/pagerank.rs, src/cluster/mod.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: src/algo/scc.rs <-> src/algo/spectral.rs
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.5)
- **File(s)**: src/algo/scc.rs, src/algo/spectral.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: src/algo/scc.rs <-> src/views/mod.rs
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.5)
- **File(s)**: src/algo/scc.rs, src/views/mod.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: src/algo/spectral.rs <-> src/cluster/mod.rs
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.5)
- **File(s)**: src/algo/spectral.rs, src/cluster/mod.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: src/algo/subgraph.rs <-> src/cluster/mod.rs
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.5)
- **File(s)**: src/algo/subgraph.rs, src/cluster/mod.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: src/analysis/diff.rs <-> src/analysis/smells.rs
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.5)
- **File(s)**: src/analysis/diff.rs, src/analysis/smells.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: src/diagnostic.rs <-> src/model/mod.rs
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.5)
- **File(s)**: src/diagnostic.rs, src/model/mod.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: src/mcp/state.rs <-> tests/mcp_tests.rs
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.5)
- **File(s)**: src/mcp/state.rs, tests/mcp_tests.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: src/model/node.rs <-> src/serial/convert.rs
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.5)
- **File(s)**: src/model/node.rs, src/serial/convert.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

### Co-change coupling: src/serial/convert.rs <-> src/views/impact.rs
- **Category**: structural coupling
- **Evidence**: ariadne temporal analysis (confidence: 0.5)
- **File(s)**: src/serial/convert.rs, src/views/impact.rs
- **Confidence**: high
- **Observation count**: 1
- **Failed observations**: 0
- **Consecutive passes**: 0
- **Lifecycle**: NEW

## Strong

(populated by observation — no entries at init)

