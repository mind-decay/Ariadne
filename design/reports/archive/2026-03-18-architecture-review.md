# Architecture Review

**Date:** 2026-03-18
**Focus:** Full System
**Mode:** Pre-implementation
**Reviewed:** architecture.md, ROADMAP.md, decisions/log.md (D-001–D-023), path-resolution.md, determinism.md, performance.md, error-handling.md, testing.md, distribution.md
**Status:** All findings resolved (see fixes applied below)

## Executive Summary

The Ariadne design is architecturally sound — module boundaries are clean, dependency direction is consistent, and the trait-based pipeline is well-motivated. The core abstractions (newtypes, BTreeMap determinism, LanguageParser/ImportResolver separation) are good choices for the problem domain. However, the design has **meaningful complexity ahead of need**: several Phase 2 concerns leak into Phase 1a (notably `arch_depth`), and 4 types referenced in trait signatures are never defined. The most dangerous gap is under-specification of TypeScript module resolution — the highest-complexity parser with the least design detail. The most impactful quick win is resolving the `arch_depth` Phase 1a question before any code is written.

## Key Themes

### Theme 1: Undefined Types in Core Interfaces

All 4 reviewers flagged missing type definitions that block implementation. Four types appear in trait signatures or function returns but are never specified anywhere:

- **`FileSet`** — used in `ImportResolver::resolve(import, from_file, known_files: &FileSet)`. Is it `HashSet<CanonicalPath>`? `BTreeSet`? A custom wrapper? Every language resolver depends on this type.
- **`FileSkipReason`** — returned by `FileReader::read()`. Distinct from `FatalError` and `Warning`, but its variants are undefined.
- **`WalkConfig`** — passed to `FileWalker::walk()`. What fields? `max_files`? `max_depth`? `follow_symlinks`?
- **`BuildOutput`** — returned by `pipeline.run()`. What does it contain?

These are not design decisions that can be deferred — they are prerequisites for writing any pipeline code.

### Theme 2: Phase 1a/Phase 2 Boundary Violation via `arch_depth`

Three reviewers independently flagged the `arch_depth: u32` field in `Node`. This field requires topological sort (a Phase 2 algorithm) and correct cycle handling (Tarjan SCC, also Phase 2). Yet it exists in the Phase 1a data model and appears in graph.json output. The design never specifies how Phase 1a populates it. This forces an unplanned decision during implementation that affects the output schema, git-committed graph diffs, and Phase 2 migration.

### Theme 3: Under-Specified Resolution Logic

The TypeScript/JavaScript parser is rated "High" complexity but has the least resolution specification. The path-resolution.md describes a clean 4-step flow that is significantly simpler than real TS module resolution (tsconfig `paths`, `baseUrl`, package.json `exports` field, CJS vs ESM, `.d.ts` precedence). Similarly, Rust's `use crate::...` module path resolution is fundamentally different from filesystem-path resolution but the trait interface assumes path-based imports. The `tests` edge inference algorithm and `re_exports` edge semantics are also unspecified.

### Theme 4: Complexity Ahead of Need

The design imports Phase 2 complexity into Phase 1a's cognitive surface area: 10 design documents, 23 decisions, 8 modules with enforced dependency rules, 13 invariants, 4 test levels. The LanguageParser/ImportResolver trait split and Internal/Output model separation are sound abstractions but solve problems that don't exist until Phase 1b (workspace resolution) and Phase 3+ (second output format). The 7 Phase 2 algorithms include 2 (Louvain clustering, Brandes centrality) that are over-engineered — simpler alternatives (directory clustering, in-degree counts) achieve 80% of the benefit.

## Detailed Findings

### Foundational Issues

**F-01: `FileSet`, `FileSkipReason`, `WalkConfig`, `BuildOutput` undefined (HIGH)**
All four types appear in trait signatures in `design/architecture.md` but are never specified. Every pipeline implementor and test writer is blocked without these definitions.
*Direction:* Define all four types in `architecture.md` before implementation. `FileSet` should be `BTreeSet<CanonicalPath>` for determinism consistency. `FileSkipReason` should be an enum with variants matching the W-codes.

**F-02: `arch_depth` in Phase 1a requires Phase 2 algorithm (HIGH)**
`Node.arch_depth: u32` is in the Phase 1a data model (`architecture.md` Internal Model) but topological sort and Tarjan SCC are Phase 2 algorithms. Phase 1a has no way to populate this field correctly.
*Direction:* Choose one: (a) `arch_depth` is always 0 in Phase 1a (document in schema), (b) include simplified BFS-based depth without SCC handling, or (c) pull Tarjan into Phase 1a. Option (a) is simplest but produces a large diff when Phase 2 activates.

