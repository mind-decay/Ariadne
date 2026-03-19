# Architecture Review

**Date:** 2026-03-19
**Focus:** Full System
**Mode:** Post-implementation (64 source files across all phases through 3c)
**Reviewed:** architecture.md, ROADMAP.md, decisions/log.md (D-001 through D-064), error-handling.md, performance.md, testing.md, path-resolution.md, determinism.md, distribution.md, all src/ modules

## Executive Summary

Ariadne's core architecture is sound. The module layering is clean — dependency direction is confirmed by `use` statements across all 64 source files with no circular dependencies. The newtype pattern (D-017), trait separation (D-018), pipeline abstraction (D-019), and determinism strategy (D-006/D-049) are well-executed. The design document ecosystem is unusually thorough for a project this size.

The top concerns are: (1) **design document drift** — several docs are stale after Phase 3 implementation, creating false expectations; (2) **complexity overshoot** in Phase 3c — spectral analysis shipped despite its own ORANGE risk flag, and the system has grown from a dependency graph tool into a research-grade graph analytics engine; (3) **safety gaps** — systematic `unwrap()` on `Mutex::lock()` in `DiagnosticCollector` means a rayon worker panic would cascade; (4) **main.rs has become a 1100-line god object** that violates the Composition Root principle it was designed to enforce.

## Key Themes

### Theme 1: Design Document Drift

Four agents independently found stale design documentation. This is the most pervasive issue — the design docs were the project's strength in early phases but have not kept pace with Phase 3 implementation.

**Contributing findings:**
- `performance.md` still describes delta computation as "O(changed files) / Only re-parses changed files" — directly contradicted by D-050's full-rebuild decision
- `error-handling.md` is missing E010-E013 (fatal) and W014-W018 (warnings) that exist in code; W013 (StaleStats) is defined but never emitted
- `architecture.md` module dependency table is missing `analysis/` and `mcp/` rows despite D-033 defining them
- `architecture.md` storage format and git tracking policy omit `raw_imports.json` (introduced by D-054)
- `determinism.md` sort points table does not cover `raw_imports.json`
- D-047 ("no async runtime") is not marked as superseded by D-051 (tokio for serve)
- D-022 still describes `From<ProjectGraph>` impl; actual implementation uses a free function
- `BuildOutput` in code has `stats_path` and `counts` fields absent from architecture.md

**Why it matters:** Design docs are declared "source of truth" in CLAUDE.md. Stale docs create false expectations for contributors and contradict the project's own development protocol.

### Theme 2: Complexity Beyond the Core Problem

The system has grown from "parse imports + build a graph" to a full graph analytics engine. Phase 3c in particular added features that the roadmap itself flagged as risky.

**Contributing findings:**
- Spectral analysis shipped despite ORANGE risk flag (D-043: "defer if determinism cost is too high") — the Fiedler vector sign convention at exactly 0.0 is indeterminate
- `GraphState::from_loaded_data()` eagerly computes PageRank, spectral, compression, and metrics synchronously at every server startup and hot-reload
- Structural diff includes smell diffing, which runs `detect_smells()` twice (each calling `blast_radius()` for every high-in-degree file)
- `raw_imports.json` and two-level structural confidence add a fourth output file and re-parsing logic for a marginal improvement over hash-only confidence
- Forward and reverse indices in `GraphState` clone every `Edge` struct, roughly doubling memory footprint

**Why it matters:** The ratio of algorithm surface area to the core graph-building problem is approximately 5:1. Each addition increases startup time, memory footprint, and maintenance burden.

### Theme 3: Safety Gaps in Error Handling

Systematic `unwrap()` patterns create panic-on-error behavior where graceful degradation is expected.

**Contributing findings:**
- `diagnostic.rs:257,313,319` — `Mutex::lock().unwrap()` panics on poison. If a rayon worker panics while holding the lock, all subsequent `warn()` calls crash
- `mcp/tools.rs` — ~25 instances of `serde_json::to_string_pretty().unwrap()` that would crash the MCP server if any serialized value contains `f64::NAN` or `f64::INFINITY`
- `algo/centrality.rs:57,74,78` — `get_mut(...).unwrap()` in back-propagation assumes INV-1 holds; a dangling edge reference panics
- `algo/scc.rs:66` — `stack.pop().unwrap()` relies on Tarjan invariant; correct but not defensive
- W007 (partial parse) claims "extract from valid subtrees" but no parser actually filters ERROR subtrees — imports from corrupt AST regions are included indiscriminately

