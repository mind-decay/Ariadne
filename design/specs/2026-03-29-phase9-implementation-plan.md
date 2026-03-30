# Phase 9: Recommendation Engine — Implementation Plan

**Spec:** `design/specs/2026-03-29-phase9-recommendations.md`
**Date:** 2026-03-29

## Overview

7 implementation chunks, 8 new files, 3 modified files. Chunks 1-2 are foundational (types, algorithms). Chunk 3 is the Pareto algorithm. Chunks 4-6 implement the three deliverables (D22, D23, D24) and can be partially parallelized after Chunk 2 completes (D22 must precede D24 since D24 reuses `analyze_split`). Chunk 7 covers benchmarks and design doc updates.

**Estimated total new lines:** ~2,200-2,800

## Dependency Graph

```
Chunk 1: Module Skeleton + Types
  │
  ├──► Chunk 2: Stoer-Wagner Min-Cut
  │       │
  │       ├──► Chunk 4: D22 suggest_split ──────────┐
  │       │                                          │
  ├──► Chunk 3: Pareto Frontier                      │
  │       │                                          ▼
  │       ├──► Chunk 6: D24 refactor_opportunities ◄─┘
  │       │
  │       └──► Chunk 5: D23 suggest_placement (parallel with Chunk 4)
  │
  └─────────────────────────────────────────────────► Chunk 7: Benchmarks + Docs
```

Parallelizable: Chunk 4 and Chunk 5 (after Chunks 2+3). Chunk 6 depends on Chunk 4 (reuses analyze_split).

---

## Chunk 1: Module Skeleton + Types

**Goal:** Create the `src/recommend/` module with all type definitions and wire it into `src/lib.rs`.

**Files:**

| File | Action | Description |
|------|--------|-------------|
| `src/recommend/mod.rs` | NEW | Module root with submodule declarations and re-exports |
| `src/recommend/types.rs` | NEW | All recommendation enums, structs (Effort, Impact, DataQuality, RefactorType, SplitGroup, SplitAnalysis, SplitImpact, PlacementSuggestion, PlacementAlternative, RefactorOpportunity, RefactorAnalysis, SymbolGraph, MinCutResult) |
| `src/lib.rs` | MODIFY | Add `pub mod recommend;` |

**Design sources:** D-109, D-110, TD-1, TD-2, TD-7

**What to implement:**

1. Create `src/recommend/` directory.
2. Create `src/recommend/types.rs` with all type definitions from the spec:
   - Enums: `Effort`, `Impact`, `DataQuality`, `RefactorType` — all with `Serialize, Deserialize, Debug, Clone, Copy` (Copy where applicable), `PartialEq, Eq, PartialOrd, Ord` where needed for BTreeSet. Use `#[serde(rename_all = "snake_case")]`.
   - Structs: `SplitGroup`, `SplitAnalysis`, `SplitImpact`, `PlacementSuggestion`, `PlacementAlternative`, `RefactorOpportunity`, `RefactorAnalysis`, `SymbolGraph`, `MinCutResult` — all with `Serialize, Deserialize` where they appear in MCP output. Use `BTreeSet<String>` for symbol collections. Use `BTreeSet<usize>` for partition indices.
   - All floating-point fields use `f64`.
3. Create `src/recommend/mod.rs`:
   - Declare submodules: `pub mod types;` (min_cut, pareto, split, placement, refactor added in later chunks).
   - Re-export key types: `pub use types::*;`
4. Modify `src/lib.rs`: add `pub mod recommend;` line in the module declarations section.

**Acceptance criteria:**
- `cargo check` passes with no errors
- All types are defined with correct derives
- `src/recommend/` module is accessible from `src/lib.rs`
- No additions to `src/model/types.rs`

---

## Chunk 2: Stoer-Wagner Min-Cut

**Goal:** Implement the Stoer-Wagner global minimum cut algorithm for symbol-level graph partitioning (FM-9.1).

