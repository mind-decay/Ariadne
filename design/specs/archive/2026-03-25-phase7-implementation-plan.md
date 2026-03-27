# Phase 7: Git Temporal Analysis — Implementation Plan

**Spec:** `design/specs/2026-03-25-phase7-git-temporal-analysis.md`
**Date:** 2026-03-25

## Overview

8 implementation chunks. Chunks 3 and 4 can run in parallel. All others are sequential.

## Dependency Graph

```
Chunk 1 (Types + Warning Codes)
    │
    v
Chunk 2 (Git Parser)
    │
    ├──────────┐
    v          v
Chunk 3      Chunk 4
(Churn+Own)  (Coupling)
    │          │
    └────┬─────┘
         v
    Chunk 5 (Hotspot)
         │
         v
    Chunk 6 (Integration)
         │
         v
    Chunk 7 (MCP Tools)
         │
         v
    Chunk 8 (Tests)
```

Chunks 3 and 4 are independent and can execute in parallel. Chunk 5 depends on Chunk 3 (needs churn data for ranking) and implicitly on Chunk 4 being available for integration, but can begin implementation once Chunk 3 is done.

---

## Chunk 1: Types + Warning Codes

**Goal:** Establish all data types and warning codes that downstream chunks depend on.

**Files:**

| File | Action | Description |
|------|--------|-------------|
| `src/model/temporal.rs` | NEW | `TemporalState`, `ChurnMetrics`, `CoChange`, `OwnershipInfo`, `Hotspot` structs — all `#[derive(Debug, Clone, Serialize, Deserialize)]`, no logic |
| `src/model/mod.rs` | MODIFY | Add `pub mod temporal;` and re-export types |
| `src/model/smell.rs` | MODIFY | Add `TemporalCouplingWithoutImport` variant to `SmellType` enum, update `Display`, `severity()`, and any match arms |
| `src/diagnostic.rs` | MODIFY | Add `W024GitNotFound`, `W025NotGitRepository`, `W026ShallowRepository`, `W027GitCommandFailed`, `W028TemporalAnalysisFailed` — enum variants, `code()` match arms, `Display` match arms |
| `design/error-handling.md` | MODIFY | Document W024-W028 in the warning codes table |

**Design sources:** D-093, D-095, D-096, GAP-02, GAP-04, GAP-06, GAP-07

**What to implement:**

1. Create `src/model/temporal.rs` with the 5 structs from spec D1. All fields use `BTreeMap` (not `HashMap`) for determinism. `CanonicalPath` from `super::types`. `top_authors` and `top_contributors` are `Vec<(String, u32)>`.

2. In `src/model/mod.rs`, add `pub mod temporal;` and re-export: `pub use temporal::{TemporalState, ChurnMetrics, CoChange, OwnershipInfo, Hotspot};`

3. In `src/model/smell.rs`, add `TemporalCouplingWithoutImport` to the `SmellType` enum. Update `Display` to return `"temporal-coupling-without-import"`. Update `severity()` to return `Severity::Medium`. Update any other match arms that exist on `SmellType`.

4. In `src/diagnostic.rs`, add 5 new warning variants. Each needs:
   - Enum variant (e.g., `W024GitNotFound`)
   - `code()` returns `"W024"` etc.
   - `Display` returns the message from spec D7

5. In `design/error-handling.md`, add rows for W024-W028 in the warning codes table following existing format.

**Acceptance criteria:**
- `cargo build` succeeds with no errors or warnings
- Types are importable as `crate::model::{TemporalState, ChurnMetrics, CoChange, OwnershipInfo, Hotspot}`
- Warning codes have working `code()` and `Display` implementations
- `SmellType::TemporalCouplingWithoutImport` has correct display name and severity
- `cargo test` passes (existing tests unbroken)

---

## Chunk 2: Git Parser

**Goal:** Parse git log output into structured commit data. Establish the `temporal/` module.

**Files:**

| File | Action | Description |
|------|--------|-------------|
| `src/temporal/mod.rs` | NEW | Module stub: `pub mod git;` and sub-module declarations. No orchestration logic yet (added in Chunk 6). |
| `src/temporal/git.rs` | NEW | Git availability detection, command execution, streaming output parser, rename map construction |
| `src/lib.rs` | MODIFY | Add `pub mod temporal;` |

**Design sources:** D-092, D-097, GAP-01, GAP-03

