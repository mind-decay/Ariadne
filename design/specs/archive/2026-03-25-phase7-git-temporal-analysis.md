# Phase 7: Git Temporal Analysis — Specification

## Goal

Add time dimension to Ariadne — co-change patterns, code churn, file ownership, hotspot detection via git history analysis. Transforms Ariadne from "static snapshot" to "structural + temporal intelligence."

## Dependencies

**Phase 6 (MCP Protocol Expansion) must be complete.** Phase 7 builds on:

- `GraphState` with `ProjectGraph`, `ClusterMap`, `StatsOutput`, derived indices
- `SymbolIndex`, `CallGraph`, `reverse_index`, `forward_index`, `layer_index`
- PageRank, spectral analysis, community detection
- 19 MCP tools (ariadne_overview through ariadne_compressed)
- Agent Context Engine (`ariadne_context`, `ariadne_importance`, `ariadne_reading_order`)
- `DiagnosticCollector` with W001-W023 warning codes
- `SmellType` enum with existing smell variants in `model/smell.rs`
- `algo/blast_radius.rs` for blast radius computation (used in hotspot scoring)
- Deterministic output infrastructure (D-049)

## Risk Classification

**Overall: YELLOW**

Phase 7 introduces external process dependency (git CLI) and O(n^2) potential in co-change analysis. Git output parsing is well-understood but platform-sensitive. All computation is front-loaded at startup — no runtime complexity in tool handlers.

### Per-Deliverable Risk

| # | Deliverable | Risk | Rationale |
|---|------------|------|-----------|
| D1 | Temporal Data Types | GREEN | Pure data structs in `model/temporal.rs`. No logic, no dependencies. |
| D2 | Git History Parser | YELLOW | Shells out to `git log`. Output format varies across git versions. Rename detection adds parsing complexity. Platform path separators. |
| D3 | Churn/Ownership Computation | GREEN | Straightforward aggregation over parsed commit data. Well-defined time windows. |
| D4 | Co-Change Analysis | YELLOW | O(n^2) potential mitigated by guards (min 3 co-occurrences, max 10k pairs, skip bulk commits). Jaccard is simple but must handle edge cases (division by zero). |
| D5 | Hotspot Scoring | GREEN | Rank-based formula combining churn, LOC, blast radius. All inputs pre-computed. |
| D6 | MCP Tool Integration | GREEN | 5 new tools + 5 enhanced tools. All read pre-computed `TemporalState`. Thin wrappers. |
| D7 | Graceful Degradation | GREEN | Three-tier git detection (no binary, not a repo, shallow clone). Follows established `Option` pattern. |

## Design Sources

| Decision | Description | Source |
|----------|------------|--------|
| D-091 | Temporal as optional feature (`Option`, graceful degradation, no feature flag) | epic-architecture.md |
| D-092 | Single `git log --numstat -M` with streaming parse (no git2 dependency) | epic-architecture.md, GAP-03 |
| D-093 | Types in `model/temporal.rs`, computation in `temporal/` | epic-architecture.md |
| D-094 | Jaccard-only co-change confidence (FM deferred to Phase 7b) | epic-architecture.md, GAP-12 |
| D-095 | LOC as complexity proxy (`Node.lines`), honest naming (`loc_rank` not `complexity_rank`) | epic-architecture.md, GAP-07 |
| D-096 | Temporal coupling smell detection in `analysis/smells.rs` | epic-architecture.md |
| D-097 | Rename tracking via `-M` flag path mapping | epic-architecture.md |
| — | ROADMAP Phase 7 (D14, D15, D16) | ROADMAP.md |
| — | architecture.md (model/, algo/ patterns) | architecture.md |

## Deliverables

### D1: Temporal Data Types

**New file:** `src/model/temporal.rs`
**Modified file:** `src/model/mod.rs` (re-exports)

Pure data types for temporal analysis results. Leaf module — no dependencies beyond `serde` and `model/types.rs`.

