# Cluster: parser

## Files

| File | Type | Layer | In | Out | Centrality |
|------|------|------:|---:|----:|-----------:|
| `src/parser/config/gomod.rs` | source | 0 | 1 | 0 | 0.0000 |
| `src/parser/config/jsonc.rs` | source | 0 | 1 | 0 | 0.0000 |
| `src/parser/config/mod.rs` | source | 5 | 6 | 6 | 0.0012 |
| `src/parser/config/pyproject.rs` | source | 0 | 1 | 0 | 0.0000 |
| `src/parser/config/tsconfig.rs` | source | 5 | 1 | 3 | 0.0007 |
| `src/parser/csharp.rs` | source | 4 | 1 | 5 | 0.0000 |
| `src/parser/go.rs` | source | 5 | 1 | 7 | 0.0000 |
| `src/parser/helpers.rs` | source | 0 | 5 | 0 | 0.0000 |
| `src/parser/java.rs` | source | 4 | 1 | 5 | 0.0000 |
| `src/parser/json_lang.rs` | source | 4 | 1 | 3 | 0.0000 |
| `src/parser/markdown.rs` | source | 4 | 1 | 3 | 0.0000 |
| `src/parser/mod.rs` | source | 5 | 5 | 14 | 0.0019 |
| `src/parser/python.rs` | source | 5 | 1 | 7 | 0.0000 |
| `src/parser/registry.rs` | source | 5 | 1 | 5 | 0.0003 |
| `src/parser/rust_lang.rs` | source | 4 | 1 | 6 | 0.0000 |
| `src/parser/symbols.rs` | source | 1 | 7 | 1 | 0.0000 |
| `src/parser/traits.rs` | source | 3 | 10 | 2 | 0.0000 |
| `src/parser/typescript.rs` | source | 5 | 1 | 7 | 0.0000 |
| `src/parser/yaml.rs` | source | 4 | 1 | 3 | 0.0000 |

## Internal Dependencies

- `src/parser/config/mod.rs` → `src/parser/config/gomod.rs` (imports)
- `src/parser/config/mod.rs` → `src/parser/config/jsonc.rs` (imports)
- `src/parser/config/mod.rs` → `src/parser/config/pyproject.rs` (imports)
- `src/parser/config/mod.rs` → `src/parser/config/tsconfig.rs` (imports)
- `src/parser/config/tsconfig.rs` → `src/parser/mod.rs` (imports)
- `src/parser/csharp.rs` → `src/parser/symbols.rs` (imports)
- `src/parser/csharp.rs` → `src/parser/traits.rs` (imports)
- `src/parser/go.rs` → `src/parser/config/mod.rs` (imports)
- `src/parser/go.rs` → `src/parser/helpers.rs` (imports)
- `src/parser/go.rs` → `src/parser/symbols.rs` (imports)
- `src/parser/go.rs` → `src/parser/traits.rs` (imports)
- `src/parser/java.rs` → `src/parser/symbols.rs` (imports)
- `src/parser/java.rs` → `src/parser/traits.rs` (imports)
- `src/parser/json_lang.rs` → `src/parser/traits.rs` (imports)
- `src/parser/markdown.rs` → `src/parser/traits.rs` (imports)
- `src/parser/mod.rs` → `src/parser/config/mod.rs` (imports)
- `src/parser/mod.rs` → `src/parser/csharp.rs` (imports)
- `src/parser/mod.rs` → `src/parser/go.rs` (imports)
- `src/parser/mod.rs` → `src/parser/helpers.rs` (imports)
- `src/parser/mod.rs` → `src/parser/java.rs` (imports)
- `src/parser/mod.rs` → `src/parser/json_lang.rs` (imports)
- `src/parser/mod.rs` → `src/parser/markdown.rs` (imports)
- `src/parser/mod.rs` → `src/parser/python.rs` (imports)
- `src/parser/mod.rs` → `src/parser/registry.rs` (imports)
- `src/parser/mod.rs` → `src/parser/rust_lang.rs` (imports)
- `src/parser/mod.rs` → `src/parser/symbols.rs` (imports)
- `src/parser/mod.rs` → `src/parser/traits.rs` (imports)
- `src/parser/mod.rs` → `src/parser/typescript.rs` (imports)
- `src/parser/mod.rs` → `src/parser/yaml.rs` (imports)
- `src/parser/python.rs` → `src/parser/config/mod.rs` (imports)
- `src/parser/python.rs` → `src/parser/helpers.rs` (imports)
- `src/parser/python.rs` → `src/parser/symbols.rs` (imports)
- `src/parser/python.rs` → `src/parser/traits.rs` (imports)
- `src/parser/registry.rs` → `src/parser/config/mod.rs` (imports)
- `src/parser/rust_lang.rs` → `src/parser/helpers.rs` (imports)
- `src/parser/rust_lang.rs` → `src/parser/symbols.rs` (imports)
- `src/parser/rust_lang.rs` → `src/parser/traits.rs` (imports)
- `src/parser/typescript.rs` → `src/parser/config/mod.rs` (imports)
- `src/parser/typescript.rs` → `src/parser/helpers.rs` (imports)
- `src/parser/typescript.rs` → `src/parser/symbols.rs` (imports)
- `src/parser/typescript.rs` → `src/parser/traits.rs` (imports)
- `src/parser/yaml.rs` → `src/parser/traits.rs` (imports)