**F-03: D-002 contradicts D-018 — both marked Accepted (MEDIUM)**
D-002 defines a 6-method `LanguageParser` trait including `resolve_import_path`. D-018 splits this into two traits and removes `resolve_import_path`. D-002's status remains `Accepted` rather than `Superseded`, creating contradicting trait definitions in the decision log.
*Direction:* Mark D-002 as `Superseded by D-018`.

**F-04: Rust `ImportResolver` model is fundamentally different from path-based languages (MEDIUM)**
`ImportResolver::resolve` assumes `import.path` is a filesystem path. Rust's `use crate::auth::login` is a module path, not a filesystem path. The resolution requires mapping the module tree to files via `mod` declarations. The trait interface works but the contract for non-path-based languages is undocumented.
*Direction:* Document how Rust's resolver works within the shared trait: either the parser pre-converts module paths to filesystem paths in `extract_imports`, or the resolver uses a different internal strategy. The trait need not change.

**F-05: `ContentHash` example is 8 chars, spec says 16 (MEDIUM)**
`architecture.md` graph.json example shows `"hash": "a1b2c3d4"` (8 hex chars). D-013 and the Domain Types section both specify 16 chars (xxHash64 = 64 bits = 16 hex). The example will mislead implementors.
*Direction:* Fix the example to 16 characters.

### Structural Issues

**S-01: `pipeline/build.rs` is a hidden accumulator of responsibilities (HIGH)**
This single file must: iterate parsed files, call `detect/filetype.rs`, call `detect/layer.rs`, call `ImportResolver` per import, validate paths, deduplicate edges, compute `arch_depth`, sort exports/symbols, and assemble `ProjectGraph`. None of this is documented. The topological depth computation is a graph algorithm embedded in Phase 1 data assembly.
*Direction:* Enumerate `build.rs`'s sub-responsibilities explicitly in `architecture.md`. Consider splitting into `build.rs` (assembly) and a separate module for derived metrics.

**S-02: `detect/` is not listed in `pipeline/`'s dependency table (MEDIUM)**
The D-023 dependency table says `pipeline/` depends on "traits from `parser/`, `serial/`; types from `model/`". But `pipeline/build.rs` must call `detect/filetype.rs` and `detect/layer.rs` during node assembly. This dependency is missing.
*Direction:* Add `detect/` to `pipeline/`'s "Depends on" row.

**S-03: `serial/` → `cluster/` conversion path violates stated dependency rules (MEDIUM)**
`GraphSerializer` has `write_clusters(&ClusterOutput)`. `ClusterOutput` lives in `serial/`. The conversion from `ClusterMap` (produced by `cluster/`) to `ClusterOutput` must happen somewhere. If in `serial/`, it imports from `cluster/`, violating the stated rule that `serial/` depends on `model/` only.
*Direction:* Place the conversion in `pipeline/` (which legitimately knows both) and have `serial/` accept only pre-converted `ClusterOutput`.

**S-04: `cluster/` has no documented interface (MEDIUM)**
No function signatures, input types, or output types for the clustering module. Unknown whether it returns a `ClusterMap` or mutates `ProjectGraph` directly. The `ClusterOutput` struct is never concretely defined either.
*Direction:* Add a brief interface spec: function signature, inputs, outputs, and note on whether it returns data or mutates the graph.

**S-05: `WorkspaceInfo` has no module home (MEDIUM)**
Defined in `path-resolution.md` but not assigned to any module in `architecture.md`. If placed in `pipeline/`, `ImportResolver` must import from `pipeline/` — reversing dependency direction.
*Direction:* Place `WorkspaceInfo` in `model/` (pure data structure, no behavior).

**S-06: `diagnostic.rs` and `hash.rs` missing from dependency table (LOW)**
Neither appears as a row in the D-023 dependency table. `hash.rs` produces `ContentHash` (from `model/`) but its dependency on `model/` is undocumented.
*Direction:* Add both as explicit rows.

### Surface Issues

**U-01: TypeScript module resolution is far more complex than the design acknowledges (HIGH)**
The path-resolution.md describes a clean 4-step flow. Real TS resolution involves tsconfig `paths`/`baseUrl`, package.json `exports` field with conditions, CJS vs ESM, `.d.ts` vs `.ts` precedence, and barrel re-export transitive analysis. This is the hardest parser and the least specified.
*Direction:* Add a TS-specific resolution decision tree or table before implementing `typescript.rs`. At minimum: bare specifier → skip (external), relative → join + probe extensions, directory → try index files. Defer tsconfig `paths` to Phase 1b.

**U-02: Architectural layer heuristics are named but not specified (MEDIUM)**
8 layers are defined but the actual path patterns that trigger detection are nowhere in any document. Backend projects (Go, Java) may produce many `unknown` assignments since `component` and `hook` are frontend-only.
*Direction:* Add a heuristic table (path pattern → layer) before implementing `detect/layer.rs`.

