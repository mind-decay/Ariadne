# Cluster: mcp

## Files

| File | Type | Layer | In | Out | Centrality |
|------|------|------:|---:|----:|-----------:|
| `src/mcp/lock.rs` | source | 4 | 2 | 1 | 0.0000 |
| `src/mcp/mod.rs` | source | 10 | 1 | 5 | 0.0001 |
| `src/mcp/server.rs` | source | 9 | 1 | 8 | 0.0003 |
| `src/mcp/state.rs` | source | 6 | 5 | 9 | 0.0007 |
| `src/mcp/tools.rs` | source | 7 | 2 | 4 | 0.0003 |
| `src/mcp/watch.rs` | source | 8 | 2 | 5 | 0.0001 |

## Internal Dependencies

- `src/mcp/mod.rs` → `src/mcp/lock.rs` (imports)
- `src/mcp/mod.rs` → `src/mcp/server.rs` (imports)
- `src/mcp/mod.rs` → `src/mcp/state.rs` (imports)
- `src/mcp/mod.rs` → `src/mcp/tools.rs` (imports)
- `src/mcp/mod.rs` → `src/mcp/watch.rs` (imports)
- `src/mcp/server.rs` → `src/mcp/lock.rs` (imports)
- `src/mcp/server.rs` → `src/mcp/state.rs` (imports)
- `src/mcp/server.rs` → `src/mcp/tools.rs` (imports)
- `src/mcp/server.rs` → `src/mcp/watch.rs` (imports)
- `src/mcp/tools.rs` → `src/mcp/state.rs` (imports)
- `src/mcp/watch.rs` → `src/mcp/state.rs` (imports)

## External Dependencies

- `src/mcp/lock.rs` → `src/diagnostic.rs` (imports)
- `src/mcp/server.rs` → `src/diagnostic.rs` (imports)
- `src/mcp/server.rs` → `src/parser/mod.rs` (imports)
- `src/mcp/server.rs` → `src/pipeline/mod.rs` (imports)
- `src/mcp/server.rs` → `src/serial/json.rs` (imports)
- `src/mcp/state.rs` → `src/algo/callgraph.rs` (imports)
- `src/mcp/state.rs` → `src/algo/compress.rs` (imports)
- `src/mcp/state.rs` → `src/algo/pagerank.rs` (imports)
- `src/mcp/state.rs` → `src/algo/spectral.rs` (imports)
- `src/mcp/state.rs` → `src/analysis/metrics.rs` (imports)
- `src/mcp/state.rs` → `src/diagnostic.rs` (imports)
- `src/mcp/state.rs` → `src/model/mod.rs` (imports)
- `src/mcp/state.rs` → `src/model/symbol_index.rs` (imports)
- `src/mcp/state.rs` → `src/serial/mod.rs` (imports)
- `src/mcp/tools.rs` → `src/algo/mod.rs` (imports)
- `src/mcp/tools.rs` → `src/analysis/smells.rs` (imports)
- `src/mcp/tools.rs` → `src/model/mod.rs` (imports)
- `src/mcp/watch.rs` → `src/analysis/diff.rs` (imports)
- `src/mcp/watch.rs` → `src/diagnostic.rs` (imports)
- `src/mcp/watch.rs` → `src/pipeline/mod.rs` (imports)
- `src/mcp/watch.rs` → `src/serial/json.rs` (imports)

## External Dependents

- `src/mcp/state.rs` ← `benches/mcp_bench.rs` (imports)
- `src/mcp/mod.rs` ← `src/lib.rs` (imports)

