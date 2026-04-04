# Cluster: semantic

## Files

| File | Type | Layer | In | Out | Centrality |
|------|------|------:|---:|----:|-----------:|
| `src/semantic/edges.rs` | source | 1 | 1 | 2 | 0.0000 |
| `src/semantic/events.rs` | source | 2 | 1 | 3 | 0.0000 |
| `src/semantic/http.rs` | source | 2 | 1 | 3 | 0.0000 |
| `src/semantic/mod.rs` | source | 2 | 5 | 5 | 0.0005 |

## Internal Dependencies

- `src/semantic/events.rs` → `src/semantic/mod.rs` (imports)
- `src/semantic/http.rs` → `src/semantic/mod.rs` (imports)
- `src/semantic/mod.rs` → `src/semantic/edges.rs` (imports)
- `src/semantic/mod.rs` → `src/semantic/events.rs` (imports)
- `src/semantic/mod.rs` → `src/semantic/http.rs` (imports)

## External Dependencies

- `src/semantic/edges.rs` → `src/model/semantic.rs` (imports)
- `src/semantic/edges.rs` → `src/model/types.rs` (imports)
- `src/semantic/events.rs` → `src/model/semantic.rs` (imports)
- `src/semantic/events.rs` → `src/model/types.rs` (imports)
- `src/semantic/http.rs` → `src/model/semantic.rs` (imports)
- `src/semantic/http.rs` → `src/model/types.rs` (imports)
- `src/semantic/mod.rs` → `src/model/semantic.rs` (imports)
- `src/semantic/mod.rs` → `src/model/types.rs` (imports)

## External Dependents

- `src/semantic/mod.rs` ← `src/lib.rs` (imports)
- `src/semantic/mod.rs` ← `src/parser/registry.rs` (imports)
- `src/semantic/mod.rs` ← `src/pipeline/mod.rs` (imports)

