# Cluster: temporal

## Files

| File | Type | Layer | In | Out | Centrality |
|------|------|------:|---:|----:|-----------:|
| `src/temporal/churn.rs` | source | 3 | 1 | 2 | 0.0000 |
| `src/temporal/coupling.rs` | source | 3 | 1 | 2 | 0.0000 |
| `src/temporal/git.rs` | source | 4 | 1 | 2 | 0.0000 |
| `src/temporal/hotspot.rs` | source | 1 | 1 | 2 | 0.0000 |
| `src/temporal/mod.rs` | source | 5 | 1 | 8 | 0.0000 |
| `src/temporal/ownership.rs` | source | 3 | 1 | 2 | 0.0000 |

## Internal Dependencies

- `src/temporal/mod.rs` → `src/temporal/churn.rs` (imports)
- `src/temporal/mod.rs` → `src/temporal/coupling.rs` (imports)
- `src/temporal/mod.rs` → `src/temporal/git.rs` (imports)
- `src/temporal/mod.rs` → `src/temporal/hotspot.rs` (imports)
- `src/temporal/mod.rs` → `src/temporal/ownership.rs` (imports)

## External Dependencies

- `src/temporal/churn.rs` → `src/model/mod.rs` (imports)
- `src/temporal/churn.rs` → `src/model/temporal.rs` (imports)
- `src/temporal/coupling.rs` → `src/model/mod.rs` (imports)
- `src/temporal/coupling.rs` → `src/model/temporal.rs` (imports)
- `src/temporal/git.rs` → `src/diagnostic.rs` (imports)
- `src/temporal/git.rs` → `src/model/mod.rs` (imports)
- `src/temporal/hotspot.rs` → `src/model/temporal.rs` (imports)
- `src/temporal/hotspot.rs` → `src/model/types.rs` (imports)
- `src/temporal/mod.rs` → `src/algo/mod.rs` (imports)
- `src/temporal/mod.rs` → `src/diagnostic.rs` (imports)
- `src/temporal/mod.rs` → `src/model/mod.rs` (imports)
- `src/temporal/ownership.rs` → `src/model/mod.rs` (imports)
- `src/temporal/ownership.rs` → `src/model/temporal.rs` (imports)

## External Dependents

- `src/temporal/mod.rs` ← `src/lib.rs` (imports)

