# Cluster: mcp

## Files

| File | Type | Layer | In | Out | Centrality |
|------|------|------:|---:|----:|-----------:|
| `src/mcp/annotations.rs` | source | 4 | 1 | 2 | 0.0000 |
| `src/mcp/bookmarks.rs` | source | 4 | 1 | 2 | 0.0000 |
| `src/mcp/lock.rs` | source | 4 | 2 | 1 | 0.0000 |
| `src/mcp/mod.rs` | source | 10 | 1 | 13 | 0.0002 |
| `src/mcp/persist.rs` | source | 0 | 2 | 0 | 0.0000 |
| `src/mcp/prompts.rs` | source | 7 | 1 | 4 | 0.0000 |
| `src/mcp/resources.rs` | source | 7 | 1 | 2 | 0.0000 |
| `src/mcp/server.rs` | source | 9 | 1 | 8 | 0.0002 |
| `src/mcp/state.rs` | source | 6 | 7 | 9 | 0.0009 |
| `src/mcp/tools.rs` | source | 7 | 2 | 11 | 0.0004 |
| `src/mcp/tools_context.rs` | source | 0 | 2 | 0 | 0.0000 |
| `src/mcp/tools_temporal.rs` | source | 0 | 2 | 0 | 0.0000 |
| `src/mcp/user_state.rs` | source | 3 | 5 | 2 | 0.0001 |
| `src/mcp/watch.rs` | source | 8 | 2 | 5 | 0.0002 |

## Internal Dependencies

- `src/mcp/annotations.rs` → `src/mcp/user_state.rs` (imports)
- `src/mcp/bookmarks.rs` → `src/mcp/user_state.rs` (imports)
- `src/mcp/mod.rs` → `src/mcp/annotations.rs` (imports)
- `src/mcp/mod.rs` → `src/mcp/bookmarks.rs` (imports)
- `src/mcp/mod.rs` → `src/mcp/lock.rs` (imports)
- `src/mcp/mod.rs` → `src/mcp/persist.rs` (imports)
- `src/mcp/mod.rs` → `src/mcp/prompts.rs` (imports)
- `src/mcp/mod.rs` → `src/mcp/resources.rs` (imports)
- `src/mcp/mod.rs` → `src/mcp/server.rs` (imports)
- `src/mcp/mod.rs` → `src/mcp/state.rs` (imports)
- `src/mcp/mod.rs` → `src/mcp/tools.rs` (imports)
- `src/mcp/mod.rs` → `src/mcp/tools_context.rs` (imports)
- `src/mcp/mod.rs` → `src/mcp/tools_temporal.rs` (imports)
- `src/mcp/mod.rs` → `src/mcp/user_state.rs` (imports)
- `src/mcp/mod.rs` → `src/mcp/watch.rs` (imports)
- `src/mcp/prompts.rs` → `src/mcp/state.rs` (imports)
- `src/mcp/resources.rs` → `src/mcp/state.rs` (imports)
- `src/mcp/server.rs` → `src/mcp/lock.rs` (imports)
- `src/mcp/server.rs` → `src/mcp/state.rs` (imports)
- `src/mcp/server.rs` → `src/mcp/tools.rs` (imports)
- `src/mcp/server.rs` → `src/mcp/user_state.rs` (imports)
- `src/mcp/server.rs` → `src/mcp/watch.rs` (imports)
- `src/mcp/tools.rs` → `src/mcp/state.rs` (imports)
- `src/mcp/tools.rs` → `src/mcp/tools_context.rs` (imports)
- `src/mcp/tools.rs` → `src/mcp/tools_temporal.rs` (imports)
- `src/mcp/tools.rs` → `src/mcp/user_state.rs` (imports)
- `src/mcp/user_state.rs` → `src/mcp/persist.rs` (imports)
- `src/mcp/watch.rs` → `src/mcp/state.rs` (imports)

## External Dependencies

- `src/mcp/annotations.rs` → `src/model/mod.rs` (imports)
- `src/mcp/bookmarks.rs` → `src/model/mod.rs` (imports)
- `src/mcp/lock.rs` → `src/diagnostic.rs` (imports)
- `src/mcp/prompts.rs` → `src/algo/reading_order.rs` (imports)
- `src/mcp/prompts.rs` → `src/analysis/smells.rs` (imports)
- `src/mcp/prompts.rs` → `src/model/mod.rs` (imports)
- `src/mcp/resources.rs` → `src/analysis/smells.rs` (imports)
- `src/mcp/server.rs` → `src/diagnostic.rs` (imports)
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
- `src/mcp/tools.rs` → `src/algo/context.rs` (imports)
- `src/mcp/tools.rs` → `src/algo/impact.rs` (imports)
- `src/mcp/tools.rs` → `src/algo/mod.rs` (imports)
- `src/mcp/tools.rs` → `src/algo/reading_order.rs` (imports)
- `src/mcp/tools.rs` → `src/algo/test_map.rs` (imports)
- `src/mcp/tools.rs` → `src/analysis/smells.rs` (imports)
- `src/mcp/tools.rs` → `src/model/mod.rs` (imports)
- `src/mcp/user_state.rs` → `src/model/mod.rs` (imports)
- `src/mcp/watch.rs` → `src/analysis/diff.rs` (imports)
- `src/mcp/watch.rs` → `src/diagnostic.rs` (imports)
- `src/mcp/watch.rs` → `src/pipeline/mod.rs` (imports)
- `src/mcp/watch.rs` → `src/serial/json.rs` (imports)

## External Dependents

- `src/mcp/state.rs` ← `benches/mcp_bench.rs` (imports)
- `src/mcp/mod.rs` ← `src/lib.rs` (imports)

