# Cluster: detect

## Files

| File | Type | Layer | In | Out | Centrality |
|------|------|------:|---:|----:|-----------:|
| `src/detect/case_sensitivity.rs` | source | 1 | 1 | 1 | 0.0000 |
| `src/detect/filetype.rs` | source | 3 | 1 | 1 | 0.0000 |
| `src/detect/layer.rs` | source | 3 | 1 | 1 | 0.0000 |
| `src/detect/mod.rs` | source | 5 | 4 | 4 | 0.0003 |
| `src/detect/workspace.rs` | source | 4 | 1 | 3 | 0.0001 |

## Internal Dependencies

- `src/detect/mod.rs` → `src/detect/case_sensitivity.rs` (imports)
- `src/detect/mod.rs` → `src/detect/filetype.rs` (imports)
- `src/detect/mod.rs` → `src/detect/layer.rs` (imports)
- `src/detect/mod.rs` → `src/detect/workspace.rs` (imports)

## External Dependencies

- `src/detect/case_sensitivity.rs` → `src/model/types.rs` (imports)
- `src/detect/filetype.rs` → `src/model/mod.rs` (imports)
- `src/detect/layer.rs` → `src/model/mod.rs` (imports)
- `src/detect/workspace.rs` → `src/diagnostic.rs` (imports)
- `src/detect/workspace.rs` → `src/model/mod.rs` (imports)
- `src/detect/workspace.rs` → `src/model/workspace.rs` (imports)

## External Dependents

- `src/detect/mod.rs` ← `src/lib.rs` (imports)
- `src/detect/mod.rs` ← `src/pipeline/build.rs` (imports)
- `src/detect/mod.rs` ← `src/pipeline/mod.rs` (imports)
- `src/detect/mod.rs` ← `src/pipeline/resolve.rs` (imports)

