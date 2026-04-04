<!-- moira:freshness init 2026-04-04 -->
<!-- moira:knowledge conventions L2 -->

---
naming_files: snake_case
naming_functions: snake_case
naming_constants: UPPER_SNAKE_CASE
naming_types: PascalCase
indent: 4 spaces
import_style: grouped_std_crate_super
export_style: pub_use_reexport
---

## Naming Conventions

| Category | Convention | Evidence |
|----------|-----------|----------|
| Files | `snake_case.rs` | `src/parser/rust_lang.rs`, `src/parser/json_lang.rs`, `src/model/symbol_index.rs`, `src/algo/blast_radius.rs`, `src/serial/convert.rs` |
| Functions | `snake_case` | `src/hash.rs:4` `pub fn hash_content`, `src/pipeline/build.rs:23` `pub fn resolve_and_build`, `src/views/mod.rs:12` `pub fn generate_all_views`, `src/algo/mod.rs:30` `pub fn round4` |
| Constants | `UPPER_SNAKE_CASE` | `src/pipeline/walk.rs:69` `const MAX_DEPTH`, `src/algo/louvain.rs:110-112` `const MAX_OUTER_ITERATIONS / MAX_INNER_ITERATIONS / CONVERGENCE_THRESHOLD`, `src/semantic/http.rs:6` `const MAX_BOUNDARIES_PER_FILE`, `src/semantic/http.rs:52` `const HTTP_METHODS` |
| Types (structs) | `PascalCase` | `src/model/types.rs:7` `pub struct CanonicalPath`, `src/diagnostic.rs:10` `pub enum FatalError`, `src/parser/traits.rs:17` `pub struct RawImport`, `src/analysis/metrics.rs:10` `pub struct ClusterMetrics` |
| Enum variants | `PascalCase` | `src/diagnostic.rs:44` `W001ParseFailed`, `src/analysis/metrics.rs:24` `MainSequence / ZoneOfPain`, `src/model/node.rs` `ArchLayer` variants |
| Modules | `snake_case` | `src/lib.rs:1-16` all module declarations: `algo`, `analysis`, `cluster`, `detect`, `serial`, `temporal`, `semantic` |
| Test functions | `snake_case` (descriptive, no `test_` prefix) | `tests/graph_tests.rs:8` `fn typescript_app()`, `tests/graph_tests.rs:15` `fn go_service()`, `tests/pipeline_tests.rs:15` `fn diagnostic_collector_empty()` |
| Warning codes | `PascalCase` with code prefix | `src/diagnostic.rs:46-77` `W001ParseFailed` through `W033ExtendsNotFound` |
| Error codes | Numeric string prefix in display | `src/diagnostic.rs:13` `"E001: project root not found"` through `E014` |

## Import Style

Imports follow a consistent **three-group** ordering separated by blank lines:

1. `std::` imports (standard library)
2. External crate imports (e.g., `serde`, `clap`, `tree_sitter`)
3. `crate::` / `super::` imports (internal)

Evidence:
- `src/algo/centrality.rs:1,3-4` — `use std::collections::{...}` then blank line then `use crate::algo::...` and `use crate::model::...`
- `src/views/index.rs:1-4` — `use std::collections::BTreeMap` / `use std::fmt::Write` then blank line then `use crate::model::...`
- `src/pipeline/walk.rs:1,3-4` — `use std::path::{...}` then blank line then `use crate::diagnostic::...` / `use crate::model::...`
- `src/main.rs:1-18` — `use std::` block, then `use clap::`, then `use ariadne_graph::` block

Within each group, imports are alphabetically ordered. Multiple items from the same crate path are grouped with `{...}` syntax.

Glob imports (`use crate::model::*`) appear occasionally in implementation files (`src/pipeline/build.rs:6`, `src/analysis/metrics.rs:6`) but not in public interfaces.

## Export Style

**Re-export via `pub use` in `mod.rs` files.** Each module's `mod.rs` declares submodules, then re-exports key types.