**Files:**

| File | Action | Description |
|------|--------|-------------|
| `src/recommend/min_cut.rs` | NEW | Stoer-Wagner algorithm implementation |
| `src/recommend/mod.rs` | MODIFY | Add `pub mod min_cut;` |

**Design sources:** D-111, TD-3, ROADMAP FM-9.1

**What to implement:**

1. Create `src/recommend/min_cut.rs` with:
   - `pub fn stoer_wagner(graph: &SymbolGraph) -> Option<MinCutResult>`
   - Return `None` if `graph.nodes.len() < 2`
   - Implement the standard Stoer-Wagner algorithm:
     a. Maintain a working graph (adjacency matrix + merged-node tracking).
     b. For each phase: run maximum adjacency ordering to find the last two vertices (s, t).
     c. Record the cut-of-the-phase (weight of edges to t).
     d. Merge s and t (combine rows/columns, sum weights).
     e. Track which original nodes end up in the t-side of the best phase cut.
     f. After V-1 phases, return the minimum cut found.
   - Use `BTreeSet<usize>` for partition tracking (determinism per TD-7).
   - All internal collections use deterministic ordering.
2. Add `pub mod min_cut;` to `src/recommend/mod.rs`.
3. Write unit tests in the same file (`#[cfg(test)] mod tests`):
   - Test: 2-node graph with single edge → cut weight = edge weight, partitions = {0}, {1}
   - Test: 4-node path graph (0-1-2-3) → min cut = minimum edge weight
   - Test: Complete graph K4 with uniform weights → min cut = 3 * weight
   - Test: Graph with obvious 2-cluster structure → cut separates clusters
   - Test: Single node → returns None
   - Test: Empty graph → returns None
   - Test: Disconnected graph → cut weight = 0

**Acceptance criteria:**
- `cargo test` passes — all min-cut unit tests green
- Stoer-Wagner returns correct min-cut for all test cases
- Partitions use BTreeSet for deterministic ordering
- No external graph crate dependencies

---

## Chunk 3: Pareto Frontier

**Goal:** Implement 2D Pareto frontier computation for recommendation ranking (FM-9.2).

**Files:**

| File | Action | Description |
|------|--------|-------------|
| `src/recommend/pareto.rs` | NEW | Pareto frontier algorithm |
| `src/recommend/mod.rs` | MODIFY | Add `pub mod pareto;` |

**Design sources:** TD-4, ROADMAP FM-9.2

**What to implement:**

1. Create `src/recommend/pareto.rs` with:
   - `pub fn pareto_frontier(points: &[(f64, f64)]) -> Vec<(bool, Option<usize>)>`
   - Semantics: points are (effort_score, impact_score). A point is dominated if another point has <= effort AND >= impact (with at least one strict inequality).
   - For each point, check all other points for domination.
   - If dominated, record the index of the first dominating point found (sorted by index for determinism).
   - Return vector of `(is_on_frontier, dominated_by_index)`.
   - Handle edge cases: empty input returns empty vec; single point is always on frontier.
2. Add `pub mod pareto;` to `src/recommend/mod.rs`.
3. Write unit tests:
   - Test: empty input → empty output
   - Test: single point → `[(true, None)]`
   - Test: two points where one dominates → correct frontier/dominated assignment
   - Test: two points on frontier (neither dominates) → both `(true, None)`
   - Test: 5 points with known Pareto frontier
   - Test: all points identical → all on frontier (none strictly dominates)
   - Test: all points dominated by one → one on frontier, rest dominated

**Acceptance criteria:**
- `cargo test` passes — all Pareto unit tests green
- Correct frontier identification for all test cases
- Dominated points correctly reference their dominator
- O(n^2) complexity (no worse)

---

## Chunk 4: D22 — suggest_split + MCP Tool

**Goal:** Implement file split analysis (D22) and wire it as the `ariadne_suggest_split` MCP tool.