```rust
use std::collections::BTreeMap;
use serde::{Serialize, Deserialize};
use super::types::CanonicalPath;

/// Complete temporal analysis state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemporalState {
    /// Per-file change frequency metrics
    pub churn: BTreeMap<CanonicalPath, ChurnMetrics>,
    /// File pairs that change together above threshold
    pub co_changes: Vec<CoChange>,
    /// Per-file author/ownership data
    pub ownership: BTreeMap<CanonicalPath, OwnershipInfo>,
    /// Combined structural + temporal risk scores
    pub hotspots: Vec<Hotspot>,
    /// Whether this was computed from a shallow clone (limited history)
    pub shallow: bool,
    /// Number of commits analyzed
    pub commits_analyzed: u32,
    /// Analysis window start (ISO 8601)
    pub window_start: String,
    /// Analysis window end (ISO 8601)
    pub window_end: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChurnMetrics {
    pub commits_30d: u32,
    pub commits_90d: u32,
    pub commits_1y: u32,
    pub lines_changed_30d: u32,
    pub lines_changed_90d: u32,
    pub authors_30d: u32,
    /// ISO 8601 date of last modification
    pub last_changed: Option<String>,
    /// Top 3 authors by commit count in analysis window
    pub top_authors: Vec<(String, u32)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoChange {
    pub file_a: CanonicalPath,
    pub file_b: CanonicalPath,
    pub co_change_count: u32,
    /// Jaccard index: co_changes(A,B) / (changes(A) + changes(B) - co_changes(A,B))
    pub confidence: f64,
    /// true if also connected in import graph
    pub has_structural_link: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OwnershipInfo {
    /// Most recent author who modified this file
    pub last_author: String,
    /// Top contributors by number of commits (sorted descending, max 5)
    pub top_contributors: Vec<(String, u32)>,
    /// Total distinct authors in analysis window
    pub author_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hotspot {
    pub path: CanonicalPath,
    /// Combined score: normalize(churn_rank) * normalize(loc_rank) * normalize(blast_radius_rank)
    pub score: f64,
    pub churn_rank: u32,
    /// LOC rank — NOT complexity rank. Uses Node.lines as proxy.
    pub loc_rank: u32,
    pub blast_radius_rank: u32,
}
```

**Design source:** D-093, D-095, ROADMAP D14, GAP-04, GAP-06, GAP-07

### D2: Git History Parser

**New file:** `src/temporal/git.rs`

Executes a single `git log` command and parses the output into structured commit data.

**Git command:**

```
git log --numstat -M --format="commit %H%nauthor %aN%ndate %aI" --since="1 year ago"
```

**Internal types (not exported to model/):**

```rust
pub(crate) struct CommitData {
    pub hash: String,
    pub author: String,
    pub date: String,  // ISO 8601
    pub files: Vec<FileChange>,
}

pub(crate) struct FileChange {
    pub additions: u32,
    pub deletions: u32,
    pub path: String,
    pub old_path: Option<String>,  // Present when -M detects rename
}
```

**Git availability detection (three tiers):**

1. `which git` fails → emit W024, return `None`
2. `git rev-parse --git-dir` fails → emit W025, return `None`
3. `git rev-parse --is-shallow-repository` returns true → emit W026, proceed with available history, annotate `TemporalState.shallow = true`

**Rename tracking (D-097):**

Parse `-M` rename output format (`{old} => {new}` in numstat lines). Build `BTreeMap<String, CanonicalPath>` mapping old paths to current canonical paths. Only track renames to paths that exist in the current graph.

**Streaming parse:** Output piped through `std::process::Command` and parsed line-by-line. No buffering of entire git output.

**Design source:** D-092, D-097, ROADMAP D14, GAP-03

### D3: Churn and Ownership Computation

**New files:** `src/temporal/churn.rs`, `src/temporal/ownership.rs`

**Churn computation (`churn.rs`):**
- Input: `&[CommitData]`, rename map, analysis window end timestamp
- Output: `BTreeMap<CanonicalPath, ChurnMetrics>`
- Iterates commits, maps file paths through rename map, buckets into 30d/90d/1y windows
- Tracks per-file: commit counts, lines changed, author sets, last change date, top authors

**Ownership computation (`ownership.rs`):**
- Input: `&[CommitData]`, rename map
- Output: `BTreeMap<CanonicalPath, OwnershipInfo>`
- Per-file: last author (most recent commit), top 5 contributors by commit count, distinct author count