**Why it matters:** The MCP server is a long-running process. A panic in a tool handler crashes the entire server rather than returning an error response.

### Theme 4: Composition Root Erosion

`main.rs` was designed as the sole Composition Root (D-020) but has accumulated substantial business logic.

**Contributing findings:**
- `main.rs` is 1103 lines containing: CLI parsing, `run_build`, `run_update`, 430-line `run_query` with 13 inline subcommand handlers, markdown rendering (`print_stats_md`), ad-hoc serialization (`serialize_subgraph_result`), and helper functions
- `mcp/server.rs` has a second `make_pipeline()` function that duplicates the concrete type wiring from `main.rs`, violating D-020
- Severity filtering logic is duplicated between `main.rs` and `mcp/tools.rs`
- `update()` pipeline warnings bypass `--warnings json` formatting via direct `eprintln!`, violating D-030's orthogonality guarantee

**Why it matters:** The Composition Root pattern exists to keep the system testable. Two wiring points means changes to concrete types must be coordinated in two places. Business logic in `main.rs` cannot be tested without the CLI layer.

## Detailed Findings

### Foundational Issues

**F-1: Betweenness centrality O(VE) cliff at scale**
Brandes algorithm is O(VE). At 50k files (the configured limit) with average degree 3, this is ~7.5 billion operations — estimated 140+ seconds. The design doc says "<500ms for 3000 nodes" but does not acknowledge the quadratic scaling. Centrality runs unconditionally on every build.
*Direction:* Document the scaling cliff. Consider making centrality opt-in above a file count threshold (e.g., >10k files), or switching to approximate centrality.

**F-2: Go resolver is effectively a stub**
`parser/go.rs:123-135` `find_module_path` always returns `None` because it cannot read file contents from `FileSet`. All Go imports containing dots are classified as external. For any non-trivial Go project, the graph will have near-zero edges. The architecture doc understates this as "stdlib-only resolution" — the actual impact is that Go support is non-functional for real projects.
*Direction:* Either fix Go module resolution (requires reading `go.mod` content, which means extending `FileSet` or the resolver interface) or reclassify Go as Tier 2 with an explicit accuracy disclaimer.

**F-3: `DiagnosticCollector` mutex poison cascade**
`diagnostic.rs:257` uses `.lock().unwrap()`. A rayon worker panic while holding the lock poisons the mutex, causing all subsequent `warn()`, `increment_unresolved()`, and `drain()` calls to panic. This converts a single-file failure into a full pipeline crash.
*Direction:* Replace `.lock().unwrap()` with `.lock().unwrap_or_else(|e| e.into_inner())` to recover from poisoned state.

**F-4: MCP server panics on NaN/Inf serialization**
~25 instances of `serde_json::to_string_pretty(&result).unwrap()` in `mcp/tools.rs`. If any algorithm produces `f64::NAN` or `f64::INFINITY` (possible in degenerate graph configurations), the server process crashes instead of returning an error response.
*Direction:* Use `serde_json::to_string_pretty(&result).map_err(|e| format!("serialization error: {e}"))` throughout `tools.rs`.

### Structural Issues

**S-1: `main.rs` god object (1103 lines)**
Contains CLI parsing, 13 query command handlers inline, markdown rendering, ad-hoc serialization, and cross-cutting helpers. Violates SRP and the Composition Root principle.
*Direction:* Extract into `src/cli/` module — `build.rs`, `query.rs`, `views.rs`. `main.rs` becomes argument parser and dispatcher only. Move `print_stats_md` to `views/`.

**S-2: Second Composition Root in `mcp/server.rs`**
`make_pipeline()` at `mcp/server.rs:211` duplicates the concrete type wiring from `main.rs`, violating D-020.
*Direction:* Pass a pre-built `Arc<BuildPipeline>` into `mcp::server::run()` from `main.rs`.

**S-3: `GraphState` eager pre-computation blocks startup**
`from_loaded_data()` runs PageRank, spectral analysis, graph compression, and Martin metrics synchronously before accepting any MCP request. For large graphs, this could block for seconds.
*Direction:* Split into `CoreState` (graph, indices — loaded immediately) and `AnalyticsCache` (PageRank, spectral, compression — computed lazily or asynchronously).

**S-4: `CompressedNode` is an implicit tagged union**
Uses `Option` fields to represent level-specific data. `CompressedNodeType::Cluster` nodes carry `file_count`/`cohesion`; `File` nodes carry `file_type`/`layer`. No type-level enforcement.
*Direction:* Use proper enum variants with `#[serde(tag = "type")]` to eliminate impossible states.