**Files:**

| File | Action | Description |
|------|--------|-------------|
| `src/recommend/split.rs` | NEW | Split analysis algorithm |
| `src/mcp/tools_recommend.rs` | NEW | MCP param structs + handler for suggest_split |
| `src/recommend/mod.rs` | MODIFY | Add `pub mod split;` |
| `src/mcp/mod.rs` | MODIFY | Add `mod tools_recommend;` |
| `src/mcp/tools.rs` | MODIFY | Import SuggestSplitParam, add tool schema + dispatch arm |

**Design sources:** D-109, D-112, TD-1, TD-3, TD-5, TD-6, ROADMAP D22

**What to implement:**

1. Create `src/recommend/split.rs` with:
   - `pub fn analyze_split(path: &CanonicalPath, edges: &[Edge], nodes: &[Node], symbol_index: Option<&SymbolIndex>, call_graph: Option<&CallGraph>, temporal: Option<&TemporalData>) -> SplitAnalysis`
   - Step 1: Determine data quality (Full/Structural/Minimal based on available optional inputs).
   - Step 2: If symbol_index + call_graph available, build a SymbolGraph from intra-file call edges. If not, fall back to file-level analysis (should_split based on line count > threshold, high centrality, high export count).
   - Step 3: If SymbolGraph has < 2 nodes → return `should_split: false` (EC-1).
   - Step 4: If SymbolGraph nodes > 200 → skip Stoer-Wagner, use Louvain only (EC-7, W013).
   - Step 5: Run `stoer_wagner(&symbol_graph)`. If partition is unbalanced (<20% in smaller side), fall back to Louvain (W014).
   - Step 6: Build `SplitGroup` entries from partition. Assign suggested names based on common symbol prefixes or responsibility keywords.
   - Step 7: Compute `SplitImpact` using blast_radius and centrality from algo/.
   - Step 8: Handle special cases: re-export hubs (EC-8), test files (EC-5).
   - Helper: `fn build_symbol_graph(path: &CanonicalPath, symbol_index: &SymbolIndex, call_graph: &CallGraph, temporal: Option<&TemporalData>) -> SymbolGraph`
2. Create `src/mcp/tools_recommend.rs` with:
   - `pub struct SuggestSplitParam { pub path: String }` (with Deserialize)
   - `pub async fn handle_suggest_split(state: &GraphState, param: SuggestSplitParam) -> Result<serde_json::Value>`
   - Handler: validate path exists in graph (EC-4), extract edges/nodes/symbol_index/call_graph/temporal from state, call `analyze_split`, serialize result.
3. Modify `src/mcp/mod.rs`: add `mod tools_recommend;`
4. Modify `src/mcp/tools.rs`:
   - Import `SuggestSplitParam` and `handle_suggest_split` from `tools_recommend`.
   - Add tool schema entry for `ariadne_suggest_split` in the tool list.
   - Add dispatch arm: `"ariadne_suggest_split" => { let param: SuggestSplitParam = ...; handle_suggest_split(&state, param).await }`
5. Add `pub mod split;` to `src/recommend/mod.rs`.

**Acceptance criteria:**
- `cargo test` passes
- `ariadne_suggest_split` MCP tool is registered and callable
- Returns `should_split: false` for files with < 2 symbols (EC-1)
- Returns `should_split: false` for re-export hubs (EC-8)
- Falls back to Louvain for large files (EC-7)
- Returns error for nonexistent paths (EC-4)
- `data_quality` field correctly reflects available data (EC-29, EC-30)
- All output uses BTreeSet/sorted ordering (TD-7)

---

## Chunk 5: D23 — suggest_placement + MCP Tool

**Goal:** Implement new file placement recommendations (D23) and wire it as the `ariadne_suggest_placement` MCP tool.

**Files:**

