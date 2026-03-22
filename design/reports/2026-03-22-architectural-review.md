# Architectural Review Report

**Date:** 2026-03-22
**Project:** Ariadne — Structural Dependency Graph Engine
**Methodology:** Risk-weighted architectural review with 3-phase analysis (Structure+Performance, Maintainability+Reliability, Extensibility+Design Conformance)
**Evidence standard:** All findings require exact file:line citations verified by independent review

---

## 1. Executive Summary

Ariadne's architecture is fundamentally sound. The trait-based design (LanguageParser, ImportResolver, GraphSerializer, GraphReader) provides clean separation of concerns, the module dependency rules (D-033) are respected, and all seven roadmap phases (1a through 3c) have been implemented. The codebase demonstrates strong design discipline: decision log entries (D-001 through D-054+) accurately document deviations, and the Composition Root pattern (D-020) keeps wiring isolated in main.rs.

The top three risks are: **(1)** The graph's edge storage as a flat `Vec<Edge>` without a persistent adjacency index forces O(E) rebuilds for every algorithm invocation -- this is the single highest-impact structural and performance issue, with 4+ redundant adjacency constructions per build. **(2)** Critical code paths lack unit-level test coverage: 5 of 6 parsers (~2,500 lines), the entire views module (~426 lines), and MCP tool functions (709 lines) have zero inline tests, creating a significant regression risk as the project matures. **(3)** Design documentation has drifted from implementation in ROADMAP.md and architecture.md, though the decision log remains accurate -- this creates a reading-order problem for new contributors.

Recommended priorities: First, compute the adjacency index once and share across all algorithms (addresses the root cause of 5 findings). Second, add unit tests for parsers, views, and MCP tools. Third, update ROADMAP.md with implementation status annotations and add extension point documentation.

---

## 2. Methodology

This review employed a risk-weighted, evidence-based approach across three phases:

- **Phase 1 (Structure + Performance):** Analyzed 16 source files for structural patterns and performance characteristics. Produced 10 findings (5 structural, 5 performance).
- **Phase 2 (Maintainability + Reliability):** Analyzed 20 source files including parsers, diagnostics, MCP, views, and tests. Produced 11 findings (6 maintainability, 5 reliability).
- **Phase 3 (Extensibility + Design Conformance):** Analyzed 8 source files and 2 design documents (63 decisions reviewed). Produced 8 findings (4 extensibility, 4 design conformance).

Each phase included independent review (Themis) that verified evidence at the file:line level and calibrated severity. Two severity adjustments were applied during review:

- W-RELY-003: Medium to Low (speculative impact, intentional behavior with tests)
- W-RELY-005: Low to Medium (misleading comment, significant CPU waste on failure-path systems)

**Finding ID scheme:** `W-{DIMENSION}-{NNN}` where dimension is STRUCT, PERF, MAINT, RELY, EXT, or DESIGN.

**Severity levels:** Critical (system-breaking), High (significant impact at target scale), Medium (moderate impact or scaling concern), Low (minor friction or future concern), Informational (documented design evolution, no action needed).

---

## 3. Findings by Dimension

### 3.1 W-STRUCT: Structural Findings

#### W-STRUCT-001: ProjectGraph.edges is Vec with no adjacency index

- **Severity:** High
- **Source:** `src/model/graph.rs:11`
- **Evidence:** `pub edges: Vec<Edge>` is the sole edge storage. Every algorithm calls `build_adjacency()` (`src/algo/mod.rs:32-56`) which performs a full O(E) scan, builds BTreeSet-backed forward and reverse maps, then converts to sorted Vecs.
- **Impact:** Every graph query requires a full edge scan. The adjacency index is rebuilt from scratch for each algorithm invocation rather than being stored as a persistent index. At 3,000 files with ~9,000 edges, each `build_adjacency` call scans all edges and builds two BTreeMaps. This is the root cause of W-PERF-001.
- **Recommendation:** Pre-compute the adjacency index once after graph construction and pass it to all algorithms. This eliminates 3+ redundant O(E) scans per build.

#### W-STRUCT-002: find_child_by_kind duplicated across 3 parser files

- **Severity:** Low
- **Source:** `src/parser/typescript.rs:29`, `src/parser/python.rs:32`, `src/parser/rust_lang.rs:14`
- **Evidence:** Each parser defines an identical `find_child_by_kind` method. Used at 42+ call sites across the three files.
- **Impact:** Maintenance burden only -- changes must be applied in 3 places. No runtime cost.
- **Recommendation:** Extract to a shared `parser::helpers` module. Cross-ref: W-MAINT-003.

#### W-STRUCT-003: detect_god_files scans full edge list per node