**Design source:** D-093, GAP-04, GAP-06

### D4: Co-Change Analysis

**New file:** `src/temporal/coupling.rs`

Computes file pairs that change together, with Jaccard confidence scoring.

**Input:** `&[CommitData]`, rename map, graph edges (for `has_structural_link`)
**Output:** `Vec<CoChange>` sorted by confidence descending, capped at 10,000 pairs

**Algorithm:**
1. For each commit, collect the set of files changed (mapped through rename map)
2. For each pair in the commit's file set, increment co-occurrence count
3. After all commits processed, compute Jaccard index for each pair:
   `confidence = co_changes(A,B) / (changes(A) + changes(B) - co_changes(A,B))`
4. Filter: keep only pairs with `co_change_count >= 3`
5. Sort by confidence descending, keep top 10,000
6. For each kept pair, check if an import edge exists → set `has_structural_link`

**Guards (D-094, GAP-09):**
- Minimum co-occurrence threshold: 3 (eliminates noise)
- Maximum pairs cap: 10,000 (sorted by confidence, keep top)
- Per-commit file cap: skip commits touching > 100 files (bulk operations)
- If >10% of commits are skipped, emit W028

**Memory estimate:** 10,000 pairs x ~200 bytes = ~2MB

**Floating-point determinism:** Round Jaccard confidence to 4 decimal places (D-049 pattern).

**Design source:** D-094, ROADMAP D14, GAP-09

### D5: Hotspot Scoring

**New file:** `src/temporal/hotspot.rs`

Combines churn, LOC (as complexity proxy), and blast radius into a single risk score.

**Input:** `&BTreeMap<CanonicalPath, ChurnMetrics>`, `&ProjectGraph` (for `Node.lines`), blast radius function
**Output:** `Vec<Hotspot>` sorted by score descending

**Formula:**

```
hotspot_score = normalize(churn_rank) * normalize(loc_rank) * normalize(blast_radius_rank)
```

Where each rank is position in sorted order (1 = highest), normalized to [0, 1] by dividing by total file count.

**Design source:** D-095, GAP-07

### D6: Temporal Module Orchestrator

**New files:** `src/temporal/mod.rs`

Public API:

```rust
pub fn analyze(
    project_root: &Path,
    graph: &ProjectGraph,
    edges: &[Edge],
    collector: &DiagnosticCollector,
) -> Option<TemporalState>
```

Orchestrates: git.rs (parse) → churn.rs + ownership.rs + coupling.rs + hotspot.rs (compute) → assemble `TemporalState`.

Returns `None` with appropriate warnings when git is unavailable.

**Design source:** D-091, D-093, GAP-01

### D7: Warning Codes

**Modified file:** `src/diagnostic.rs`

| Code | Name | Cause | Handling |
|------|------|-------|----------|
| W024 | `GitNotFound` | `git` binary not in PATH | Skip temporal analysis, emit warning |
| W025 | `NotGitRepository` | Project root is not inside a git working tree | Skip temporal analysis, emit warning |
| W026 | `ShallowRepository` | Repository is a shallow clone (limited history) | Proceed with available history, annotate results |
| W027 | `GitCommandFailed` | `git log` or other git command returned error | Skip temporal analysis, emit warning |
| W028 | `TemporalAnalysisFailed` | Temporal computation failed (e.g., OOM on co-change) | Return partial results, emit warning |

**Design source:** GAP-02

### D8: Smell Type Extension

**Modified files:** `src/model/smell.rs`, `src/analysis/smells.rs`

New variant: `SmellType::TemporalCouplingWithoutImport`

**Detection criteria:** Co-change pairs with `confidence >= 0.5` AND `has_structural_link == false`.

**Severity:** MEDIUM

**Integration:** `detect_smells` signature updated to accept `Option<&TemporalState>`. When `None`, this smell type is simply not checked. Detection logic lives in `analysis/smells.rs`.

**Design source:** D-096

### D9: New MCP Tools

**Modified file:** `src/mcp/tools.rs`
**New file:** `src/mcp/tools_temporal.rs` (parameter types)

5 new MCP tools. All check `state.temporal.is_some()` and return `{"error": "temporal_unavailable", "reason": "..."}` when `None`.

