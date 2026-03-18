# Architecture Review

**Date:** 2026-03-18
**Focus:** Full System
**Mode:** Post-implementation (Phase 1a + 1b complete)
**Reviewed:** architecture.md, ROADMAP.md, decisions/log.md (D-001–D-031), error-handling.md, performance.md, testing.md, path-resolution.md, determinism.md, distribution.md, all src/ modules, all tests/

## Executive Summary

The architecture is sound and well-implemented. Module boundaries are clean, dependency directions are correct, determinism guarantees hold, and the data model is well-suited for Phase 2 extension. The top concerns are: (1) `project_root` in graph.json is an absolute path, breaking cross-machine determinism; (2) walk-level errors bypass the structured diagnostic system; (3) architecture.md has not been updated after Phase 1b changes (ImportResolver signature, DiagnosticCounts fields, layer table expansion); (4) `find_case_insensitive()` is implemented but never called in the resolution pipeline.

## Key Themes

### Theme 1: Design Docs Lag Behind Implementation

Multiple agents independently found that architecture.md, error-handling.md, and determinism.md no longer match the code after Phase 1b. The `ImportResolver::resolve` signature now takes `workspace: Option<&WorkspaceInfo>` but the architecture doc shows the 3-parameter version. `DiagnosticCounts` has 8 fields in code vs 3 in the design. The architectural layer table lists ~20 directory patterns but the code has ~50. The `From<ProjectGraph> for GraphOutput` conversion described in D-022 is implemented as a free function in pipeline/mod.rs.

**Impact:** New contributors reading design docs will get a misleading picture. Phase 2 spec writers may base decisions on stale interfaces.

### Theme 2: Walk Stage Lacks Structured Error Reporting

The `FileWalker` trait returns `Result<Vec<FileEntry>, FatalError>` with no mechanism for per-entry warnings. Walk-level permission errors go to `eprintln!` (walk.rs:88), bypassing `DiagnosticCollector`. The max-files limit silently truncates with no warning. The `.ariadne/` exclusion override failure falls through with an `eprintln!` and no actual fallback. These bypass `--warnings json`, `--strict`, and warning counts.

**Impact:** Machine consumers (CI, `--warnings json`) get incomplete diagnostic information. Users hitting max-files get no indication their graph is partial.

### Theme 3: Absolute `project_root` Breaks Cross-Machine Determinism

`pipeline/mod.rs` line 265 sets `project_root` to `std::fs::canonicalize(root)` — an absolute machine-specific path. When graph.json is committed to git (per D-015), every machine produces a different `project_root`, creating spurious diffs. This directly contradicts D-006 (byte-identical output) for the primary use case.

**Impact:** High — affects the core determinism guarantee that enables git-committed graphs.

### Theme 4: Implemented But Unwired Features

`find_case_insensitive()` in detect/case_sensitivity.rs is implemented and tested but never called from the import resolution pipeline. W007 (PartialParse) is defined in the error taxonomy and has a counter in DiagnosticCounts but is never emitted — the parser always extracts from all subtrees including ERROR nodes. `ImportKind::ModDeclaration` is parsed by the Rust parser but never consumed downstream.

**Impact:** Medium — features documented as working are actually no-ops. Case-insensitive resolution on macOS is the most user-visible gap.

## Detailed Findings

### Foundational Issues

**F1. `project_root` absolute path (HIGH confidence)**
`pipeline/mod.rs:265` — `project_root: project_root.to_string_lossy().to_string()` where `project_root` is the canonicalized absolute path. Should be relative (e.g., `.`) or omitted. Breaks D-006 and D-015.

**F2. `ImportResolver::resolve` signature divergence (HIGH confidence)**
architecture.md shows 3-parameter `resolve()`. Actual trait has 4 parameters (+ workspace). All 6 parsers carry `_workspace: Option<&WorkspaceInfo>` — only TS uses it. The architecture doc's trait definition is the canonical reference for new parser authors.