**S-5: `update()` warning routing bypasses `--warnings json`**
`pipeline/mod.rs:369-400` creates a local `DiagnosticCollector` and `eprintln!`s warnings directly, bypassing `format_warnings()` and `--strict` exit code check.
*Direction:* Propagate warnings back to the caller via the return type.

**S-6: `StatsOutput` re-exported from `serial/` despite living in `model/`**
`serial/mod.rs:12` re-exports `crate::model::{StatsOutput, StatsSummary}` "for backwards compatibility." This blurs the canonical type location.
*Direction:* Import directly from `model/` at all call sites. Remove the re-export.

### Surface Issues

**U-1: `LanguageParser::tree_sitter_language()` leaks implementation detail**
The trait exposes `tree_sitter::Language`, making tree-sitter a permanent type-level dependency. Only `ParserRegistry::parse_source` calls this method.
*Direction:* Move AST construction into parser implementations. The trait method becomes `parse(&self, source: &[u8]) -> ParseOutcome`.

**U-2: Two parser registration patterns in `registry.rs`**
TypeScript/Python/Rust use `::new()` struct constructors; Go/C#/Java use factory functions. Both work but create inconsistency.
*Direction:* Standardize on the struct pattern for all parsers.

**U-3: W006 unresolved imports record only a counter, not individual details**
`pipeline/resolve.rs:39` calls `increment_unresolved()` which provides no per-import detail. A user with `--verbose --warnings json` cannot get a machine-readable list of which imports failed.
*Direction:* In verbose mode, emit full W006 `Warning` structs with paths.

**U-4: `arch_depth: 0` is ambiguous**
No way to distinguish "Phase 1a placeholder (not computed)" from "genuine Layer 0 (no outgoing architectural dependencies)."
*Direction:* Use `Option<u32>` in `NodeOutput` for the serialized form.

**U-5: Design says read stage is parallel; implementation is sequential**
`architecture.md:339` says "parallel via rayon on sorted list." `pipeline/mod.rs:99-118` is a sequential `for` loop.
*Direction:* Either update the design doc or parallelize the read stage.

**U-6: Cluster assignment applied twice in `pipeline/mod.rs`**
Directory clustering writes to `graph.nodes`, then Louvain writes again — even when assignments are unchanged.
*Direction:* Apply cluster assignments once after all clustering is complete.

**U-7: Missing depth-exceeded warning**
Files beyond `MAX_DEPTH = 64` are silently skipped by `pipeline/walk.rs:42` with no warning emitted.
*Direction:* Add a W-code for depth truncation.

**U-8: Python src-layout absolute imports never resolve**
`import mypackage.utils` becomes path `mypackage/utils` which won't match `src/mypackage/utils.py` in src-layout projects.
*Direction:* Document as a known limitation or add src-layout probing.

## Discussion Points

### 1. Should spectral analysis be deferred?

**Tension:** Spectral analysis provides unique insights (monolith detection, natural refactoring boundaries) that no other metric offers. However, D-043 explicitly flagged it ORANGE and said "defer if determinism cost is too high." The Fiedler vector sign at exactly 0.0 is indeterminate, and the feature adds power-iteration computation to every server startup.

**Arguments for keeping:** Already implemented and tested. Provides algebraic connectivity metric unavailable elsewhere. Power iteration is O(V+E) per iteration, manageable for most projects.

**Arguments for deferring:** The ROADMAP's own risk assessment recommended conditional deferral. Adds latency to every `GraphState` load. The Fiedler bisection's practical advantage over Louvain clustering for import graphs is unclear.

**What's at stake:** If kept, every server startup and hot-reload pays the cost. If deferred, the `ariadne_spectral` tool and `ariadne query spectral` CLI command disappear.

**Recommended direction:** Move behind a `--spectral` flag (off by default). Keep the implementation; remove from default startup path.

### 2. Should Louvain be opt-in or opt-out?

**Tension:** Louvain provides genuinely useful cluster refinement but adds non-trivial computation to every build. Most codebases have directory structures that already reflect module boundaries.

**Arguments for opt-out (current):** Louvain produces better clusters by default. Users don't need to know it exists.

**Arguments for opt-in:** Saves build time. Directory clustering is "good enough" for most projects. Louvain's non-deterministic assignment behavior (D-057 filtering) adds complexity to structural diffs.

**Recommended direction:** Keep as opt-out (`--no-louvain`) but document the performance impact for large projects.

### 3. Should C#/Java remain Tier 1?

**Tension:** Both languages have heuristic-only import resolution with acknowledged low accuracy. Including them in Tier 1 may give users false confidence in incomplete graphs.

