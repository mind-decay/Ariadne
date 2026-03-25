# Cluster: root

## Files

| File | Type | Layer | In | Out | Centrality |
|------|------|------:|---:|----:|-----------:|
| `.mcp.json` | data | 0 | 0 | 0 | 0.0000 |
| `CLAUDE.md` | doc | 0 | 0 | 0 | 0.0000 |
| `README.md` | doc | 0 | 0 | 0 | 0.0000 |
| `src/diagnostic.rs` | source | 3 | 14 | 1 | 0.0003 |
| `src/hash.rs` | source | 3 | 3 | 1 | 0.0002 |
| `src/lib.rs` | source | 11 | 0 | 12 | 0.0000 |
| `src/main.rs` | source | 8 | 0 | 7 | 0.0000 |

## Internal Dependencies

- `src/lib.rs` → `src/diagnostic.rs` (imports)
- `src/lib.rs` → `src/hash.rs` (imports)
- `src/main.rs` → `src/diagnostic.rs` (imports)

## External Dependencies

- `src/diagnostic.rs` → `src/model/mod.rs` (imports)
- `src/hash.rs` → `src/model/mod.rs` (imports)
- `src/lib.rs` → `src/algo/mod.rs` (imports)
- `src/lib.rs` → `src/analysis/mod.rs` (imports)
- `src/lib.rs` → `src/cluster/mod.rs` (imports)
- `src/lib.rs` → `src/detect/mod.rs` (imports)
- `src/lib.rs` → `src/mcp/mod.rs` (imports)
- `src/lib.rs` → `src/model/mod.rs` (imports)
- `src/lib.rs` → `src/parser/mod.rs` (imports)
- `src/lib.rs` → `src/pipeline/mod.rs` (imports)
- `src/lib.rs` → `src/serial/mod.rs` (imports)
- `src/lib.rs` → `src/views/mod.rs` (imports)
- `src/main.rs` → `src/algo/mod.rs` (imports)
- `src/main.rs` → `src/model/mod.rs` (imports)
- `src/main.rs` → `src/parser/mod.rs` (imports)
- `src/main.rs` → `src/pipeline/mod.rs` (imports)
- `src/main.rs` → `src/serial/json.rs` (imports)
- `src/main.rs` → `src/serial/mod.rs` (imports)

## External Dependents

- `src/hash.rs` ← `benches/parser_bench.rs` (imports)
- `src/diagnostic.rs` ← `src/detect/workspace.rs` (imports)
- `src/diagnostic.rs` ← `src/mcp/lock.rs` (imports)
- `src/diagnostic.rs` ← `src/mcp/server.rs` (imports)
- `src/diagnostic.rs` ← `src/mcp/state.rs` (imports)
- `src/diagnostic.rs` ← `src/mcp/watch.rs` (imports)
- `src/diagnostic.rs` ← `src/pipeline/build.rs` (imports)
- `src/diagnostic.rs` ← `src/pipeline/mod.rs` (imports)
- `src/hash.rs` ← `src/pipeline/read.rs` (imports)
- `src/diagnostic.rs` ← `src/pipeline/resolve.rs` (imports)
- `src/diagnostic.rs` ← `src/pipeline/walk.rs` (imports)
- `src/diagnostic.rs` ← `src/serial/json.rs` (imports)
- `src/diagnostic.rs` ← `src/serial/mod.rs` (imports)
- `src/diagnostic.rs` ← `src/views/mod.rs` (imports)

## Tests

- `tests/graph_tests.rs` tests `src/diagnostic.rs`
- `tests/pipeline_tests.rs` tests `src/diagnostic.rs`
- `tests/properties.rs` tests `src/diagnostic.rs`
- `tests/properties.rs` tests `src/hash.rs`