**F3. `find_case_insensitive` never called (HIGH confidence)**
detect/case_sensitivity.rs implements the function. No call site exists in any resolver or pipeline code. The case-insensitive FS detection (`is_case_insensitive`) is also never called. The path-resolution.md spec for macOS case-insensitive matching is unimplemented.

### Structural Issues

**S1. Walk errors bypass DiagnosticCollector (HIGH confidence)**
walk.rs:88 uses `eprintln!` for walk entry errors. error-handling.md specifies W002. The `FileWalker` trait interface has no mechanism to report per-entry warnings. Fix: pass `&DiagnosticCollector` to walker, or return `(Vec<FileEntry>, Vec<Warning>)`.

**S2. Max-files silent truncation (HIGH confidence)**
walk.rs:113-116 breaks the loop at max_files with no warning. error-handling.md specifies: "Emit a single warning and stop file collection."

**S3. `.ariadne/` exclusion fallback missing (HIGH confidence)**
walk.rs:72-79 — `OverrideBuilder` failure logs to `eprintln!` with a comment "falling back to manual exclusion" but no manual exclusion code follows. If the override fails, `.ariadne/` output files would be parsed as source.

**S4. `detect/workspace.rs` depends on `diagnostic.rs` (MEDIUM confidence)**
D-023 dependency table says `detect/` depends on `model/` only. `detect/workspace.rs` imports `DiagnosticCollector`. The table should be updated or the function signature changed to return errors instead of emitting warnings.

**S5. `project_graph_to_output` placement (MEDIUM confidence)**
D-022 describes `impl From<ProjectGraph> for GraphOutput` in serial/. Actual implementation is a free function in pipeline/mod.rs. Sort-point enforcement happens in build.rs, not in the conversion function.

**S6. HashMap in ParserRegistry (MEDIUM confidence)**
registry.rs uses `HashMap<String, usize>` for `extension_index`. D-006 and determinism.md prescribe "BTreeMap everywhere." The HashMap is lookup-only (no output-affecting iteration), but is an undocumented exception to the blanket rule.

### Surface Issues

**U1. W007 PartialParse never emitted (HIGH confidence)**
registry.rs:parse_source returns `Ok(None)` for >50% errors (→ W001) or `Ok(Some(...))` for ≤50% (no warning). The <50% partial-parse path extracts from all subtrees including ERROR nodes without emitting W007. The warning code exists but is dead.

**U2. Parser construction asymmetry (MEDIUM confidence)**
TS/Python/Rust expose `pub(crate) struct XParser::new()`. Go/C#/Java use factory functions `pub(crate) fn parser()`. Both work but inconsistent patterns confuse new contributors.

**U3. `ImportKind::ModDeclaration` unused downstream (HIGH confidence)**
Defined in traits.rs, set by Rust parser, never consumed in build.rs or anywhere else. Dead variant in a public enum.

**U4. `arch_depth: 0` always emitted in graph.json (MEDIUM confidence)**
Every node has `arch_depth: 0`. Could use `skip_serializing_if` to omit until Phase 2 computes real values. Current output is misleading.

**U5. architecture.md layer table outdated (HIGH confidence)**
Code has ~50 directory patterns (DDD, CQRS, Hexagonal, Angular, SvelteKit, etc.). architecture.md table has ~20. The table is the reference for users — it should match.

**U6. `DiagnosticCounts` design/code divergence (HIGH confidence)**
error-handling.md shows 3 fields. Code has 8 (per-reason breakdown added in Phase 1b review fixes). Doc needs updating.

**U7. Cohesion rounding not enforced at serialization boundary (MEDIUM confidence)**
determinism.md specifies 4 decimal places. cluster/mod.rs rounds correctly, but serial/mod.rs has no enforcement — a future code change to clustering could produce unrounded values that pass through to JSON.

## Discussion Points

### DP1: Should `project_root` be relative or removed?

