# Cluster: serial

## Files

| File | Type | Layer | In | Out | Centrality |
|------|------|------:|---:|----:|-----------:|
| `src/serial/convert.rs` | source | 2 | 1 | 1 | 0.0000 |
| `src/serial/json.rs` | source | 3 | 5 | 2 | 0.0003 |
| `src/serial/mod.rs` | source | 4 | 4 | 4 | 0.0005 |

## Internal Dependencies

- `src/serial/mod.rs` → `src/serial/convert.rs` (imports)
- `src/serial/mod.rs` → `src/serial/json.rs` (imports)

## External Dependencies

- `src/serial/convert.rs` → `src/model/mod.rs` (imports)
- `src/serial/json.rs` → `src/diagnostic.rs` (imports)
- `src/serial/json.rs` → `src/model/mod.rs` (imports)
- `src/serial/mod.rs` → `src/diagnostic.rs` (imports)
- `src/serial/mod.rs` → `src/model/mod.rs` (imports)

## External Dependents

- `src/serial/json.rs` ← `benches/build_bench.rs` (imports)
- `src/serial/mod.rs` ← `src/lib.rs` (imports)
- `src/serial/json.rs` ← `src/main.rs` (imports)
- `src/serial/mod.rs` ← `src/main.rs` (imports)
- `src/serial/json.rs` ← `src/mcp/server.rs` (imports)
- `src/serial/mod.rs` ← `src/mcp/state.rs` (imports)
- `src/serial/json.rs` ← `src/mcp/watch.rs` (imports)
- `src/serial/mod.rs` ← `src/pipeline/mod.rs` (imports)

## Tests

- `tests/helpers.rs` tests `src/serial/json.rs`
- `tests/pipeline_tests.rs` tests `src/serial/json.rs`
- `tests/pipeline_tests.rs` tests `src/serial/mod.rs`

