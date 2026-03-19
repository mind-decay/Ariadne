# Architecture Review

**Date:** 2026-03-19
**Focus:** Full System
**Mode:** Post-implementation (Phase 1a + 1b + 2a + 2b complete)
**Reviewed:** architecture.md, ROADMAP.md, decisions/log.md (D-001–D-049), error-handling.md, performance.md, testing.md, path-resolution.md, determinism.md, distribution.md, all src/ modules, all tests/

## Executive Summary

The architecture remains sound through Phase 2b. Module boundaries are clean, dependency directions are mostly correct, and the Phase 2a algorithms (SCC, blast radius, centrality, topo sort, subgraph) are well-implemented and well-tested. The top concerns are: (1) `ariadne update` does not implement incremental re-parsing — it always does a full rebuild, contradicting architecture.md §6 and the ROADMAP; (2) Louvain's `converged` flag is hardcoded to `true`, making W012 dead code; (3) `algo/stats.rs` imports from `serial/`, violating D-033's dependency rules; (4) `update()` emits W010/W011 via bare `eprintln!`, bypassing DiagnosticCollector. Most previous review findings (F1–F3, S1–S3, U1, U4, U7) are resolved. Three carry-overs remain (S5, U2, U3).

## Key Themes

### Theme 1: `ariadne update` Is Not Incremental

All 4 agents independently flagged this. architecture.md §Algorithms §6 describes a 4-phase delta algorithm: detect changes → re-parse only affected files → remove stale data → recompute derived data (with a 5% threshold). The `algo/delta.rs` implementation correctly detects changes (Phase 1 of the design), but `pipeline/mod.rs:434-437` discards the delta and does a full rebuild for any non-zero changes. The code comment acknowledges this ("correctness over optimization") but no design document records this scoping decision. The ROADMAP still describes Phase 2b as delivering "incremental via content hash, 5% threshold for full recompute."

**Impact:** High — the primary design document describes behavior that does not exist. `ariadne update` provides no performance advantage over `ariadne build` except when zero files changed.

### Theme 2: Louvain Convergence Signal Is Dead

`louvain.rs:113` sets `converged = true` and never mutates it. If the outer loop exhausts `MAX_OUTER_ITERATIONS` without converging, the partially-iterated result is silently returned as converged. The W012 emission in `pipeline/mod.rs:220-226` is unreachable. This means the error-handling design for Louvain non-convergence (documented in error-handling.md and D-034) is not functional.

**Impact:** Medium — W012 is advisory. The result is still used. But the design's fallback guarantee ("fall back to directory clusters on non-convergence") cannot trigger.

### Theme 3: Dependency Rule Violations in Phase 2 Code

`algo/stats.rs:6` imports `StatsOutput` and `StatsSummary` from `crate::serial`. D-033 states `algo/` depends on `model/` only, never on `serial/`. Additionally, `views/mod.rs:8` imports `FatalError` from `crate::diagnostic`, which is not listed as a permitted dependency for `views/`. Both are small violations that don't create cycles, but they erode the documented module boundaries.

**Impact:** Medium — if unchecked, dependency creep between modules accelerates. The `algo/ → serial/` dependency is the more concerning one because it couples algorithm computation to the serialization format.

### Theme 4: Diagnostic System Bypass in `update()`

`pipeline/mod.rs:347-348` and `365-367` emit W010/W011 via bare `eprintln!` instead of `DiagnosticCollector`. These warnings don't appear in `--warnings json` output, don't increment `DiagnosticCounts`, and aren't subject to `--strict`. This is the same class of issue as the previously-resolved S1 (walk errors), now present in the update path.

**Impact:** Medium — machine consumers (CI, JSON warning consumers) get incomplete diagnostic information from the update command.

## Previous Review Status

| ID | Finding | Status |
| -- | ------- | ------ |
| F1 | `project_root` absolute path | RESOLVED — now uses CLI argument as-is |
| F2 | ImportResolver signature divergence | RESOLVED — architecture.md updated |
| F3 | `find_case_insensitive` never called | RESOLVED — wired into pipeline/resolve.rs |
| S1 | Walk errors bypass DiagnosticCollector | RESOLVED — warnings forwarded via pipeline |
| S2 | Max-files silent truncation | RESOLVED — warning emitted (reuses W003, see N1) |
| S3 | `.ariadne/` exclusion fallback missing | RESOLVED — manual exclusion implemented |
| S4 | `detect/workspace.rs` depends on `diagnostic.rs` | ACCEPTED — architecture.md updated to permit it |
| S5 | `project_graph_to_output` in wrong module | NOT RESOLVED — still free function in pipeline/ |
| S6 | HashMap in ParserRegistry | ACCEPTED — documented exception in determinism.md |
| U1 | W007 PartialParse never emitted | RESOLVED — emitted on ParseOutcome::Partial |
| U2 | Parser construction asymmetry | NOT RESOLVED — cosmetic, low priority |
| U3 | ImportKind::ModDeclaration unused downstream | NOT RESOLVED — dead weight from pipeline perspective |
| U4 | arch_depth: 0 always emitted | RESOLVED — computed via topo sort in Phase 2a |
| U5 | architecture.md layer table outdated | RESOLVED — table updated |
| U6 | DiagnosticCounts design/code divergence | REGRESSED — see N5 |
| U7 | Cohesion rounding not enforced at serialization | RESOLVED — rounded at construction time |

