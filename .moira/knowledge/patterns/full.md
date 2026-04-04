<!-- moira:freshness init 2026-04-04 -->
<!-- moira:knowledge patterns L2 -->

---
api_style: CLI
api_handler_structure: clap subcommands with nested enums (Commands, QueryCommands, ViewsCommands)
api_response_format: pretty-printed JSON via serde_json::to_string_pretty
error_handling: custom FatalError enum (thiserror, non_exhaustive, 14 variants E001-E014) + WarningCode enum (33 variants W001-W033) + DiagnosticCollector (Mutex-based thread-safe accumulator) producing DiagnosticReport
component_structure: trait-per-concern with registry dispatch (LanguageParser, ImportResolver, SymbolExtractor, BoundaryExtractor registered per file extension)
server_state: Arc<ArcSwap<GraphState>> for lock-free read access with AtomicBool rebuilding flag
---

## API Pattern

Ariadne exposes two API surfaces:

1. **CLI** (`src/main.rs`): clap `#[derive(Parser)]` with `#[command(subcommand)]`. Top-level `Commands` enum dispatches to `Build`, `Info`, `Query`, `Views`, `Serve`, `Restart`, `Stop`. `Query` and `Views` have nested subcommand enums. Each subcommand handler is a block in a `match` arm in `main()` that calls library functions, then uses `process::exit()` for error codes.

2. **MCP Server** (`src/mcp/tools.rs`): 44 tool functions annotated with `#[tool(name = "...", description = "...")]` on an `impl AriadneTools` block using `#[tool_router]`. Each tool is a thin wrapper: load state from `Arc<ArcSwap<GraphState>>`, call an algo/analysis function, serialize result via `to_json()` helper returning `String`. Tool parameters are standalone structs deriving `Debug, Deserialize, JsonSchema`, injected via `Parameters(params)` extractor pattern from the `rmcp` framework.

**Evidence:**
- `src/main.rs:20-93` -- clap CLI definition
- `src/mcp/tools.rs:34-59` -- AriadneTools struct with Arc<ArcSwap<GraphState>>
- `src/mcp/tools.rs:303-366` -- representative tool (overview) showing state.load() + algo call + to_json pattern
- `src/mcp/tools.rs:130-269` -- 15+ parameter structs all following `#[derive(Debug, Deserialize, JsonSchema)]`

## Data Access Pattern

No database. All data access is through in-memory graph structures and JSON file serialization.

**Read path:** `GraphReader` trait (`src/serial/mod.rs:132-144`) with methods `read_graph`, `read_clusters`, `read_stats`, `read_raw_imports`, `read_boundaries`. Single implementation: `JsonSerializer` (`src/serial/json.rs`) using `serde_json::from_reader(BufReader)`. Errors map to `FatalError` variants (GraphNotFound, GraphCorrupted).

**Write path:** `GraphSerializer` trait (`src/serial/mod.rs:114-128`) with matching write methods. `JsonSerializer` implements atomic writes (write to temp file, rename). Both traits return `Result<_, FatalError>`.

**In-memory access:** MCP server holds `Arc<ArcSwap<GraphState>>` -- all 44 tools call `self.state.load()` to get a snapshot, then read from `state.graph`, `state.stats`, `state.clusters`, `state.temporal`, `state.symbol_index`, `state.forward_index`, `state.reverse_index`.

**Evidence:**
- `src/serial/mod.rs:114-144` -- GraphSerializer + GraphReader trait definitions
- `src/serial/json.rs:16-49` -- JsonSerializer implementing both traits
- `src/mcp/tools.rs:311-313` -- `let state = self.state.load()` pattern in every tool

## Common Abstractions

| Abstraction | Kind | Location | Purpose |
|---|---|---|---|
| `LanguageParser` | trait | `src/parser/traits.rs:33-44` | Extract imports/exports from tree-sitter AST per language |
| `ImportResolver` | trait | `src/parser/traits.rs:47-55` | Resolve raw import paths to canonical file paths |
| `SymbolExtractor` | trait | `src/parser/symbols.rs:5-7` | Extract symbol definitions from parsed AST (separate from parser per D-077) |
| `BoundaryExtractor` | trait | `src/semantic/mod.rs:15-26` | Extract HTTP routes, events, and other semantic boundaries from AST |
| `GraphSerializer` | trait | `src/serial/mod.rs:114-128` | Write graph/clusters/stats/boundaries to output directory |
| `GraphReader` | trait | `src/serial/mod.rs:132-144` | Read graph/clusters/stats/boundaries from output directory |
| `FileWalker` | trait | `src/pipeline/walk.rs` | Walk filesystem to discover project files |
| `FileReader` | trait | `src/pipeline/read.rs:27-33` | Read and filter individual files (size, binary, encoding) |
| `ParserRegistry` | struct (registry) | `src/parser/registry.rs:32-40` | Extension-indexed dispatch to parser/resolver/symbol/boundary extractors |
| `ExtractorRegistry` | struct (registry) | `src/semantic/mod.rs:32-35` | Extension-indexed dispatch to boundary extractors (allows N-per-extension) |
| `AdjacencyIndex` | struct (cache) | `src/algo/mod.rs:36-41` | Pre-built forward/reverse adjacency maps with degree counts; built once, shared across all graph algorithms |
| `DiagnosticCollector` | struct (accumulator) | `src/diagnostic.rs:282-284` | Thread-safe (Mutex) warning accumulator with automatic count tracking, drained into sorted DiagnosticReport |
| `BuildPipeline` | struct (orchestrator) | `src/pipeline/mod.rs:64-69` | Composes walker + reader + registry + serializer via trait objects |

