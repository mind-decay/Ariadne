<!-- moira:freshness init 2026-03-21 -->
<!-- moira:knowledge conventions L2 -->

---
naming_files: snake_case
naming_functions: snake_case
naming_components: PascalCase
naming_constants: SCREAMING_SNAKE_CASE
naming_types: PascalCase
indent: "4 spaces"
semicolons: "N/A (Rust)"
max_line_length: "100 (rustfmt default, no override config found)"
import_style: "use-declarations, grouped by std then external crates then crate-internal"
export_style: "pub use re-exports in mod.rs barrel files"
---

# Coding Convention Scan — Ariadne

**Scanned**: 2026-03-21
**Language**: Rust (edition 2021)
**Files sampled**: 22 source files, 3 test files

## 1. Naming Conventions

### File Naming: `snake_case`
All source files use snake_case naming. No exceptions found in 63 `.rs` files.
- Evidence: `src/parser/rust_lang.rs` (file:1), `src/detect/case_sensitivity.rs` (file:1), `src/algo/blast_radius.rs` (file:1), `src/algo/topo_sort.rs` (file:1)
- Module files: `mod.rs` pattern used for all module directories (e.g., `src/model/mod.rs`, `src/parser/mod.rs`, `src/algo/mod.rs`)

### Function Naming: `snake_case`
All functions and methods follow Rust standard `snake_case`.
- Evidence: `src/pipeline/build.rs:15` — `pub fn resolve_and_build(...)`, `src/algo/scc.rs:9` — `pub fn find_sccs(...)`, `src/hash.rs:4` — `pub fn hash_content(...)`, `src/algo/mod.rs:24` — `pub fn round4(...)`, `src/cluster/mod.rs:8` — `pub fn assign_clusters(...)`

### Type/Struct Naming: `PascalCase`
All structs, enums, and traits use PascalCase.
- Evidence: `src/model/types.rs:7` — `pub struct CanonicalPath`, `src/model/types.rs:77` — `pub struct ContentHash`, `src/model/node.rs:61` — `pub struct Node`, `src/diagnostic.rs:10` — `pub enum FatalError`, `src/parser/traits.rs:31` — `pub trait LanguageParser`, `src/serial/mod.rs:14` — `pub struct GraphOutput`

### Enum Variant Naming: `PascalCase`
- Evidence: `src/model/node.rs:8-15` — `FileType::Source`, `FileType::Test`, etc.; `src/model/edge.rs:8-13` — `EdgeType::Imports`, `EdgeType::ReExports`; `src/diagnostic.rs:10-37` — `FatalError::ProjectNotFound`, `FatalError::NotADirectory`

### Warning Code Variants: `PascalCase` with `W###` prefix embedded in name
- Evidence: `src/diagnostic.rs:43-59` — `WarningCode::W001ParseFailed`, `WarningCode::W006ImportUnresolved`, `WarningCode::W018BlastRadiusTimeout`

### Constant Naming: `SCREAMING_SNAKE_CASE`
Only one constant found in sampled files.
- Evidence: `src/pipeline/walk.rs:69` — `const MAX_DEPTH: usize = 64;`

### Test Function Naming: `snake_case`, descriptive
- Evidence: `src/model/types.rs:199` — `fn normalize_basic_path_unchanged()`, `src/algo/scc.rs:132` — `fn linear_chain_no_sccs()`, `src/algo/scc.rs:139` — `fn simple_cycle()`, `src/diagnostic.rs:356` — `fn human_format_with_detail()`

## 2. Import Style

### Ordering Pattern: `std` -> external crates -> `crate::` internal
Consistent across all sampled files. Blank line separates groups.
- Evidence: `src/pipeline/mod.rs:6-20`:
  - `std::path::{Path, PathBuf}` and `std::time::Instant` (std group)
  - `rayon::prelude::*` (external crate)
  - `crate::algo`, `crate::cluster::assign_clusters`, etc. (crate-internal)
- Evidence: `src/diagnostic.rs:1-5`:
  - `std::fmt`, `std::path::PathBuf`, `std::sync::Mutex` (std)
  - `crate::model::CanonicalPath` (crate-internal)
