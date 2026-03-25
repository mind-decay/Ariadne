# Cluster: tests

## Files

| File | Type | Layer | In | Out | Centrality |
|------|------|------:|---:|----:|-----------:|
| `tests/callgraph_tests.rs` | test | 0 | 0 | 0 | 0.0000 |
| `tests/fixtures/csharp-project/Data/UserRepository.cs` | source | 0 | 0 | 0 | 0.0000 |
| `tests/fixtures/csharp-project/Program.cs` | source | 0 | 0 | 0 | 0.0000 |
| `tests/fixtures/csharp-project/Services/AuthService.cs` | source | 0 | 0 | 0 | 0.0000 |
| `tests/fixtures/csharp-project/Tests/AuthTests.cs` | test | 0 | 0 | 0 | 0.0000 |
| `tests/fixtures/data-files/malformed.json` | data | 0 | 0 | 0 | 0.0000 |
| `tests/fixtures/data-files/malformed.yaml` | data | 0 | 0 | 0 | 0.0000 |
| `tests/fixtures/data-files/sample.json` | data | 0 | 0 | 0 | 0.0000 |
| `tests/fixtures/data-files/sample.yaml` | data | 0 | 0 | 0 | 0.0000 |
| `tests/fixtures/data-files/sample.yml` | data | 0 | 0 | 0 | 0.0000 |
| `tests/fixtures/edge-cases/.ariadne/graph/clusters.json` | data | 0 | 0 | 0 | 0.0000 |
| `tests/fixtures/edge-cases/.ariadne/graph/graph.json` | data | 0 | 0 | 0 | 0.0000 |
| `tests/fixtures/edge-cases/circular-a.ts` | source | 0 | 1 | 1 | 0.0000 |
| `tests/fixtures/edge-cases/circular-b.ts` | source | 0 | 1 | 1 | 0.0000 |
| `tests/fixtures/edge-cases/deeply/nested/a/b/c/d/file.ts` | source | 0 | 0 | 0 | 0.0000 |
| `tests/fixtures/edge-cases/empty.ts` | source | 0 | 0 | 0 | 0.0000 |
| `tests/fixtures/edge-cases/graph/clusters.json` | data | 0 | 0 | 0 | 0.0000 |
| `tests/fixtures/edge-cases/graph/graph.json` | data | 0 | 0 | 0 | 0.0000 |
| `tests/fixtures/edge-cases/helper.ts` | source | 0 | 0 | 0 | 0.0000 |
| `tests/fixtures/edge-cases/syntax-error.ts` | source | 1 | 0 | 1 | 0.0000 |
| `tests/fixtures/edge-cases/valid.ts` | source | 0 | 1 | 0 | 0.0000 |
| `tests/fixtures/go-service/.ariadne/graph/clusters.json` | data | 0 | 0 | 0 | 0.0000 |
| `tests/fixtures/go-service/.ariadne/graph/graph.json` | data | 0 | 0 | 0 | 0.0000 |
| `tests/fixtures/go-service/cmd/server/main.go` | source | 0 | 0 | 0 | 0.0000 |
| `tests/fixtures/go-service/internal/handler/handler.go` | source | 0 | 0 | 0 | 0.0000 |
| `tests/fixtures/go-service/internal/repository/repo.go` | source | 0 | 0 | 0 | 0.0000 |
| `tests/fixtures/go-service/internal/service/service.go` | source | 0 | 0 | 0 | 0.0000 |
| `tests/fixtures/java-project/src/main/java/com/example/App.java` | source | 0 | 0 | 0 | 0.0000 |
| `tests/fixtures/java-project/src/main/java/com/example/data/UserRepo.java` | source | 0 | 0 | 0 | 0.0000 |
| `tests/fixtures/java-project/src/main/java/com/example/service/AuthService.java` | source | 0 | 0 | 0 | 0.0000 |
| `tests/fixtures/java-project/src/test/java/com/example/AppTest.java` | test | 0 | 0 | 0 | 0.0000 |
| `tests/fixtures/markdown-docs/README.md` | doc | 0 | 0 | 0 | 0.0000 |
| `tests/fixtures/markdown-docs/docs/api.md` | doc | 0 | 0 | 0 | 0.0000 |
| `tests/fixtures/markdown-docs/docs/guide.md` | doc | 0 | 0 | 0 | 0.0000 |
| `tests/fixtures/mixed-project/.ariadne/graph/clusters.json` | data | 0 | 0 | 0 | 0.0000 |
| `tests/fixtures/mixed-project/.ariadne/graph/graph.json` | data | 0 | 0 | 0 | 0.0000 |
| `tests/fixtures/mixed-project/README.md` | doc | 0 | 0 | 0 | 0.0000 |
| `tests/fixtures/mixed-project/backend/main.go` | source | 0 | 0 | 0 | 0.0000 |
| `tests/fixtures/mixed-project/frontend/package.json` | config | 0 | 0 | 0 | 0.0000 |
| `tests/fixtures/mixed-project/frontend/src/App.tsx` | source | 1 | 0 | 1 | 0.0000 |
| `tests/fixtures/mixed-project/frontend/src/components/Button.tsx` | source | 0 | 1 | 0 | 0.0000 |
| `tests/fixtures/mixed-project/scripts/deploy.py` | test | 0 | 0 | 0 | 0.0000 |
| `tests/fixtures/python-package/.ariadne/graph/clusters.json` | data | 0 | 0 | 0 | 0.0000 |
| `tests/fixtures/python-package/.ariadne/graph/graph.json` | data | 0 | 0 | 0 | 0.0000 |
| `tests/fixtures/python-package/src/__init__.py` | test | 0 | 0 | 0 | 0.0000 |
| `tests/fixtures/python-package/src/auth/__init__.py` | test | 0 | 1 | 0 | 0.0000 |
| `tests/fixtures/python-package/src/auth/login.py` | test | 1 | 0 | 1 | 0.0000 |
| `tests/fixtures/python-package/src/main.py` | test | 1 | 0 | 2 | 0.0000 |
| `tests/fixtures/python-package/src/utils/__init__.py` | test | 0 | 0 | 0 | 0.0000 |
| `tests/fixtures/python-package/src/utils/helpers.py` | test | 0 | 2 | 0 | 0.0000 |
| `tests/fixtures/python-package/tests/conftest.py` | test | 0 | 0 | 0 | 0.0000 |
| `tests/fixtures/python-package/tests/test_auth.py` | test | 0 | 0 | 0 | 0.0000 |
| `tests/fixtures/rust-crate/.ariadne/graph/clusters.json` | data | 0 | 0 | 0 | 0.0000 |
| `tests/fixtures/rust-crate/.ariadne/graph/graph.json` | data | 0 | 0 | 0 | 0.0000 |
| `tests/fixtures/rust-crate/src/auth/login.rs` | test | 0 | 1 | 0 | 0.0000 |
| `tests/fixtures/rust-crate/src/auth/mod.rs` | test | 1 | 1 | 1 | 0.0000 |
| `tests/fixtures/rust-crate/src/lib.rs` | test | 2 | 0 | 2 | 0.0000 |
| `tests/fixtures/rust-crate/src/utils/format.rs` | test | 0 | 1 | 0 | 0.0000 |
| `tests/fixtures/rust-crate/src/utils/mod.rs` | test | 1 | 1 | 1 | 0.0000 |
| `tests/fixtures/rust-crate/tests/integration.rs` | test | 0 | 0 | 0 | 0.0000 |
| `tests/fixtures/tsx-components/App.tsx` | source | 1 | 0 | 10 | 0.0000 |
| `tests/fixtures/tsx-components/components/Callback.tsx` | source | 0 | 1 | 0 | 0.0000 |
| `tests/fixtures/tsx-components/components/Card.tsx` | source | 0 | 1 | 0 | 0.0000 |
| `tests/fixtures/tsx-components/components/Conditional.tsx` | source | 0 | 1 | 0 | 0.0000 |
| `tests/fixtures/tsx-components/components/DefaultAnon.tsx` | source | 0 | 1 | 0 | 0.0000 |
| `tests/fixtures/tsx-components/components/Fragment.tsx` | source | 0 | 1 | 0 | 0.0000 |
| `tests/fixtures/tsx-components/components/GenericBox.tsx` | source | 0 | 1 | 0 | 0.0000 |
| `tests/fixtures/tsx-components/components/Header.tsx` | source | 0 | 1 | 0 | 0.0000 |
| `tests/fixtures/tsx-components/components/LegacyButton.jsx` | source | 0 | 1 | 0 | 0.0000 |
| `tests/fixtures/tsx-components/components/SpreadProps.tsx` | source | 0 | 1 | 0 | 0.0000 |
| `tests/fixtures/tsx-components/components/StyledBox.tsx` | source | 0 | 1 | 0 | 0.0000 |
| `tests/fixtures/typescript-app/.ariadne/graph/clusters.json` | data | 0 | 0 | 0 | 0.0000 |
| `tests/fixtures/typescript-app/.ariadne/graph/graph.json` | data | 0 | 0 | 0 | 0.0000 |
| `tests/fixtures/typescript-app/.ariadne/graph/raw_imports.json` | data | 0 | 0 | 0 | 0.0000 |
| `tests/fixtures/typescript-app/.ariadne/graph/stats.json` | data | 0 | 0 | 0 | 0.0000 |
| `tests/fixtures/typescript-app/package.json` | config | 0 | 0 | 0 | 0.0000 |
| `tests/fixtures/typescript-app/src/__tests__/login.test.ts` | test | 0 | 0 | 0 | 0.0000 |
| `tests/fixtures/typescript-app/src/auth/login.ts` | source | 1 | 2 | 1 | 0.0000 |
| `tests/fixtures/typescript-app/src/auth/register.ts` | source | 2 | 0 | 1 | 0.0000 |
| `tests/fixtures/typescript-app/src/index.ts` | source | 2 | 0 | 2 | 0.0000 |
| `tests/fixtures/typescript-app/src/types/index.d.ts` | type_def | 0 | 0 | 0 | 0.0000 |
| `tests/fixtures/typescript-app/src/utils/format.ts` | source | 0 | 2 | 0 | 0.0000 |
| `tests/fixtures/typescript-app/tsconfig.json` | config | 0 | 0 | 0 | 0.0000 |
| `tests/fixtures/workspace-project/.ariadne/graph/clusters.json` | data | 0 | 0 | 0 | 0.0000 |
| `tests/fixtures/workspace-project/.ariadne/graph/graph.json` | data | 0 | 0 | 0 | 0.0000 |
| `tests/fixtures/workspace-project/package.json` | config | 0 | 0 | 0 | 0.0000 |
| `tests/fixtures/workspace-project/packages/api/package.json` | config | 0 | 0 | 0 | 0.0000 |
| `tests/fixtures/workspace-project/packages/api/src/index.ts` | source | 1 | 0 | 1 | 0.0000 |
| `tests/fixtures/workspace-project/packages/api/src/router.ts` | source | 0 | 1 | 0 | 0.0000 |
| `tests/fixtures/workspace-project/packages/auth/package.json` | config | 0 | 0 | 0 | 0.0000 |
| `tests/fixtures/workspace-project/packages/auth/src/index.ts` | source | 1 | 0 | 1 | 0.0000 |
| `tests/fixtures/workspace-project/packages/auth/src/login.ts` | source | 0 | 1 | 0 | 0.0000 |
| `tests/fixtures/workspace-project/packages/shared/package.json` | config | 0 | 0 | 0 | 0.0000 |
| `tests/fixtures/workspace-project/packages/shared/src/format.ts` | source | 0 | 1 | 0 | 0.0000 |
| `tests/fixtures/workspace-project/packages/shared/src/index.ts` | source | 1 | 0 | 1 | 0.0000 |
| `tests/graph_tests.rs` | test | 0 | 0 | 0 | 0.0000 |
| `tests/helpers.rs` | test | 0 | 0 | 0 | 0.0000 |
| `tests/invariants.rs` | test | 0 | 0 | 0 | 0.0000 |
| `tests/mcp_tests.rs` | test | 0 | 0 | 0 | 0.0000 |
| `tests/pipeline_tests.rs` | test | 0 | 0 | 0 | 0.0000 |
| `tests/properties.rs` | test | 0 | 0 | 0 | 0.0000 |
| `tests/symbol_tests.rs` | test | 0 | 0 | 0 | 0.0000 |