- **Severity:** Medium
- **Source:** `src/analysis/smells.rs:40-44`
- **Evidence:** For each node in the graph, the full edge list is scanned to compute out-degree. This is O(V\*E) in theory. However, an early-continue at line 36 (`if centrality <= 0.8 { continue; }`) means only nodes with very high centrality (typically <5) proceed to the edge scan.
- **Impact:** Effective complexity is O(k\*E) where k is very small. Only problematic if many nodes have centrality >0.8 (unusual). Theoretical concern at 50,000+ files.
- **Recommendation:** Pre-compute a degree map to eliminate the inner scan. Would benefit all smell detection functions.

#### W-STRUCT-004: detect_hub_and_spoke and detect_dead_clusters scan full edge list per cluster

- **Severity:** Medium
- **Source:** `src/analysis/smells.rs:129-142`, `src/analysis/smells.rs:225-229`
- **Evidence:** Both functions iterate `graph.edges` for each cluster to count or check edges. Both are O(C\*E) where C is cluster count.
- **Impact:** At moderate scale (50 clusters, 9,000 edges), 900K iterations total. Not a bottleneck at current target scale. Noticeable at 10,000+ files with 100+ clusters.
- **Recommendation:** Pre-computed edge index or degree map addresses this.

#### W-STRUCT-005: detect_shotgun_surgery calls blast_radius per high-in-degree file

- **Severity:** Medium
- **Source:** `src/analysis/smells.rs:268-276`
- **Evidence:** Each `blast_radius` call internally calls `build_adjacency` (O(E)) then BFS (O(V+E)). The `in_deg <= 10` filter limits calls to files with >10 incoming edges.
- **Impact:** For M qualifying files, total cost is O(M \* E) for adjacency construction alone. At 3,000 files, typically 10-50 files qualify, giving 90K-450K edge iterations. Becomes painful at 10,000+ files.
- **Recommendation:** Pass a pre-built reverse adjacency index to blast_radius.

---

### 3.2 W-PERF: Performance Findings

#### W-PERF-001: build_adjacency called 4+ times during single pipeline build

- **Severity:** High
- **Source:** `src/algo/scc.rs:10`, `src/algo/topo_sort.rs:14`, `src/algo/centrality.rs:24`, `src/algo/blast_radius.rs:20`
- **Evidence:** The pipeline calls `find_sccs`, `topological_layers`, and `betweenness_centrality` sequentially -- each independently calls `build_adjacency`. Additionally, `detect_shotgun_surgery` calls `blast_radius` which calls `build_adjacency` once per qualifying file. Minimum 3 calls from the algo stage plus N calls from shotgun surgery (N = files with in-degree >10). Each call builds BOTH forward and reverse maps, but callers only use one -- half the work in each call is wasted.
- **Impact:** At 3,000 files / 9,000 edges: 27,000+ edge iterations plus BTreeSet overhead for the fixed calls alone. With shotgun surgery, potentially 90K-450K+ edge iterations. The design doc (`design/performance.md`) does not note the repeated adjacency construction.
- **Recommendation:** Compute adjacency once (or compute forward/reverse separately when only one is needed) and share across all algorithms. This is the single highest-impact optimization available.

#### W-PERF-002: Stage 2 (file read) is sequential while Stage 3 (parse) is parallel

- **Severity:** Medium
- **Source:** `src/pipeline/mod.rs:118-135` (sequential) vs `src/pipeline/mod.rs:158-205` (parallel)
- **Evidence:** Stage 2 reads files sequentially. Per `design/performance.md`, this takes ~150ms at 3,000 files (~3% of total time). The design doc explicitly notes reading does NOT parallelize and explains why.
- **Impact:** At 10,000+ files, sequential read could reach ~500ms. Still minor compared to parse time. On NVMe storage, I/O parallelism provides limited benefit for small sequential reads.
- **Recommendation:** Leave as-is until scale demands it. Design doc acknowledges this as acceptable.

#### W-PERF-003: tree_sitter::Parser created fresh each parse call

- **Severity:** Medium
- **Source:** `src/parser/registry.rs:89`
- **Evidence:** A new `tree_sitter::Parser` is allocated for every file parsed. `Parser::new()` is lightweight (~1KB), but tree-sitter parsers are designed to be reused (internal buffers avoid reallocation on subsequent parses).
- **Impact:** At 3,000 files, adds perhaps 3-10ms total (negligible vs ~3,000ms parse time). Not mentioned in `design/performance.md`.
- **Recommendation:** Thread-local parser pooling could eliminate this allocation overhead. Low priority.

#### W-PERF-004: Betweenness centrality is O(V\*(V+E)) via Brandes algorithm