- Evidence: `src/main.rs:1-16`:
  - `std::path::PathBuf`, `std::process`, `std::time::Instant` (std)
  - `clap::{Parser, Subcommand}` (external)
  - `ariadne_graph::*` (crate, via library name)

### Named Imports Preferred
Named/specific imports are used. Glob imports (`*`) are rare.
- Evidence: `src/pipeline/mod.rs:9` — `use rayon::prelude::*;` (only idiomatic Rayon glob)
- Evidence: `src/pipeline/mod.rs:14` — `use crate::model::*;` (model glob in pipeline — exception)
- Evidence: `src/algo/scc.rs:101` — `use crate::model::*;` (in test module only)
- Typical: `src/serial/mod.rs:7` — `use serde::{Deserialize, Serialize};` (specific items)

### `use super::*` in test modules
- Evidence: `src/model/types.rs:193` — `use super::*;` inside `#[cfg(test)] mod tests`
- Evidence: `src/diagnostic.rs:340` — `use super::*;` inside `#[cfg(test)] mod tests`
- Evidence: `src/algo/scc.rs:100` — `use super::*;` inside `#[cfg(test)] mod tests`

## 3. Export Style

### Barrel Re-exports via `mod.rs`
Each module directory has a `mod.rs` that declares submodules and re-exports public items with `pub use`.
- Evidence: `src/model/mod.rs:1-23` — declares 10 submodules, then `pub use` re-exports for all major types (`Edge`, `EdgeType`, `Node`, `ArchLayer`, `ProjectGraph`, `CanonicalPath`, etc.)
- Evidence: `src/parser/mod.rs:1-11` — declares 7 submodules (5 private, 2 public), re-exports `ParserRegistry`, `ParseOutcome`, traits, and raw types
- Evidence: `src/detect/mod.rs:1-9` — declares 4 submodules, re-exports `detect_file_type`, `infer_arch_layer`, `detect_workspace`, `is_case_insensitive`, `find_case_insensitive`
- Evidence: `src/pipeline/mod.rs:22-23` — `pub use read::...` and `pub use walk::...`

### `src/lib.rs` as top-level barrel
- Evidence: `src/lib.rs:1-13` — `pub mod algo; pub mod analysis; ... pub mod views;` — all modules declared public, with `#[cfg(feature = "serve")] pub mod mcp;` for conditional compilation

### Visibility: Mix of `pub` and private
Most submodules in `parser/mod.rs` are private (`mod csharp;`, `mod go;`), with only `pub mod registry;` and `pub mod traits;` public. Re-exports expose the needed items.
- Evidence: `src/parser/mod.rs:1-8`

## 4. Error Handling

### `thiserror`-derived `FatalError` enum for fatal/unrecoverable errors
- Evidence: `src/diagnostic.rs:9-37` — `#[derive(Debug, thiserror::Error)] pub enum FatalError` with 13 variants (E001-E013), each with `#[error("...")]` format strings

### `Result<T, FatalError>` return type for fallible operations
- Evidence: `src/views/mod.rs:17` — `fn generate_all_views(...) -> Result<usize, FatalError>`
- Evidence: `src/serial/mod.rs:75-76` — `fn write_graph(...) -> Result<(), FatalError>`
- Evidence: `src/pipeline/mod.rs:80` — `pub fn run(...) -> Result<BuildOutput, FatalError>`

### `DiagnosticCollector` for non-fatal warnings (thread-safe, `Mutex<...>`)
- Evidence: `src/diagnostic.rs:250-336` — `pub struct DiagnosticCollector` with `Mutex<(Vec<Warning>, DiagnosticCounts)>`, `.warn()`, `.drain()` methods
- Evidence: `src/pipeline/mod.rs:93` — `let diagnostics = DiagnosticCollector::new();` instantiated at pipeline start
- Poison recovery: `src/diagnostic.rs:263` — `.unwrap_or_else(|e| e.into_inner())` on Mutex lock

### Warning taxonomy: `WarningCode` enum (W001-W018)
- Evidence: `src/diagnostic.rs:42-61` — 18 warning codes covering parse failures, read errors, unresolved imports, algorithm failures, etc.

