<!-- moira:freshness init 2026-03-21 -->
<!-- moira:knowledge patterns L2 -->

---
api_style: "CLI (clap subcommands) + MCP server (rmcp/JSON-RPC over stdio)"
api_handler_structure: "clap enum dispatch in main.rs; each subcommand handled inline with match arms"
api_validation: "clap derives with arg constraints (value_parser, default_value_t); FatalError enum for runtime validation"
api_response_format: "JSON files on disk (graph.json, clusters.json, stats.json); human or JSON warning output to stderr"
data_fetching: "N/A - not a client application"
error_handling: "Two-tier: FatalError (thiserror enum, coded E001-E013, stops pipeline) + Warning (coded W001-W018, collected via DiagnosticCollector, non-fatal)"
component_structure: "N/A - no frontend"
component_state: "N/A - no frontend"
component_styling: "N/A - no frontend"
client_state: "N/A - no frontend"
server_state: "Arc<ArcSwap<GraphState>> for MCP server hot-reload; AtomicBool for rebuild-in-progress flag"
---

# Recurring Pattern Scan: Ariadne

Scanned 2026-03-21. 22 files read across 10 modules.

## 1. Trait-Based Abstraction Pattern (Strategy)

Recurring across parser, pipeline, and serial modules. Interfaces defined as traits, with concrete filesystem implementations and the ability to swap in test doubles.

| Trait | Concrete Impl | Module | File |
|-------|--------------|--------|------|
| `LanguageParser` | `TypeScriptParser`, `PythonParser`, `RustParser`, `go::parser()`, `csharp::parser()`, `java::parser()` | parser | `src/parser/traits.rs` |
| `ImportResolver` | `TypeScriptResolver`, `PythonResolver`, `RustResolver`, `go::resolver()`, `csharp::resolver()`, `java::resolver()` | parser | `src/parser/traits.rs` |
| `FileWalker` | `FsWalker` | pipeline | `src/pipeline/walk.rs` |
| `FileReader` | `FsReader` | pipeline | `src/pipeline/read.rs` |
| `GraphSerializer` | `JsonSerializer` | serial | `src/serial/mod.rs` |
| `GraphReader` | `JsonSerializer` | serial | `src/serial/mod.rs` |

All traits require `Send + Sync`. The `BuildPipeline` struct accepts `Box<dyn T>` for walker, reader, and serializer, wired in `main.rs`.

## 2. Newtype Wrapper Pattern

All domain identifiers are newtypes wrapping `String`. Each follows an identical structure:

- Private `String` field
- `new(impl Into<String>)` constructor
- `as_str() -> &str` accessor
- `into_string() -> String` consumer
- `Serialize` impl that delegates to the inner string
- Standard derives: `Clone, Debug, PartialEq, Eq, Hash`
- Ordered types also derive `PartialOrd, Ord`

Instances observed in `src/model/types.rs`:
- `CanonicalPath(String)` -- additionally has `normalize()`, `parent()`, `extension()`, `file_name()`, `Display`
- `ContentHash(String)`
- `ClusterId(String)` -- additionally has `Display`
- `Symbol(String)`

## 3. Enum Classification Pattern

Domain enums with `as_str()` method returning `&'static str`. Used for classification categories serialized to JSON strings.

| Enum | Variants | File |
|------|----------|------|
| `FileType` | Source, Test, Config, Style, Asset, TypeDef | `src/model/node.rs` |
| `ArchLayer` | Api, Service, Data, Util, Component, Hook, Config, Unknown | `src/model/node.rs` |
| `EdgeType` | Imports, Tests, ReExports, TypeImports | `src/model/edge.rs` |

All derive `Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize` with `#[serde(rename_all = "snake_case")]`.

## 4. Parser Implementation Pattern

Each language parser follows an identical structure. Observed in: `TypeScriptParser`, `PythonParser`, `RustParser` (files: `src/parser/typescript.rs`, `src/parser/python.rs`, `src/parser/rust_lang.rs`).

Recurring shape:
1. Unit struct (no fields): `pub(crate) struct XxxParser;`
2. `pub fn new() -> Self { Self }` constructor
3. Private helper: `fn string_content(node, source) -> Option<&str>` -- strips quotes from AST string nodes
4. Private helper: `fn find_child_by_kind(node, kind) -> Option<Node>` -- tree-sitter child traversal
5. Additional language-specific helpers as private methods
6. `impl LanguageParser for XxxParser` -- `language()`, `extensions()`, `tree_sitter_language()`, `extract_imports()`, `extract_exports()`
7. Separate resolver struct `XxxResolver` implementing `ImportResolver`

The `find_child_by_kind` helper is duplicated identically in all three parser files (TypeScript, Python, Rust).

## 5. Pipeline Stage Pattern

The build pipeline follows a linear stage architecture in `src/pipeline/mod.rs`:

```
walk -> read -> parse -> resolve_and_build -> cluster -> algorithms -> serialize
```

Each stage:
- Receives output of the previous stage as input
- Reports timing via `Instant::now()` / `elapsed()` under `verbose` flag
- Emits warnings to a shared `DiagnosticCollector`
- Produces a typed output struct

Stage-specific output types: `FileEntry`, `FileContent`, `ParsedFile`, `ProjectGraph`, `ClusterMap`, `StatsOutput`.

## 6. Error Handling Pattern

Two distinct error tiers used consistently across the codebase:

**Fatal (FatalError)**: `thiserror::Error` enum, coded E001-E013. Used as `Result<T, FatalError>` return type. Observed in: pipeline, serial, views, MCP server. Fatal errors propagate via `?` and terminate the process in `main.rs` with `process::exit(1)`.

**Warning (Warning struct)**: Collected via `DiagnosticCollector` (thread-safe, `Mutex<(Vec<Warning>, DiagnosticCounts)>`). Each warning has a `WarningCode` (W001-W018), a `CanonicalPath`, a message, and optional detail. Warnings are sorted deterministically by `(path, code)` on drain. Used throughout pipeline stages via `diagnostics.warn(Warning { ... })`.

No `panic!` or `unwrap()` on fallible operations observed in production code paths. Mutex poisoning is handled via `unwrap_or_else(|e| e.into_inner())`.

## 7. Deterministic Output Pattern

BTreeMap used everywhere instead of HashMap for node/edge/cluster storage. Observed in:
- `ProjectGraph.nodes: BTreeMap<CanonicalPath, Node>`
- `ClusterMap.clusters: BTreeMap<ClusterId, Cluster>`
- `FileSet` wraps `BTreeSet<CanonicalPath>`
- All algorithm outputs use `BTreeMap`
- All serialization output structs use `BTreeMap`

Lists are sorted before output: warnings sorted by `(path, code)`, edges sorted, SCC members sorted lexicographically, cluster files sorted.

## 8. Algorithm Module Pattern

Each algorithm in `src/algo/` follows a consistent shape:
- Pure function taking `&ProjectGraph` (and optionally prior algorithm results)
- Uses `build_adjacency()` helper from `algo/mod.rs` to construct forward/reverse adjacency lists
- Filters edges via `is_architectural` predicate (excludes test edges per D-034)
- Returns owned collections (not references)
- Rounds floats via `round4()` for deterministic output

Observed in: `scc.rs`, `centrality.rs`, `topo_sort.rs`, `pagerank.rs`, `blast_radius.rs`, `louvain.rs`.

## 9. Test Pattern

Tests are inline `#[cfg(test)] mod tests` within each file. Observed in: `src/model/types.rs`, `src/diagnostic.rs`, `src/algo/scc.rs`.

Test helper pattern for graph construction:
```rust
fn make_graph(node_names: &[&str], edges: &[(&str, &str)]) -> ProjectGraph
```
This helper appears in `src/algo/scc.rs` and constructs minimal `ProjectGraph` with default field values. Similar helpers likely exist in other algo test modules.

## 10. Serialization Output Model Pattern

Separate "Output" structs mirror internal model structs for serialization. Internal types use newtypes; output types use plain `String`.

| Internal | Output | File |
|----------|--------|------|
| `ProjectGraph` | `GraphOutput` | `src/serial/mod.rs` |
| `Node` | `NodeOutput` | `src/serial/mod.rs` |
| `ClusterMap` | `ClusterOutput` | `src/serial/mod.rs` |
| `Cluster` | `ClusterEntryOutput` | `src/serial/mod.rs` |

Conversion functions in `src/pipeline/mod.rs`: `project_graph_to_output()`, `cluster_map_to_output()`. A reverse conversion exists via `TryFrom<GraphOutput> for ProjectGraph` in `src/serial/convert.rs`.

## 11. Atomic Write Pattern

File writes use write-to-temp-then-rename strategy in `src/serial/json.rs`:
1. Write to `<filename>.<pid>.tmp`
2. Rename to final path
3. On error, clean up temp file

This pattern is applied to all JSON output files (graph.json, clusters.json, stats.json, raw_imports.json).

## 12. Registry Pattern

`ParserRegistry` in `src/parser/registry.rs` acts as a service locator:
- Stores `Vec<Box<dyn LanguageParser>>` and `Vec<Box<dyn ImportResolver>>` in parallel
- Indexes by file extension via `HashMap<String, usize>`
- `with_tier1()` factory method registers all supported languages
- Lookup via `parser_for(extension)` and `resolver_for(extension)`

## 13. Composition Root Pattern

`src/main.rs` is the composition root (referenced as D-020 in design docs):
- Constructs all concrete implementations (`FsWalker`, `FsReader`, `JsonSerializer`, `ParserRegistry::with_tier1()`)
- Wires them into `BuildPipeline`
- Dispatches CLI subcommands via `match`
- Handles `FatalError` at the top level with `process::exit(1)`
- No business logic in main.rs; all logic delegated to library crate

## Items Searched For But Not Found

- **Base classes / inheritance**: None. Rust trait-based composition used exclusively.
- **Middleware chains**: None observed. Pipeline is linear, not middleware-based.
- **Decorators / HOCs**: N/A (Rust codebase, no frontend).
- **ORM / database access**: None. Data is files on disk, read/written as JSON.
- **REST/GraphQL endpoints**: None. CLI tool; MCP server uses JSON-RPC over stdio, not HTTP.
- **Dependency injection framework**: None. Manual constructor injection in main.rs.
- **Macro-based patterns**: Only `#[derive(...)]` and `clap` proc macros. No custom macros.
- **Async patterns in core logic**: Core pipeline is synchronous with `rayon` for parallelism. Async only in MCP server (`tokio`).
- **Generic type parameters on core types**: Not used. Traits use dynamic dispatch (`Box<dyn T>`) rather than monomorphization.