## Recurring Structures

| Pattern | Frequency | Evidence |
|---|---|---|
| **Newtype wrapper over String** | 5 types | `CanonicalPath(String)`, `ContentHash(String)`, `ClusterId(String)`, `Symbol(String)` in `src/model/types.rs`; all share identical shape: `new(impl Into<String>)`, `as_str()`, `into_string()`, manual `Serialize` impl |
| **Enum with `as_str()` method** | 7 enums | `FileType` (8 variants), `ArchLayer` (8), `FsdLayer` (7), `EdgeType` (5), `WarningCode` (33), `SmellType`, `SmellSeverity` -- all implement `fn as_str(&self) -> &'static str` via match |
| **#[derive(Debug, Deserialize, JsonSchema)] param struct** | 20+ structs | Every MCP tool parameter struct in `src/mcp/tools.rs`, `tools_context.rs`, `tools_temporal.rs`, `tools_semantic.rs`, `tools_recommend.rs` -- identical derive triplet, doc comments on each field |
| **BTreeMap for deterministic ordering** | pervasive (15+ usage sites) | `ProjectGraph.nodes: BTreeMap<CanonicalPath, Node>`, `ClusterMap.clusters`, `GraphOutput.nodes`, `algo::AdjacencyIndex.forward/reverse`, `centrality results`, `stats.centrality` -- BTreeMap used everywhere HashMap could be, per D-006 determinism |
| **trait + registry + extension-index dispatch** | 4 registrations | `LanguageParser`/`ImportResolver` pair, `SymbolExtractor`, `BoundaryExtractor` -- all registered in `ParserRegistry` via `HashMap<String, usize>` extension-to-index lookup |
| **State snapshot via `self.state.load()`** | 44 occurrences | Every `#[tool]` method in `src/mcp/tools.rs` begins with `let state = self.state.load();` then reads from the `GraphState` snapshot |
| **Sort for determinism then collect** | 10+ sites | `edges.sort_by(...)` in `build.rs:180-186`, `smells.sort_by(...)` in `smells.rs:37-41`, `warnings.sort_by(...)` in `diagnostic.rs:376`, `export_symbols.sort(); export_symbols.dedup()` in `build.rs:65-66` -- recurring `sort_by` + deterministic key pattern |
| **`serde_json::json!({...})` response construction** | 44 tools | Each MCP tool builds a `serde_json::json!({...})` value, then returns `to_json(&result)` -- no shared response envelope, just raw JSON objects |
| **Parser struct per language** | 9 parsers | `TypeScriptParser`, `PythonParser`, `RustParser`, `GoParser`, `CSharpParser`, `JavaParser`, `MarkdownParser`, `JsonLangParser`, `YamlParser` -- each implements `LanguageParser` + `ImportResolver` (sometimes split into separate structs) |
| **tree-sitter AST walking with cursor** | 9 parser files | Every language parser walks tree-sitter nodes via `node.walk()` cursor + `node.children(&mut cursor)` + `child.kind()` string matching -- identical traversal pattern across all parsers |
| **`pub(crate)` visibility on parser structs** | 5+ parser files | `TypeScriptParser`, `PythonParser`, `RustParser` etc. use `pub(crate)` -- exposed only within the crate, with public factory functions (`parser()`, `resolver()`) in some modules |
| **Atomic write (write-to-temp, rename)** | 1 impl, 5 call sites | `atomic_write()` in `src/serial/json.rs` used by all 5 `GraphSerializer` methods -- consistent crash-safe write pattern |
| **Option<&T> for optional analysis state** | 4 sites | `temporal: Option<&TemporalState>`, `semantic: Option<&SemanticState>` passed to `detect_smells`, MCP tool responses conditionally include temporal/semantic data -- recurring "optional enrichment" pattern |