| Tool | Param Struct | Defaults | Response |
|------|-------------|----------|----------|
| `ariadne_churn` | `ChurnParam { period?: String, top?: u32 }` | period: "30d", top: 20 | Files sorted by change frequency for period |
| `ariadne_coupling` | `CouplingParam { min_confidence?: f64 }` | min_confidence: 0.3 | Co-change pairs above confidence threshold |
| `ariadne_hotspots` | `HotspotsParam { top?: u32 }` | top: 20 | Files ranked by churn x LOC x blast_radius |
| `ariadne_ownership` | `OwnershipParam { path?: String }` | path: None (project-wide) | Authors/contributors per file or project-wide |
| `ariadne_hidden_deps` | `HiddenDepsParam` | (none) | Co-change pairs with NO structural (import) link |

**Design source:** ROADMAP D15, GAP-05

### D10: Enhanced Existing MCP Tools

**Modified file:** `src/mcp/tools.rs`

5 existing tools enhanced with temporal data when `TemporalState` is available. All fall back to existing behavior when temporal data is `None`.

1. **`ariadne_file`** — Add `temporal` field to response:
   ```json
   { "temporal": { "commits_30d": 5, "commits_90d": 12, "last_changed": "2026-03-20", "top_authors": [["alice", 8]] } }
   ```

2. **`ariadne_overview`** — Add `temporal` summary:
   ```json
   { "temporal": { "total_commits_30d": 142, "hotspot_count": 8, "hidden_dep_count": 3 } }
   ```

3. **`ariadne_context`** — High-churn files get relevance boost in context assembly scoring. Add churn factor to `relevance_score` computation in `algo/context.rs`.

4. **`ariadne_importance`** — Enhanced formula:
   ```
   enhanced_importance = structural_importance * (1.0 + ln_1p(churn_30d) / max_log_churn)
   ```
   When `churn=0`: `ln(1+0) = 0`, importance unchanged. When temporal unavailable, structural-only (no change).

5. **`ariadne_smells`** — Includes `TemporalCouplingWithoutImport` smell when temporal data available.

**Design source:** ROADMAP D16, GAP-11

### D11: GraphState Integration

**Modified file:** `src/mcp/state.rs`

Add to `GraphState`:

```rust
pub temporal: Option<TemporalState>,
```

Computed at load time via `temporal::analyze()` in `from_loaded_data()`. Not persisted — git IS the persistence layer. Recomputed on auto-update (fs watcher rebuild).

**Design source:** D-091, GAP-08

### D12: Lib and Module Wiring

**Modified files:** `src/lib.rs`

Add `pub mod temporal;` to module exports.

## NOT in Phase 7

The following are explicitly deferred to Phase 7b (future):

- **FM-7.1:** Mutual Information (NMI) for co-change analysis
- **FM-7.2:** Bayesian confidence with credible intervals
- **FM-7.3:** Change-Point Detection (PELT algorithm)
- **FM-7.4:** Survival Analysis (Kaplan-Meier stability scores)
- Temporal data persistence (`.ariadne/temporal.json`)
- Cyclomatic complexity (LOC proxy is sufficient)

**Reasoning:** FM features are research-grade with "Evaluate" validation steps. Jaccard is well-understood and sufficient for initial release. Phase 7b can add refinements after the Jaccard baseline is validated on real repos.

## New Error and Warning Codes

### Warnings

| Code | Condition | Message |
|------|-----------|---------|
| W024 | git binary not in PATH | "Git not found in PATH. Temporal analysis unavailable." |
| W025 | Not inside a git working tree | "Not a git repository. Temporal analysis unavailable." |
| W026 | Shallow clone detected | "Shallow repository detected. Temporal analysis will use available history (results may be incomplete)." |
| W027 | git command returned error | "Git command failed: {reason}. Temporal analysis unavailable." |
| W028 | Temporal computation failed | "Temporal analysis failed: {reason}. Returning partial results." |

No new fatal errors — temporal analysis is always optional, never fatal (D-091).

## New Decision Log Entries