**U-03: `tests` edge inference algorithm unspecified (MEDIUM)**
The criteria for producing `EdgeType::tests` edges — which naming conventions, which import patterns, whether language-specific rules apply — are not documented. Go uses `_test.go` (a language rule), TS uses `.test.ts`/`__tests__/` (conventions).
*Direction:* Specify the algorithm and per-language patterns in `architecture.md` detect section.

**U-04: `re_exports` edge semantics not specified with example (MEDIUM)**
When is an edge `re_exports` vs `imports`? The interaction between barrel re-exports, the `exports` field, and edge creation is underdocumented.
*Direction:* Add an example graph shape for a barrel re-export pattern.

**U-05: `performance.md` uses `HashMap` — contradicts D-006 BTreeMap (MEDIUM)**
Memory Model section says `HashMap<String, Node>`. Should be `BTreeMap<CanonicalPath, Node>`.
*Direction:* Text correction.

**U-06: Workspace detection scope contradicts ROADMAP (MEDIUM)**
D-008 says "Phase 1 covers npm/yarn/pnpm workspaces." ROADMAP defers workspace detection entirely to Phase 1b. D-008 predates the Phase 1a/1b split (D-011).
*Direction:* Update D-008 to say "Phase 1b" explicitly.

**U-07: `GraphSerializer` will need breaking change in Phase 2 (LOW)**
Adding `write_stats` for `stats.json` breaks all existing `impl GraphSerializer` types.
*Direction:* Note this in the trait design; consider a default method returning `Ok(())`.

**U-08: Floating-point cohesion may break byte-identical output (LOW)**
Cluster cohesion (`internal / (internal + external)`) produces `f64` that `serde_json` serializes with platform-dependent precision for boundary values.
*Direction:* Round to fixed decimal precision (e.g., 4 places) or store as integer fraction.

**U-09: `DiagnosticCollector` has two Mutexes where one suffices (LOW)**
`warnings: Mutex<Vec<Warning>>` and `counts: Mutex<DiagnosticCounts>` — counts are derivable from warnings at drain time. Two mutexes introduce lock ordering risk.
*Direction:* Merge into one `Mutex` or derive counts during `drain()`.

**U-10: W005 (SymlinkLoop) is unreachable dead design (LOW)**
Acknowledged in error-handling.md but still occupies a taxonomy slot. Adds confusion.
*Direction:* Remove until symlink following is implemented.

**U-11: `.ariadne/` output directory not explicitly excluded from walking (LOW)**
If a future parser adds `.json` support, output files would be walked. Currently safe since no parser handles `.json`.
*Direction:* Explicitly exclude `.ariadne/` from walking.

**U-12: TOCTOU — walked file deleted before read creates dangling edge target (LOW)**
A file in `known_files` that fails to read still appears in the walked set. If another file's import resolved to it, the edge's `to` target has no node, violating INV-1.
*Direction:* Resolution should use successfully-read files, not walked files.

**U-13: Case-insensitive FS fallback is O(n) per import without secondary index (LOW for Phase 1a, HIGH for Phase 1b)**
Naive case-insensitive matching against `BTreeSet` requires linear scan. For 150k imports on 50k files: 7.5B comparisons.
*Direction:* Build a `lowercase_path → canonical_path` lookup map during walking. Phase 1b concern.

**U-14: File descriptor exhaustion mapped to W002 without distinguishing cause (LOW for Phase 1a)**
`ErrorKind::TooManyOpenFiles` produces generic "read failed" warnings. In CI containers with tight limits, this causes silently partial graphs.
*Direction:* Detect `TooManyOpenFiles` specifically in Phase 1b.

**U-15: TypeScript→JSON imports produce silent W006 (LOW for Phase 1a)**
`import config from './config.json'` is extremely common in TS but produces no edge since no JSON parser exists.
*Direction:* Document as known limitation; consider "data" file type in future.

## Discussion Points

### 1. Should Phase 1a ship with 3 languages instead of 6?

**Tension:** 6 Tier 1 parsers is ambitious for an MVP. Each parser has language-specific edge cases in resolution.
**For 6:** Validates the trait abstraction across diverse languages. Go/C#/Java are low complexity.
**For 3 (TS/JS + Go + Python):** Covers ~70% of use cases. Validates the pipeline. Reduces risk of discovering resolution bugs across 6 parsers simultaneously.
**At stake:** Implementation timeline and risk of shipping parsers with untested resolution edge cases.
**Recommendation:** Keep 6 — Go, C#, and Java parsers are genuinely low complexity (simple import syntax, straightforward resolution). The trait abstraction is validated better with 6 diverse implementations.

### 2. Should Louvain and Brandes be deferred beyond Phase 2?