- **Severity:** Medium
- **Source:** `src/algo/centrality.rs:8-90`
- **Evidence:** Brandes algorithm performs BFS from every node. With BTreeMap overhead, actual cost is O(V*(V+E)*log V). At 3,000 files: ~400M BTreeMap operations. This is the most expensive single algorithm in the pipeline. `design/performance.md` sets "<500ms" budget for Brandes at O(VE) for 3,000 nodes.
- **Impact:** Within budget at 3,000 files. At 10,000 files: could exceed 10 seconds. At 50,000 files: prohibitively expensive.
- **Recommendation:** Monitor via benchmarks. If 10K+ file support becomes a goal, consider approximate centrality algorithms or sampling.

#### W-PERF-005: update() falls back to full rebuild on any change

- **Severity:** Low
- **Source:** `src/pipeline/mod.rs:394-533`
- **Evidence:** The `update()` method detects changes via content hash comparison, but if ANY file changed, falls back to `run_with_output()` (full build). The optimization is limited to the "no changes" fast path. This is an intentional design choice documented in `design/performance.md` and D-050.
- **Impact:** At 3,000 files, full rebuild takes <10s per design targets. Acceptable at current scale.
- **Recommendation:** Accept for now. True incremental rebuild is deferred to future phases.

---

### 3.3 W-MAINT: Maintainability Findings

#### W-MAINT-001: 5 of 6 parsers have zero inline unit tests

- **Severity:** High
- **Source:** `src/parser/python.rs` (506 lines), `src/parser/rust_lang.rs` (597 lines), `src/parser/go.rs` (220 lines), `src/parser/java.rs` (238 lines), `src/parser/csharp.rs` (245 lines), `src/parser/typescript.rs` (710 lines -- tests only cover resolver, not parser)
- **Evidence:** Only `typescript.rs` contains a `#[cfg(test)]` module (line 548), and those tests exclusively test `TypeScriptResolver` workspace resolution. No parser file tests `extract_imports()` or `extract_exports()`. The Python parser's `extract_type_checking_imports` (lines 324-411) and Rust parser's `extract_scoped_use_list` (lines 151-221) are particularly complex recursive functions with no direct tests.
- **Impact:** Parser correctness for edge cases is unverified at the unit level. No fast feedback loop for parser bugs. New contributors have no test template.
- **Recommendation:** Add `#[cfg(test)]` modules to each parser. Priority: Python (TYPE_CHECKING blocks), Rust (nested use lists), then Go/Java/C#.

#### W-MAINT-002: views/ module has zero tests (4 files, ~426 lines)

- **Severity:** High
- **Source:** `src/views/index.rs` (101 lines), `src/views/cluster.rs` (173 lines), `src/views/impact.rs` (97 lines), `src/views/mod.rs` (56 lines)
- **Evidence:** No `#[cfg(test)]` module, no `#[test]` function anywhere in `src/views/`. No integration tests exercise views directly. The `sanitize_filename` function (mod.rs:53-55) which prevents path traversal is also untested.
- **Impact:** Markdown view generation is completely untested. Regressions in table formatting, missing sections, or incorrect metrics would go unnoticed. `sanitize_filename` is security-relevant with no tests.
- **Recommendation:** Add unit tests using small synthetic graphs. Test `sanitize_filename` with adversarial inputs.

#### W-MAINT-003: find_child_by_kind and string_content duplicated across parsers

- **Severity:** Medium
- **Source:** `src/parser/typescript.rs:29-38`, `src/parser/python.rs:32-41`, `src/parser/rust_lang.rs:14-23`
- **Evidence:** Identical `find_child_by_kind` in 3 files. Additionally, `string_content` duplicated between typescript.rs:15-26 and python.rs:14-29 (with minor variation for Python triple-quote handling). Two parallel tree-sitter traversal idioms across all 6 parsers (see W-MAINT-005).
- **Impact:** Maintenance burden: changes must be applied in 3+ places. New contributors face inconsistent patterns. No runtime impact. Cross-ref: W-STRUCT-002.
- **Recommendation:** Extract shared helpers into a `parser::helpers` module. Standardize on one traversal idiom.

#### W-MAINT-004: MCP tools.rs is 709 lines with zero unit tests

- **Severity:** Medium
- **Source:** `src/mcp/tools.rs` (709 lines)
- **Evidence:** No `#[cfg(test)]` module. Integration tests in `tests/mcp_tests.rs` only test server initialization and tool listing -- none of the 16 tool functions are tested for correct output. JSON serialization, error handling, and parameter validation are unverified.
- **Impact:** Tool-level regressions (wrong JSON structure, missing fields, incorrect error messages) would not be caught. The `smells` tool filtering logic and `compressed` tool's 4-branch logic are untested.
- **Recommendation:** Add integration tests that call each tool function with test graph state and verify JSON structure.