## Internal Dependencies

- `tests/fixtures/edge-cases/circular-a.ts` → `tests/fixtures/edge-cases/circular-b.ts` (imports)
- `tests/fixtures/edge-cases/circular-b.ts` → `tests/fixtures/edge-cases/circular-a.ts` (imports)
- `tests/fixtures/edge-cases/syntax-error.ts` → `tests/fixtures/edge-cases/valid.ts` (imports)
- `tests/fixtures/mixed-project/frontend/src/App.tsx` → `tests/fixtures/mixed-project/frontend/src/components/Button.tsx` (imports)
- `tests/fixtures/python-package/src/auth/login.py` → `tests/fixtures/python-package/src/utils/helpers.py` (imports)
- `tests/fixtures/python-package/src/main.py` → `tests/fixtures/python-package/src/auth/__init__.py` (imports)
- `tests/fixtures/python-package/src/main.py` → `tests/fixtures/python-package/src/utils/helpers.py` (imports)
- `tests/fixtures/rust-crate/src/auth/mod.rs` → `tests/fixtures/rust-crate/src/auth/login.rs` (imports)
- `tests/fixtures/rust-crate/src/lib.rs` → `tests/fixtures/rust-crate/src/auth/mod.rs` (imports)
- `tests/fixtures/rust-crate/src/lib.rs` → `tests/fixtures/rust-crate/src/utils/mod.rs` (imports)
- `tests/fixtures/rust-crate/src/utils/mod.rs` → `tests/fixtures/rust-crate/src/utils/format.rs` (imports)
- `tests/fixtures/tsx-components/App.tsx` → `tests/fixtures/tsx-components/components/Callback.tsx` (imports)
- `tests/fixtures/tsx-components/App.tsx` → `tests/fixtures/tsx-components/components/Card.tsx` (imports)
- `tests/fixtures/tsx-components/App.tsx` → `tests/fixtures/tsx-components/components/Conditional.tsx` (imports)
- `tests/fixtures/tsx-components/App.tsx` → `tests/fixtures/tsx-components/components/DefaultAnon.tsx` (imports)
- `tests/fixtures/tsx-components/App.tsx` → `tests/fixtures/tsx-components/components/Fragment.tsx` (imports)
- `tests/fixtures/tsx-components/App.tsx` → `tests/fixtures/tsx-components/components/GenericBox.tsx` (imports)
- `tests/fixtures/tsx-components/App.tsx` → `tests/fixtures/tsx-components/components/Header.tsx` (imports)
- `tests/fixtures/tsx-components/App.tsx` → `tests/fixtures/tsx-components/components/LegacyButton.jsx` (imports)
- `tests/fixtures/tsx-components/App.tsx` → `tests/fixtures/tsx-components/components/SpreadProps.tsx` (imports)
- `tests/fixtures/tsx-components/App.tsx` → `tests/fixtures/tsx-components/components/StyledBox.tsx` (imports)
- `tests/fixtures/typescript-app/src/auth/login.ts` → `tests/fixtures/typescript-app/src/utils/format.ts` (imports)
- `tests/fixtures/typescript-app/src/auth/register.ts` → `tests/fixtures/typescript-app/src/auth/login.ts` (imports)
- `tests/fixtures/typescript-app/src/index.ts` → `tests/fixtures/typescript-app/src/auth/login.ts` (imports)
- `tests/fixtures/typescript-app/src/index.ts` → `tests/fixtures/typescript-app/src/utils/format.ts` (imports)
- `tests/fixtures/workspace-project/packages/api/src/index.ts` → `tests/fixtures/workspace-project/packages/api/src/router.ts` (re_exports)
- `tests/fixtures/workspace-project/packages/auth/src/index.ts` → `tests/fixtures/workspace-project/packages/auth/src/login.ts` (re_exports)
- `tests/fixtures/workspace-project/packages/shared/src/index.ts` → `tests/fixtures/workspace-project/packages/shared/src/format.ts` (re_exports)

