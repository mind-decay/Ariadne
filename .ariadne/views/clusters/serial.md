# Cluster: serial

## Files

| File | Type | Layer | In | Out | Centrality |
|------|------|------:|---:|----:|-----------:|
| `src/serial/convert.rs` | source | 3 | 1 | 1 | 0.0000 |
| `src/serial/json.rs` | source | 4 | 3 | 2 | 0.0000 |
| `src/serial/mod.rs` | source | 5 | 3 | 7 | 0.0001 |

## Internal Dependencies

- `src/serial/mod.rs` → `src/serial/convert.rs` (imports)
- `src/serial/mod.rs` → `src/serial/json.rs` (imports)

## External Dependencies

- `src/serial/convert.rs` → `src/model/mod.rs` (imports)
- `src/serial/json.rs` → `src/diagnostic.rs` (imports)
- `src/serial/json.rs` → `src/model/mod.rs` (imports)
- `src/serial/mod.rs` → `src/diagnostic.rs` (imports)
- `src/serial/mod.rs` → `src/model/mod.rs` (imports)
- `src/serial/mod.rs` → `src/model/semantic.rs` (imports)
- `src/serial/mod.rs` → `src/model/symbol.rs` (imports)
- `src/serial/mod.rs` → `src/model/types.rs` (imports)

## External Dependents

- `src/serial/mod.rs` ← `src/lib.rs` (imports)
- `src/serial/json.rs` ← `src/mcp/server.rs` (imports)
- `src/serial/mod.rs` ← `src/mcp/state.rs` (imports)
- `src/serial/json.rs` ← `src/mcp/watch.rs` (imports)
- `src/serial/mod.rs` ← `src/pipeline/mod.rs` (imports)

