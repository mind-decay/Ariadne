# Architecture Review

**Date:** 2026-03-18
**Focus:** Full System
**Mode:** Post-implementation
**Reviewed:** design/architecture.md, design/ROADMAP.md, design/decisions/log.md (D-001–D-025), design/error-handling.md, design/performance.md, design/testing.md, design/path-resolution.md, design/determinism.md, design/distribution.md, all source files in src/

## Executive Summary

Ariadne's architecture is well-structured at the module level — dependency direction is correct throughout, the layered module design is clean, and the core data model (newtypes, BTreeMap determinism) is sound. However, the review uncovered **one likely bug** (walk.rs override loop replaces itself, potentially failing to exclude `.ariadne/`), **one production panic** (`expect()` on grammar version mismatch), and **significant resolver gaps** (Go produces zero edges for real projects; C#/Java produce sparse results). There is meaningful divergence between design documents and implementation in several areas (conversion location, DiagnosticCollector ownership, error-handling stage model). The complexity budget is mostly proportional to the problem, with some premature abstractions (dual output model, trait-injected pipeline stages without mock tests).

## Key Themes

### Theme 1: Resolver Correctness Gap

Three of six language resolvers have fundamental limitations that are inadequately documented:

- **Go:** `GoResolver::find_module_path()` (`go.rs:121-133`) explicitly returns `None` because `FileSet` contains only paths, not file contents. Every Go import containing a dot (i.e., every real Go module import like `github.com/org/repo/internal/handler`) is classified as external and dropped. **Zero inter-package edges for any standard Go project.**
- **C#:** Namespace-to-path heuristic (`csharp.rs:199-234`) converts `using MyApp.Services` to filesystem paths. C# namespaces do not map to filesystem paths — a file in `src/auth/` can declare `namespace MyApp.Services`. Near-zero resolution for real projects.
- **Java:** Same namespace-to-path heuristic (`java.rs:154-222`), but Java packages do map to paths under `src/main/java/`. Works better than C# but the prefix is hardcoded.

The architecture doc presents all 6 parsers as Tier 1 without distinguishing resolution quality. The parsers (import extraction) work correctly — the problem is purely in resolution.