| # | Decision | Rationale |
|---|----------|-----------|
| D-091 | Temporal module as optional feature: `Option<TemporalState>`, runtime detection, no feature flag | Matches existing optional patterns (spectral can fail with W012). Runtime optionality simpler than compile-time. Code is small (~1000 lines). |
| D-092 | Single `git log --numstat -M` with streaming parse, no git2 dependency | Keeps build simple (no libgit2/OpenSSL). Git binary is universal. Single invocation = O(1) process spawns regardless of project size. |
| D-093 | Data types in `model/temporal.rs`, computation in `temporal/` | Follows established pattern: `model/` = leaf data types, computation modules = logic. Consistent with D-017. |
| D-094 | Jaccard-only co-change confidence, FM deferred to Phase 7b | Jaccard is well-understood, deterministic, sufficient for initial release. FM methods require empirical validation. |
| D-095 | LOC as complexity proxy in hotspot formula, `loc_rank` naming | LOC correlates with complexity (0.87 Pearson). Available via `Node.lines`. Honest naming avoids false precision. |
| D-096 | Temporal coupling smell: confidence >= 0.5, `has_structural_link == false`, severity MEDIUM | Follows existing smell detection pattern. Optional when temporal data unavailable. |
| D-097 | Rename tracking via `-M` flag, `BTreeMap<String, CanonicalPath>` path mapping | Standard approach. Only maps to paths in current graph. Avoids per-file `--follow` (O(n) git calls). |

## Module Structure

```
src/temporal/                   # NEW — depends on model/, algo/, diagnostic
├── mod.rs                      # Public API: analyze() -> Option<TemporalState>
├── git.rs                      # Git command execution + output parsing
├── churn.rs                    # ChurnMetrics computation from parsed git data
├── coupling.rs                 # CoChange computation (Jaccard, co-occurrence matrix)
├── hotspot.rs                  # Hotspot scoring (churn × LOC × blast_radius)
└── ownership.rs                # OwnershipInfo computation from author data
```

**New file:** `src/mcp/tools_temporal.rs` — Temporal tool parameter types

**Modified existing files:**

| File | Change |
|------|--------|
| `src/model/mod.rs` | Add temporal type re-exports |
| `src/model/temporal.rs` | NEW — temporal data types |
| `src/model/smell.rs` | Add `TemporalCouplingWithoutImport` variant |
| `src/mcp/state.rs` | Add `Option<TemporalState>` to `GraphState`, call `temporal::analyze()` |
| `src/mcp/tools.rs` | 5 new tool handlers + 5 enhanced tool handlers |
| `src/mcp/tools_temporal.rs` | NEW — temporal tool parameter types |
| `src/analysis/smells.rs` | Add temporal coupling smell detector, update `detect_smells` signature |
| `src/lib.rs` | Add `pub mod temporal;` |
| `src/diagnostic.rs` | Add W024-W028 warning codes |
| `design/error-handling.md` | Document W024-W028 |
| `design/decisions/log.md` | Add D-091 through D-097 |

**Dependency rules:**

