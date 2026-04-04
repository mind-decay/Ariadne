# Cluster: algo

## Files

| File | Type | Layer | In | Out | Centrality |
|------|------|------:|---:|----:|-----------:|
| `src/algo/blast_radius.rs` | source | 4 | 4 | 2 | 0.0000 |
| `src/algo/callgraph.rs` | source | 2 | 4 | 3 | 0.0000 |
| `src/algo/centrality.rs` | source | 4 | 1 | 2 | 0.0000 |
| `src/algo/compress.rs` | source | 3 | 2 | 2 | 0.0000 |
| `src/algo/context.rs` | source | 4 | 3 | 2 | 0.0000 |
| `src/algo/delta.rs` | source | 3 | 1 | 1 | 0.0000 |
| `src/algo/impact.rs` | source | 4 | 2 | 5 | 0.0000 |
| `src/algo/louvain.rs` | source | 3 | 1 | 1 | 0.0000 |
| `src/algo/mod.rs` | source | 4 | 20 | 17 | 0.0037 |
| `src/algo/pagerank.rs` | source | 4 | 2 | 2 | 0.0000 |
| `src/algo/reading_order.rs` | source | 4 | 3 | 4 | 0.0001 |
| `src/algo/scc.rs` | source | 4 | 3 | 2 | 0.0000 |
| `src/algo/spectral.rs` | source | 4 | 2 | 2 | 0.0000 |
| `src/algo/stats.rs` | source | 4 | 1 | 2 | 0.0000 |
| `src/algo/subgraph.rs` | source | 3 | 1 | 1 | 0.0000 |
| `src/algo/test_map.rs` | source | 4 | 3 | 2 | 0.0000 |
| `src/algo/topo_sort.rs` | source | 4 | 2 | 2 | 0.0000 |

## Internal Dependencies

- `src/algo/blast_radius.rs` → `src/algo/mod.rs` (imports)
- `src/algo/centrality.rs` → `src/algo/mod.rs` (imports)
- `src/algo/context.rs` → `src/algo/mod.rs` (imports)
- `src/algo/impact.rs` → `src/algo/blast_radius.rs` (imports)
- `src/algo/impact.rs` → `src/algo/context.rs` (imports)
- `src/algo/impact.rs` → `src/algo/mod.rs` (imports)
- `src/algo/impact.rs` → `src/algo/test_map.rs` (imports)
- `src/algo/mod.rs` → `src/algo/blast_radius.rs` (imports)
- `src/algo/mod.rs` → `src/algo/callgraph.rs` (imports)
- `src/algo/mod.rs` → `src/algo/centrality.rs` (imports)
- `src/algo/mod.rs` → `src/algo/compress.rs` (imports)
- `src/algo/mod.rs` → `src/algo/context.rs` (imports)
- `src/algo/mod.rs` → `src/algo/delta.rs` (imports)
- `src/algo/mod.rs` → `src/algo/impact.rs` (imports)
- `src/algo/mod.rs` → `src/algo/louvain.rs` (imports)
- `src/algo/mod.rs` → `src/algo/pagerank.rs` (imports)
- `src/algo/mod.rs` → `src/algo/reading_order.rs` (imports)
- `src/algo/mod.rs` → `src/algo/scc.rs` (imports)
- `src/algo/mod.rs` → `src/algo/spectral.rs` (imports)
- `src/algo/mod.rs` → `src/algo/stats.rs` (imports)
- `src/algo/mod.rs` → `src/algo/subgraph.rs` (imports)
- `src/algo/mod.rs` → `src/algo/test_map.rs` (imports)
- `src/algo/mod.rs` → `src/algo/topo_sort.rs` (imports)
- `src/algo/pagerank.rs` → `src/algo/mod.rs` (imports)
- `src/algo/reading_order.rs` → `src/algo/mod.rs` (imports)
- `src/algo/reading_order.rs` → `src/algo/scc.rs` (imports)
- `src/algo/reading_order.rs` → `src/algo/topo_sort.rs` (imports)
- `src/algo/scc.rs` → `src/algo/mod.rs` (imports)
- `src/algo/spectral.rs` → `src/algo/mod.rs` (imports)
- `src/algo/stats.rs` → `src/algo/mod.rs` (imports)
- `src/algo/test_map.rs` → `src/algo/mod.rs` (imports)
- `src/algo/topo_sort.rs` → `src/algo/mod.rs` (imports)