### No `panic!`/`unwrap` in production code paths
`unwrap()` usage observed only in test helpers and diagnostic Mutex recovery (intentional poison recovery).
- Evidence: `tests/helpers.rs:20` — `.expect("create tempdir")` (test code)
- Evidence: `src/diagnostic.rs:263` — `.unwrap_or_else(|e| e.into_inner())` (Mutex poison recovery, not a bare unwrap)

## 5. Logging

### No logging framework used
No `log`, `tracing`, `env_logger`, `slog`, or any structured logging crate found in `Cargo.toml` or source files.

### `eprintln!` for verbose/diagnostic output
All runtime diagnostic output uses `eprintln!` directly, gated by a `verbose: bool` flag.
- Evidence: `src/pipeline/mod.rs:109-113` — `if verbose { eprintln!("[walk]      {:>6}ms  {} files found", ...); }`
- Evidence: `src/pipeline/mod.rs:136-143` — `if verbose { eprintln!("[read+hash] {:>6}ms  ..."); }`
- Pattern: `[stage_name]` prefix with right-aligned millisecond timing: `[walk]`, `[read+hash]`, `[parse]`, `[resolve]`, `[cluster]`, `[louvain]`, `[algorithms]`, `[serialize]`, `[total]`, `[delta]`
- Count: ~46 `eprintln!` occurrences across 5 source files (`main.rs`, `pipeline/mod.rs`, `pipeline/walk.rs`, `mcp/server.rs`, `mcp/watch.rs`)

### `println!` for user-facing output
- Evidence: `src/main.rs` — 91 occurrences of `println!` for CLI output (build summaries, query results, etc.)

## 6. Code Organization

### File Length Distribution
- Largest: `src/main.rs` at 1160 lines (CLI composition root with all subcommand handlers)
- Typical range: 50-700 lines
- Smallest: `src/analysis/mod.rs` at 3 lines, `src/mcp/mod.rs` at 5 lines
- Median range: ~200-500 lines for substantial modules

### Module Structure
- Deep module hierarchy: `src/{domain}/mod.rs` + `src/{domain}/{concern}.rs`
- Domains: `model/`, `parser/`, `pipeline/`, `algo/`, `detect/`, `serial/`, `views/`, `cluster/`, `mcp/`, `analysis/`
- Leaf modules: `diagnostic.rs`, `hash.rs`, `lib.rs`, `main.rs` at `src/` root

### Comment Style
- Doc comments (`///`) are the dominant comment form: 497 occurrences across 58 files
- Inline comments (`// ...`) are used sparingly: 4 occurrences of standalone line comments in only 3 files
- Step-numbered comments in implementation: `// Step 1:`, `// Step 2:`, etc.
  - Evidence: `src/cluster/mod.rs:9` — `// Step 1: Group files by cluster name`, `src/cluster/mod.rs:22` — `// Step 2: Build a lookup...`
  - Evidence: `src/pipeline/build.rs:23` — `// 1. Build FileSet`, `// 2-3. Detect FileType...`, `// 4-7. Resolve imports...`
- Stage-numbered comments in pipeline: `// Stage 1: Walk`, `// Stage 2: Read`, etc.
  - Evidence: `src/pipeline/mod.rs:100-101` — `// Stage 1: Walk`
- Decision references in comments: `(D-006)`, `(D-022)`, `(D-024)`, `(D-025)`, `(D-034)`, `(D-049)`, `(D-054)`
  - Evidence: `src/algo/mod.rs:23` — `/// Round to 4 decimal places — standardized float determinism utility (D-049).`
  - Evidence: `src/model/types.rs:154` — `/// Set of known files for import resolution existence checks (D-024).`
  - Evidence: `src/serial/mod.rs:12` — `/// Output model for graph.json (D-022).`
- No `//!` (inner doc comments) found in any sampled file.

### Inline Test Modules
Tests are co-located using `#[cfg(test)] mod tests { ... }` at the bottom of source files.
- Evidence: `src/model/types.rs:192-335` — tests block within the types module
- Evidence: `src/diagnostic.rs:338-507` — tests block within diagnostic module
- Evidence: `src/algo/scc.rs:98-223` — tests block within SCC module

