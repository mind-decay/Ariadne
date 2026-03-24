# Cluster: model

## Files

| File | Type | Layer | In | Out | Centrality |
|------|------|------:|---:|----:|-----------:|
| `src/model/compress.rs` | source | 0 | 2 | 0 | 0.0000 |
| `src/model/diff.rs` | source | 0 | 1 | 0 | 0.0000 |
| `src/model/edge.rs` | source | 0 | 1 | 0 | 0.0000 |
| `src/model/graph.rs` | source | 0 | 1 | 0 | 0.0000 |
| `src/model/mod.rs` | source | 1 | 48 | 10 | 0.0179 |
| `src/model/node.rs` | source | 0 | 1 | 0 | 0.0000 |
| `src/model/query.rs` | source | 0 | 1 | 0 | 0.0000 |
| `src/model/smell.rs` | source | 0 | 1 | 0 | 0.0000 |
| `src/model/stats.rs` | source | 0 | 1 | 0 | 0.0000 |
| `src/model/types.rs` | source | 0 | 2 | 0 | 0.0000 |
| `src/model/workspace.rs` | source | 0 | 12 | 0 | 0.0000 |

## Internal Dependencies

- `src/model/mod.rs` → `src/model/compress.rs` (imports)
- `src/model/mod.rs` → `src/model/diff.rs` (imports)
- `src/model/mod.rs` → `src/model/edge.rs` (imports)
- `src/model/mod.rs` → `src/model/graph.rs` (imports)
- `src/model/mod.rs` → `src/model/node.rs` (imports)
- `src/model/mod.rs` → `src/model/query.rs` (imports)
- `src/model/mod.rs` → `src/model/smell.rs` (imports)
- `src/model/mod.rs` → `src/model/stats.rs` (imports)
- `src/model/mod.rs` → `src/model/types.rs` (imports)
- `src/model/mod.rs` → `src/model/workspace.rs` (imports)

## External Dependents

- `src/model/mod.rs` ← `benches/algo_bench.rs` (imports)
- `src/model/mod.rs` ← `benches/analysis_bench.rs` (imports)
- `src/model/mod.rs` ← `benches/mcp_bench.rs` (imports)
- `src/model/mod.rs` ← `src/algo/blast_radius.rs` (imports)
- `src/model/mod.rs` ← `src/algo/centrality.rs` (imports)
- `src/model/compress.rs` ← `src/algo/compress.rs` (imports)
- `src/model/mod.rs` ← `src/algo/compress.rs` (imports)
- `src/model/mod.rs` ← `src/algo/delta.rs` (imports)
- `src/model/mod.rs` ← `src/algo/louvain.rs` (imports)
- `src/model/mod.rs` ← `src/algo/mod.rs` (imports)
- `src/model/mod.rs` ← `src/algo/pagerank.rs` (imports)
- `src/model/mod.rs` ← `src/algo/scc.rs` (imports)
- `src/model/mod.rs` ← `src/algo/spectral.rs` (imports)
- `src/model/mod.rs` ← `src/algo/stats.rs` (imports)
- `src/model/mod.rs` ← `src/algo/subgraph.rs` (imports)
- `src/model/mod.rs` ← `src/algo/topo_sort.rs` (imports)
- `src/model/mod.rs` ← `src/analysis/diff.rs` (imports)
- `src/model/mod.rs` ← `src/analysis/metrics.rs` (imports)
- `src/model/mod.rs` ← `src/analysis/smells.rs` (imports)
- `src/model/mod.rs` ← `src/cluster/mod.rs` (imports)
- `src/model/types.rs` ← `src/detect/case_sensitivity.rs` (imports)
- `src/model/mod.rs` ← `src/detect/filetype.rs` (imports)
- `src/model/mod.rs` ← `src/detect/layer.rs` (imports)
- `src/model/mod.rs` ← `src/detect/workspace.rs` (imports)
- `src/model/workspace.rs` ← `src/detect/workspace.rs` (imports)
- `src/model/mod.rs` ← `src/diagnostic.rs` (imports)
- `src/model/mod.rs` ← `src/hash.rs` (imports)
- `src/model/mod.rs` ← `src/lib.rs` (imports)
- `src/model/mod.rs` ← `src/main.rs` (imports)
- `src/model/mod.rs` ← `src/mcp/state.rs` (imports)
- `src/model/mod.rs` ← `src/mcp/tools.rs` (imports)
- `src/model/mod.rs` ← `src/parser/csharp.rs` (imports)
- `src/model/workspace.rs` ← `src/parser/csharp.rs` (imports)
- `src/model/mod.rs` ← `src/parser/go.rs` (imports)
- `src/model/workspace.rs` ← `src/parser/go.rs` (imports)
- `src/model/mod.rs` ← `src/parser/java.rs` (imports)
- `src/model/workspace.rs` ← `src/parser/java.rs` (imports)
- `src/model/mod.rs` ← `src/parser/markdown.rs` (imports)
- `src/model/workspace.rs` ← `src/parser/markdown.rs` (imports)
- `src/model/mod.rs` ← `src/parser/python.rs` (imports)
- `src/model/workspace.rs` ← `src/parser/python.rs` (imports)
- `src/model/mod.rs` ← `src/parser/rust_lang.rs` (imports)
- `src/model/workspace.rs` ← `src/parser/rust_lang.rs` (imports)
- `src/model/mod.rs` ← `src/parser/traits.rs` (imports)
- `src/model/workspace.rs` ← `src/parser/traits.rs` (imports)
- `src/model/mod.rs` ← `src/parser/typescript.rs` (imports)
- `src/model/workspace.rs` ← `src/parser/typescript.rs` (imports)
- `src/model/mod.rs` ← `src/pipeline/build.rs` (imports)
- `src/model/workspace.rs` ← `src/pipeline/build.rs` (imports)
- `src/model/mod.rs` ← `src/pipeline/mod.rs` (imports)
- `src/model/mod.rs` ← `src/pipeline/read.rs` (imports)
- `src/model/mod.rs` ← `src/pipeline/resolve.rs` (imports)
- `src/model/workspace.rs` ← `src/pipeline/resolve.rs` (imports)
- `src/model/mod.rs` ← `src/pipeline/walk.rs` (imports)
- `src/model/mod.rs` ← `src/serial/convert.rs` (imports)
- `src/model/mod.rs` ← `src/serial/json.rs` (imports)
- `src/model/mod.rs` ← `src/serial/mod.rs` (imports)
- `src/model/mod.rs` ← `src/views/cluster.rs` (imports)
- `src/model/mod.rs` ← `src/views/impact.rs` (imports)
- `src/model/mod.rs` ← `src/views/index.rs` (imports)
- `src/model/mod.rs` ← `src/views/mod.rs` (imports)

## Tests

- `tests/pipeline_tests.rs` tests `src/model/mod.rs`
- `tests/properties.rs` tests `src/model/mod.rs`