**Contributing findings:** Agent 1 D-3, Agent 3 §5 (C#/Java complexity), Agent 4 §6.1/6.2

### Theme 2: Design-Code Divergence

Multiple design documents describe structures that differ from implementation:

| Design says                                                             | Code does                                                                               | Impact                                  |
| ----------------------------------------------------------------------- | --------------------------------------------------------------------------------------- | --------------------------------------- |
| `impl From<ProjectGraph> for GraphOutput` (D-022, determinism.md)       | Free functions `project_graph_to_output` / `cluster_map_to_output` in `pipeline/mod.rs` | Sort-point guarantee harder to audit    |
| `DiagnosticCollector` stored as `BuildPipeline` field (architecture.md) | Scoped to `run_with_output` call, not a struct field                                    | Minor — code is actually better         |
| `Mutex<Vec<Warning>>` singular (D-021)                                  | Two separate `Mutex` instances (warnings + counts)                                      | Double-lock pattern undocumented        |
| Error-handling.md Stage 1 includes file reading                         | Walk and read are separate pipeline stages                                              | Misleading mental model                 |
| `arch_depth: 2` in graph.json example                                   | Phase 1a always outputs `arch_depth: 0` (D-025)                                         | Direct contradiction in same doc        |
| `--output` flag is functional (architecture.md CLI)                     | `output: _` silently discarded in `main.rs:36`                                          | Users get no error when flag is ignored |
| `serial/` depends on `model/` only (dependency table)                   | `serial/mod.rs` imports `diagnostic::FatalError`                                        | Undocumented dependency                 |

**Contributing findings:** Agent 1 L-1/L-2, Agent 1 D-1, Agent 2 §1 (missing decisions), Agent 2 §2 (inconsistencies 1-9)

### Theme 3: Pipeline Robustness Issues

Several bugs and safety gaps in the pipeline:

- **Override builder loop bug** (`walk.rs:67-74`): Each iteration creates a new `OverrideBuilder` and calls `walker.overrides()`, which **replaces** the previous override. Only the last directory in `exclude_dirs` is actually excluded. If `.ariadne` is not the last entry, it won't be excluded, causing Ariadne's own output to be scanned as source.
- **`expect()` panic** (`registry.rs:88`): `ts_parser.set_language(...).expect(...)` panics in production code path. Grammar ABI version mismatch (from incompatible crate versions) would crash the binary instead of producing a `FatalError`.
- **Walk errors silently dropped** (`walk.rs:80-82`): `Err(_) => continue` discards the error with no W002 warning. Directories that can't be read produce no warning.
- **Missing binary detection** (`read.rs:62`): No null-byte scan as specified in `error-handling.md`. Binary files that happen to be valid UTF-8 are passed to tree-sitter.
- **Self-imports not filtered**: No check prevents a file from creating an edge to itself, violating INV-2.
- **W007 PartialParse unreachable**: The pipeline's `Option` return from `parse_source` cannot express "partial success with warnings." W007 is defined but dead code.

**Contributing findings:** Agent 4 §1.1 (all gaps), Agent 4 §5.1/5.2

### Theme 4: Complexity vs. Value

Several design choices add complexity without proportional current benefit:

- **Dual output model** (D-022): `NodeOutput` is structurally identical to `Node` with strings instead of newtypes. Since newtypes already implement `Serialize`, the entire parallel type hierarchy and manual conversion could be replaced with serde attributes on the internal types. The compact tuple edge format is the one genuinely useful difference.
- **Trait-injected pipeline stages** (D-019): `FileWalker`, `FileReader`, `GraphSerializer` traits exist for testability, but no mock implementations or mock-based tests exist. The test suite uses fixture-based end-to-end tests. The abstraction is correct but premature.
- **`Symbol` and `ClusterId` newtypes**: Unlike `CanonicalPath` (which enforces normalization) and `ContentHash` (which enforces format), these accept any string with no invariant enforcement. They add boilerplate for zero safety benefit.
- **`DiagnosticCounts` separate struct**: Derivable from `Vec<Warning>` at drain time. The separate `Mutex` adds lock contention for a micro-optimization.

**Contributing findings:** Agent 3 §2-4 (over-engineering), Agent 1 A-4/A-5

## Detailed Findings

### Foundational Issues

**F-1: Go resolver produces zero edges for real Go projects**

- Files: `src/parser/go.rs:121-133`, `src/parser/go.rs:172`
- Every import containing a dot is classified as external. All non-stdlib Go imports are unresolved. The `FileSet` abstraction provides paths but not file contents, so `go.mod` cannot be read during resolution.
- Direction: Either read `go.mod` during a pre-build config scan phase and pass module info to resolvers, or document Go as "stdlib-only resolution" in Phase 1a.

**F-2: Override builder loop replaces previous overrides (likely bug)**

- File: `src/pipeline/walk.rs:67-74`
- `walker.overrides(overrides)` replaces any previously set overrides. With `exclude_dirs = [".ariadne"]` (a single entry), this works correctly. But the loop structure would break if additional exclude dirs are ever added. Additionally, lines 92-100 apply a second manual check for excluded directories, suggesting the override approach was not confirmed working.
- Direction: Build all excludes in a single `OverrideBuilder` before calling `walker.overrides()` once. Remove the redundant manual check.

**F-3: `expect()` panic in production code path**

- File: `src/parser/registry.rs:88`
- Grammar ABI version mismatch panics the binary. In a rayon parallel context, this panics a worker thread. If a `DiagnosticCollector` mutex is held at panic time, subsequent `unwrap()` calls on the poisoned mutex will cascade.
- Direction: Replace with `map_err` returning a `FatalError`, or add a startup self-test that validates all grammars before the pipeline runs.

**F-4: `RawImport.path` carries Rust-specific sentinel encoding**

- File: `src/parser/rust_lang.rs:303`
- `mod` declarations produce `RawImport { path: "mod::auth", ... }` — a language-specific encoding in a shared type. Only `RustResolver` knows to check for `mod::` prefix. Any code that inspects `RawImport.path` for logging or display will see invalid paths.
- Direction: Add an `ImportKind` discriminant to `RawImport`, or resolve `mod` declarations to filesystem paths before entering the `RawImport` stream.

**F-5: `project_root` serialized as absolute machine-specific path**

- File: `src/pipeline/mod.rs:204`
- D-015 says graph output is designed to be committed to git for portability. An absolute path in a committed file is machine-specific, undermining portability.
- Direction: Document the tension. Consider a placeholder or relative sentinel.

### Structural Issues

**S-1: Conversion functions live in `pipeline/mod.rs`, not at the output type boundary**

- Files: `src/pipeline/mod.rs:173-230`, design D-022
- D-022 specifies `impl From<ProjectGraph> for GraphOutput` as the single conversion point. The implementation uses free functions in `pipeline/`, making sort-point enforcement harder to audit.
- Direction: Move conversion to `impl From` on the output types, or update D-022 to reflect the actual design.

**S-2: `pipeline/build.rs` embeds language-specific test patterns**

- File: `src/pipeline/build.rs:177-187`
- `infer_test_edges_by_naming` hardcodes language-specific patterns (`.test.ts`, `_test.go`, `_test.py`) in language-agnostic pipeline code. This is a leaky abstraction — the patterns belong in `detect/` or `parser/`.
- Direction: Move naming patterns to `detect/filetype.rs` or expose them via `LanguageParser`.

**S-3: Cluster naming hardcodes `src/` convention**

- File: `src/cluster/mod.rs:93-114`
- `extract_cluster_name` skips `src/` as a prefix. For Go projects (`cmd/`, `pkg/`, `internal/`), Java projects (`src/main/java/com/example/`), or flat Python projects, the first path segment becomes the cluster name, producing meaningless clusters.
- Direction: Document the `src/`-centric assumption. Consider language-aware prefix stripping.

**S-4: Three inconsistent parser registration patterns**

- Files: `src/parser/typescript.rs` (named struct + `new()`), `src/parser/go.rs` (private struct + factory fn), `src/parser/registry.rs:71-73` (calls both patterns)
- No standard for new contributors adding Tier 2 parsers.
- Direction: Standardize on one pattern across all parsers.

**S-5: `ArchLayer::Config` naming collision with `FileType::Config`**

- File: `src/model/node.rs`
- Orthogonal concepts share the same name. `FileType::Config` = build configuration files (Cargo.toml). `ArchLayer::Config` = files in `config/` directories. The naming invites confusion.
- Direction: Rename one (e.g., `ArchLayer::Configuration` or `FileType::BuildConfig`).

**S-6: `--output` flag accepted but silently ignored**

- File: `src/main.rs:36`
- `output: _` discards the value. Users get no error or warning.
- Direction: Wire the flag to `run_with_output`, or remove it from the CLI until it's implemented.

### Surface Issues

**U-1: `Node.cluster` initialized to empty string sentinel**

- File: `src/pipeline/build.rs:39`
- `ClusterId::new("")` is a temporary invalid state patched later. `Option<ClusterId>` would make the temporariness explicit.

**U-2: `CanonicalPath::extension()` mishandles leading-dot files**

- File: `src/model/types.rs:51`
- `rsplit_once('.')` on `.gitignore` returns `"gitignore"` as extension. Currently harmless due to walk filtering, but imprecise.

**U-3: C# and Java parsers use raw text parsing instead of AST traversal**

- Files: `src/parser/csharp.rs:79-126`, `src/parser/java.rs:34-50`
- `extract_using_directive` and `extract_imports` get the node's raw text and manually strip prefixes. Fragile against unusual formatting.

**U-4: Python `TYPE_CHECKING` detection is brittle**

- File: `src/parser/python.rs:337-343`
- Only handles simple `if TYPE_CHECKING:` conditions. Misses compound conditions (`if TYPE_CHECKING and ...`).

**U-5: Mixed AST traversal patterns (index vs iterator) across parsers**

- Go/Java use `child(i)` index traversal; TypeScript/Python/Rust use `children(&mut cursor)` iterator. Inconsistency has no functional impact but reduces readability.

**U-6: `DiagnosticCollector::warn` acquires two separate mutexes**

- File: `src/diagnostic.rs:88-108`
- Two sequential lock acquisitions where one would suffice. Could combine into `Mutex<(Vec<Warning>, DiagnosticCounts)>`.

**U-7: `test_*.py` prefix pattern not handled by naming-convention test inference**

- File: `src/pipeline/build.rs:187`
- Only suffix-based patterns are implemented. Python's `test_auth.py → auth.py` prefix pattern is in the architecture doc but not in the code.

**U-8: O(n\*e) duplicate check in test edge inference**

- File: `src/pipeline/build.rs:204-209`
- `edges.iter().any(...)` is O(e) per test file. Use a `HashSet` for O(1) existence checks.

**U-9: O(n) file scan in Go/C#/Java resolvers**

- Files: `src/parser/go.rs:191-199`, `csharp.rs:222-229`, `java.rs:170-188`
- Iterates all `known_files` for each import. Pre-build a directory prefix map for O(1) lookups.

**U-10: W005 warning code is missing**

- Files: `design/error-handling.md`, `src/diagnostic.rs`
- Warning codes jump from W004 to W006. Unexplained gap.

**U-11: Memory estimate discrepancy (250MB vs 400MB for 50k files)**

- File: `design/performance.md`
- Scaling table says 400MB; Memory Estimates table says 250MB. No explanation.

**U-12: Stale `.tmp` files accumulate on write failure**

- File: `src/serial/json.rs:47`
- Failed writes leave `.tmp` files. No cleanup in error path.

## Discussion Points

### 1. Should Go/C#/Java be demoted from "fully functional Tier 1" to "parsing only, limited resolution"?

**Tension:** The ROADMAP presents all 6 languages as Phase 1a Tier 1. In practice, Go produces zero edges for real projects, and C#/Java produce sparse results. The parsers work correctly — the gap is in resolution.

**Arguments for demotion:** Prevents false confidence. Users running Ariadne on a Go project get an empty graph and may distrust the tool entirely. Documenting "parser-only, no cross-package resolution" sets correct expectations.

**Arguments against:** Having 6 languages demonstrates breadth. Go resolution can be fixed in Phase 1b when config reading is available. Shipping with limitations is better than not shipping.

**Recommended direction:** Keep all 6 parsers, but add prominent warnings when a language's resolver is limited. Print "Go: stdlib resolution only (go.mod reading deferred to Phase 1b)" in the build summary.

### 2. Should the dual output model (`NodeOutput` / `GraphOutput`) be simplified?

**Tension:** D-022 justifies the split for separation of concerns. In practice, `NodeOutput` is structurally identical to `Node` with strings instead of newtypes, and `Node`'s newtypes already implement `Serialize`.

**Arguments for simplification:** Eliminate ~60 lines of manual conversion code. Reduce maintenance surface. The compact tuple edge format (the genuine value-add) can be achieved with a custom `Serialize` impl on `Edge`.

**Arguments against:** Adding a new output format (YAML, binary) in the future would benefit from a separate output type. The current design is correct even if premature.

**Recommended direction:** Simplify for Phase 1a. The abstraction can be re-introduced if a second output format materializes.

### 3. How should the `arch_depth` placeholder be handled?

**Tension:** D-025 says output `0` for all nodes. The graph.json example shows `2`. Consumers may write code assuming non-zero depth values.

**Arguments for keeping the field:** Schema stability — adding it later is also a large diff.

**Arguments for omitting:** Including always-`0` data erodes output trust. `#[serde(skip_serializing_if = "is_zero")]` would omit it cleanly.

**Recommended direction:** Fix the graph.json example to show `0`. Add a note in the output schema: "arch_depth is a Phase 2 field; always 0 in Phase 1."

### 4. Should pipeline stage traits be deferred to Phase 1b?

**Tension:** `FileWalker`/`FileReader`/`GraphSerializer` traits exist for testability, but no mock tests exist. The abstraction is prepayment for infrastructure that isn't built yet.

**Arguments for keeping:** The traits are already written and working. Removing them adds churn for no behavioral benefit. They'll be needed in Phase 1b anyway.

**Arguments for deferring:** Simplifies the composition root. Reduces indirection for developers reading the code.

**Recommended direction:** Keep. The cost of removal exceeds the cost of keeping them, and they'll be needed soon.

## Strengths

**Clean module dependency direction.** Every `use` statement in the codebase confirms the intended dependency rules. `model/` is a true leaf. No circular imports. No unexpected cross-module dependencies. This is hard to get right and easy to break — the codebase gets it right.

**Newtypes for domain primitives.** `CanonicalPath` with normalization at construction, `ContentHash` with format enforcement — these eliminate categories of bugs at compile time. The normalization logic in `CanonicalPath::normalize()` is correct and thorough.

**Determinism by construction.** `BTreeMap` for all collections, explicit edge sorting, no timestamp by default, sorted exports and symbols. The determinism strategy is well-thought-out and consistently applied. The one rayon `filter_map` concern is mitigated by the final edge sort.

**Well-structured decision log.** D-001 through D-025 form a coherent narrative. Each decision cites alternatives, rationale, and affected documents. The supersession chain (D-002 → D-018) is tracked. This is unusually thorough for a project of this size.

**TypeScript/JavaScript parser quality.** The most complex parser handles `import`, `require()`, dynamic `import()`, `export`, re-exports, `import type`, and barrel patterns. The implementation uses structured AST traversal throughout and handles edge cases well.

**Graceful degradation philosophy.** The error model (fatal stops pipeline, warnings skip-and-continue) is sound. The W001-W009 taxonomy covers the important cases. The `DiagnosticCollector` pattern for thread-safe warning aggregation is correct.

## Recommendations

### Quick Wins (doc updates only)

1. Fix `arch_depth: 2` in architecture.md graph.json example to `0` (contradicts D-025)
2. Update architecture.md dependency table: `serial/` depends on `model/` + `diagnostic.rs` (for `FatalError`)
3. Update architecture.md to show `DiagnosticCollector` scoped to `run_with_output`, not a `BuildPipeline` field
4. Update error-handling.md Stage 1 pseudocode to separate walk and read phases
5. Assign or document the W005 gap in the warning code sequence
6. Reconcile memory estimates in performance.md (250MB vs 400MB)
7. Add a note to architecture.md: Go resolution is stdlib-only in Phase 1a; C#/Java use namespace-to-path heuristic with known limitations
8. Document the `src/`-centric assumption in cluster naming

### Targeted Improvements (localized design changes)

1. **Fix the override builder loop** (`walk.rs:67-74`): Build all excludes in a single `OverrideBuilder`. Remove redundant manual path component check.
2. **Replace `expect()` with `FatalError`** (`registry.rs:88`): Grammar version mismatch should produce a structured error, not a panic.
3. **Wire or remove `--output` flag** (`main.rs:36`): Either pass the value to `run_with_output`, or remove the flag from the CLI definition.
4. **Add `ImportKind` to `RawImport`**: Replace the `mod::` sentinel string encoding with a proper discriminant.
5. **Move test naming patterns out of `build.rs`**: Into `detect/` or expose via `LanguageParser` trait.
6. **Add `test_*.py` prefix pattern**: The naming-convention inference handles suffixes but not Python's common `test_` prefix.
7. **Standardize parser registration pattern**: Pick one (named struct + `new()` or factory function) and apply uniformly.

### Strategic Considerations (bigger architectural shifts)

1. **Go module path pre-reading**: Introduce a config-scan phase before resolution that reads `go.mod` (and potentially `tsconfig.json`, `Cargo.toml`) and passes language config to resolvers. This is architecturally blocked by `FileSet` containing only paths. Phase 1b dependency.
2. **Simplify output model**: Consider eliminating `NodeOutput` in favor of serde attributes on `Node`. Keep `GraphOutput` wrapper for version/counts/edges tuple format. Reduces maintenance surface.
3. **Memory optimization for large projects**: `file_contents` retains all file bytes through the build stage. Consider separating `lines`/`hash` metadata from raw bytes so bytes can be freed after parsing.
4. **Grammar version drift detection**: Add per-language integration tests that assert expected node kinds from a known input. Silent grammar updates that rename node kinds would break parsers without compile errors.