| Module | Depends on | Never depends on |
|--------|-----------|-----------------|
| `temporal/` | `model/`, `algo/` (blast_radius), `diagnostic` | `mcp/`, `serial/`, `pipeline/`, `parser/`, `views/` |
| `mcp/` (temporal tools) | `model/temporal`, `temporal/` | `parser/` (existing rule) |
| `analysis/smells.rs` | `model/temporal`, `model/smell` | `temporal/` (receives data, doesn't call compute) |

No reverse dependencies. No cycles introduced.

## Data Flow

```
Server Startup (existing)
  │
  ├─ Load graph.json, clusters.json, stats.json (existing)
  ├─ Build indices: reverse, forward, layer, symbol, call graph (existing)
  ├─ Compute: PageRank, spectral (existing)
  │
  └─ NEW: temporal::analyze(project_root, graph, edges, collector)
       │
       ├─ Check git availability (3-tier)
       │   ├─ No git binary → W024, return None
       │   ├─ Not a git repo → W025, return None
       │   └─ Shallow clone → W026, proceed with shallow=true
       │
       ├─ git log --numstat -M --format="commit %H%nauthor %aN%ndate %aI" --since="1 year ago"
       │
       ├─ Parse commits stream → Vec<CommitData>
       ├─ Build rename map: old_path → canonical_path
       ├─ Compute churn per file (30d/90d/1y windows)
       ├─ Compute co-change matrix (guards: min 3, max 10k pairs, skip >100 file commits)
       ├─ Compute ownership per file
       ├─ Compute hotspots (churn × LOC × blast_radius)
       │
       └─ return Some(TemporalState { ... })

MCP Tool Query
  │
  ├─ state.temporal.is_none() → {"error": "temporal_unavailable", "reason": "..."}
  └─ state.temporal.is_some() → serialize pre-computed data
```

## Performance Targets

| Metric | Target |
|--------|--------|
| Git log parse + temporal analysis (50k commits) | <5s |
| Git log parse + temporal analysis (10k commits) | <3s |
| Churn computation | <500ms |
| Co-change computation | <2s |
| Total temporal analysis (large repo) | <10s |
| `ariadne_churn` tool response | <10ms |
| `ariadne_hotspots` tool response | <10ms |
| `ariadne_ownership` tool response | <10ms |
| `ariadne_coupling` tool response | <50ms |
| `ariadne_hidden_deps` tool response | <50ms |

All computation is front-loaded at startup. Tools are lookups over pre-computed data, not computations.

## Success Criteria

1. `cargo build --release` compiles with no errors or warnings
2. `temporal::analyze()` returns `Some(TemporalState)` in git repositories
3. `temporal::analyze()` returns `None` with appropriate warnings (W024/W025) in non-git directories
4. All 5 new MCP tools (`ariadne_churn`, `ariadne_coupling`, `ariadne_hotspots`, `ariadne_ownership`, `ariadne_hidden_deps`) return correct data
5. All 5 enhanced MCP tools (`ariadne_file`, `ariadne_overview`, `ariadne_context`, `ariadne_importance`, `ariadne_smells`) include temporal data when available
6. Enhanced tools fall back gracefully to existing behavior when temporal data is `None`
7. Git parsing handles repos with 10k+ commits in <5s
8. Determinism: identical git history produces identical `TemporalState`
9. Shallow clone detection works and annotates results with `shallow: true`
10. All `cargo test` pass (existing + new temporal tests)

## Testing Requirements

### L1: Unit Tests

- **`temporal/git.rs`:** Parse mock git log output (captured from real repo). Test rename detection parsing. Test error handling for malformed output.
- **`temporal/churn.rs`:** Known commit data → expected churn metrics. Verify 30d/90d/1y windowing. Verify `last_changed` and `top_authors`.
- **`temporal/coupling.rs`:** Known co-occurrence data → expected Jaccard scores. Test min threshold filtering. Test max pairs cap. Test bulk commit skip.
- **`temporal/hotspot.rs`:** Known inputs → expected rankings. Verify score normalization to [0, 1].
- **`temporal/ownership.rs`:** Known author data → expected ownership info. Verify top 5 cap. Verify `author_count`.

### L2: Integration Tests

- Create temporary git repository with scripted history (10-20 commits with known patterns)
- Run `temporal::analyze()` → verify `TemporalState` matches expected values
- Verify MCP tool responses contain temporal data when served from fixture repo
- Test graceful degradation: run in non-git directory → verify `None` + warnings

### L3: Invariant Tests

| ID | Invariant |
|----|-----------|
| INV-T1 | Jaccard confidence always in [0.0, 1.0] |
| INV-T2 | Co-change pairs always have `co_change_count >= 3` |
| INV-T3 | Hotspot scores always in [0.0, 1.0] |
| INV-T4 | Churn metrics monotonic: `commits_30d <= commits_90d <= commits_1y` |
| INV-T5 | Determinism: same git history → identical TemporalState (byte-for-byte after serialization) |

### L4: Performance Tests

- Git parse: <5s for 50k commits (mock data)
- Churn computation: <500ms
- Co-change computation: <2s
- Total temporal analysis: <10s for large repos

### Additional Test Scenarios

- Rename tracking: file renamed across commits → churn attributed to current path
- Empty repository (no commits) → return `TemporalState` with empty collections
- Repository with only merge commits → skip bulk commits, handle gracefully
- Windows path separators in git output → normalize to OS separator
- Git command timeout → W027, return `None`

## Risk Assessment

| Risk | Severity | Mitigation |
|------|----------|------------|
| Git command output format varies across versions | MEDIUM | Pin to `--format` controlled output, test with git 2.20+ |
| Large repos (100k+ commits) slow git log | MEDIUM | `--since="1 year ago"` bounds the window |
| Rename chains (A→B→C) | LOW | Follow full chain via iterative mapping |
| Windows path separators in git output | LOW | Normalize `/` to OS separator in parser |
| Floating-point determinism in Jaccard | LOW | Round to 4 decimal places (D-049 pattern) |
| Co-change memory explosion | MEDIUM | Three guards: min 3 threshold, max 10k pairs, skip >100-file commits |
| Zero-churn importance formula | LOW | Use `ln_1p` (GAP-11), numerically stable |

## Contract Interfaces (for Parallel Implementation)

### Batch 1: Types + Git Parser (no dependencies between them except model)

**Contract 1A: `src/model/temporal.rs`**
- Pure data types, Serialize/Deserialize, no logic
- Re-exported from `src/model/mod.rs`

**Contract 1B: `src/diagnostic.rs` extensions**
- Add W024-W028 variants
- Add `code()` and `Display` matches

**Contract 1C: `src/temporal/git.rs`**
- Input: project root `&Path`
- Output: `Result<Vec<CommitData>, TemporalError>`
- Internal struct `CommitData { hash, author, date, files: Vec<FileChange> }`
- Internal struct `FileChange { additions, deletions, path, old_path }`
- Checks git availability, handles errors

### Batch 2: Computation (depends on Batch 1)

**Contract 2A: `src/temporal/churn.rs`**
- Input: `&[CommitData]`, `&BTreeMap<String, CanonicalPath>` (rename map), `window_end: DateTime`
- Output: `BTreeMap<CanonicalPath, ChurnMetrics>`

**Contract 2B: `src/temporal/coupling.rs`**
- Input: `&[CommitData]`, `&BTreeMap<String, CanonicalPath>` (rename map), graph edges `&[Edge]`
- Output: `Vec<CoChange>` (sorted by confidence desc, capped at 10k)

**Contract 2C: `src/temporal/ownership.rs`**
- Input: `&[CommitData]`, `&BTreeMap<String, CanonicalPath>` (rename map)
- Output: `BTreeMap<CanonicalPath, OwnershipInfo>`

**Contract 2D: `src/temporal/hotspot.rs`**
- Input: `&BTreeMap<CanonicalPath, ChurnMetrics>`, `&ProjectGraph` (for LOC), blast_radius function
- Output: `Vec<Hotspot>` (sorted by score desc)

### Batch 3: Integration (depends on Batch 2)

**Contract 3A: `src/temporal/mod.rs`**
- `pub fn analyze(project_root: &Path, graph: &ProjectGraph, edges: &[Edge], collector: &DiagnosticCollector) -> Option<TemporalState>`
- Orchestrates git.rs → churn.rs, coupling.rs, ownership.rs, hotspot.rs

**Contract 3B: `src/mcp/state.rs` changes**
- Add `temporal: Option<TemporalState>` to `GraphState`
- Call `temporal::analyze()` in `from_loaded_data()`

**Contract 3C: `src/model/smell.rs` + `src/analysis/smells.rs`**
- Add `TemporalCouplingWithoutImport` variant
- Update `detect_smells` signature to accept `Option<&TemporalState>`

### Batch 4: MCP Tools (depends on Batch 3)

**Contract 4A: 5 new tools in `src/mcp/tools.rs` + `src/mcp/tools_temporal.rs`**
- `ariadne_churn`, `ariadne_coupling`, `ariadne_hotspots`, `ariadne_ownership`, `ariadne_hidden_deps`

**Contract 4B: 5 enhanced tools in `src/mcp/tools.rs`**
- `ariadne_file`, `ariadne_overview`, `ariadne_context`, `ariadne_importance`, `ariadne_smells`

### Batch 5: Tests (depends on Batch 4)

- L1 unit tests for git.rs, churn.rs, coupling.rs, hotspot.rs, ownership.rs
- L2 integration test: fixture git repo with known history
- L3 invariant tests (INV-T1 through INV-T5)
- L4 performance benchmarks