#### W-MAINT-005: Two parallel tree-sitter traversal idioms across parsers

- **Severity:** Medium
- **Source:** Cursor-based (`typescript.rs`, `python.rs`, `rust_lang.rs`) vs index-based (`go.rs`, `java.rs`, `csharp.rs`)
- **Evidence:** TypeScript/Python/Rust use `node.walk()` + `node.children(&mut cursor)`. Go/Java/C# use `for i in 0..node.child_count()` + `node.child(i)`. Both correct but inconsistent.
- **Impact:** Increased cognitive load for contributors working across parsers. New language parsers have no canonical pattern to follow.
- **Recommendation:** Standardize on cursor-based pattern (more idiomatic tree-sitter Rust API). Pair with parser test additions (W-MAINT-001).

#### W-MAINT-006: update() method has 7 parameters (too_many_arguments suppressed)

- **Severity:** Low
- **Source:** `src/pipeline/mod.rs:402-411`
- **Evidence:** The `update` method has `#[allow(clippy::too_many_arguments)]` (line 401). Takes 7 parameters: `root`, `config`, `reader`, `output_dir`, `timestamp`, `verbose`, `no_louvain`.
- **Impact:** Minor readability concern. Acknowledged lint suppression.
- **Recommendation:** Consider a `BuildOptions` struct to bundle `timestamp`, `verbose`, `no_louvain`. Low priority.

---

### 3.4 W-RELY: Reliability Findings

#### W-RELY-001: FatalError::GraphNotFound reused for clusters.json missing file

- **Severity:** High
- **Source:** `src/serial/json.rs:55-57`
- **Evidence:** `read_clusters()` opens `clusters.json` but maps the error to `FatalError::GraphNotFound`, which says "graph not found in {path}. Run 'ariadne build' first." This is misleading when `graph.json` exists but `clusters.json` is missing (e.g., partial write, corruption).
- **Impact:** User sees "graph not found" which is factually wrong and misdirects debugging. The MCP server catches and handles this transparently, but CLI paths display the wrong message.
- **Recommendation:** Add a `ClustersNotFound` variant to `FatalError`, or use `GraphCorrupted` with an accurate message.

#### W-RELY-002: W010 (GraphVersionMismatch) and W011 (GraphCorrupted) defined but never emitted as warnings

- **Severity:** Medium
- **Source:** `src/diagnostic.rs:52-53` (definition), `src/pipeline/mod.rs:440-448` (handling site)
- **Evidence:** Both warning codes are defined in `WarningCode` enum and have handler branches in `DiagnosticCollector::warn()`. However, no code path calls `diagnostics.warn()` with either code. Both conditions fall back to full rebuild with optional `eprintln` (verbose-only). The `graph_load_warnings` counter is always 0.
- **Impact:** Users in non-verbose mode get no indication of graph corruption or version mismatch. The diagnostic report is incomplete for graph lifecycle events.
- **Recommendation:** Emit W010/W011 warnings to the `DiagnosticCollector` before falling back to full rebuild.

#### W-RELY-003: CanonicalPath::new("") and pathological inputs produce empty strings

- **Severity:** Low _(adjusted from Medium by review -- speculative impact, intentional behavior with tests)_
- **Source:** `src/model/types.rs:17-33`
- **Evidence:** The `normalize` function produces empty strings for `""`, `"."`, `"./"`, `"../../.."`. Tests at types.rs:267-285 confirm this is intentional. An empty `CanonicalPath` is a valid value that can be used as a graph node key, but no concrete code path has been demonstrated to produce one in practice.
- **Impact:** If a bug in path resolution produces an empty specifier, it silently becomes a valid `CanonicalPath("")` rather than being caught. TypeScript resolver checks for empty specifiers but other resolvers do not. Impact is speculative.
- **Recommendation:** Add `debug_assert!(!normalized.is_empty())` in `CanonicalPath::new()` or add empty-path guards to all resolvers.

#### W-RELY-004: Go resolver find_module_path always returns None

- **Severity:** Low
- **Source:** `src/parser/go.rs:121-133`
- **Evidence:** `find_module_path` scans for `go.mod` but returns `None` even when found (with explanatory comment). This means `is_external()` cannot distinguish internal from external imports -- all dot-containing imports (standard for Go modules) are classified as external and skipped.
- **Impact:** Go projects have all internal module imports silently dropped. Only stdlib imports are resolved. Documented limitation with inline comment.
- **Recommendation:** Thread `go.mod` content through the pipeline, or document Go as limited to stdlib-only resolution in CLI help.

