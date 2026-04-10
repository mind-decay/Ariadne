# Cluster: pipeline

## Files

| File | Type | Layer | In | Out | Centrality |
|------|------|------:|---:|----:|-----------:|
| `src/pipeline/build.rs` | config | 6 | 1 | 5 | 0.0000 |
| `src/pipeline/mod.rs` | source | 7 | 3 | 15 | 0.0011 |
| `src/pipeline/read.rs` | source | 4 | 1 | 2 | 0.0000 |
| `src/pipeline/resolve.rs` | source | 6 | 1 | 5 | 0.0000 |
| `src/pipeline/walk.rs` | source | 4 | 1 | 2 | 0.0000 |

## Internal Dependencies

- `src/pipeline/mod.rs` → `src/pipeline/build.rs` (imports)
- `src/pipeline/mod.rs` → `src/pipeline/read.rs` (imports)
- `src/pipeline/mod.rs` → `src/pipeline/resolve.rs` (imports)
- `src/pipeline/mod.rs` → `src/pipeline/walk.rs` (imports)

## External Dependencies

- `src/pipeline/build.rs` → `src/detect/mod.rs` (imports)
- `src/pipeline/build.rs` → `src/diagnostic.rs` (imports)
- `src/pipeline/build.rs` → `src/model/mod.rs` (imports)
- `src/pipeline/build.rs` → `src/model/workspace.rs` (imports)
- `src/pipeline/build.rs` → `src/parser/mod.rs` (imports)
- `src/pipeline/mod.rs` → `src/algo/mod.rs` (imports)
- `src/pipeline/mod.rs` → `src/cluster/mod.rs` (imports)
- `src/pipeline/mod.rs` → `src/detect/mod.rs` (imports)
- `src/pipeline/mod.rs` → `src/diagnostic.rs` (imports)
- `src/pipeline/mod.rs` → `src/model/mod.rs` (imports)
- `src/pipeline/mod.rs` → `src/model/semantic.rs` (imports)
- `src/pipeline/mod.rs` → `src/model/symbol.rs` (imports)
- `src/pipeline/mod.rs` → `src/parser/config/mod.rs` (imports)
- `src/pipeline/mod.rs` → `src/parser/mod.rs` (imports)
- `src/pipeline/mod.rs` → `src/semantic/mod.rs` (imports)
- `src/pipeline/mod.rs` → `src/serial/mod.rs` (imports)
- `src/pipeline/read.rs` → `src/hash.rs` (imports)
- `src/pipeline/read.rs` → `src/model/mod.rs` (imports)
- `src/pipeline/resolve.rs` → `src/detect/mod.rs` (imports)
- `src/pipeline/resolve.rs` → `src/diagnostic.rs` (imports)
- `src/pipeline/resolve.rs` → `src/model/mod.rs` (imports)
- `src/pipeline/resolve.rs` → `src/model/workspace.rs` (imports)
- `src/pipeline/resolve.rs` → `src/parser/mod.rs` (imports)
- `src/pipeline/walk.rs` → `src/diagnostic.rs` (imports)
- `src/pipeline/walk.rs` → `src/model/mod.rs` (imports)

## External Dependents

- `src/pipeline/mod.rs` ← `src/lib.rs` (imports)
- `src/pipeline/mod.rs` ← `src/mcp/server.rs` (imports)
- `src/pipeline/mod.rs` ← `src/mcp/watch.rs` (imports)

