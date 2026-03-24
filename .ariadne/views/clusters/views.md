# Cluster: views

## Files

| File | Type | Layer | In | Out | Centrality |
|------|------|------:|---:|----:|-----------:|
| `src/views/cluster.rs` | source | 2 | 1 | 1 | 0.0000 |
| `src/views/impact.rs` | source | 2 | 1 | 1 | 0.0000 |
| `src/views/index.rs` | source | 2 | 1 | 1 | 0.0000 |
| `src/views/mod.rs` | source | 3 | 1 | 5 | 0.0001 |

## Internal Dependencies

- `src/views/mod.rs` → `src/views/cluster.rs` (imports)
- `src/views/mod.rs` → `src/views/impact.rs` (imports)
- `src/views/mod.rs` → `src/views/index.rs` (imports)

## External Dependencies

- `src/views/cluster.rs` → `src/model/mod.rs` (imports)
- `src/views/impact.rs` → `src/model/mod.rs` (imports)
- `src/views/index.rs` → `src/model/mod.rs` (imports)
- `src/views/mod.rs` → `src/diagnostic.rs` (imports)
- `src/views/mod.rs` → `src/model/mod.rs` (imports)

## External Dependents

- `src/views/mod.rs` ← `src/lib.rs` (imports)