#### W-RELY-005: Poll fallback in MCP server does full rebuild every 30 seconds without change detection

- **Severity:** Medium _(adjusted from Low by review -- misleading comment, significant CPU waste on failure-path systems)_
- **Source:** `src/mcp/server.rs:166-208`
- **Evidence:** `start_poll_fallback` calls `pipeline.run_with_output()` every 30 seconds unconditionally. The comment says "let the pipeline's delta logic handle the no-op case" but `run_with_output` always does a full build -- the comment is misleading. This runs when the file watcher fails (WSL1, Docker).
- **Impact:** On failure-path systems, full pipeline build every 30 seconds even with no changes. At 3,000 files: ~10 seconds of CPU every 30 seconds (33% utilization).
- **Recommendation:** Use `pipeline.update()` instead of `pipeline.run_with_output()` in the poll fallback.

---

### 3.5 W-EXT: Extensibility Findings

#### W-EXT-001: FileSet lacks FromIterator trait implementation

- **Severity:** Low
- **Source:** `src/model/types.rs:164-167`
- **Evidence:** `FileSet` has an inherent `from_iter` method with `#[allow(clippy::should_implement_trait)]` rather than implementing `std::iter::FromIterator`. `.collect::<FileSet>()` syntax is unavailable.
- **Impact:** Minor ergonomic friction. The functionality exists via `FileSet::from_iter(iter)`.
- **Recommendation:** Implement `FromIterator<CanonicalPath> for FileSet` and remove the inherent method.

#### W-EXT-002: Adding a new language parser requires touching 4 files minimum

- **Severity:** Low
- **Source:** `src/parser/` module
- **Evidence:** Adding a parser requires: (1) new `src/parser/<lang>.rs`, (2) edit `src/parser/mod.rs`, (3) edit `src/parser/registry.rs`, (4) edit `Cargo.toml`. The trait surface is well-designed (5+1 methods). However, no template or contributing guide documents this process, and two construction patterns coexist (struct with `new()` vs module-level functions).
- **Impact:** Friction is in registration boilerplate and lack of documented pattern, not trait complexity. Adequate for current in-tree-only model.
- **Recommendation:** Document the process and standardize on one construction pattern.

#### W-EXT-003: Adding a new output format requires implementing 2 traits (4 methods each)

- **Severity:** Low
- **Source:** `src/serial/mod.rs:74-95`
- **Evidence:** New format requires implementing `GraphSerializer` (4 methods) and `GraphReader` (4 methods) against well-separated output types (D-022). Wiring in main.rs (D-020).
- **Impact:** Low friction. The design is well-suited for extension.
- **Recommendation:** None urgent. Extensibility story for output formats is well-designed.

#### W-EXT-004: No extension point documentation in code or design docs

- **Severity:** Medium
- **Source:** Across codebase
- **Evidence:** The trait-based design is well-implemented but extension information is scattered across 9+ decision log entries (D-002, D-018, D-019, D-020, D-022, D-023, D-032, D-033, D-048). No CONTRIBUTING.md exists. No module-level `//!` doc comments explain how to extend parser/, serial/, algo/, or mcp/ modules.
- **Impact:** High onboarding cost for external contributors despite clean code-level interfaces.
- **Recommendation:** Create an "Extension Guide" section in CONTRIBUTING.md, or add `//!` doc comments to key module files.

---

### 3.6 W-DESIGN: Design Conformance Findings

#### W-DESIGN-001: D-047 partially superseded -- tokio present despite "No Async Runtime" decision

- **Severity:** Informational
- **Source:** `Cargo.toml:35`, `src/main.rs:338`, `design/decisions/log.md` D-047 + D-051
- **Evidence:** D-047 decided "no async runtime." D-051 supersedes this for the `serve` subcommand, introducing tokio behind the `serve` feature flag. D-047's status correctly says "Partially superseded by D-051." Tokio is properly isolated: feature-gated, runtime created only in `Serve` match arm, non-serve paths remain synchronous.
- **Impact:** None. Design drift is fully acknowledged and documented.
- **Recommendation:** None needed.

#### W-DESIGN-002: GraphState has fields not specified in ROADMAP.md

- **Severity:** Low
- **Source:** `src/mcp/state.rs:21`, `design/ROADMAP.md` Phase 3a D1
- **Evidence:** ROADMAP Phase 3a specifies 8 fields for `GraphState`. Implementation adds 8 more: `forward_index`, `raw_imports`, `cluster_metrics`, `pagerank`, `combined_importance`, `compressed_l0`, `spectral`, `last_diff`. All additional fields serve Phase 3b/3c features.
- **Impact:** Implementation is a superset, not a deviation. All specified fields are present.
- **Recommendation:** Update ROADMAP Phase 3a to note the spec shows minimum viable state.

