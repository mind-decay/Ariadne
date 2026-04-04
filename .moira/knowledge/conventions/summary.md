<!-- moira:freshness init 2026-04-04 -->
<!-- moira:knowledge conventions L1 -->

## Naming Conventions
| Files | `snake_case.rs` | `src/parser/rust_lang.rs`, `src/parser/json_lang.rs`, `src/model/symbol_index.rs`, `src/algo/blast_radius.rs`, `src/serial/convert.rs` |
| Functions | `snake_case` | `src/hash.rs:4` `pub fn hash_content`, `src/pipeline/build.rs:23` `pub fn resolve_and_build`, `src/views/mod.rs:12` `pub fn generate_all_views`, `src/algo/mod.rs:30` `pub fn round4` |
## Import Style
## Export Style
## Error Handling
- Functions return `Result<T, FatalError>` for recoverable operations: `src/views/mod.rs:17` `-> Result<usize, FatalError>`
## Logging
## Code Organization
| File | Lines | Role |
