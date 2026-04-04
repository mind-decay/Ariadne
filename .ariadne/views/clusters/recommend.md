# Cluster: recommend

## Files

| File | Type | Layer | In | Out | Centrality |
|------|------|------:|---:|----:|-----------:|
| `src/recommend/min_cut.rs` | source | 0 | 2 | 0 | 0.0000 |
| `src/recommend/mod.rs` | source | 7 | 2 | 6 | 0.0002 |
| `src/recommend/pareto.rs` | source | 0 | 2 | 0 | 0.0000 |
| `src/recommend/placement.rs` | source | 3 | 1 | 1 | 0.0000 |
| `src/recommend/refactor.rs` | source | 6 | 1 | 12 | 0.0001 |
| `src/recommend/split.rs` | source | 5 | 2 | 12 | 0.0002 |
| `src/recommend/types.rs` | source | 0 | 3 | 0 | 0.0000 |

## Internal Dependencies

- `src/recommend/mod.rs` → `src/recommend/min_cut.rs` (imports)
- `src/recommend/mod.rs` → `src/recommend/pareto.rs` (imports)
- `src/recommend/mod.rs` → `src/recommend/placement.rs` (imports)
- `src/recommend/mod.rs` → `src/recommend/refactor.rs` (imports)
- `src/recommend/mod.rs` → `src/recommend/split.rs` (imports)
- `src/recommend/mod.rs` → `src/recommend/types.rs` (imports)
- `src/recommend/refactor.rs` → `src/recommend/pareto.rs` (imports)
- `src/recommend/refactor.rs` → `src/recommend/split.rs` (imports)
- `src/recommend/refactor.rs` → `src/recommend/types.rs` (imports)
- `src/recommend/split.rs` → `src/recommend/min_cut.rs` (imports)
- `src/recommend/split.rs` → `src/recommend/types.rs` (imports)

## External Dependencies

- `src/recommend/placement.rs` → `src/model/mod.rs` (imports)
- `src/recommend/refactor.rs` → `src/algo/blast_radius.rs` (imports)
- `src/recommend/refactor.rs` → `src/algo/callgraph.rs` (imports)
- `src/recommend/refactor.rs` → `src/algo/mod.rs` (imports)
- `src/recommend/refactor.rs` → `src/algo/scc.rs` (imports)
- `src/recommend/refactor.rs` → `src/model/graph.rs` (imports)
- `src/recommend/refactor.rs` → `src/model/mod.rs` (imports)
- `src/recommend/refactor.rs` → `src/model/smell.rs` (imports)
- `src/recommend/refactor.rs` → `src/model/symbol_index.rs` (imports)
- `src/recommend/refactor.rs` → `src/model/temporal.rs` (imports)
- `src/recommend/split.rs` → `src/algo/blast_radius.rs` (imports)
- `src/recommend/split.rs` → `src/algo/callgraph.rs` (imports)
- `src/recommend/split.rs` → `src/algo/mod.rs` (imports)
- `src/recommend/split.rs` → `src/model/edge.rs` (imports)
- `src/recommend/split.rs` → `src/model/graph.rs` (imports)
- `src/recommend/split.rs` → `src/model/mod.rs` (imports)
- `src/recommend/split.rs` → `src/model/node.rs` (imports)
- `src/recommend/split.rs` → `src/model/symbol.rs` (imports)
- `src/recommend/split.rs` → `src/model/symbol_index.rs` (imports)
- `src/recommend/split.rs` → `src/model/temporal.rs` (imports)

## External Dependents

- `src/recommend/mod.rs` ← `src/lib.rs` (imports)
- `src/recommend/mod.rs` ← `src/mcp/tools.rs` (imports)