#### W-DESIGN-003: mcp/ depends on algo/ directly for computation during state initialization

- **Severity:** Low
- **Source:** `src/mcp/state.rs:5-6`
- **Evidence:** `state.rs` imports `crate::algo::{compress, pagerank, spectral}` and `crate::analysis::metrics`. This dependency is explicitly allowed by D-033. The computation is initialization-time work, not request-time.
- **Impact:** Conformant with D-033. Raises a design pattern question about computation in the transport layer, but the alternative would violate separation of concerns.
- **Recommendation:** None needed. Dependency is sanctioned.

#### W-DESIGN-004: ROADMAP phases lag behind decision log

- **Severity:** Medium
- **Source:** `design/ROADMAP.md` vs `design/decisions/log.md`
- **Evidence:** The decision log is accurate (D-050, D-051 properly document changes). However, ROADMAP.md has not been retroactively updated: Phase 2b still says "Delta computation" (actual: full rebuild, D-050); Phase 3a still references D-047's "no async" (actual: tokio for serve, D-051). A new reader encounters outdated information in ROADMAP before finding corrections in the decision log.
- **Impact:** Documentation reading-order problem for new contributors. The decision log is the source of truth but is not the first document read.
- **Recommendation:** Add "Status" annotations to each ROADMAP phase and note implementation deviations inline.

---

## 4. Module Health Matrix

| Module        | W-STRUCT                        | W-PERF                            | W-MAINT                                          | W-RELY                          | W-EXT              | W-DESIGN              |
| ------------- | ------------------------------- | --------------------------------- | ------------------------------------------------ | ------------------------------- | ------------------ | --------------------- |
| `model/`      | Good                            | Good                              | Good                                             | Low (W-RELY-003)                | Low (W-EXT-001)    | Good                  |
| `parser/`     | Low (W-STRUCT-002)              | Medium (W-PERF-003)               | **Poor** (W-MAINT-001, W-MAINT-003, W-MAINT-005) | Low (W-RELY-004)                | Low (W-EXT-002)    | Good                  |
| `pipeline/`   | Good                            | Medium (W-PERF-002)               | Low (W-MAINT-006)                                | Good                            | Good               | Good                  |
| `algo/`       | **Poor** (W-STRUCT-001)         | **Poor** (W-PERF-001, W-PERF-004) | Good                                             | Good                            | Good               | Good                  |
| `analysis/`   | Medium (W-STRUCT-003, 004, 005) | Good                              | Good                                             | Good                            | Good               | Good                  |
| `serial/`     | Good                            | Good                              | Good                                             | **Poor** (W-RELY-001)           | Low (W-EXT-003)    | Good                  |
| `views/`      | Good                            | Good                              | **Poor** (W-MAINT-002)                           | Good                            | Good               | Good                  |
| `mcp/`        | Good                            | Good                              | Medium (W-MAINT-004)                             | Medium (W-RELY-002, W-RELY-005) | Good               | Low (W-DESIGN-003)    |
| `mcp/state`   | Good                            | Good                              | Good                                             | Good                            | Good               | Low (W-DESIGN-002)    |
| `diagnostic`  | Good                            | Good                              | Good                                             | Medium (W-RELY-002)             | Good               | Good                  |
| `detect/`     | Good                            | Good                              | Good                                             | Good                            | Good               | Good                  |
| `cluster/`    | Good                            | Good                              | Good                                             | Good                            | Good               | Good                  |
| _design docs_ | --                              | --                                | --                                               | --                              | Medium (W-EXT-004) | Medium (W-DESIGN-004) |

**Rating criteria:**

- **Good:** No findings, or only Informational severity
- **Fair/Low:** Low severity findings only
- **Medium:** Medium severity findings only
- **Poor:** High or Critical severity findings

---

## 5. Prioritized Action Items

### Priority 1: High Impact, Moderate Effort

These address the root causes of the most findings and highest-severity issues.

| #   | Action                                                                                                                                                                  | Addresses                              | Effort |
| --- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------- | ------ |
| A1  | Compute adjacency index once after graph construction; pass to all algorithms. Compute forward/reverse separately when only one is needed.                              | W-STRUCT-001, W-PERF-001, W-STRUCT-005 | Medium |
| A2  | Add `#[cfg(test)]` unit tests to all 6 parsers for `extract_imports` and `extract_exports`. Priority: Python (TYPE_CHECKING), Rust (nested use lists), then Go/Java/C#. | W-MAINT-001                            | Medium |
| A3  | Add unit tests for views module (`generate_index`, `generate_cluster_view`, `generate_blast_radius_view`, `sanitize_filename`).                                         | W-MAINT-002                            | Small  |
| A4  | Fix `GraphNotFound` misuse: add `ClustersNotFound` variant to `FatalError` or use `GraphCorrupted` with accurate message for `read_clusters`.                           | W-RELY-001                             | Small  |