| File | Action | Description |
|------|--------|-------------|
| `src/recommend/placement.rs` | NEW | Placement suggestion algorithm |
| `src/mcp/tools_recommend.rs` | MODIFY | Add SuggestPlacementParam + handler |
| `src/recommend/mod.rs` | MODIFY | Add `pub mod placement;` |
| `src/mcp/tools.rs` | MODIFY | Import SuggestPlacementParam, add tool schema + dispatch arm |

**Design sources:** D-109, D-112, TD-1, TD-5, TD-6, ROADMAP D23

**What to implement:**

1. Create `src/recommend/placement.rs` with:
   - `pub fn suggest_placement(description: &str, depends_on: &[CanonicalPath], depended_by: &[CanonicalPath], edges: &[Edge], nodes: &[Node], clusters: &BTreeMap<ClusterId, Vec<CanonicalPath>>, layers: &BTreeMap<u32, Vec<CanonicalPath>>) -> PlacementSuggestion`
   - Step 1: Resolve each depends_on path to its cluster and layer.
   - Step 2: Score clusters by dependency count (majority vote). Break ties by cluster with lowest average depth (more stable).
   - Step 3: Compute arch_depth: max depth of depends_on files + 1 (one layer above). If depended_by constrains, adjust to be at or below min depth of depended_by files.
   - Step 4: Determine layer name from depth using detect module's layer mapping.
   - Step 5: Generate suggested_path: use winning cluster's directory prefix + derive filename from description (lowercase, underscored, .rs extension for Rust projects).
   - Step 6: Check for path conflicts (EC-15): if suggested path exists, append numeric suffix.
   - Step 7: Check for cycle creation (EC-14): if depends_on and depended_by would create a cycle, add warning W015.
   - Step 8: Generate 1-3 alternatives from runner-up clusters with risk annotations.
   - Step 9: Set data_quality based on available data.
2. Add to `src/mcp/tools_recommend.rs`:
   - `pub struct SuggestPlacementParam { pub description: String, pub depends_on: Vec<String>, #[serde(default)] pub depended_by: Vec<String> }`
   - `pub async fn handle_suggest_placement(state: &GraphState, param: SuggestPlacementParam) -> Result<serde_json::Value>`
   - Handler: validate depends_on non-empty (EC-11, E007), validate all paths exist in graph (EC-13), extract cluster/layer data from state, call `suggest_placement`, serialize result.
3. Modify `src/mcp/tools.ts`:
   - Import `SuggestPlacementParam` and `handle_suggest_placement`.
   - Add tool schema entry for `ariadne_suggest_placement`.
   - Add dispatch arm.
4. Add `pub mod placement;` to `src/recommend/mod.rs`.

**Acceptance criteria:**
- `cargo test` passes
- `ariadne_suggest_placement` MCP tool is registered and callable
- Returns error for empty depends_on (EC-11, E007)
- Returns error for nonexistent dependency paths (EC-13)
- Suggests correct cluster based on dependency majority (EC-12)
- Warns about cycle creation when detected (EC-14)
- Handles path conflicts (EC-15)
- Suggests appropriate layer depth (EC-18)
- Generates alternatives with risk annotations

---

## Chunk 6: D24 — refactor_opportunities + MCP Tool

**Goal:** Implement refactoring opportunity scanning with Pareto ranking (D24) and wire it as the `ariadne_refactor_opportunities` MCP tool.

**Files:**

| File | Action | Description |
|------|--------|-------------|
| `src/recommend/refactor.rs` | NEW | Opportunity scanning and ranking |
| `src/mcp/tools_recommend.rs` | MODIFY | Add RefactorOpportunitiesParam + handler |
| `src/recommend/mod.rs` | MODIFY | Add `pub mod refactor;` |
| `src/mcp/tools.rs` | MODIFY | Import RefactorOpportunitiesParam, add tool schema + dispatch arm |

**Design sources:** D-109, D-112, TD-1, TD-4, TD-5, TD-6, ROADMAP D24

