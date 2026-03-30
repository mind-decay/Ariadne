# Phase 9: Recommendation Engine — Specification

## Goal

Move Ariadne from "here is data" to "here is what you should do." The recommendation engine suggests concrete, actionable refactoring recommendations based on graph analysis, symbol clustering, temporal data, and existing metrics (centrality, blast radius, smells).

## Dependencies

| Phase | Status | What It Provides |
|-------|--------|-----------------|
| Phase 4 (Symbol Graph) | DONE | SymbolIndex, CallGraph — intra-file symbol call edges |
| Phase 5 (Context Engine) | DONE | GraphState, file/edge/node access via MCP |
| Phase 7 (Git Temporal) | DONE | TemporalData — churn, co-change coupling, hotspots |
| Phase 8 (Semantic Boundaries) | DONE | Boundary extraction — HTTP routes, events, DI |

Phase 8 benefits recommendations but is not strictly required (graceful degradation per TD-6).

## Risk Classification

**Overall: YELLOW** — New algorithms (min-cut, Pareto) are well-understood but integration surface is broad.

| # | Deliverable | Risk | Rationale |
|---|-------------|------|-----------|
| D22 | `ariadne_suggest_split` | YELLOW | Min-cut algorithm is straightforward; integration with SymbolIndex needs validation |
| D23 | `ariadne_suggest_placement` | GREEN | Reuses existing layer/cluster infrastructure |
| D24 | `ariadne_refactor_opportunities` | YELLOW | Broadest integration surface; false positive rate is the key risk |
| FM-9.1 | Stoer-Wagner min-cut | GREEN | Well-known O(V^3) algorithm, small inputs (5-50 symbols) |
| FM-9.2 | Pareto frontier | GREEN | Trivial O(n^2), <100 recommendations |

## Design Sources

| Decision | Description | Source |
|----------|-------------|--------|
| TD-1 / D-109 | Recommendation engine in `src/recommend/` (not `src/algo/`) | `design/architecture.md` — algo/ 12-file circular dependency |
| TD-2 / D-110 | Types in `src/recommend/types.rs` (not `src/model/types.rs`) | `design/architecture.md` — model/types.rs blast radius rank 1 |
| TD-3 / D-111 | Stoer-Wagner in-house (~150 lines) | ROADMAP FM-9.1 |
| TD-4 | Pareto frontier in `src/recommend/pareto.rs` | ROADMAP FM-9.2 |
| TD-5 / D-112 | MCP handlers in `src/mcp/tools_recommend.rs` | Existing pattern: tools_context.rs, tools_semantic.rs, tools_temporal.rs |
| TD-6 / D-113 | Graceful degradation with DataQuality enum | Requirements EC-29, EC-30 |
| TD-7 | Determinism via BTreeMap/BTreeSet | `design/determinism.md` |

## Deliverables

### D22: `ariadne_suggest_split` — File Decomposition Recommendations

**New files:**
- `src/recommend/split.rs` — core split analysis algorithm

**Type definitions:**

```rust
/// A suggested file split with the symbols to move
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SplitGroup {
    pub name: String,                    // suggested filename
    pub symbols: BTreeSet<String>,       // symbol names to move
    pub estimated_lines: u32,
    pub rationale: String,
}

/// Result of file split analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SplitAnalysis {
    pub path: String,                    // analyzed file path
    pub should_split: bool,
    pub reason: String,
    pub suggested_splits: Vec<SplitGroup>,
    pub cut_weight: f64,                 // coupling between groups (min-cut value)
    pub impact: SplitImpact,
    pub data_quality: DataQuality,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SplitImpact {
    pub blast_radius_before: u32,
    pub blast_radius_after_estimate: u32,
    pub centrality_before: f64,
    pub centrality_reduction_estimate: f64,
}
```

**Interface contract:**

```rust
// src/recommend/split.rs
pub fn analyze_split(
    path: &CanonicalPath,
    edges: &[Edge],
    nodes: &[Node],
    symbol_index: Option<&SymbolIndex>,
    call_graph: Option<&CallGraph>,
    temporal: Option<&TemporalData>,
) -> SplitAnalysis
```