### Priority 2: Medium Impact, Small Effort

Quick wins that improve reliability and code quality.

| #   | Action                                                                                                                            | Addresses                              | Effort |
| --- | --------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------- | ------ |
| A5  | Emit W010/W011 warnings to `DiagnosticCollector` before falling back to full rebuild.                                             | W-RELY-002                             | Small  |
| A6  | Use `pipeline.update()` instead of `pipeline.run_with_output()` in MCP poll fallback. Fix misleading comment.                     | W-RELY-005                             | Small  |
| A7  | Pre-compute degree maps (in-degree, out-degree per node) for smell detection functions.                                           | W-STRUCT-003, W-STRUCT-004             | Small  |
| A8  | Extract `find_child_by_kind`, `string_content`, `node_text` into `parser::helpers` module. Standardize on cursor-based traversal. | W-MAINT-003, W-MAINT-005, W-STRUCT-002 | Small  |

### Priority 3: Medium Impact, Medium Effort

Documentation and testing improvements for project maturity.

| #   | Action                                                                                                            | Addresses                  | Effort |
| --- | ----------------------------------------------------------------------------------------------------------------- | -------------------------- | ------ |
| A9  | Add extension point documentation (CONTRIBUTING.md or module-level `//!` docs for parser/, serial/, algo/, mcp/). | W-EXT-004, W-EXT-002       | Medium |
| A10 | Update ROADMAP.md with implementation status annotations and inline deviation notes (D-050, D-051).               | W-DESIGN-004, W-DESIGN-002 | Small  |
| A11 | Add MCP tool-level integration tests. Priority: `file`, `compressed`, `smells`.                                   | W-MAINT-004                | Medium |

### Priority 4: Low Impact or Deferred

Acknowledged design choices or minor improvements.

| #   | Action                                                                                                     | Addresses   | Effort   |
| --- | ---------------------------------------------------------------------------------------------------------- | ----------- | -------- |
| A12 | Implement `FromIterator<CanonicalPath> for FileSet`.                                                       | W-EXT-001   | Trivial  |
| A13 | Add `debug_assert!(!normalized.is_empty())` in `CanonicalPath::new()`.                                     | W-RELY-003  | Trivial  |
| A14 | Monitor Brandes centrality via benchmarks; consider approximate algorithms if 10K+ files becomes a target. | W-PERF-004  | Deferred |
| A15 | Thread `go.mod` content through pipeline for Go module path detection, or document Go as stdlib-only.      | W-RELY-004  | Medium   |
| A16 | Consider `BuildOptions` struct for `update()` parameters.                                                  | W-MAINT-006 | Small    |
| A17 | Evaluate parser pooling (thread-local `tree_sitter::Parser` reuse).                                        | W-PERF-003  | Small    |
| A18 | Sequential file read parallelization -- defer unless scale demands.                                        | W-PERF-002  | Deferred |
| A19 | Incremental rebuild (true delta re-parse) -- defer per D-050.                                              | W-PERF-005  | Deferred |

---

## 6. Design Drift Log