**Tension:** Louvain clustering and Brandes centrality are the most complex Phase 2 algorithms. Simpler alternatives exist.
**For keeping:** Louvain detects real module boundaries; Brandes identifies true bottlenecks beyond in-degree.
**For deferring:** Directory clustering is "free" and intuitive. In-degree is a strong proxy for centrality. Both simpler alternatives should be validated with users before investing in the complex versions.
**At stake:** Phase 2 scope and timeline.
**Recommendation:** Defer both. Phase 2 ships with: Reverse BFS, Tarjan SCC, topological sort, subgraph extraction, delta computation. Add Brandes/Louvain in Phase 3 if user feedback demands it.

### 3. How should `arch_depth` be handled in Phase 1a?

**Tension:** The field is in the data model but the algorithm is Phase 2.
**Option A:** Store 0, document in schema. Simple. Produces large diff when Phase 2 activates.
**Option B:** Simple BFS depth without SCC handling. Cycles get arbitrary depth. Partially useful.
**Option C:** Pull Tarjan into Phase 1a. Correct but expands scope.
**Recommendation:** Option A (store 0). The git diff concern is real but one-time. Correctness > convenience.

### 4. Is the LanguageParser/ImportResolver trait split justified before Phase 1b?

**Tension:** The split solves a Phase 1b problem (workspace-aware resolution) but adds Phase 1a complexity.
**For split:** Clean design from day one. Same struct implements both traits — minimal overhead.
**For single trait:** Simpler. Split when actually needed (YAGNI).
**Recommendation:** Keep the split. Implementation cost is near-zero when one struct implements both. The mental model cost is worth the clean Phase 1b extension path.

## Strengths

1. **Dependency direction discipline.** The module dependency table (D-023) with explicit "Never depends on" columns is excellent. This level of explicit architectural constraint is rare and prevents the most common source of complexity growth.

2. **Determinism as a first-class design principle.** D-006, `determinism.md`, and the sort-point analysis show deep consideration. BTreeMap everywhere, sorted rayon output, opt-in timestamps — this is the right approach for git-committed output.

3. **Composition Root pattern (D-020).** Concentrating all wiring in `main.rs` is textbook Clean Architecture. The result: `lib.rs` is fully testable with mocks, concrete types never leak into library code.

4. **Decision log quality.** D-001 through D-023 are well-structured: context, decision, alternatives rejected, reasoning. Each decision cites what it affects. This is unusually thorough for a pre-implementation design.

5. **Phase 1a/1b split (D-011).** Recognizing that the design had grown beyond MVP scope and splitting proactively — before any code — shows good engineering judgment. The rationale ("working software validates design faster than documents") is exactly right.

6. **Newtype pattern (D-017).** Zero-cost type safety for `CanonicalPath`, `ContentHash`, `ClusterId`, `Symbol`. This eliminates an entire category of runtime bugs at zero performance cost.

## Recommendations

### Quick Wins (doc updates only)

1. Fix `ContentHash` example in `architecture.md` graph.json to 16 hex chars
2. Fix `performance.md` Memory Model to use `BTreeMap<CanonicalPath, Node>`
3. Mark D-002 as `Superseded by D-018`
4. Add `detect/` to `pipeline/`'s dependency row in D-023 table
5. Add `hash.rs` and `diagnostic.rs` as rows in D-023 dependency table
6. Update D-008 to say "Phase 1b" instead of "Phase 1"
7. Remove W005 from taxonomy until symlink following is implemented

### Targeted Improvements (localized design changes)

1. **Define `FileSet`, `FileSkipReason`, `WalkConfig`, `BuildOutput`** in `architecture.md` — blocks implementation
2. **Decide `arch_depth` Phase 1a behavior** — add to `architecture.md` Node section and Phase 1a spec
3. **Add TS resolution specifics** — decision tree for import forms → resolution logic
4. **Add layer heuristic table** — path patterns → layer assignment, covering both frontend and backend conventions
5. **Specify `tests` edge and `re_exports` edge inference algorithms** with examples
6. **Document Rust `ImportResolver` contract** — how module paths map to filesystem within the shared trait
7. **Place `ClusterMap → ClusterOutput` conversion** in pipeline, not serial
8. **Place `WorkspaceInfo`** in `model/`

### Strategic Considerations (bigger architectural shifts)

1. **Defer Louvain and Brandes beyond Phase 2** — validate simpler alternatives with real users first
2. **Consider reducing Phase 1a to 3-4 languages** — TS/JS + Go + Python + Rust covers most use cases with lower risk
3. **Add a "Limitations" section to `architecture.md`** — syntactic-only imports, no dynamic requires, no cross-language deps, no JSON file imports
4. **Explicitly exclude `.ariadne/`** from walking to prevent future self-referential parsing