**What to implement:**

1. Create `src/recommend/refactor.rs` with:
   - `pub fn find_refactor_opportunities(scope: Option<&str>, edges: &[Edge], nodes: &[Node], symbol_index: Option<&SymbolIndex>, temporal: Option<&TemporalData>, smells: &[SmellReport], min_impact: Option<Impact>) -> RefactorAnalysis`
   - Step 1: Filter nodes by scope prefix. If scope provided and no nodes match, return empty result (EC-19, EC-21).
   - Step 2: Detect opportunity types:
     a. **break_cycle**: Run SCC on filtered subgraph. Each SCC with >1 node generates a BreakCycle opportunity. Effort = f(cycle_size), impact = f(total_blast_radius_of_cycle_members).
     b. **split_file**: For files flagged as god-files by smell detection, run `analyze_split`. If `should_split: true`, create a SplitFile opportunity. Effort = High (refactoring effort), impact = f(centrality_reduction).
     c. **reduce_coupling**: For file pairs with temporal co-change coupling > threshold (if temporal available) or structural coupling > threshold, create ReduceCoupling opportunity. Effort = Medium, impact = f(coupling_strength * blast_radius).
     d. **merge_modules**: For file pairs with high mutual dependency and small individual size (< threshold lines each), create MergeModules opportunity. Effort = Low, impact = Low/Medium.
     e. **extract_interface**: For files with high afferent coupling (many distinct importers) and concrete implementations, create ExtractInterface opportunity. Effort = Medium, impact = f(afferent_coupling).
   - Step 3: Assign numeric effort_score (0.0-1.0) and impact_score (0.0-1.0) to each opportunity.
   - Step 4: Run `pareto_frontier` on the (effort_score, impact_score) pairs. Map results back to opportunities.
   - Step 5: Deduplicate conflicting recommendations (EC-23): if same target appears in split_file and merge_modules, keep higher impact_score.
   - Step 6: Filter by min_impact if provided.
   - Step 7: Sort by impact_score descending, then effort_score ascending.
   - Step 8: Cap total recommendations (configurable, default 50) for large scopes (EC-22).
   - Step 9: Set data_quality and compute pareto_count.
2. Add to `src/mcp/tools_recommend.rs`:
   - `pub struct RefactorOpportunitiesParam { pub scope: Option<String>, pub min_impact: Option<String> }`
   - `pub async fn handle_refactor_opportunities(state: &GraphState, param: RefactorOpportunitiesParam) -> Result<serde_json::Value>`
   - Handler: validate scope exists if provided (EC-20, E009), parse min_impact string to Impact enum (E008), extract edges/nodes/smells/temporal from state, call `find_refactor_opportunities`, serialize result.
3. Modify `src/mcp/tools.ts`:
   - Import `RefactorOpportunitiesParam` and `handle_refactor_opportunities`.
   - Add tool schema entry for `ariadne_refactor_opportunities`.
   - Add dispatch arm.
4. Add `pub mod refactor;` to `src/recommend/mod.rs`.

**Acceptance criteria:**
- `cargo test` passes
- `ariadne_refactor_opportunities` MCP tool is registered and callable
- Detects break_cycle opportunities from known SCCs (AC-3)
- Detects split_file opportunities from known god files (AC-3)
- All opportunities have effort/impact estimates (AC-4)
- Pareto frontier correctly computed — frontier points marked `pareto: true` (FM-9.2)
- Dominated points reference their dominator
- Returns empty array for empty scope (EC-19) and no-opportunities (EC-21)
- Returns error for nonexistent scope (EC-20, E009)
- Returns error for invalid min_impact (E008)
- Handles conflicting recommendations (EC-23)
- Deterministic output (AC-9, TD-7)

---

## Chunk 7: Benchmarks + Design Doc Updates

**Goal:** Add performance benchmarks for new algorithms and update all design documentation.