### Integration tests in `tests/` directory
Separate integration test files in `tests/`:
- `tests/graph_tests.rs` — fixture-based graph construction tests
- `tests/pipeline_tests.rs` — pipeline integration tests
- `tests/invariants.rs` — property/invariant tests
- `tests/properties.rs` — property-based tests (proptest)
- `tests/helpers.rs` — shared test utilities
- `tests/mcp_tests.rs` — MCP server tests

### Test Helper Pattern
Shared helper functions in `tests/helpers.rs`, imported as `mod helpers;` in test files.
- Evidence: `tests/graph_tests.rs:1` — `mod helpers;`
- Evidence: `tests/helpers.rs:8-13` — `pub fn fixture_path(name: &str) -> PathBuf`
- Evidence: `tests/helpers.rs:17-37` — `pub fn build_fixture(name: &str) -> BuildOutput`

### Newtype Pattern
Pervasive use of newtype wrappers for domain types:
- `CanonicalPath(String)`, `ContentHash(String)`, `ClusterId(String)`, `Symbol(String)`, `FileSet(BTreeSet<CanonicalPath>)`
- Evidence: `src/model/types.rs:7`, `src/model/types.rs:77`, `src/model/types.rs:102`, `src/model/types.rs:132`, `src/model/types.rs:157`

### Derive Patterns
- Data types: `#[derive(Clone, Debug)]` as baseline, adding `PartialEq, Eq, Hash` as needed
- Serializable enums: `#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]` with `#[serde(rename_all = "snake_case")]`
  - Evidence: `src/model/node.rs:6-7` — `FileType` enum
  - Evidence: `src/model/edge.rs:6-7` — `EdgeType` enum
- Output structs: `#[derive(Clone, Debug, Serialize, Deserialize)]`
  - Evidence: `src/serial/mod.rs:13` — `GraphOutput`

### `BTreeMap`/`BTreeSet` over `HashMap`/`HashSet` for determinism
- Evidence: `src/model/types.rs:1` — `use std::collections::BTreeSet;` for `FileSet`
- Evidence: `src/model/graph.rs` (implied by `ProjectGraph` using `BTreeMap<CanonicalPath, Node>`)
- Evidence: `src/algo/scc.rs:1` — `use std::collections::BTreeMap;`
- Evidence: `src/cluster/mod.rs:1` — `use std::collections::BTreeMap;`
- Evidence: `src/pipeline/build.rs:1` — `use std::collections::BTreeMap;`

### Trait-Based Abstraction at Module Boundaries
- `LanguageParser` and `ImportResolver` traits: `src/parser/traits.rs:31,40`
- `GraphSerializer` and `GraphReader` traits: `src/serial/mod.rs:74,87`
- `FileWalker` and `FileReader` traits: `src/pipeline/mod.rs` (re-exported from `walk.rs` and `read.rs`)

### `#[allow(...)]` annotations (sparingly used)
- `#[allow(clippy::too_many_arguments)]`: `src/main.rs:389,461`, `src/analysis/diff.rs:9`, `src/pipeline/mod.rs:401`
- `#[allow(clippy::should_implement_trait)]`: `src/model/types.rs:164`
- `#[allow(clippy::type_complexity)]`: `src/algo/mod.rs:31`
- `#[allow(dead_code)]`: `tests/helpers.rs:41`

## 7. Items Searched For But Not Found

- **rustfmt.toml / .rustfmt.toml**: No formatter configuration file. Project uses rustfmt defaults.
- **clippy.toml / .clippy.toml**: No clippy configuration file. Project uses clippy defaults.
- **.editorconfig**: Not present.
- **Logging crate** (`log`, `tracing`, `env_logger`, `slog`): Not used. `eprintln!` only.
- **`//!` inner doc comments**: None found in any source file.
- **`static` declarations**: None found in source files.
- **`unsafe` blocks**: Not searched exhaustively but none observed in sampled files.
- **Macros (`macro_rules!`)**: None observed in sampled files.
- **Feature gates beyond `serve`**: Only `#[cfg(feature = "serve")]` found (for MCP server module).