## External Dependencies

- `src/algo/blast_radius.rs` → `src/model/mod.rs` (imports)
- `src/algo/callgraph.rs` → `src/model/edge.rs` (imports)
- `src/algo/callgraph.rs` → `src/model/symbol_index.rs` (imports)
- `src/algo/callgraph.rs` → `src/model/types.rs` (imports)
- `src/algo/centrality.rs` → `src/model/mod.rs` (imports)
- `src/algo/compress.rs` → `src/model/compress.rs` (imports)
- `src/algo/compress.rs` → `src/model/mod.rs` (imports)
- `src/algo/context.rs` → `src/model/mod.rs` (imports)
- `src/algo/delta.rs` → `src/model/mod.rs` (imports)
- `src/algo/impact.rs` → `src/model/mod.rs` (imports)
- `src/algo/louvain.rs` → `src/model/mod.rs` (imports)
- `src/algo/mod.rs` → `src/model/mod.rs` (imports)
- `src/algo/pagerank.rs` → `src/model/mod.rs` (imports)
- `src/algo/reading_order.rs` → `src/model/mod.rs` (imports)
- `src/algo/scc.rs` → `src/model/mod.rs` (imports)
- `src/algo/spectral.rs` → `src/model/mod.rs` (imports)
- `src/algo/stats.rs` → `src/model/mod.rs` (imports)
- `src/algo/subgraph.rs` → `src/model/mod.rs` (imports)
- `src/algo/test_map.rs` → `src/model/mod.rs` (imports)
- `src/algo/topo_sort.rs` → `src/model/mod.rs` (imports)

## External Dependents

- `src/algo/mod.rs` ← `src/analysis/diff.rs` (imports)
- `src/algo/mod.rs` ← `src/analysis/metrics.rs` (imports)
- `src/algo/mod.rs` ← `src/analysis/smells.rs` (imports)
- `src/algo/mod.rs` ← `src/lib.rs` (imports)
- `src/algo/reading_order.rs` ← `src/mcp/prompts.rs` (imports)
- `src/algo/callgraph.rs` ← `src/mcp/state.rs` (imports)
- `src/algo/compress.rs` ← `src/mcp/state.rs` (imports)
- `src/algo/pagerank.rs` ← `src/mcp/state.rs` (imports)
- `src/algo/spectral.rs` ← `src/mcp/state.rs` (imports)
- `src/algo/context.rs` ← `src/mcp/tools.rs` (imports)
- `src/algo/impact.rs` ← `src/mcp/tools.rs` (imports)
- `src/algo/mod.rs` ← `src/mcp/tools.rs` (imports)
- `src/algo/reading_order.rs` ← `src/mcp/tools.rs` (imports)
- `src/algo/test_map.rs` ← `src/mcp/tools.rs` (imports)
- `src/algo/mod.rs` ← `src/pipeline/mod.rs` (imports)
- `src/algo/blast_radius.rs` ← `src/recommend/refactor.rs` (imports)
- `src/algo/callgraph.rs` ← `src/recommend/refactor.rs` (imports)
- `src/algo/mod.rs` ← `src/recommend/refactor.rs` (imports)
- `src/algo/scc.rs` ← `src/recommend/refactor.rs` (imports)
- `src/algo/blast_radius.rs` ← `src/recommend/split.rs` (imports)
- `src/algo/callgraph.rs` ← `src/recommend/split.rs` (imports)
- `src/algo/mod.rs` ← `src/recommend/split.rs` (imports)
- `src/algo/mod.rs` ← `src/temporal/mod.rs` (imports)