## Detailed Findings

### Foundational Issues

**N1. `algo/stats.rs` imports `serial/` — D-033 violation (HIGH confidence, NEW) — FIXED**
`StatsOutput` and `StatsSummary` moved to `model/stats.rs`. `serial/mod.rs` re-exports for backwards compatibility. `algo/stats.rs` now imports from `model/`.

**N2. `ariadne update` is not incremental (HIGH confidence, NEW) — FIXED (documented)**
D-050 added to decision log. architecture.md §6 updated to describe actual behavior (full rebuild with no-op fast path). ROADMAP Phase 2b updated.

**N3. Louvain `converged` always `true` (HIGH confidence, NEW) — FIXED**
`converged` now initialized to `false`, set to `true` only on convergence break. W012 path is now reachable.

### Structural Issues

**N4. `update()` W010/W011 bypass DiagnosticCollector (HIGH confidence, NEW) — FIXED**
W010/W011 now routed through `DiagnosticCollector` in `update()`.

**N5. `DiagnosticCounts` has 3 undocumented fields (HIGH confidence, REGRESSION of U6) — FIXED**
error-handling.md updated to include all 11 fields.

**N6. `FatalError::E008 GraphCorrupted` not in design (HIGH confidence, NEW) — FIXED**
E008 (`GraphCorrupted`) and E009 (`FileNotInGraph`) added to error-handling.md taxonomy and FatalError enum listing.

**N7. `round4` duplicated 3 times — violates D-049 (HIGH confidence, NEW) — FIXED**
`round4` moved to `algo/mod.rs` as `pub fn`. All 3 copies removed and replaced with imports.

**N8. `views/` imports `diagnostic.rs` — unlisted dependency (MEDIUM confidence, NEW) — FIXED**
architecture.md dependency table updated: `views/` now lists `diagnostic.rs` (for `FatalError`).

**N9. S5 persists: `project_graph_to_output` not `From` impl (HIGH confidence, PREVIOUSLY FLAGGED) — FIXED**
architecture.md updated to describe actual pattern: free function `project_graph_to_output(graph, project_root)` with rationale.

### Surface Issues

**N10. `W003FileTooLarge` reused for file-count limit (MEDIUM confidence, NEW) — FIXED**
`W005MaxFilesReached` added to warning taxonomy. `walk.rs` now uses W005 for file-count limit.

**N11. `query file` uses wrong `FatalError` variant (MEDIUM confidence, NEW) — FIXED**
Added `FatalError::FileNotInGraph` (E009). `query file` now uses it.

**N12. `stats.json` layers sort lexicographically, not numerically (MEDIUM confidence, NEW) — FIXED**
Layer keys now zero-padded (`format!("{:05}", layer)`) for correct numeric ordering in BTreeMap.

**N13. `W013StaleStats` never emitted (HIGH confidence, NEW)**
`WarningCode::W013StaleStats` is defined with a counter but no code path emits it. The design specifies: "stats.json modification time older than graph.json → recompute stats, emit warning."
Direction: Implement mtime comparison in query commands, or remove the warning code if the design intent changed.

**N14. L0 index format differs from architecture.md (MEDIUM confidence, NEW) — FIXED**
architecture.md L0 example updated to match actual `generate_index` output format.

**N15. `views/cluster.rs` uses O(N×M) membership test (MEDIUM confidence, NEW) — FIXED**
`cluster_files` changed from `Vec<&str>` to `BTreeSet<&str>` for O(log n) membership tests.

**N16. `_is_re_export` dead parameter in `classify_edge_type` (LOW confidence, NEW) — FIXED**
Dead parameter removed from `classify_edge_type`.

**N17. `StatsOutput.version` not validated on read (MEDIUM confidence, NEW) — FIXED**
Version check added in `serial/json.rs` `read_stats()`. Unsupported version returns `GraphCorrupted`.

**N18. Views use non-atomic writes (LOW confidence, NEW)**
`views/mod.rs:29,42` uses `fs::write()` directly. `serial/json.rs` uses atomic writes (tmp + rename).
Direction: Consider atomic writes for views, or document the deliberate difference.

**N19. Louvain W012 empty path (LOW confidence, NEW) — FIXED**
W012 now uses `CanonicalPath::new("<louvain>")` as sentinel path.

**N20. `serde_json::to_string_pretty` unwraps in `main.rs` (LOW confidence, NEW) — FIXED**
All `.unwrap()` calls replaced with `json_pretty()` helper that maps errors to `FatalError::OutputNotWritable`.
Direction: Map to `FatalError` variant for clean error output.

## Discussion Points

### DP1: Should `ariadne update` document its full-rebuild behavior?

