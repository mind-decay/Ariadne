# Cluster: parser

## Files

| File | Type | Layer | In | Out | Centrality |
|------|------|------:|---:|----:|-----------:|
| `src/parser/csharp.rs` | source | 3 | 1 | 3 | 0.0001 |
| `src/parser/go.rs` | source | 3 | 1 | 3 | 0.0001 |
| `src/parser/helpers.rs` | source | 0 | 4 | 0 | 0.0000 |
| `src/parser/java.rs` | source | 3 | 1 | 3 | 0.0001 |
| `src/parser/markdown.rs` | source | 3 | 1 | 3 | 0.0001 |
| `src/parser/mod.rs` | source | 4 | 8 | 10 | 0.0033 |
| `src/parser/python.rs` | source | 3 | 1 | 4 | 0.0001 |
| `src/parser/registry.rs` | source | 0 | 1 | 0 | 0.0000 |
| `src/parser/rust_lang.rs` | source | 3 | 1 | 4 | 0.0001 |
| `src/parser/traits.rs` | source | 2 | 8 | 2 | 0.0001 |
| `src/parser/typescript.rs` | source | 3 | 1 | 4 | 0.0001 |

## Internal Dependencies

- `src/parser/csharp.rs` → `src/parser/traits.rs` (imports)
- `src/parser/go.rs` → `src/parser/traits.rs` (imports)
- `src/parser/java.rs` → `src/parser/traits.rs` (imports)
- `src/parser/markdown.rs` → `src/parser/traits.rs` (imports)
- `src/parser/mod.rs` → `src/parser/csharp.rs` (imports)
- `src/parser/mod.rs` → `src/parser/go.rs` (imports)
- `src/parser/mod.rs` → `src/parser/helpers.rs` (imports)
- `src/parser/mod.rs` → `src/parser/java.rs` (imports)
- `src/parser/mod.rs` → `src/parser/markdown.rs` (imports)
- `src/parser/mod.rs` → `src/parser/python.rs` (imports)
- `src/parser/mod.rs` → `src/parser/registry.rs` (imports)
- `src/parser/mod.rs` → `src/parser/rust_lang.rs` (imports)
- `src/parser/mod.rs` → `src/parser/traits.rs` (imports)
- `src/parser/mod.rs` → `src/parser/typescript.rs` (imports)
- `src/parser/python.rs` → `src/parser/helpers.rs` (imports)
- `src/parser/python.rs` → `src/parser/traits.rs` (imports)
- `src/parser/rust_lang.rs` → `src/parser/helpers.rs` (imports)
- `src/parser/rust_lang.rs` → `src/parser/traits.rs` (imports)
- `src/parser/typescript.rs` → `src/parser/helpers.rs` (imports)
- `src/parser/typescript.rs` → `src/parser/traits.rs` (imports)

## External Dependencies

- `src/parser/csharp.rs` → `src/model/mod.rs` (imports)
- `src/parser/csharp.rs` → `src/model/workspace.rs` (imports)
- `src/parser/go.rs` → `src/model/mod.rs` (imports)
- `src/parser/go.rs` → `src/model/workspace.rs` (imports)
- `src/parser/java.rs` → `src/model/mod.rs` (imports)
- `src/parser/java.rs` → `src/model/workspace.rs` (imports)
- `src/parser/markdown.rs` → `src/model/mod.rs` (imports)
- `src/parser/markdown.rs` → `src/model/workspace.rs` (imports)
- `src/parser/python.rs` → `src/model/mod.rs` (imports)
- `src/parser/python.rs` → `src/model/workspace.rs` (imports)
- `src/parser/rust_lang.rs` → `src/model/mod.rs` (imports)
- `src/parser/rust_lang.rs` → `src/model/workspace.rs` (imports)
- `src/parser/traits.rs` → `src/model/mod.rs` (imports)
- `src/parser/traits.rs` → `src/model/workspace.rs` (imports)
- `src/parser/typescript.rs` → `src/model/mod.rs` (imports)
- `src/parser/typescript.rs` → `src/model/workspace.rs` (imports)

## External Dependents

- `src/parser/mod.rs` ← `benches/build_bench.rs` (imports)
- `src/parser/mod.rs` ← `benches/parser_bench.rs` (imports)
- `src/parser/mod.rs` ← `src/lib.rs` (imports)
- `src/parser/mod.rs` ← `src/main.rs` (imports)
- `src/parser/mod.rs` ← `src/mcp/server.rs` (imports)
- `src/parser/mod.rs` ← `src/pipeline/build.rs` (imports)
- `src/parser/mod.rs` ← `src/pipeline/mod.rs` (imports)
- `src/parser/mod.rs` ← `src/pipeline/resolve.rs` (imports)

## Tests

- `tests/helpers.rs` tests `src/parser/mod.rs`
- `tests/pipeline_tests.rs` tests `src/parser/mod.rs`