**Algorithm:**
1. Build a `SymbolGraph` from intra-file call edges (from SymbolIndex/CallGraph).
2. If symbol data unavailable, fall back to file-level metrics (line count, export count, fan-out). Set `data_quality: Structural` or `Minimal`.
3. Run Stoer-Wagner min-cut on the SymbolGraph (FM-9.1).
4. Run Louvain on the same SymbolGraph. If >80% agreement with min-cut partition, prefer Louvain (already validated).
5. Validate partition balance: if one partition has <20% of symbols, apply recursive bipartitioning or fall back to Louvain.
6. Compute blast radius before/after estimates via `algo::blast_radius`.
7. Compute centrality reduction estimates via `algo::centrality`.
8. If temporal data available, weight symbol edges by co-change frequency.

**Edge cases:** EC-1 through EC-10 (see Edge Cases section).

**Design sources:** TD-1, TD-3, TD-6, TD-7, ROADMAP D22.

---

### D23: `ariadne_suggest_placement` — New File Location Recommendations

**New files:**
- `src/recommend/placement.rs` — placement suggestion algorithm

**Type definitions:**

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlacementSuggestion {
    pub suggested_path: String,
    pub cluster: String,                 // ClusterId as string
    pub layer: String,                   // architectural layer name
    pub arch_depth: u32,
    pub reasoning: Vec<String>,
    pub alternatives: Vec<PlacementAlternative>,
    pub data_quality: DataQuality,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlacementAlternative {
    pub path: String,
    pub cluster: String,
    pub risk: String,                    // human-readable risk annotation
}
```

**Interface contract:**

```rust
// src/recommend/placement.rs
pub fn suggest_placement(
    description: &str,
    depends_on: &[CanonicalPath],
    depended_by: &[CanonicalPath],
    edges: &[Edge],
    nodes: &[Node],
    clusters: &BTreeMap<ClusterId, Vec<CanonicalPath>>,
    layers: &BTreeMap<u32, Vec<CanonicalPath>>,
) -> PlacementSuggestion
```

**Algorithm:**
1. Resolve dependency files to their clusters and layers.
2. Score each cluster by dependency overlap (majority vote: cluster with most `depends_on` members wins).
3. Determine architectural depth: one level above the highest dependency layer (or same level if `depended_by` constrains it).
4. Infer layer from detect module's layer classification for the dependency neighborhood.
5. Generate path based on cluster directory pattern + description keywords.
6. Generate 1-3 alternatives with risk annotations (e.g., "cross-cluster placement", "creates upward dependency").
7. Check for cycle creation if `depended_by` is provided; add warning if detected (EC-14).

**Edge cases:** EC-11 through EC-18 (see Edge Cases section).

**Design sources:** TD-1, TD-6, TD-7, ROADMAP D23.

---

### D24: `ariadne_refactor_opportunities` — Proactive Refactoring Analysis

**New files:**
- `src/recommend/refactor.rs` — opportunity scanning and ranking

**Type definitions:**

```rust
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RefactorType {
    ExtractInterface,
    BreakCycle,
    MergeModules,
    SplitFile,
    ReduceCoupling,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefactorOpportunity {
    pub refactor_type: RefactorType,
    pub target: Vec<String>,             // file path(s)
    pub symbols: BTreeSet<String>,       // relevant symbols (may be empty)
    pub benefit: String,
    pub effort: Effort,
    pub impact: Impact,
    pub effort_score: f64,               // numeric for Pareto: 0.0 (easy) to 1.0 (hard)
    pub impact_score: f64,               // numeric for Pareto: 0.0 (none) to 1.0 (transformative)
    pub pareto: bool,
    pub dominated_by: Option<usize>,     // index into the opportunities array
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefactorAnalysis {
    pub scope: String,
    pub opportunities: Vec<RefactorOpportunity>,
    pub pareto_count: usize,
    pub data_quality: DataQuality,
}
```

**Interface contract:**

```rust
// src/recommend/refactor.rs
pub fn find_refactor_opportunities(
    scope: Option<&str>,
    edges: &[Edge],
    nodes: &[Node],
    symbol_index: Option<&SymbolIndex>,
    temporal: Option<&TemporalData>,
    smells: &[SmellReport],
    min_impact: Option<Impact>,
) -> RefactorAnalysis
```

**Algorithm:**
1. Filter nodes by scope prefix (if provided).
2. Scan for opportunity types:
   - **break_cycle**: Use SCC detection from `algo::scc`. Each non-trivial SCC is a cycle-breaking candidate. Effort based on cycle size, impact based on blast radius of cycle members.
   - **split_file**: Reuse `analyze_split` (D22) for files exceeding thresholds (line count, centrality, export count). Pre-filter using existing smell signals (god files).
   - **reduce_coupling**: Use temporal co-change coupling (if available) and structural coupling. Files with high coupling and low cohesion are candidates.
   - **merge_modules**: Identify file pairs with high mutual dependency and small individual size. Merge if combined would not exceed size thresholds.
   - **extract_interface**: Identify concrete types with high afferent coupling (many dependents). Extract interface to reduce coupling.
3. Assign effort/impact scores (numeric 0.0-1.0) to each opportunity.
4. Compute Pareto frontier over (effort_score, impact_score) via FM-9.2. Lower effort + higher impact = better.
5. Tag each opportunity with `pareto: true/false` and `dominated_by` reference.
6. Post-process: deduplicate conflicting recommendations (EC-23). If same file appears in conflicting types (split + merge), keep the higher-impact one.
7. Filter by `min_impact` if provided.
8. Sort by impact descending, then effort ascending.

**Edge cases:** EC-19 through EC-25 (see Edge Cases section).

**Design sources:** TD-1, TD-4, TD-6, TD-7, ROADMAP D24.

---

### Shared Types

**New file:** `src/recommend/types.rs`

```rust
use std::collections::BTreeSet;
use serde::{Serialize, Deserialize};

/// Effort level for a recommendation
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Effort {
    Low,
    Medium,
    High,
}

/// Impact level for a recommendation
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Impact {
    Low,
    Medium,
    High,
}

/// Data quality level indicating available analysis depth
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DataQuality {
    Full,        // symbol index + temporal + semantic
    Structural,  // file-level graph only (no symbol index)
    Minimal,     // graph only, no temporal
}

/// Weighted undirected graph for symbol-level analysis
#[derive(Debug, Clone)]
pub struct SymbolGraph {
    pub nodes: Vec<String>,              // symbol names
    pub weights: Vec<Vec<f64>>,          // adjacency matrix (symmetric)
}

/// Result of Stoer-Wagner min-cut
#[derive(Debug, Clone)]
pub struct MinCutResult {
    pub cut_weight: f64,
    pub partition_a: BTreeSet<usize>,    // node indices
    pub partition_b: BTreeSet<usize>,    // node indices
}
```

**Design sources:** TD-2, TD-7.

---

### FM-9.1: Stoer-Wagner Min-Cut

**New file:** `src/recommend/min_cut.rs`

**Interface contract:**

```rust
/// Stoer-Wagner global minimum cut on a weighted undirected graph.
/// Returns None if graph has fewer than 2 nodes.
pub fn stoer_wagner(graph: &SymbolGraph) -> Option<MinCutResult>
```

**Algorithm:** Standard Stoer-Wagner global minimum cut. O(V^3) time, O(V^2) space. For typical file symbol graphs (5-50 nodes), this completes in microseconds.

**Threshold:** If symbol count exceeds 200 (EC-7), fall back to Louvain clustering and skip min-cut. Log warning W013.

**Design sources:** TD-3, ROADMAP FM-9.1.

---

### FM-9.2: Pareto Frontier

**New file:** `src/recommend/pareto.rs`

**Interface contract:**

```rust
/// Compute 2D Pareto frontier. Lower effort + higher impact = better.
/// Returns (is_on_frontier, dominated_by_index) for each point.
/// Points: (effort_score, impact_score) where lower effort and higher impact are preferred.
pub fn pareto_frontier(points: &[(f64, f64)]) -> Vec<(bool, Option<usize>)>
```

**Algorithm:** For each point, check if any other point dominates it (lower or equal effort AND higher or equal impact, with at least one strict inequality). O(n^2) for n recommendations (typically <100).

**Design sources:** TD-4, ROADMAP FM-9.2.

---

### MCP Tool Registration

**New file:** `src/mcp/tools_recommend.rs`

**Modified files:**
- `src/mcp/mod.rs` — add `mod tools_recommend;`
- `src/mcp/tools.rs` — import param types, add 3 dispatch arms

**Param structs:**

```rust
#[derive(Deserialize)]
pub struct SuggestSplitParam {
    pub path: String,
}

#[derive(Deserialize)]
pub struct SuggestPlacementParam {
    pub description: String,
    pub depends_on: Vec<String>,
    #[serde(default)]
    pub depended_by: Vec<String>,
}

#[derive(Deserialize)]
pub struct RefactorOpportunitiesParam {
    pub scope: Option<String>,
    pub min_impact: Option<String>,
}
```

**Dispatch pattern (in tools.ts):**

```rust
"ariadne_suggest_split" => {
    let param: SuggestSplitParam = serde_json::from_value(args)?;
    handle_suggest_split(&state, param).await
}
"ariadne_suggest_placement" => {
    let param: SuggestPlacementParam = serde_json::from_value(args)?;
    handle_suggest_placement(&state, param).await
}
"ariadne_refactor_opportunities" => {
    let param: RefactorOpportunitiesParam = serde_json::from_value(args)?;
    handle_refactor_opportunities(&state, param).await
}
```

**Design sources:** TD-5.

---

## NOT in Phase 9

- **FM-9.3 (Network Motifs):** EVALUATE-FIRST per ROADMAP. Deferred until evaluation determines value. Will be a separate phase or sub-task if evaluation is positive.
- **FM-9.4 (Formal Concept Analysis):** HIGH RISK, EVALUATE-FIRST per ROADMAP. Concept lattice may be exponentially large. Deferred to research backlog.
- **Moira integration:** Estimated ~Phase 20 per ROADMAP. Affects MCP Registry, Metis, Daedalus, Mnemosyne, Analytical Pipeline, Q2 Gates.
- **CLI subcommands for recommendations:** MCP-only for now.
- **External dependency intelligence:** Phase 10.

## New Error and Warning Codes

| Code | Condition | Message |
|------|-----------|---------|
| E006 | File not in graph (D22) | `File not found in graph: {path}` |
| E007 | Empty depends_on (D23) | `depends_on must contain at least one valid file path` |
| E008 | Invalid min_impact value (D24) | `Invalid min_impact value: {value}. Expected: low, medium, high` |
| E009 | Scope not found (D24) | `No files found in scope: {scope}` |
| W010 | Stale graph data | `Graph data may be stale — source files changed since last build` |
| W011 | Symbol index unavailable | `Symbol index not available — using file-level analysis only` |
| W012 | Temporal data unavailable | `Temporal data not available — skipping churn-based weighting` |
| W013 | Min-cut fallback to Louvain | `Symbol count exceeds threshold ({count}) — using Louvain clustering` |
| W014 | Trivial min-cut partition | `Min-cut produced unbalanced partition — falling back to Louvain` |
| W015 | Cycle creation warning (D23) | `Placing file here creates dependency cycle with {paths}` |

## New Decision Log Entries

| # | Decision | Rationale |
|---|----------|-----------|
| D-109 | Recommendation engine lives in `src/recommend/` (not `src/algo/`) | Avoid worsening 12-file algo/ circular dependency; recommendations are leaf consumers |
| D-110 | Recommendation types in `src/recommend/types.rs` (not `src/model/types.rs`) | Avoid blast radius amplification on project's highest-blast-radius file (rank 1, score 0.694) |
| D-111 | Stoer-Wagner min-cut implemented in-house (~150 lines) | No external graph crate needed; symbol graphs are small (5-50 nodes); O(V^3) trivially fast |
| D-112 | MCP tool handlers in `src/mcp/tools_recommend.rs` | Follow established split pattern (tools_context.rs, tools_semantic.rs, tools_temporal.rs) |
| D-113 | Graceful degradation with DataQuality enum | Recommendations degrade from full (symbol+temporal) to structural (file-level) to minimal (graph-only) |

## Module Structure

### New Files

```
src/recommend/
├── mod.rs              # Module root: re-exports, shared utilities
├── types.rs            # All recommendation output types + enums
├── min_cut.rs          # Stoer-Wagner global min-cut (FM-9.1)
├── pareto.rs           # 2D Pareto frontier computation (FM-9.2)
├── split.rs            # D22: file split analysis (suggest_split)
├── placement.rs        # D23: new file placement (suggest_placement)
└── refactor.rs         # D24: refactoring opportunity scan (refactor_opportunities)

src/mcp/
└── tools_recommend.rs  # MCP tool param structs + handlers for D22/D23/D24
```

### Modified Existing Files

| File | Change | Risk |
|------|--------|------|
| `src/lib.rs` | Add `pub mod recommend;` | LOW — single line |
| `src/mcp/mod.rs` | Add `mod tools_recommend;` | LOW — single line |
| `src/mcp/tools.rs` | Import param types, add 3 tool dispatch arms + 3 tool schema entries | MEDIUM — follows existing pattern |

### Dependency Rules

| Module | Depends On | Never Depends On |
|--------|-----------|-----------------|
| `recommend/types.rs` | `serde`, `std::collections` | everything else |
| `recommend/min_cut.rs` | `recommend/types.rs` | `model/`, `algo/`, `mcp/` |
| `recommend/pareto.rs` | `recommend/types.rs` | `model/`, `algo/`, `mcp/` |
| `recommend/split.rs` | `recommend/types.rs`, `recommend/min_cut.rs`, `model/`, `algo/` | `mcp/`, `serial/`, `pipeline/`, `parser/` |
| `recommend/placement.rs` | `recommend/types.rs`, `model/`, `algo/` | `mcp/`, `serial/`, `pipeline/`, `parser/` |
| `recommend/refactor.rs` | `recommend/types.rs`, `recommend/pareto.rs`, `model/`, `algo/`, `analysis/` | `mcp/`, `serial/`, `pipeline/`, `parser/` |
| `mcp/tools_recommend.rs` | `recommend/`, `mcp/state.rs`, `model/` | `parser/` |

## Data Flow

```
MCP Request (JSON)
  │
  ▼
tools.rs ──dispatch──► tools_recommend.rs
                            │
                            ├── validate input params
                            ├── extract data from GraphState
                            │
                            ▼
                      recommend/split.rs          (D22)
                      recommend/placement.rs      (D23)
                      recommend/refactor.rs       (D24)
                            │
                            ├── algo::blast_radius
                            ├── algo::centrality
                            ├── algo::scc
                            ├── algo::louvain
                            ├── recommend/min_cut.rs  (FM-9.1)
                            ├── recommend/pareto.rs   (FM-9.2)
                            ├── analysis::smells
                            ├── temporal data (optional)
                            │
                            ▼
                      recommend/types.rs (output DTOs)
                            │
                            ▼
                      JSON serialization → MCP Response
```

## Performance Targets

| Metric | Target |
|--------|--------|
| `suggest_split` (typical file, 5-50 symbols) | < 10ms |
| `suggest_split` (large file, 200 symbols) | < 100ms |
| `suggest_placement` | < 5ms |
| `refactor_opportunities` (entire project, 300 files) | < 500ms |
| `refactor_opportunities` (scoped, 50 files) | < 100ms |
| Stoer-Wagner min-cut (50 nodes) | < 1ms |
| Pareto frontier (100 points) | < 1ms |
| Memory overhead per recommendation call | < 10MB |

## Success Criteria

1. **AC-1:** `ariadne_suggest_split` identifies valid decomposition for god files using symbol clustering — tested against known god files (tools.ts is a known split candidate).
2. **AC-2:** `ariadne_suggest_placement` recommends correct layer/cluster based on dependency analysis — tested with known file locations.
3. **AC-3:** `ariadne_refactor_opportunities` finds cycles, coupling issues, merge candidates — run against project with known issues.
4. **AC-4:** All suggestions include effort/impact estimates — schema validation on all output types.
5. **AC-5:** Less than 10% false positive rate on split/placement suggestions — evaluated against human-assessed test cases.
6. **AC-6:** All 3 MCP tools registered and callable — MCP tool list includes all 3 new tools.
7. **AC-7:** `cargo test` passes with no regressions — CI green.
8. **AC-8:** Design docs updated (ROADMAP status, decision log, architecture if needed) — doc review.
9. **AC-9:** Deterministic output (byte-identical on repeated runs) — invariant test.

## Testing Requirements

### L1: Unit Tests

- **min_cut.rs:** Test Stoer-Wagner on known graphs (complete graph, path graph, disconnected graph, single-edge graph, weighted graph with known min-cut).
- **pareto.rs:** Test frontier computation (all dominated, none dominated, mixed, ties, single point, empty input).
- **split.rs:** Test with mock symbol graphs (clear bipartition, no split needed, unbalanced partition, fallback to Louvain).
- **placement.rs:** Test cluster scoring (single cluster, multi-cluster, empty clusters, conflicting layers).
- **refactor.rs:** Test opportunity detection for each RefactorType individually. Test Pareto ranking integration. Test deduplication of conflicting recommendations.
- **types.rs:** Test serialization/deserialization roundtrip for all types. Test BTreeSet ordering.

### L2: Integration Tests

- **D22 via MCP:** Call `ariadne_suggest_split` on a test fixture with known god file. Verify output schema and that suggestions are coherent.
- **D23 via MCP:** Call `ariadne_suggest_placement` with known dependency context. Verify suggested layer and cluster match expectations.
- **D24 via MCP:** Call `ariadne_refactor_opportunities` on test fixture. Verify known cycles and smells are detected.
- **Graceful degradation:** Test all three tools with missing symbol index and missing temporal data. Verify `data_quality` field is correct.
- **Input validation:** Test all error cases (nonexistent path, empty depends_on, invalid min_impact, etc.).

### L3: Invariant Tests

- **Determinism:** Run each tool twice on the same input, compare output byte-for-byte.
- **Pareto correctness:** For every point marked `pareto: false`, verify that `dominated_by` refers to a point that actually dominates it.
- **Min-cut optimality:** For small graphs, verify min-cut weight against brute-force enumeration.

### L4: Performance Tests

- **Stoer-Wagner benchmark:** 10, 25, 50, 100, 200 node complete graphs.
- **Pareto benchmark:** 10, 50, 100, 500 points.
- **suggest_split benchmark:** Real project file with varying symbol counts.
- **refactor_opportunities benchmark:** Full project scan vs scoped scan.

## Risk Assessment

| Risk | Severity | Mitigation |
|------|----------|------------|
| tools.ts god-file growth | HIGH | All handler logic in tools_recommend.rs (TD-5/D-112); tools.ts gets only 3 dispatch arms |
| algo/ circular dependency worsening | HIGH | Separate recommend/ module (TD-1/D-109); import only from algo/mod.rs re-exports |
| model/types.rs blast radius amplification | MEDIUM | Types in recommend/types.rs (TD-2/D-110); no additions to model/types.rs |
| Min-cut produces trivial partitions | MEDIUM | Validate partition balance; fall back to Louvain if <20% in smaller partition |
| False positive rate >10% | MEDIUM | Conservative thresholds; calibrate against known split candidates; require 3+ symbols per cluster |
| Symbol graph construction mismatch | MEDIUM | Verify SymbolIndex provides intra-file call edges; fall back to co-location heuristics |
| FM-9.3/FM-9.4 scope creep | LOW | Explicitly deferred; evaluate-first gate before any implementation |
| Determinism guarantee | MEDIUM | BTreeMap/BTreeSet throughout (TD-7); round4() for all floats; sorted iteration |
| Performance on large projects | LOW | Cap files analyzed for split_file in D24; pre-filter using smell signals |

## Edge Cases

### D22: suggest_split (EC-1 through EC-10)

| # | Edge Case | Expected Behavior |
|---|-----------|-------------------|
| EC-1 | File has 0 or 1 symbols | `should_split: false`, reason: "insufficient symbols" |
| EC-2 | File has no internal call edges between symbols | Each symbol is its own cluster; suggest split only if export count is high |
| EC-3 | All symbols tightly coupled (single cluster) | `should_split: false`, reason: "single responsibility cluster" |
| EC-4 | File not in graph (nonexistent path) | Error E006: file not found |
| EC-5 | File is a test file | `should_split: false` or warning — test files have different split criteria |
| EC-6 | File has circular internal dependencies among symbols | Min-cut still works; report but do not block |
| EC-7 | Very large file (500+ symbols) | Fall back to Louvain; warn W013 |
| EC-8 | File with only re-exports (mod.rs pattern) | `should_split: false`, reason: "re-export hub" |
| EC-9 | Binary/generated files | Reject with appropriate error |
| EC-10 | File with mixed visibility (pub + private symbols) | Private symbols cluster with their public consumers |

### D23: suggest_placement (EC-11 through EC-18)

| # | Edge Case | Expected Behavior |
|---|-----------|-------------------|
| EC-11 | Empty `depends_on` list | Error E007: cannot determine placement without dependencies |
| EC-12 | `depends_on` files span multiple clusters | Suggest cluster with most dependencies; include others as alternatives |
| EC-13 | `depends_on` files don't exist in graph | Error E006: referenced files not found |
| EC-14 | Circular dependency would be created | Warning W015 in output; still suggest path with caveat |
| EC-15 | Suggested path already exists | Include conflict in output; suggest alternative name |
| EC-16 | `depended_by` conflicts with `depends_on` (layer violation) | Warning: "requested consumers at lower depth than dependencies" |
| EC-17 | Description matches no existing patterns | Fall back to dependency-only analysis; note "no similar files found" |
| EC-18 | All dependencies are in different layers | Suggest layer one above the highest dependency |

### D24: refactor_opportunities (EC-19 through EC-25)

| # | Edge Case | Expected Behavior |
|---|-----------|-------------------|
| EC-19 | Empty project / no files in scope | Return empty opportunities array |
| EC-20 | Scope path doesn't exist | Error E009: scope directory not found |
| EC-21 | No opportunities found | Return empty opportunities array (not an error) |
| EC-22 | Very large scope (300+ files) | Cap recommendations; pre-filter with smell signals |
| EC-23 | Conflicting recommendations (split + merge same file) | Keep higher-impact; mark conflicting |
| EC-24 | All recommendations have same effort/impact | All on Pareto frontier (`pareto: true` for all) |
| EC-25 | Scope contains only test files | Return empty or test-specific recommendations |

### Cross-Cutting (EC-26 through EC-30)

| # | Edge Case | Expected Behavior |
|---|-----------|-------------------|
| EC-26 | Graph not built yet | Error: "run ariadne build first" |
| EC-27 | Stale graph data | Warning W010 |
| EC-28 | Concurrent MCP calls | Safe for concurrent read access; no mutable state |
| EC-29 | Symbol index not available | Graceful degradation; `data_quality: Structural`; warn W011 |
| EC-30 | Temporal data not available | Graceful degradation; `data_quality: Minimal`; warn W012 |

## Contract Interfaces (for Parallel Implementation)

### Batch 1: Types + Algorithms (no external deps except std)

Files: `src/recommend/types.rs`, `src/recommend/min_cut.rs`, `src/recommend/pareto.rs`, `src/recommend/mod.rs`

These files depend only on `serde`, `std::collections`, and each other. They can be implemented and tested in isolation.

### Batch 2: Core Logic (depends on Batch 1 + algo/ + model/)

Files: `src/recommend/split.rs`, `src/recommend/placement.rs`, `src/recommend/refactor.rs`

These files depend on Batch 1 types and import from `algo/`, `model/`, `analysis/`. They can be implemented in parallel with each other after Batch 1 is complete.

### Batch 3: MCP Integration (depends on Batch 2 + mcp/)

Files: `src/mcp/tools_recommend.rs`, modifications to `src/mcp/tools.rs`, `src/mcp/mod.rs`, `src/lib.rs`

This batch wires everything into the MCP server. Depends on all Batch 2 deliverables.

### Batch 4: Tests + Benchmarks + Docs

Files: `tests/recommend_tests.rs`, benchmark additions, design doc updates.

Depends on all prior batches.
