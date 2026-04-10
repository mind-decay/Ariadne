# Cluster: conventions

## Files

| File | Type | Layer | In | Out | Centrality |
|------|------|------:|---:|----:|-----------:|
| `src/conventions/imports.rs` | source | 3 | 1 | 2 | 0.0000 |
| `src/conventions/mod.rs` | source | 4 | 1 | 5 | 0.0000 |
| `src/conventions/naming.rs` | source | 3 | 1 | 2 | 0.0000 |
| `src/conventions/tech_stack.rs` | source | 3 | 1 | 2 | 0.0000 |
| `src/conventions/trends.rs` | source | 3 | 1 | 2 | 0.0000 |
| `src/conventions/types.rs` | source | 0 | 5 | 0 | 0.0000 |

## Internal Dependencies

- `src/conventions/imports.rs` → `src/conventions/types.rs` (imports)
- `src/conventions/mod.rs` → `src/conventions/imports.rs` (imports)
- `src/conventions/mod.rs` → `src/conventions/naming.rs` (imports)
- `src/conventions/mod.rs` → `src/conventions/tech_stack.rs` (imports)
- `src/conventions/mod.rs` → `src/conventions/trends.rs` (imports)
- `src/conventions/mod.rs` → `src/conventions/types.rs` (imports)
- `src/conventions/naming.rs` → `src/conventions/types.rs` (imports)
- `src/conventions/tech_stack.rs` → `src/conventions/types.rs` (imports)
- `src/conventions/trends.rs` → `src/conventions/types.rs` (imports)

## External Dependencies

- `src/conventions/imports.rs` → `src/model/mod.rs` (imports)
- `src/conventions/naming.rs` → `src/model/mod.rs` (imports)
- `src/conventions/tech_stack.rs` → `src/model/mod.rs` (imports)
- `src/conventions/trends.rs` → `src/model/mod.rs` (imports)

## External Dependents

- `src/conventions/mod.rs` ← `src/lib.rs` (imports)