Evidence:
- `src/model/mod.rs:18-35` — 18 `pub use` lines re-exporting types from submodules (e.g., `pub use edge::{Edge, EdgeType}`, `pub use graph::{Cluster, ClusterMap, ProjectGraph}`)
- `src/detect/mod.rs:6-9` — `pub use` for `detect_file_type`, `infer_arch_layer`, `detect_workspace`
- `src/recommend/mod.rs:8-13` — `pub use` for functions and a glob re-export (`pub use types::*`)
- `src/lib.rs:1-16` — flat `pub mod` declarations, no re-exports at crate root

Internal types use `pub(crate)` for restricted visibility (41 occurrences across 16 files).

## Error Handling

**`Result<T, FatalError>` pattern with `thiserror` derive for error types.**

- Fatal errors: `src/diagnostic.rs:10` — `#[derive(Debug, thiserror::Error)] pub enum FatalError` with 14 variants (E001-E014), each with `#[error("...")]` format string
- Warnings: `src/diagnostic.rs:43` — `pub enum WarningCode` with 33 variants (W001-W033), collected via `DiagnosticCollector` (thread-safe via `Mutex`)
- Functions return `Result<T, FatalError>` for recoverable operations: `src/views/mod.rs:17` `-> Result<usize, FatalError>`
- Non-fatal issues are collected as warnings, not propagated: `src/pipeline_tests.rs:32-36` `collector.warn(Warning { code, path, message, detail })`
- No `unwrap()` in library code; `unwrap()` used only in tests and `main.rs`

## Logging

**No logging framework.** Output uses raw `eprintln!` for warnings/errors and `println!` for user-facing output, exclusively in `src/main.rs`.

Evidence:
- `src/main.rs:501` `eprintln!("{}", e)` for fatal errors
- `src/main.rs:651,737` `println!` for build/update summaries
- `src/main.rs:664,750` `eprintln!` for warning output
- No `log`, `tracing`, or `env_logger` dependency in `Cargo.toml`
- Library crates (`src/`) produce no output — all IO is in the CLI binary (`main.rs`)

## Code Organization

### File lengths (sampled)

| File | Lines | Role |
|------|-------|------|
| `src/main.rs` | 1875 | CLI entry point (largest file) |
| `src/parser/typescript.rs` | 1686 | Language parser (includes inline tests) |
| `src/parser/rust_lang.rs` | 1012 | Language parser (includes inline tests) |
| `src/analysis/smells.rs` | 879 | Smell detection |
| `src/diagnostic.rs` | 556 | Error/warning types |
| `src/pipeline/build.rs` | 367 | Graph building stage |
| `src/algo/centrality.rs` | 202 | BFS centrality algorithm |
| `src/serial/json.rs` | 183 | JSON serialization |
| `src/model/graph.rs` | 28 | Data model struct |
| `src/hash.rs` | 7 | Hash utility (minimal) |

Most files are under 500 lines. Parser files are the largest due to inline `#[cfg(test)] mod tests` blocks.

### Comment style

- Doc comments: `///` on all public items (`src/model/types.rs:4` `/// Canonical file path relative to project root.`, `src/parser/traits.rs:32` `/// Extracts imports/exports from AST`)
- Inline comments: `//` for implementation notes, often referencing decision IDs (`src/pipeline/build.rs:50` `// D-025: placeholder, computed in Phase 2`)
- Section separators in test files: `// ---------------------------------------------------------------------------` (`tests/pipeline_tests.rs:12-13`)
- No `/* */` block comments observed

### Test organization

- **Integration tests**: `tests/*.rs` files with a shared `tests/helpers.rs` module
- **Unit tests**: `#[cfg(test)] mod tests` blocks inside source files (found in 15+ source files)
- Test helper pattern: `tests/helpers.rs` provides `build_fixture()` for fixture-based testing
- Test fixtures: `tests/fixtures/` directory with per-language sample projects

### Module structure

- Each major feature is a directory module with `mod.rs`
- `mod.rs` contains submodule declarations + `pub use` re-exports
- Leaf modules are single `.rs` files
- Trait definitions separated from implementations (`src/parser/traits.rs` defines traits, `src/parser/typescript.rs` implements them)