**Tension:** The ROADMAP promises incremental re-parsing. The code does full rebuild. This is a pragmatic choice ("algorithms are fast; correctness over optimization") but contradicts the primary design document.
**Options:** (a) Add a decision entry documenting the deferral, update ROADMAP to note Phase 3 scope. (b) Implement true incremental re-parse now. (c) Remove `ariadne update` and just use `ariadne build` until Phase 3.
**Recommendation:** Option (a) — document the deferral. The delta detection scaffolding is correctly positioned for Phase 3's auto-update. Implementing true incrementality now is premature without the MCP server's in-memory graph to benefit from it.

### DP2: Should `StatsOutput`/`StatsSummary` move to `model/`?

**Tension:** `algo/stats.rs` needs to produce these types. The dependency rule says algo/ depends on model/ only. Moving them to model/ fixes the violation but puts serialization-oriented types in the model layer.
**Options:** (a) Move to model/ — they are pure data, not serialization logic. (b) Have algo/ return a raw intermediate type, convert in pipeline/. (c) Update D-033 to allow algo/ → serial/ (output types only).
**Recommendation:** Option (a) — `StatsOutput` is structurally identical to other model types (BTreeMaps of strings). The "output" suffix is misleading; it's just a data container.

### DP3: Should the layer numbering semantics be explicitly documented?

**Tension:** Layer 0 = leaves (no outgoing deps). Higher layers = more dependencies. This is internally consistent but potentially counterintuitive ("Layer 0 = foundations, not top-level").
**Options:** (a) Document explicitly in architecture.md. (b) Invert the numbering (max depth - current = display layer). (c) Keep as-is, let users discover the convention.
**Recommendation:** Option (a) — add a one-line note: "Layer 0 = leaf files with no outgoing architectural dependencies. Higher layers import from lower layers."

## Strengths

- **Phase 2a algorithms are clean and well-tested.** Tarjan SCC uses iterative DFS (avoids stack overflow on deep graphs). Brandes centrality handles disconnected graphs correctly. Topo sort contracts SCCs before layering. All edge cases (empty graph, single node, disconnected) are handled.
- **Determinism is maintained through Phase 2.** BTreeMap ordering throughout. `round4` applied to all float outputs (though duplicated). Louvain uses deterministic iteration via BTreeMap. No non-determinism threats found in any algorithm.
- **Previous review findings largely resolved.** 11 of 15 findings are resolved or accepted. The resolved items (F1-F3, S1-S3, U1, U4, U7) represent genuine improvements to the codebase.
- **`algo/mod.rs` shared utilities are well-designed.** `build_adjacency` and `is_architectural` provide a clean foundation for all algorithms. Edge filtering policy (exclude tests from structural algorithms) is correctly centralized.
- **Subgraph extraction with cluster expansion** (D-035, 100-file cap) is a thoughtful design that balances context richness with output size.
- **Delta detection in `algo/delta.rs`** is correctly implemented as a pure function with thorough tests (9 unit tests). Good scaffolding for Phase 3.
- **`--no-louvain` escape hatch** is the right product decision. Running Louvain on every build is justified given its O(n log n) complexity.

## Recommendations

### Quick Wins (doc updates only)

1. ~~**Update error-handling.md**~~ — DONE (DiagnosticCounts 11 fields, E008/E009, W005, FatalError enum)
2. ~~**Update architecture.md**~~ — DONE (From→free function, layer convention, L0 format, dependency table, model/stats.rs)
3. ~~**Update ROADMAP**~~ — DONE (Phase 2b delta description)
4. ~~**Add decision D-050**~~ — DONE (ariadne update full-rebuild-always)

### Targeted Improvements (localized code changes)

1. ~~**Fix Louvain convergence flag**~~ — DONE
2. ~~**Move `round4` to `algo/mod.rs`**~~ — DONE
3. ~~**Fix `algo/stats.rs` dependency**~~ — DONE (moved `StatsOutput`/`StatsSummary` to `model/`)
4. ~~**Route W010/W011 through DiagnosticCollector**~~ — DONE
5. ~~**Add W005MaxFilesReached**~~ — DONE
6. ~~**Fix `query file` error variant**~~ — DONE (added E009 `FileNotInGraph`)
7. ~~**Replace `Vec<&str>` with `BTreeSet<&str>`**~~ in `views/cluster.rs` — DONE
8. ~~**Remove `_is_re_export` dead parameter**~~ — DONE
9. ~~**Remove unused `_graph` parameter**~~ from `generate_subgraph_view` — DONE
10. ~~**Add version check for `StatsOutput`**~~ — DONE
11. ~~**Zero-pad layer keys**~~ in `stats.json` for correct numeric ordering — DONE
12. ~~**Replace `.unwrap()` on JSON serialization**~~ in `main.rs` with `json_pretty()` — DONE

### Strategic Considerations (bigger changes for Phase 3)

1. **Consider pre-building deduplicated adjacency once** — currently `build_adjacency` is called per algorithm; at Phase 3 (in-memory server), caching this structure saves repeated BTreeSet allocation
2. **Consider views receiving `ClusterMap`** — `generate_cluster_view` could avoid O(N) full-graph scan per cluster if it received pre-grouped file lists
3. **Consider implementing true incremental re-parse** when Phase 3 MCP server needs fast auto-updates on file changes