## External Dependencies

- `src/parser/config/mod.rs` → `src/diagnostic.rs` (imports)
- `src/parser/config/mod.rs` → `src/model/mod.rs` (imports)
- `src/parser/config/tsconfig.rs` → `src/diagnostic.rs` (imports)
- `src/parser/config/tsconfig.rs` → `src/model/mod.rs` (imports)
- `src/parser/csharp.rs` → `src/model/mod.rs` (imports)
- `src/parser/csharp.rs` → `src/model/symbol.rs` (imports)
- `src/parser/csharp.rs` → `src/model/workspace.rs` (imports)
- `src/parser/go.rs` → `src/model/mod.rs` (imports)
- `src/parser/go.rs` → `src/model/symbol.rs` (imports)
- `src/parser/go.rs` → `src/model/workspace.rs` (imports)
- `src/parser/java.rs` → `src/model/mod.rs` (imports)
- `src/parser/java.rs` → `src/model/symbol.rs` (imports)
- `src/parser/java.rs` → `src/model/workspace.rs` (imports)
- `src/parser/json_lang.rs` → `src/model/mod.rs` (imports)
- `src/parser/json_lang.rs` → `src/model/workspace.rs` (imports)
- `src/parser/markdown.rs` → `src/model/mod.rs` (imports)
- `src/parser/markdown.rs` → `src/model/workspace.rs` (imports)
- `src/parser/python.rs` → `src/model/mod.rs` (imports)
- `src/parser/python.rs` → `src/model/symbol.rs` (imports)
- `src/parser/python.rs` → `src/model/workspace.rs` (imports)
- `src/parser/registry.rs` → `src/model/semantic.rs` (imports)
- `src/parser/registry.rs` → `src/model/symbol.rs` (imports)
- `src/parser/registry.rs` → `src/model/types.rs` (imports)
- `src/parser/registry.rs` → `src/semantic/mod.rs` (imports)
- `src/parser/rust_lang.rs` → `src/model/mod.rs` (imports)
- `src/parser/rust_lang.rs` → `src/model/symbol.rs` (imports)
- `src/parser/rust_lang.rs` → `src/model/workspace.rs` (imports)
- `src/parser/symbols.rs` → `src/model/symbol.rs` (imports)
- `src/parser/traits.rs` → `src/model/mod.rs` (imports)
- `src/parser/traits.rs` → `src/model/workspace.rs` (imports)
- `src/parser/typescript.rs` → `src/model/mod.rs` (imports)
- `src/parser/typescript.rs` → `src/model/symbol.rs` (imports)
- `src/parser/typescript.rs` → `src/model/workspace.rs` (imports)
- `src/parser/yaml.rs` → `src/model/mod.rs` (imports)
- `src/parser/yaml.rs` → `src/model/workspace.rs` (imports)

## External Dependents

- `src/parser/mod.rs` ← `src/lib.rs` (imports)
- `src/parser/mod.rs` ← `src/pipeline/build.rs` (imports)
- `src/parser/config/mod.rs` ← `src/pipeline/mod.rs` (imports)
- `src/parser/mod.rs` ← `src/pipeline/mod.rs` (imports)
- `src/parser/mod.rs` ← `src/pipeline/resolve.rs` (imports)