**Files:**

| File | Action | Description |
|------|--------|-------------|
| `benches/algo_bench.rs` | MODIFY | Add Stoer-Wagner and Pareto benchmarks |
| `benches/mcp_bench.rs` | MODIFY | Add recommendation tool benchmarks (if bench pattern exists) |
| `design/ROADMAP.md` | MODIFY | Update Phase 9 status: PLANNED → DONE |
| `design/decisions/log.md` | MODIFY | Add D-109 through D-113 |
| `design/architecture.md` | MODIFY | Add recommend/ to module dependency table |

**Design sources:** All D-109 through D-113, AC-7, AC-8

**What to implement:**

1. Add benchmarks to `benches/algo_bench.rs`:
   - `bench_stoer_wagner_10`, `bench_stoer_wagner_25`, `bench_stoer_wagner_50`, `bench_stoer_wagner_100`, `bench_stoer_wagner_200` — complete graphs of increasing size.
   - `bench_pareto_10`, `bench_pareto_50`, `bench_pareto_100`, `bench_pareto_500` — random points.
2. Add recommendation tool benchmarks to `benches/mcp_bench.rs` (if the file follows the pattern of benchmarking tool handlers).
3. Update `design/ROADMAP.md`:
   - Change Phase 9 status from `[PLANNED]` to `[DONE]`.
   - Update completion date.
4. Update `design/decisions/log.md`:
   - Add D-109: Recommendation engine in src/recommend/ (not algo/)
   - Add D-110: Recommendation types in src/recommend/types.rs (not model/types.rs)
   - Add D-111: Stoer-Wagner min-cut in-house implementation
   - Add D-112: MCP tool handlers in src/mcp/tools_recommend.rs
   - Add D-113: Graceful degradation with DataQuality enum
5. Update `design/architecture.md`:
   - Add `recommend/` row to the module dependency table: depends on `model/`, `algo/`, `analysis/`, `temporal/`, `semantic/`; never depends on `serial/`, `pipeline/`, `parser/`, `views/`, `mcp/`, `detect/`, `cluster/`.
   - Update the `mcp/` dependency list to include `recommend/`.

**Acceptance criteria:**
- `cargo bench` runs all new benchmarks without errors
- Stoer-Wagner benchmarks complete within performance targets (< 1ms for 50 nodes)
- Pareto benchmarks complete within performance targets (< 1ms for 100 points)
- ROADMAP.md shows Phase 9 as DONE
- Decision log contains D-109 through D-113
- Architecture.md includes recommend/ module

---

## Summary

| Chunk | Name | Files | Dependencies | Parallelizable |
|-------|------|-------|-------------|----------------|
| 1 | Module Skeleton + Types | 3 (2 NEW, 1 MODIFY) | None | — |
| 2 | Stoer-Wagner Min-Cut | 2 (1 NEW, 1 MODIFY) | Chunk 1 | With Chunk 3 |
| 3 | Pareto Frontier | 2 (1 NEW, 1 MODIFY) | Chunk 1 | With Chunk 2 |
| 4 | D22 suggest_split + MCP | 5 (2 NEW, 3 MODIFY) | Chunks 1, 2 | With Chunk 5 |
| 5 | D23 suggest_placement + MCP | 4 (1 NEW, 3 MODIFY) | Chunks 1, 3 | With Chunk 4 |
| 6 | D24 refactor_opportunities + MCP | 4 (1 NEW, 3 MODIFY) | Chunks 1, 3, 4 | — |
| 7 | Benchmarks + Docs | 5 (0 NEW, 5 MODIFY) | All prior chunks | — |

**Total new files:** 8
**Total modified files:** 5 (lib.rs, mcp/mod.rs, mcp/tools.rs, benches/algo_bench.rs, benches/mcp_bench.rs) + 3 design docs
**Estimated total lines:** ~2,200-2,800