**Tension:** Useful for consumers to know the scanned directory; but absolute paths break determinism.
**Options:** (a) Use `"."` always, (b) make it relative to the git repo root, (c) use the CLI argument as-is, (d) remove entirely.
**Recommendation:** Option (c) — store the `path` argument from `ariadne build <path>` as-is. Typically `"."` — portable and deterministic.

### DP2: Should case-insensitive resolution be wired up or removed?

**Tension:** Code exists but is never called. macOS users may get different results than Linux users for case-mismatched imports.
**Options:** (a) Wire it into the resolve pipeline now, (b) defer to Phase 2 with a documented limitation, (c) remove dead code.
**Recommendation:** Option (a) — the code and tests already exist. Wire `is_case_insensitive` check into pipeline/mod.rs (cache once per build), pass result to resolvers.

### DP3: Should the `FileWalker` trait accept `DiagnosticCollector`?

**Tension:** Adding it couples the walker to the diagnostic system, reducing testability of the walker in isolation.
**Options:** (a) Pass `&DiagnosticCollector` to `walk()`, (b) return `(Vec<FileEntry>, Vec<Warning>)`, (c) keep `eprintln!` and document it as intentional.
**Recommendation:** Option (b) — return warnings alongside entries. The pipeline converts them to `DiagnosticCollector` calls. Walker stays decoupled.

## Strengths

- **BTreeMap-everywhere determinism** — D-006 is well-implemented and well-tested (INV-11). The commitment to byte-identical output is genuine and valuable.
- **CanonicalPath newtype with construction-time normalization** — Eliminates an entire class of path-related bugs. Well-tested with 19 unit tests and proptest properties.
- **Composition Root pattern** — main.rs is the sole wiring point. Library code is cleanly separated.
- **Atomic writes** — serial/json.rs correctly sequences tmp-file → flush → rename.
- **DiagnosticCollector** — Thread-safe, sorted output, per-reason breakdown. The single Mutex<(Vec, Counts)> is a better design than the two-Mutex approach in the docs.
- **Workspace detection** — npm/yarn/pnpm support with manual YAML parsing (no serde_yaml dep), glob expansion, entry point probing (D-027), name collision handling (D-029). Robust implementation.
- **Layer detection expansion** — Covering 12+ architectural patterns (DDD, Clean, Hexagonal, CQRS, MVVM, Angular, etc.) with simple pattern matching is high value for low complexity.
- **Snapshot-based testing** — insta fixtures catch any behavioral regression. The 134-test suite provides strong coverage.

## Recommendations

### Quick Wins (doc updates only)

1. **Update architecture.md ImportResolver signature** to show 4-parameter version with workspace
2. **Update architecture.md layer table** to reflect current ~50 patterns in layer.rs
3. **Update error-handling.md DiagnosticCounts** to show 8-field version
4. **Update error-handling.md DiagnosticCollector** to describe single Mutex (not two)
5. **Add note to D-023** that `detect/` may depend on `diagnostic.rs`
6. **Document HashMap exception** in determinism.md for ParserRegistry.extension_index

### Targeted Improvements (localized code changes)

7. **Fix `project_root`** — store CLI argument as-is instead of canonicalized absolute path
8. **Wire case-insensitive resolution** — call `is_case_insensitive` once per build, pass to resolvers
9. **Add max-files warning** — emit W003-like warning when walk truncates at limit
10. **Fix walk error reporting** — return `(Vec<FileEntry>, Vec<Warning>)` from FileWalker::walk
11. **Add `.ariadne/` manual exclusion fallback** when OverrideBuilder fails
12. **Emit W007** — detect partial parses in registry.rs and emit warning

### Strategic Considerations (bigger changes for Phase 2)

13. **Consider removing workspace param from ImportResolver trait** — pass workspace only to TS resolver via a wrapper or separate dispatch. Keeps the core trait clean for Tier 2 parsers.
14. **Consider standardizing parser construction** — factory function pattern for all 6 parsers
15. **Consider skip-serializing arch_depth** until Phase 2 computes real values
