# Cluster: analysis

## Files

| File | Type | Layer | In | Out | Centrality |
|------|------|------:|---:|----:|-----------:|
| `src/analysis/diff.rs` | source | 6 | 3 | 4 | 0.0005 |
| `src/analysis/metrics.rs` | source | 4 | 5 | 2 | 0.0003 |
| `src/analysis/mod.rs` | source | 7 | 1 | 3 | 0.0001 |
| `src/analysis/smells.rs` | source | 5 | 4 | 3 | 0.0003 |

## Internal Dependencies

- `src/analysis/diff.rs` → `src/analysis/metrics.rs` (imports)
- `src/analysis/diff.rs` → `src/analysis/smells.rs` (imports)
- `src/analysis/mod.rs` → `src/analysis/diff.rs` (imports)
- `src/analysis/mod.rs` → `src/analysis/metrics.rs` (imports)
- `src/analysis/mod.rs` → `src/analysis/smells.rs` (imports)
- `src/analysis/smells.rs` → `src/analysis/metrics.rs` (imports)

## External Dependencies

- `src/analysis/diff.rs` → `src/algo/mod.rs` (imports)
- `src/analysis/diff.rs` → `src/model/mod.rs` (imports)
- `src/analysis/metrics.rs` → `src/algo/mod.rs` (imports)
- `src/analysis/metrics.rs` → `src/model/mod.rs` (imports)
- `src/analysis/smells.rs` → `src/algo/mod.rs` (imports)
- `src/analysis/smells.rs` → `src/model/mod.rs` (imports)

## External Dependents

- `src/analysis/diff.rs` ← `benches/analysis_bench.rs` (imports)
- `src/analysis/metrics.rs` ← `benches/analysis_bench.rs` (imports)
- `src/analysis/smells.rs` ← `benches/analysis_bench.rs` (imports)
- `src/analysis/mod.rs` ← `src/lib.rs` (imports)
- `src/analysis/metrics.rs` ← `src/mcp/state.rs` (imports)
- `src/analysis/smells.rs` ← `src/mcp/tools.rs` (imports)
- `src/analysis/diff.rs` ← `src/mcp/watch.rs` (imports)