**What to implement:**

1. Create `src/temporal/mod.rs` as a stub with `pub mod git;` (more sub-modules added in later chunks).

2. In `src/lib.rs`, add `pub mod temporal;` in the appropriate position.

3. In `src/temporal/git.rs`, implement:

   a. **Internal types** (not exported to `model/`):
      - `pub(crate) struct CommitData { pub hash: String, pub author: String, pub date: String, pub files: Vec<FileChange> }`
      - `pub(crate) struct FileChange { pub additions: u32, pub deletions: u32, pub path: String, pub old_path: Option<String> }`
      - `pub(crate) enum GitAvailability { Available, Shallow, Unavailable(String) }`

   b. **Git availability check** (`check_git(project_root: &Path, collector: &DiagnosticCollector) -> GitAvailability`):
      - Tier 1: `which git` (or `Command::new("git").arg("--version")`) — failure emits W024, returns `Unavailable`
      - Tier 2: `git rev-parse --git-dir` in project_root — failure emits W025, returns `Unavailable`
      - Tier 3: `git rev-parse --is-shallow-repository` — true emits W026, returns `Shallow`
      - Otherwise returns `Available`

   c. **Git log execution + streaming parse** (`parse_git_log(project_root: &Path, collector: &DiagnosticCollector) -> Option<(Vec<CommitData>, bool)>`):
      - Check git availability first. If `Unavailable`, return `None`.
      - Execute: `git log --numstat -M --format="commit %H%nauthor %aN%ndate %aI" --since="1 year ago"`
      - Parse output line-by-line:
        - Lines starting with `commit ` — new commit, extract hash
        - Lines starting with `author ` — extract author name
        - Lines starting with `date ` — extract ISO 8601 date
        - Blank lines — separator
        - Lines matching `\d+\t\d+\t.+` — numstat entry (additions, deletions, path)
        - Numstat lines with `{old => new}` or `old => new` in path — rename detected, set `old_path`
      - Return `(Vec<CommitData>, shallow: bool)` or `None` on failure (emit W027)

   d. **Rename map construction** (`build_rename_map(commits: &[CommitData], graph_paths: &HashSet<CanonicalPath>) -> BTreeMap<String, CanonicalPath>`):
      - Iterate all FileChange entries with `old_path.is_some()`
      - Map `old_path` to `CanonicalPath` of `path` if `path` exists in current graph
      - Follow rename chains (A->B, B->C means A maps to C)