## Tests

- `tests/callgraph_tests.rs` tests `src/algo/callgraph.rs`
- `tests/callgraph_tests.rs` tests `src/model/edge.rs`
- `tests/callgraph_tests.rs` tests `src/model/node.rs`
- `tests/callgraph_tests.rs` tests `src/model/symbol.rs`
- `tests/callgraph_tests.rs` tests `src/model/symbol_index.rs`
- `tests/callgraph_tests.rs` tests `src/model/types.rs`
- `tests/fixtures/typescript-app/src/__tests__/login.test.ts` tests `tests/fixtures/typescript-app/src/auth/login.ts`
- `tests/graph_tests.rs` tests `src/diagnostic.rs`
- `tests/helpers.rs` tests `src/parser/mod.rs`
- `tests/helpers.rs` tests `src/pipeline/mod.rs`
- `tests/helpers.rs` tests `src/serial/json.rs`
- `tests/pipeline_tests.rs` tests `src/diagnostic.rs`
- `tests/pipeline_tests.rs` tests `src/model/mod.rs`
- `tests/pipeline_tests.rs` tests `src/parser/mod.rs`
- `tests/pipeline_tests.rs` tests `src/pipeline/mod.rs`
- `tests/pipeline_tests.rs` tests `src/serial/json.rs`
- `tests/pipeline_tests.rs` tests `src/serial/mod.rs`
- `tests/properties.rs` tests `src/diagnostic.rs`
- `tests/properties.rs` tests `src/hash.rs`
- `tests/properties.rs` tests `src/model/mod.rs`
- `tests/symbol_tests.rs` tests `src/model/symbol.rs`
- `tests/symbol_tests.rs` tests `src/parser/mod.rs`

