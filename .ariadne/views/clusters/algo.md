# Cluster: algo

## Files

| File | Type | Layer | In | Out | Centrality |
|------|------|------:|---:|----:|-----------:|
| `src/algo/blast_radius.rs` | source | 3 | 1 | 2 | 0.0000 |
| `src/algo/centrality.rs` | source | 3 | 1 | 2 | 0.0000 |
| `src/algo/compress.rs` | source | 2 | 2 | 2 | 0.0001 |
| `src/algo/delta.rs` | source | 2 | 1 | 1 | 0.0000 |
| `src/algo/louvain.rs` | source | 2 | 1 | 1 | 0.0000 |
| `src/algo/mod.rs` | source | 3 | 17 | 12 | 0.0075 |
| `src/algo/pagerank.rs` | source | 3 | 2 | 2 | 0.0001 |
| `src/algo/scc.rs` | source | 3 | 1 | 2 | 0.0000 |
| `src/algo/spectral.rs` | source | 3 | 2 | 2 | 0.0001 |
| `src/algo/stats.rs` | source | 3 | 1 | 2 | 0.0000 |
| `src/algo/subgraph.rs` | source | 2 | 1 | 1 | 0.0000 |
| `src/algo/topo_sort.rs` | source | 3 | 1 | 2 | 0.0000 |

## Internal Dependencies

- `src/algo/blast_radius.rs` → `src/algo/mod.rs` (imports)
- `src/algo/centrality.rs` → `src/algo/mod.rs` (imports)
- `src/algo/mod.rs` → `src/algo/blast_radius.rs` (imports)
- `src/algo/mod.rs` → `src/algo/centrality.rs` (imports)
- `src/algo/mod.rs` → `src/algo/compress.rs` (imports)
- `src/algo/mod.rs` → `src/algo/delta.rs` (imports)
- `src/algo/mod.rs` → `src/algo/louvain.rs` (imports)
- `src/algo/mod.rs` → `src/algo/pagerank.rs` (imports)
- `src/algo/mod.rs` → `src/algo/scc.rs` (imports)
- `src/algo/mod.rs` → `src/algo/spectral.rs` (imports)
- `src/algo/mod.rs` → `src/algo/stats.rs` (imports)
- `src/algo/mod.rs` → `src/algo/subgraph.rs` (imports)
- `src/algo/mod.rs` → `src/algo/topo_sort.rs` (imports)
- `src/algo/pagerank.rs` → `src/algo/mod.rs` (imports)
- `src/algo/scc.rs` → `src/algo/mod.rs` (imports)
- `src/algo/spectral.rs` → `src/algo/mod.rs` (imports)
- `src/algo/stats.rs` → `src/algo/mod.rs` (imports)
- `src/algo/topo_sort.rs` → `src/algo/mod.rs` (imports)

## External Dependencies

- `src/algo/blast_radius.rs` → `src/model/mod.rs` (imports)
- `src/algo/centrality.rs` → `src/model/mod.rs` (imports)
- `src/algo/compress.rs` → `src/model/compress.rs` (imports)
- `src/algo/compress.rs` → `src/model/mod.rs` (imports)
- `src/algo/delta.rs` → `src/model/mod.rs` (imports)
- `src/algo/louvain.rs` → `src/model/mod.rs` (imports)
- `src/algo/mod.rs` → `src/model/mod.rs` (imports)
- `src/algo/pagerank.rs` → `src/model/mod.rs` (imports)
- `src/algo/scc.rs` → `src/model/mod.rs` (imports)
- `src/algo/spectral.rs` → `src/model/mod.rs` (imports)
- `src/algo/stats.rs` → `src/model/mod.rs` (imports)
- `src/algo/subgraph.rs` → `src/model/mod.rs` (imports)
- `src/algo/topo_sort.rs` → `src/model/mod.rs` (imports)

## External Dependents

- `src/algo/mod.rs` ← `benches/algo_bench.rs` (imports)
- `src/algo/mod.rs` ← `benches/analysis_bench.rs` (imports)
- `src/algo/mod.rs` ← `benches/mcp_bench.rs` (imports)
- `src/algo/mod.rs` ← `src/analysis/diff.rs` (imports)
- `src/algo/mod.rs` ← `src/analysis/metrics.rs` (imports)
- `src/algo/mod.rs` ← `src/analysis/smells.rs` (imports)
- `src/algo/mod.rs` ← `src/lib.rs` (imports)
- `src/algo/mod.rs` ← `src/main.rs` (imports)
- `src/algo/compress.rs` ← `src/mcp/state.rs` (imports)
- `src/algo/pagerank.rs` ← `src/mcp/state.rs` (imports)
- `src/algo/spectral.rs` ← `src/mcp/state.rs` (imports)
- `src/algo/mod.rs` ← `src/mcp/tools.rs` (imports)
- `src/algo/mod.rs` ← `src/pipeline/mod.rs` (imports)