**Arguments for keeping:** Having any graph is better than none. Users can evaluate accuracy themselves. The parsers correctly extract import syntax — only resolution is weak.

**Arguments for reclassifying:** "Tier 1" implies production quality. The architecture doc says C# has "low accuracy" and Java uses a "hardcoded `src/main/java/` prefix." No integration tests on real C#/Java projects exist.

**Recommended direction:** Keep in Tier 1 but add a "resolution accuracy" annotation to `ariadne info` output (e.g., "C# — resolution: heuristic").

### 4. How should the design doc drift be addressed?

**Tension:** Updating all stale docs is time-consuming but critical to the project's "design docs are source of truth" principle.

**Recommended direction:** A focused documentation pass covering: (1) performance.md delta claims, (2) error-handling.md taxonomy additions, (3) architecture.md module table + storage format, (4) determinism.md sort points, (5) D-047 supersession marking.

## Strengths

**Module layering is genuinely clean.** All `use crate::` imports across 64 files confirm the intended dependency direction. No circular module dependencies. The `model/` leaf module depends on nothing — this discipline has held through 3 phases of implementation.

**The newtype pattern (D-017) pays dividends.** `CanonicalPath`, `ContentHash`, `ClusterId`, `Symbol` prevent an entire class of string-mixing bugs. The pattern is used consistently throughout.

**The decision log is exceptional.** 64 decisions with rationale, rejected alternatives, and cross-references. This is rare in any project and provides invaluable context. Even when decisions become stale (D-047), the log makes it possible to understand what changed and why.

**Determinism strategy (D-006/D-049) is thorough and well-executed.** BTreeMap everywhere, sorted edge output, `round4` for floats, deterministic iteration order. The determinism guarantee has held through Phase 3c's iterative algorithms.

**Error taxonomy design (D-005/D-021) is sound.** The two-tier model (fatal + recoverable), `DiagnosticCollector` pattern, and warning code system provide structured error handling. The taxonomy's extensibility is proven by the E010-E013 / W014-W018 additions (even though the docs haven't caught up).

**The trait-based pipeline (D-019) enables genuine testability.** `FileWalker`, `FileReader`, `GraphSerializer` abstractions allow mock-based testing without filesystem access. This is not over-engineering — the mock implementations are actively used in tests.

## Recommendations

### Quick Wins (doc updates only)

1. Update `performance.md` delta computation description to match D-050 (full-rebuild behavior)
2. Add E010-E013 and W014-W018 to `error-handling.md` taxonomy tables
3. Add `analysis/` and `mcp/` rows to `architecture.md` module dependency table
4. Add `raw_imports.json` to `architecture.md` storage format and `determinism.md` sort points
5. Mark D-047 as "Superseded by D-051" in the decision log
6. Update D-022 to reference the free function pattern instead of `From` impl
7. Add `stats_path` and `counts` to `BuildOutput` in `architecture.md` Pipeline Support Types
8. Document the `arch_depth: 0` ambiguity in the graph.json schema section

### Targeted Improvements (localized design changes)

1. Replace `Mutex::lock().unwrap()` with poison-recovery in `diagnostic.rs` (3 call sites)
2. Replace `serde_json::to_string_pretty().unwrap()` with error handling in `mcp/tools.rs` (~25 sites)
3. Move `make_pipeline()` from `mcp/server.rs` to `main.rs` — pass pipeline into `run()`
4. Route `update()` warnings through the caller instead of direct `eprintln!`
5. Remove `StatsOutput` re-export from `serial/mod.rs`
6. Add severity mapping method to `SmellSeverity` to deduplicate main.rs / tools.rs

### Strategic Considerations (bigger architectural shifts)

1. **Extract `main.rs` into `src/cli/` module.** This is the highest-impact structural improvement. The 1103-line god object undermines the Composition Root principle.
2. **Lazy analytics in `GraphState`.** Split into `CoreState` (indices, loaded immediately) and `AnalyticsCache` (PageRank, spectral, compression — computed on demand or async). This would significantly reduce MCP startup and hot-reload latency.
3. **Gate spectral analysis behind a flag.** The ORANGE risk was flagged for a reason. Moving it behind `--spectral` preserves the work while removing it from the critical path.
4. **Fix Go module resolution or reclassify to Tier 2.** The current stub makes Go support non-functional for real projects. This is the largest gap between claimed and actual capability.
5. **Document the betweenness centrality scaling cliff.** At 50k files, builds could hang for minutes. Consider an approximate algorithm or file-count threshold.