| Decision | Title                                                   | Status         | Drift                                                                                                       |
| -------- | ------------------------------------------------------- | -------------- | ----------------------------------------------------------------------------------------------------------- |
| D-001    | Structural Topology via Rust CLI + Tree-sitter          | Conformant     | Standalone Rust binary with tree-sitter parsing as designed                                                 |
| D-002    | LanguageParser + ImportResolver traits                  | Conformant     | Two construction patterns coexist (struct vs module-level functions) but trait surface is correct           |
| D-003    | Graceful Degradation                                    | Conformant     | Unparseable files logged and excluded; exit 0 with warnings as specified                                    |
| D-004    | Separate Project — Consumer-Agnostic Design             | Conformant     | Standalone project with no consumer-specific code                                                           |
| D-005    | Error Handling — Best-Effort with Structured Warnings   | Conformant     | Two-tier error model (FatalError + Warning codes) implemented; minor gap in W010/W011 emission (W-RELY-002) |
| D-006    | Byte-Identical Deterministic Output                     | Conformant     | BTreeMap for sorted keys, edges sorted by (from, to, type), rayon deterministic collection                  |
| D-007    | Path Normalization — Canonical Relative Format          | Conformant     | CanonicalPath newtype enforces normalization at construction                                                |
| D-008    | Monorepo — Single Graph with Workspace-Aware Resolution | Conformant     | Workspace detection and cross-package resolution implemented                                                |
| D-009    | MIT/Apache-2.0 Dual License                             | Conformant     | Dual-licensed as specified                                                                                  |
| D-010    | Crate Name `ariadne-graph`                              | Conformant     | Crate name ariadne-graph, binary name ariadne via [[bin]]                                                   |
| D-011    | Phase Split — MVP First, Hardening Second               | Conformant     | Phase 1a/1b split followed as designed                                                                      |
| D-012    | Compact Tuple Format for Edges                          | Conformant     | Edges serialized as [from, to, type, [symbols]] tuples                                                      |
| D-013    | xxHash64 for Content Hashing                            | Conformant     | hash.rs uses xxHash64, lowercase hex output                                                                 |
| D-014    | Layer Detection Heuristics                              | Conformant     | Eight architectural layers inferred via path-based heuristics in detect/                                    |
| D-015    | Graph Output Committed to Git                           | Conformant     | .ariadne/graph/ output designed for version control                                                         |
| D-016    | Default Output Directory `.ariadne/graph/`              | Conformant     | Default directory as specified, overridable via --output                                                    |
| D-017    | Model as leaf module                                    | Conformant     | No issues                                                                                                   |
| D-018    | Parser module structure                                 | Conformant     | Registration works; inconsistent patterns are a style issue                                                 |
| D-019    | Pipeline stages                                         | Conformant     | Sequential read acknowledged as design choice                                                               |
| D-020    | Composition Root                                        | Conformant     | Wiring in main.rs as specified                                                                              |
| D-021    | DiagnosticCollector                                     | **Minor gap**  | W010/W011 warning codes defined but never emitted (W-RELY-002)                                              |
| D-022    | Output model separation                                 | Conformant     | Clean separation between internal and output types                                                          |
| D-023    | Newtype pattern                                         | Conformant     | CanonicalPath empty-string edge case is intentional but unguarded (W-RELY-003)                              |
| D-024    | Pipeline Support Types                                  | Conformant     | FileSet, FileSkipReason, WalkConfig, BuildOutput all implemented as specified                               |
| D-025    | arch_depth Placeholder in Phase 1a                      | Conformant     | Placeholder replaced with computed values via topological_layers in Phase 2                                 |
| D-032    | GraphSerializer/GraphReader                             | Conformant     | 4th method pair added for raw_imports (D-054)                                                               |
| D-033    | Module dependency rules                                 | Conformant     | mcp/ -> algo/ dependency explicitly allowed                                                                 |
| D-047    | No async runtime                                        | **Superseded** | Tokio added for serve subcommand; documented in D-051. ROADMAP not updated.                                 |
| D-049    | Float determinism                                       | Conformant     | BTreeMap usage throughout for deterministic iteration                                                       |
| D-050    | Full-rebuild-always                                     | Conformant     | update() falls back to full rebuild as documented                                                           |
| D-051    | Tokio isolated to serve                                 | Conformant     | Feature-gated, runtime in Serve arm only                                                                    |

---

## 7. Note on L0 Graph Orphan Files

The L0 dependency graph reports 36 orphan files (nodes with zero incoming and zero outgoing edges). These are **expected orphans** and do not indicate a structural issue:

- **Test fixtures** (`tests/fixtures/*`): Sample source files used by integration tests. These are loaded via filesystem path by the test harness, not via `use`/`import` statements, so they produce no import-level edges.
- **Benchmarks** (`benches/*`): Performance benchmark files that exercise library functions directly. Like test fixtures, they are not imported by any source file tracked in the graph.
- **Integration tests** (`tests/*.rs`): Rust integration test files that are compiled separately by `cargo test` and access library code via `use ariadne_graph::...` (external crate syntax), which the graph does not track as internal edges.

The graph tracks **import-level dependencies between source files within the project**. Files loaded via path, compiled as separate test binaries, or referenced only by the build system are correctly excluded from the dependency edge set. No action is needed.

---

## 8. Appendix: Rejected Findings

No findings were rejected during review. All 29 findings across the three phases received Accept or Accept-with-adjustment verdicts from the reviewer.

**Severity adjustments applied:**

- W-PERF-001: Accepted with note that wasted dual-map construction is slightly understated (severity remains High)
- W-RELY-003: Adjusted Medium to Low (speculative impact chain, intentional behavior)
- W-RELY-005: Adjusted Low to Medium (misleading comment, significant CPU waste on failure-path systems)

---

_Report generated 2026-03-22. 29 findings across 6 dimensions. 5 High, 13 Medium, 10 Low, 1 Informational (W-DESIGN-001 is the sole Informational finding). All findings trace to verified source code evidence._