4. **Unit tests** (in `#[cfg(test)] mod tests` within `git.rs`):
   - Parse well-formed git log output (captured from a real repo)
   - Parse output with renames (both `{old => new}` and `old => new` formats)
   - Parse output with binary files (`-\t-\tpath` format)
   - Handle malformed lines gracefully (skip, don't panic)
   - Empty output produces empty `Vec<CommitData>`
   - Rename map construction with chains

**Acceptance criteria:**
- `cargo build` succeeds
- Git log parsing unit tests pass for known mock output
- Handles no-git gracefully (returns `None`, emits appropriate warning)
- Rename map correctly maps old paths to canonical paths
- `cargo test` passes

---

## Chunk 3: Churn + Ownership (parallel with Chunk 4)

**Goal:** Compute per-file change frequency and ownership from parsed commit data.

**Files:**

| File | Action | Description |
|------|--------|-------------|
| `src/temporal/churn.rs` | NEW | Churn metrics computation |
| `src/temporal/ownership.rs` | NEW | Ownership info computation |
| `src/temporal/mod.rs` | MODIFY | Add `pub mod churn; pub mod ownership;` |

**Design sources:** D-093, GAP-04, GAP-06

**What to implement:**

1. In `src/temporal/churn.rs`:

   a. **Public function:** `pub(crate) fn compute_churn(commits: &[CommitData], rename_map: &BTreeMap<String, CanonicalPath>, graph_paths: &HashSet<CanonicalPath>, window_end: &str) -> BTreeMap<CanonicalPath, ChurnMetrics>`

   b. **Algorithm:**
      - Parse `window_end` as date. Compute boundaries: 30d, 90d, 1y before `window_end`.
      - For each commit, parse its date. For each file in the commit:
        - Map path through rename_map (if old path), or convert to CanonicalPath directly
        - Skip if path not in `graph_paths` (file no longer exists in project)
        - Increment appropriate window counters (30d, 90d, 1y) based on commit date
        - Add lines changed (additions + deletions) to appropriate window
        - Track author in per-file author sets for 30d window
        - Update `last_changed` if this commit is more recent
        - Track author commit counts for `top_authors` (top 3)

   c. **Unit tests:**
      - Known commit data with dates spanning 30d/90d/1y boundaries
      - Verify `commits_30d <= commits_90d <= commits_1y` invariant
      - Verify `last_changed` is the most recent date
      - Verify `top_authors` sorted descending, capped at 3
      - Files not in graph_paths are excluded

2. In `src/temporal/ownership.rs`:

   a. **Public function:** `pub(crate) fn compute_ownership(commits: &[CommitData], rename_map: &BTreeMap<String, CanonicalPath>, graph_paths: &HashSet<CanonicalPath>) -> BTreeMap<CanonicalPath, OwnershipInfo>`

   b. **Algorithm:**
      - Commits should be in chronological order (newest first, as git log outputs)
      - For each file across all commits:
        - `last_author`: author of the most recent commit touching this file
        - `top_contributors`: count commits per author, sort descending, keep top 5
        - `author_count`: distinct authors

   c. **Unit tests:**
      - Known commit/author data, verify last_author is correct
      - Verify top_contributors sorted descending, capped at 5
      - Verify author_count matches distinct count

3. Update `src/temporal/mod.rs` to add `pub mod churn; pub mod ownership;`

**Acceptance criteria:**
- Unit tests pass for known commit data
- Churn metrics respect window boundaries
- Ownership correctly identifies last author and top contributors
- `cargo test` passes

---

## Chunk 4: Coupling (parallel with Chunk 3)

**Goal:** Compute co-change pairs with Jaccard confidence scoring.

**Files:**

| File | Action | Description |
|------|--------|-------------|
| `src/temporal/coupling.rs` | NEW | Co-change analysis with Jaccard index |
| `src/temporal/mod.rs` | MODIFY | Add `pub mod coupling;` |

**Design sources:** D-094, GAP-09

**What to implement:**

1. In `src/temporal/coupling.rs`:

   a. **Public function:** `pub(crate) fn compute_coupling(commits: &[CommitData], rename_map: &BTreeMap<String, CanonicalPath>, graph_paths: &HashSet<CanonicalPath>, edges: &[(CanonicalPath, CanonicalPath)], collector: &DiagnosticCollector) -> Vec<CoChange>`

   b. **Algorithm:**
      - Track per-file commit counts: `BTreeMap<CanonicalPath, u32>`
      - Track per-pair co-occurrence counts: `BTreeMap<(CanonicalPath, CanonicalPath), u32>` (always store with smaller path first for canonical ordering)
      - Track skipped commit count for W028 check
      - For each commit:
        - Map file paths through rename_map, filter to graph_paths
        - If commit touches > 100 files, skip it (increment skipped count), continue
        - Increment per-file commit counts
        - For each unique pair in the commit's file set, increment co-occurrence count
      - If `skipped > total_commits / 10`, emit W028
      - For each pair with `co_change_count >= 3`:
        - Compute Jaccard: `co_changes / (changes_a + changes_b - co_changes)`
        - Guard: if denominator is 0, skip (shouldn't happen with count >= 3)
        - Round confidence to 4 decimal places (D-049 determinism)
        - Check if pair has a structural link in `edges`
      - Sort by confidence descending
      - Keep top 10,000 pairs

   c. **Unit tests:**
      - Simple case: 2 files always change together across 5 commits. Jaccard = 1.0.
      - Partial overlap: file A in 10 commits, file B in 10 commits, together in 5. Jaccard = 5/(10+10-5) = 0.333...
      - Below threshold: pair with 2 co-changes filtered out
      - Bulk commit skip: commit touching 101 files excluded
      - Cap enforcement: generate >10k pairs, verify only top 10k kept
      - Structural link detection
      - W028 emission when >10% commits skipped

2. Update `src/temporal/mod.rs` to add `pub mod coupling;`

**Acceptance criteria:**
- Jaccard computation correct for known data
- Guards enforced (min 3, max 10k, skip >100-file commits)
- Confidence values rounded to 4 decimal places
- W028 emitted when appropriate
- All unit tests pass
- `cargo test` passes

---

## Chunk 5: Hotspot

**Goal:** Combine churn, LOC, and blast radius into risk scores.

**Files:**

| File | Action | Description |
|------|--------|-------------|
| `src/temporal/hotspot.rs` | NEW | Hotspot scoring computation |
| `src/temporal/mod.rs` | MODIFY | Add `pub mod hotspot;` |

**Design sources:** D-095, GAP-07

**What to implement:**

1. In `src/temporal/hotspot.rs`:

   a. **Public function:** `pub(crate) fn compute_hotspots(churn: &BTreeMap<CanonicalPath, ChurnMetrics>, graph: &ProjectGraph, blast_radius_fn: impl Fn(&CanonicalPath) -> usize) -> Vec<Hotspot>`

   b. **Algorithm:**
      - Collect all files that have churn data
      - For each file, get: `commits_1y` (churn), `Node.lines` (LOC from graph), `blast_radius_fn(path)` (blast radius)
      - Rank files by each metric independently (1 = highest value). Ties get the same rank.
      - Normalize each rank to [0, 1]: `normalized = 1.0 - (rank - 1) as f64 / total as f64`
        - Rank 1 (highest churn/LOC/blast) normalizes to ~1.0
        - Last rank normalizes to ~0.0
      - Combined score: `normalized_churn * normalized_loc * normalized_blast_radius`
      - Round score to 4 decimal places (D-049 determinism)
      - Sort by score descending

   c. **Edge cases:**
      - File in churn but not in graph (deleted between analysis window and now) — skip
      - File with 0 lines — gets lowest LOC rank
      - Single file — score is 1.0

   d. **Unit tests:**
      - 3 files with known churn, LOC, blast radius — verify rankings and scores
      - Verify all scores in [0.0, 1.0]
      - Verify sorting is descending
      - Single file case

2. Update `src/temporal/mod.rs` to add `pub mod hotspot;`

**Acceptance criteria:**
- Rankings correct for known inputs
- All scores in [0.0, 1.0]
- Scores rounded to 4 decimal places
- Sort order is descending by score
- Unit tests pass
- `cargo test` passes

---

## Chunk 6: Integration

**Goal:** Wire temporal computation into the main pipeline. Add temporal coupling smell. Update decision log.

**Files:**

| File | Action | Description |
|------|--------|-------------|
| `src/temporal/mod.rs` | MODIFY | Complete with `pub fn analyze()` orchestration function |
| `src/mcp/state.rs` | MODIFY | Add `temporal: Option<TemporalState>` to `GraphState`, call `temporal::analyze()` in `from_loaded_data()` |
| `src/analysis/smells.rs` | MODIFY | Update `detect_smells` to accept `Option<&TemporalState>`, add `TemporalCouplingWithoutImport` detection |
| `design/decisions/log.md` | MODIFY | Add D-091 through D-097 |

**Design sources:** D-091, D-093, D-096, GAP-01, GAP-08

**What to implement:**

1. In `src/temporal/mod.rs`, implement the public `analyze()` function:

   ```rust
   pub fn analyze(
       project_root: &Path,
       graph: &ProjectGraph,
       edges: &[Edge],
       collector: &DiagnosticCollector,
   ) -> Option<TemporalState>
   ```

   - Call `git::parse_git_log()` — if `None`, return `None`
   - Extract `(commits, shallow)` from result
   - Build `graph_paths: HashSet<CanonicalPath>` from `graph.nodes`
   - Build rename map via `git::build_rename_map()`
   - Determine `window_end` (most recent commit date, or current date)
   - Compute churn: `churn::compute_churn()`
   - Compute ownership: `ownership::compute_ownership()`
   - Compute coupling: `coupling::compute_coupling()` — need to convert `edges` to `(CanonicalPath, CanonicalPath)` tuples
   - Compute hotspots: `hotspot::compute_hotspots()` — need blast radius function from `algo::blast_radius`
   - Assemble `TemporalState` with all results, `shallow` flag, `commits_analyzed` count, `window_start`/`window_end`
   - Return `Some(temporal_state)`

2. In `src/mcp/state.rs`:
   - Add `pub temporal: Option<TemporalState>` field to `GraphState`
   - In `from_loaded_data()` (or equivalent constructor), call `temporal::analyze()` after existing index/computation setup
   - Pass `project_root`, `graph`, `edges`, and `collector` to `analyze()`
   - Store result in `self.temporal`

3. In `src/analysis/smells.rs`:
   - Update `detect_smells` function signature to accept `Option<&TemporalState>` as an additional parameter
   - When `temporal.is_some()`, check for `TemporalCouplingWithoutImport`:
     - For each `CoChange` with `confidence >= 0.5` AND `has_structural_link == false`:
       - Emit smell with `SmellType::TemporalCouplingWithoutImport`, severity MEDIUM
       - Include both files in the smell's affected paths
   - Update ALL callers of `detect_smells` to pass the temporal parameter (search for call sites)

4. In `design/decisions/log.md`:
   - Add entries D-091 through D-097 following existing format
   - Content from spec section "New Decision Log Entries"

**Acceptance criteria:**
- `temporal::analyze()` is callable and returns `Option<TemporalState>`
- `GraphState` has `temporal` field populated at load time
- Smell detection includes `TemporalCouplingWithoutImport` when temporal data is available
- Smell detection works unchanged when temporal data is `None`
- All callers of `detect_smells` updated
- `cargo test` passes (all existing tests still work)

---

## Chunk 7: MCP Tools

**Goal:** Add 5 new temporal MCP tools and enhance 5 existing tools with temporal data.

**Files:**

| File | Action | Description |
|------|--------|-------------|
| `src/mcp/tools_temporal.rs` | NEW | Parameter types: `ChurnParam`, `CouplingParam`, `HotspotsParam`, `OwnershipParam`, `HiddenDepsParam` |
| `src/mcp/tools.rs` | MODIFY | Add 5 new tool handlers, enhance 5 existing tool handlers |
| `src/mcp/mod.rs` | MODIFY | Add `pub mod tools_temporal;` if needed for module visibility |
| `src/algo/context.rs` | MODIFY | Add churn boost factor to relevance scoring |

**Design sources:** D9, D10 from spec, GAP-05, GAP-11

**What to implement:**

1. Create `src/mcp/tools_temporal.rs` with parameter structs:

   ```rust
   #[derive(Deserialize)]
   pub struct ChurnParam {
       pub period: Option<String>,  // "30d" | "90d" | "1y", default "30d"
       pub top: Option<u32>,        // default 20
   }

   #[derive(Deserialize)]
   pub struct CouplingParam {
       pub min_confidence: Option<f64>,  // default 0.3
   }

   #[derive(Deserialize)]
   pub struct HotspotsParam {
       pub top: Option<u32>,  // default 20
   }

   #[derive(Deserialize)]
   pub struct OwnershipParam {
       pub path: Option<String>,  // None = project-wide
   }

   #[derive(Deserialize)]
   pub struct HiddenDepsParam {}
   ```

2. In `src/mcp/tools.rs`, add 5 new tool handlers. Each must:
   - Check `state.temporal.is_some()` — if `None`, return `{"error": "temporal_unavailable", "reason": "Git temporal analysis was not available at startup. Ensure the project is a git repository with git installed."}`
   - Register in the tool list/dispatch table with name and description
   - **`ariadne_churn`:** Sort churn by commit count for the requested period. Return top N entries.
   - **`ariadne_coupling`:** Filter `co_changes` by `min_confidence`. Return sorted list.
   - **`ariadne_hotspots`:** Return top N hotspots from pre-computed list.
   - **`ariadne_ownership`:** If `path` provided, return ownership for that file. Otherwise, return project-wide top contributors across all files.
   - **`ariadne_hidden_deps`:** Filter `co_changes` where `has_structural_link == false`. Return sorted by confidence.

3. In `src/mcp/tools.rs`, enhance 5 existing tool handlers:
   - **`ariadne_file`:** Add `temporal` field to JSON response when `state.temporal` is `Some` and the file has churn data. Fields: `commits_30d`, `commits_90d`, `last_changed`, `top_authors`.
   - **`ariadne_overview`:** Add `temporal` summary when available: `total_commits_30d` (sum across files), `hotspot_count` (len of hotspots), `hidden_dep_count` (co-changes without structural link).
   - **`ariadne_context`:** Delegate to `algo/context.rs` change (see step 4).
   - **`ariadne_importance`:** When temporal available, apply enhanced formula: `structural_importance * (1.0 + ln_1p(churn_30d) / max_log_churn)`. When unavailable, no change.
   - **`ariadne_smells`:** No code change needed here — the smell detection was already updated in Chunk 6 to include `TemporalCouplingWithoutImport` when temporal data is available.

4. In `src/algo/context.rs`:
   - If temporal churn data is available (passed as `Option<&BTreeMap<CanonicalPath, ChurnMetrics>>`), add a churn boost to relevance scoring
   - High-churn files (above median churn) get a small relevance boost (e.g., multiply relevance by `1.0 + 0.2 * normalized_churn`)
   - When temporal is `None`, scoring unchanged

**Acceptance criteria:**
- All 5 new tools return correct data from pre-computed `TemporalState`
- All 5 enhanced tools include temporal data when available
- All tools return graceful error when temporal is `None`
- `ariadne_churn` respects period and top parameters
- `ariadne_coupling` filters by min_confidence
- `ariadne_importance` formula is numerically stable (no NaN/Inf)
- `cargo test` passes

---

## Chunk 8: Tests

**Goal:** Comprehensive integration and invariant tests. Full regression verification.

**Files:**

| File | Action | Description |
|------|--------|-------------|
| `tests/temporal_integration.rs` | NEW | Integration tests with fixture git repository |

**Design sources:** Spec "Testing Requirements" section

**What to implement:**

1. Create `tests/temporal_integration.rs` with the following test categories:

   a. **Fixture setup helper:** Function that creates a temporary directory, initializes a git repo, and creates a scripted commit history (10-20 commits with known files, authors, dates). Use `std::process::Command` to run git commands.

   b. **Integration test: full analysis pipeline:**
      - Create fixture repo
      - Build a minimal `ProjectGraph` with known nodes (files with known `lines` values)
      - Build edge list
      - Run `temporal::analyze()`
      - Assert `TemporalState` is `Some`
      - Verify `commits_analyzed` matches expected count
      - Verify specific files have expected churn metrics
      - Verify expected co-change pairs exist with correct confidence ranges

   c. **Invariant tests:**
      - INV-T1: All `co_changes` have `confidence` in [0.0, 1.0]
      - INV-T2: All `co_changes` have `co_change_count >= 3`
      - INV-T3: All `hotspots` have `score` in [0.0, 1.0]
      - INV-T4: For all churn entries: `commits_30d <= commits_90d <= commits_1y`

   d. **Determinism test (INV-T5):**
      - Run `temporal::analyze()` twice on the same fixture repo
      - Serialize both results to JSON
      - Assert byte-for-byte equality

   e. **Graceful degradation tests:**
      - Run `temporal::analyze()` in a non-git directory (temp dir without `git init`)
      - Assert result is `None`
      - Assert `DiagnosticCollector` contains W025 warning

   f. **Rename tracking test:**
      - Create fixture where a file is renamed across commits
      - Verify churn is attributed to the current (renamed) path, not the old path

   g. **Empty repository test:**
      - `git init` with no commits
      - Verify `analyze()` handles gracefully (returns `Some` with empty collections or `None` with appropriate warning)

2. Run `cargo test` to verify full regression — all existing tests plus new temporal tests pass.

**Acceptance criteria:**
- All integration tests pass
- All invariant tests pass (INV-T1 through INV-T5)
- Graceful degradation test confirms W025 warning on non-git directory
- Rename tracking test confirms attribution to current path
- Full `cargo test` passes with zero failures
- No existing tests broken

---

## Summary

| Chunk | Name | Files | Dependencies | Parallelizable |
|-------|------|-------|-------------|----------------|
| 1 | Types + Warning Codes | 5 (1 new, 4 modify) | None | No |
| 2 | Git Parser | 3 (2 new, 1 modify) | Chunk 1 | No |
| 3 | Churn + Ownership | 3 (2 new, 1 modify) | Chunk 2 | Yes (with Chunk 4) |
| 4 | Coupling | 2 (1 new, 1 modify) | Chunk 2 | Yes (with Chunk 3) |
| 5 | Hotspot | 2 (1 new, 1 modify) | Chunk 3 | No |
| 6 | Integration | 4 (0 new, 4 modify) | Chunks 3, 4, 5 | No |
| 7 | MCP Tools | 4 (1 new, 3 modify) | Chunk 6 | No |
| 8 | Tests | 1 (1 new) | Chunk 7 | No |

**Total new files:** 8
**Total modified files:** 12 (some modified in multiple chunks)
**Estimated total lines written:** ~2,800
