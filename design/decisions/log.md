# Decision Log

All architectural decisions made during Ariadne development.

---

## D-001: Structural Topology via Rust CLI + Tree-sitter

**Date:** 2026-03-17
**Status:** Accepted
**Context:** Need a tool that builds structural dependency graphs from source code. Must work across languages, be fast, produce deterministic output, and have zero runtime dependencies.
**Decision:** Standalone Rust binary using tree-sitter for AST-based parsing. Single binary distribution. JSON output format. No LLM involvement — purely deterministic static analysis.
**Alternatives rejected:**
- Agent-driven parsing (LLM reads files) — expensive in tokens, non-deterministic, slow on large projects
- Regex-based parsing — fragile, doesn't scale across languages
- Language-specific tools (tsc, go vet, etc.) — requires each tool installed, inconsistent output formats
- SQLite storage — unnecessary for deterministic data that doesn't need complex queries beyond what JSON provides
**Reasoning:** Tree-sitter provides accurate AST-based parsing for 100+ languages with zero token cost. Rust gives single-binary distribution with fastest execution (3000 files in under 10 seconds).

## D-002: Language Support via Tree-sitter Trait

**Date:** 2026-03-17
**Status:** Superseded by D-018
**Context:** Must work across any tech stack. Need extensible language support without per-language complexity explosion.
**Decision:** Each language implements a `LanguageParser` trait (extensions, tree-sitter grammar, import/export extraction, path resolution). Tier 1 (initial): TypeScript/JavaScript, Go, Python, Rust, C#, Java. Tier 2 (future): Kotlin, Swift, C/C++, PHP, Ruby, Dart. Adding a language = implementing one trait + adding grammar crate dependency.
**Alternatives rejected:**
- Universal regex parser — too fragile for edge cases
- Single monolithic parser — violates extensibility, hard to test per-language
**Reasoning:** Trait-based design keeps each language isolated and testable. 6 Tier 1 languages cover ~85% of projects.

## D-003: Graceful Degradation

**Date:** 2026-03-17
**Status:** Accepted
**Context:** Files may fail to parse (syntax errors, unsupported features). The tool should still produce useful output.
**Decision:** Unparseable files are logged to stderr and excluded from the graph. Exit code remains 0 (success with warnings). The graph is useful with some files missing. Fatal errors (no parseable files, invalid project root) exit with code 1.
**Alternatives rejected:**
- Fail on first error — breaks usability on real projects with occasional syntax issues
- Silent skip — violates transparency (user doesn't know files were missed)
**Reasoning:** Real projects have files that don't parse cleanly. The graph should be best-effort, with transparent reporting of what was skipped.

## D-004: Separate Project from Moira

**Date:** 2026-03-17
**Status:** Accepted
**Context:** Ariadne was originally designed as `moira-graph`, a component of the Moira orchestration framework. However, the tool has no dependency on Moira's infrastructure and uses a completely different tech stack (Rust vs shell/markdown).
**Decision:** Ariadne is a standalone project with its own repository, CI/CD, versioning, and release cycle. Moira integration (Phase 15 in Moira's roadmap) happens on Moira's side — shell wrappers invoke the `ariadne` CLI. Ariadne has zero knowledge of Moira.
**Alternatives rejected:**
- Subdirectory in Moira repo — GitHub Actions doesn't work from nested `.github/`, `cargo install` doesn't work from subdirectory, Rust toolchain not needed for core Moira
- Git submodule — worst of both approaches
**Reasoning:** Clean separation enables: standard `cargo install ariadne`, native CI/CD, independent releases. The tool is useful beyond Moira — any system that needs structural code analysis can use it.

## D-005: Error Handling Strategy — Best-Effort with Structured Warnings

**Date:** 2026-03-17
**Status:** Accepted
**Context:** Source projects contain broken files, binary files, permission-restricted files, huge generated bundles, non-UTF-8 content, and other edge cases. The tool must handle all of these without crashing.
**Decision:** Two-tier error model: Fatal errors (E001-E005) stop execution immediately (exit 1). Recoverable errors (W001-W009) skip the affected file and emit a structured warning to stderr. Warnings have codes (W001-W009) and support human and JSON output formats. Resource limits (max file size 1MB, max files 50k) prevent memory exhaustion. Partial tree-sitter parses extract from valid subtrees. Output files are written atomically (temp + rename). `--strict` flag makes warnings fatal for CI use.
**Alternatives rejected:**
- Fail on any error — unusable on real projects
- Silent skip — user doesn't know what's missing from the graph
- Log file — adds complexity, stderr is standard for warnings
**Reasoning:** Real projects are messy. Best-effort graph with transparent warnings gives the most value. Structured warning codes enable machine consumption. Resource limits prevent pathological inputs from causing OOM. Atomic writes prevent corruption on interruption.

## D-006: Byte-Identical Deterministic Output

**Date:** 2026-03-17
**Status:** Accepted
**Context:** Graph output is designed to be committed to git. Non-deterministic output (HashMap iteration order, rayon parallel collection order, timestamps) would produce meaningless diffs on every build.
**Decision:** Same input → byte-identical output. Use `BTreeMap` instead of `HashMap` for nodes and clusters (sorted keys). Sort edges by (from, to, edge_type) before serialization. Sort all internal lists (exports, symbols, cluster files). Remove `"generated"` timestamp from default output (add via `--timestamp` flag). Rayon processes files in sorted order to maintain deterministic collection.
**Alternatives rejected:**
- HashMap + post-hoc sorting — extra allocation, error-prone (easy to forget a sort point)
- Set comparison instead of byte identity — doesn't help git diffs, still produces noisy commits
- Keep timestamp, ignore in diffs — requires custom git diff driver, adds complexity
**Reasoning:** BTreeMap has O(log n) vs O(1) lookup — ~20% overhead on build phase, negligible compared to parsing. Byte-identical output is the strongest guarantee and the simplest to verify. See `design/determinism.md`.

## D-007: Path Normalization — Canonical Relative Format

**Date:** 2026-03-17
**Status:** Accepted
**Context:** The same file can be referenced via different path strings (./foo, foo, ./bar/../foo). Without normalization, the graph may have duplicate nodes or missing edges. Case-insensitive filesystems (macOS default) add another dimension.
**Decision:** All paths in the graph use a canonical format: relative to project root, forward slashes, no `./` prefix, no `..` segments, no trailing slash. Case sensitivity follows the filesystem — on case-insensitive FS, import resolution tries case-insensitive matching but stores the canonical (filesystem-reported) path. Path traversal outside project root is rejected (security). `dunce` crate used on Windows to avoid `\\?\` prefix.
**Alternatives rejected:**
- Lowercase all paths — loses information, confusing when viewing graph data
- Absolute paths — ties graph to machine, breaks portability
- No normalization — duplicate nodes, broken edge matching
**Reasoning:** Canonical relative paths are portable, deterministic, and human-readable. Following filesystem case behavior matches developer expectations (code that works on macOS but not Linux is a real bug to surface). See `design/path-resolution.md`.

## D-008: Monorepo — Single Graph with Workspace-Aware Resolution

**Date:** 2026-03-17
**Status:** Accepted
**Context:** Many real-world repositories are monorepos with multiple package.json, go.mod, or Cargo.toml files. Without workspace awareness, cross-package imports are classified as "external" and produce no edges — losing critical dependency information.
**Decision:** Ariadne always produces one graph per invocation (entire repo). Workspace detection scans for root-level indicators (package.json workspaces, go.work, Cargo.toml workspace, nx.json, pnpm-workspace.yaml). Detected workspace members are mapped (package name → path → entry point). Import resolution checks workspace map before classifying an import as external. Phase 1b covers npm/yarn/pnpm workspaces; other workspace types added incrementally. No workspace indicators → simple single-project resolution (fully backward compatible).
**Alternatives rejected:**
- Per-package sub-graphs — loses cross-package dependency information, the main value proposition
- Require explicit configuration — too much friction, most workspaces are auto-detectable
- Ignore workspaces — silently broken graphs on the most common project structures
**Reasoning:** Single graph captures the full dependency picture. Workspace detection is heuristic but high-confidence (workspace config files are standard). Additive — doesn't change behavior for non-workspace projects. See `design/path-resolution.md`.

## D-009: MIT/Apache-2.0 Dual License

**Date:** 2026-03-17
**Status:** Accepted
**Context:** Ariadne needs a license for open-source release and crates.io publishing. License choice affects adoption and compatibility with downstream projects.
**Decision:** Dual-licensed under MIT and Apache-2.0, following the Rust ecosystem convention. Users may choose either license. This is the same license model used by Rust itself, serde, tokio, clap, and most Rust ecosystem crates.
**Alternatives rejected:**
- MIT only — less protection for contributors (no patent grant)
- Apache-2.0 only — incompatible with some GPL projects
- GPL — too restrictive for a CLI tool meant to be widely adopted
**Reasoning:** MIT/Apache-2.0 dual license maximizes compatibility. MIT is simple and permissive. Apache-2.0 adds patent protection. Dual licensing lets users pick what works for their project. This is the de-facto standard in the Rust ecosystem.

## D-010: Crate Name `ariadne-graph`

**Date:** 2026-03-17
**Status:** Accepted
**Context:** The name `ariadne` is taken on crates.io by a popular error reporting library. Need an alternative crate name that's available while keeping `ariadne` as the CLI binary name.
**Decision:** Crate name `ariadne-graph` on crates.io. Binary name remains `ariadne` via `[[bin]]` in Cargo.toml. Users install with `cargo install ariadne-graph`, the installed binary is called `ariadne`.
**Alternatives rejected:**
- `ariadne-cli` — too generic, doesn't describe the tool
- `ariadne-deps` — too narrow ("deps" implies only dependencies)
- Fight for the `ariadne` name — not worth the effort, existing crate is well-established
**Reasoning:** `ariadne-graph` is descriptive and unambiguous. The `[[bin]]` mechanism is standard Rust practice (used by ripgrep/rg, fd-find/fd, etc.).

## D-011: Phase Split — MVP First, Hardening Second

**Date:** 2026-03-17
**Status:** Accepted
**Context:** Phase 1 grew from a simple MVP to a production-hardened tool during design (workspace detection, structured warnings, 7 CLI flags, atomic writes, case sensitivity, install script). Risk of over-engineering before any code exists.
**Decision:** Split Phase 1 into Phase 1a (MVP) and Phase 1b (Hardening). Phase 1a delivers a working tool: parsers, graph builder, JSON output, basic CLI, basic tests. Phase 1b adds: structured warning system, all CLI flags, workspace detection, path normalization, atomic writes, full test suite, CI/CD. This gets code running faster and validates the design with real usage.
**Alternatives rejected:**
- Ship everything at once — high risk of spending weeks on features that need redesign after first real usage
- Skip hardening entirely — too fragile for real projects
**Reasoning:** Working software validates design faster than documents. Phase 1a provides the feedback loop. Phase 1b builds on proven foundation.

## D-012: Compact Tuple Format for Edges

**Date:** 2026-03-17
**Status:** Accepted
**Context:** Graph edges need efficient serialization. Object-based edge representation (`{"from": "...", "to": "...", "type": "...", "symbols": [...]}`) is verbose and dominates file size in large graphs.
**Decision:** Edges in graph.json are serialized as compact JSON tuples `[from, to, type, [symbols]]` instead of objects. 60%+ space savings. Schema consumers must know the positional format.
**Alternatives rejected:**
- Object-based edges — readable but wasteful, 60%+ larger output
- Binary format — not human-readable, not diffable in git
**Reasoning:** Edges are the largest part of the graph by count. Compact tuples dramatically reduce output size while remaining valid JSON. The positional format is documented in the schema.
**Affects:** architecture.md Storage Format.

## D-013: xxHash64 for Content Hashing

**Date:** 2026-03-17
**Status:** Accepted
**Context:** Each file node includes a content hash for change detection. The hash algorithm must be fast, collision-resistant, and produce deterministic output across platforms.
**Decision:** Content hashes use xxHash64 (fast, collision-resistant, deterministic). Output as lowercase hex (16 characters).
**Alternatives rejected:**
- SHA-256 — cryptographic strength unnecessary, significantly slower
- CRC32 — too short, higher collision probability
- No hashing — no way to detect file changes without re-parsing
**Reasoning:** xxHash64 is one of the fastest non-cryptographic hash functions with excellent distribution. 64-bit output provides sufficient collision resistance for file-level change detection. Deterministic across platforms. See performance.md.
**Affects:** architecture.md Graph Data Model, performance.md.

## D-014: Layer Detection Heuristics

**Date:** 2026-03-17
**Status:** Accepted
**Context:** Ariadne infers architectural layers to enrich the dependency graph beyond raw file-to-file edges. Layer membership must be determined without configuration.
**Decision:** Eight architectural layers: api, service, data, util, component, hook, config, unknown. Layer membership inferred from file path patterns and naming conventions. The "component" and "hook" layers reflect React/frontend conventions.
**Alternatives rejected:**
- User-defined layers only — too much friction, no zero-config experience
- Fewer layers — loses useful distinctions (e.g., hook vs component)
- More layers — diminishing returns, harder to maintain heuristics
**Reasoning:** Path-based heuristics are surprisingly accurate for common project structures. Eight layers cover the most common architectural patterns across frontend and backend. "unknown" provides a safe fallback. Heuristics can be refined incrementally.
**Affects:** architecture.md Language Support / Layer Inference.

## D-015: Graph Output Committed to Git

**Date:** 2026-03-17
**Status:** Accepted
**Context:** Graph output needs a consumption strategy. Options include generating on-the-fly, caching locally, or committing to version control.
**Decision:** `.ariadne/graph/` output (graph.json, clusters.json) is designed to be committed to version control. D-006 (byte-identical output) is a prerequisite — deterministic output enables meaningful diffs.
**Alternatives rejected:**
- Generate on-the-fly only — requires Ariadne installed everywhere, no historical tracking
- Local cache (gitignored) — no shared access, no change tracking over time
**Reasoning:** Committing graph output enables: tracking structural changes over time via git history, CI diffing to detect unintended dependency changes, consumption by tools that don't have Ariadne installed. D-006 ensures commits only appear when the graph actually changes.
**Affects:** architecture.md Storage Format, determinism.md.

## D-016: Default Output Directory `.ariadne/graph/`

**Date:** 2026-03-17
**Status:** Accepted
**Context:** Graph output files need a default location. The directory must be predictable, not conflict with existing conventions, and support future Ariadne outputs beyond the graph.
**Decision:** Graph output files go to `.ariadne/graph/` by default. The parent `.ariadne/` directory serves as the namespace for all Ariadne outputs (graph, views, stats in later phases).
**Alternatives rejected:**
- Project root — clutters the top-level directory
- `.ariadne/` directly — no room for future output types (views, stats)
- `ariadne-output/` — visible directory is noisier, dot-prefix follows convention (.git, .github, .vscode)
**Reasoning:** Dot-prefix follows established tool conventions. Nested `graph/` subdirectory provides namespace for future output types without reorganization. Overridable via `--output` flag.
**Affects:** architecture.md Storage Format, Phase 1a spec.

## D-017: Newtype Pattern for Domain Primitives

**Date:** 2026-03-18
**Status:** Accepted
**Context:** The data model uses raw `String` for file paths, content hashes, cluster IDs, and symbol names. This allows mixing up semantically different values at function boundaries — passing a raw path where a canonical path is expected, or a hash where a cluster ID is expected. Normalization logic (path canonicalization, hash formatting) gets duplicated across call sites.
**Decision:** Introduce newtype wrappers for all domain-specific string types: `CanonicalPath` (relative, normalized, forward slashes), `ContentHash` (xxHash64, hex), `ClusterId` (cluster identifier), `Symbol` (export/import symbol name). Constructors enforce invariants at creation time. The rest of the system works with validated types — no re-validation needed. Newtypes implement `Ord` for use as BTreeMap keys and deterministic sorting.
**Alternatives rejected:**
- Raw `String` everywhere — no compile-time safety, normalization bugs at boundaries
- Validated wrapper at serialization boundary only — still allows internal misuse, DRY violation on normalization
- Type aliases (`type CanonicalPath = String`) — zero safety, just documentation
**Reasoning:** Newtypes are zero-cost abstractions in Rust — no runtime overhead. They turn a category of runtime bugs into compile-time errors. Path normalization happens once at construction (DRY). Function signatures become self-documenting: `fn add_node(path: CanonicalPath, ...)` vs `fn add_node(path: String, ...)`. This is a standard Rust pattern used in ripgrep, cargo, and rust-analyzer.
**Affects:** architecture.md Graph Data Model, determinism.md Data Structures, Phase 1a spec D2.

## D-018: Trait Separation — LanguageParser and ImportResolver

**Date:** 2026-03-18
**Status:** Accepted
**Supersedes:** Partially updates D-002 (removes `resolve_import_path` from `LanguageParser`).
**Context:** D-002 defined `LanguageParser` with 6 methods including `resolve_import_path`. Parsing and import resolution are different responsibilities: parsing extracts raw import strings from AST (language syntax knowledge), resolution maps those strings to canonical file paths (filesystem knowledge, workspace config, tsconfig paths). Combining them in one trait violates SRP and makes it impossible to swap resolution strategies independently (e.g., workspace-aware resolution in Phase 1b).
**Decision:** Split into two traits. `LanguageParser` (5 methods): `language`, `extensions`, `tree_sitter_language`, `extract_imports`, `extract_exports`. `ImportResolver` (1 method): `resolve(import, from_file, known_files) -> Option<CanonicalPath>`. Both traits require `Send + Sync`. A single struct can implement both traits. Parsers return `RawImport`/`RawExport` (unresolved), resolution produces `ResolvedImport` (with `CanonicalPath`).
**Alternatives rejected:**
- Keep resolve in LanguageParser — SRP violation, can't swap resolution strategy without touching parsers
- Standalone resolve functions (no trait) — loses polymorphism, can't have language-specific resolution behind a uniform interface
**Reasoning:** Interface Segregation (SOLID-I): pipeline stages depend only on the trait they need. Parser tests don't need filesystem. Resolution tests don't need tree-sitter. Phase 1b workspace resolution is a new `ImportResolver` impl — existing parsers unchanged (Open/Closed, SOLID-O).
**Affects:** architecture.md LanguageParser trait, Phase 1a spec D4, implementation plan Chunks 2-4 and 6.

## D-019: Pipeline Traits — Injectable Stage Abstractions

**Date:** 2026-03-18
**Status:** Accepted
**Context:** The build pipeline (walk → read → parse → resolve → serialize) is described as a monolithic `build_graph()` function with hardcoded dependencies on the filesystem (`ignore` crate), file I/O (`std::fs::read`), and JSON output (`serde_json`). This makes the pipeline untestable without real filesystem access and prevents swapping implementations (e.g., in-memory VFS for testing, different serialization formats).
**Decision:** Define traits for externally-injectable pipeline stages: `FileWalker` (directory traversal), `FileReader` (file reading + filtering), `GraphSerializer` (output writing). The pipeline struct (`BuildPipeline`) accepts these as trait objects. Concrete implementations (`FsWalker`, `FsReader`, `JsonSerializer`) are wired in `main.rs`. Tests use mock implementations (`MockWalker`, `MockReader`). Each trait requires `Send + Sync` for rayon compatibility. Intermediate data types (`FileEntry`, `FileContent`, `ParsedFile`, `BuildOutput`) define the contract between stages.
**Alternatives rejected:**
- Monolithic function — untestable without FS, can't swap stages
- Generic type parameters on pipeline — complex signatures, monomorphization bloat for 3+ type params
- Iterator/stream pipeline — harder to instrument (per-stage timing), harder error handling
**Reasoning:** Dependency Inversion (SOLID-D): pipeline depends on abstractions, not concrete FS/IO. Trait objects (`Box<dyn FileWalker>`) chosen over generics to keep pipeline signature simple — the dynamic dispatch cost is negligible (called once per build, not per file). Intermediate types make each stage independently testable and enable `--verbose` timing trivially.
**Affects:** architecture.md (new Pipeline Architecture section), Phase 1a spec D14, implementation plan Chunk 6.

## D-020: Composition Root in main.rs

**Date:** 2026-03-18
**Status:** Accepted
**Context:** With trait-based pipeline stages (D-019), concrete implementations must be assembled somewhere. Without a clear composition strategy, concrete types leak into library code, reducing testability and increasing coupling.
**Decision:** `main.rs` is the sole Composition Root — the only place where concrete types are instantiated and wired together. All code in `lib.rs` and internal modules depends only on traits. `main.rs` creates `FsWalker`, `FsReader`, `ParserRegistry::with_tier1()`, `JsonSerializer`, wires them into `BuildPipeline`, and calls `pipeline.run()`. No other module imports concrete pipeline stage implementations.
**Alternatives rejected:**
- Factory functions in lib.rs — leaks concrete types into library, test code must work around them
- Dependency injection container — over-engineering for a CLI tool, not idiomatic Rust
- Default implementations in trait — conflates abstraction with implementation
**Reasoning:** Composition Root is a standard pattern from Clean Architecture. It concentrates all "what concrete type to use" decisions in one file. Library code stays testable with mocks. Adding a new serialization format = new impl + one line in main.rs. This naturally follows from D-019.
**Affects:** Phase 1a spec D17 (CLI), implementation plan Chunk 7.

## D-021: DiagnosticCollector — Thread-Safe Warning Aggregation

**Date:** 2026-03-18
**Status:** Accepted
**Context:** D-005 defines a two-tier error model (fatal + warnings). Warnings are emitted during parallel parsing via rayon. Writing directly to stderr from parallel workers produces interleaved, non-deterministic output. Warnings need to be collected, sorted, and reported after all parallel work completes.
**Decision:** Introduce `DiagnosticCollector` — a thread-safe warning aggregator. Uses `Mutex<Vec<Warning>>` for collection during parallel parsing. After all stages complete, `drain()` sorts warnings by (path, code) for deterministic output. `DiagnosticCounts` tracks aggregate metrics (files skipped, imports unresolved, partial parses). Fatal errors remain as `FatalError` enum via `thiserror` (not collected — they stop the pipeline via `Result`). `anyhow` removed from dependencies — `thiserror` for library errors, concrete error types throughout.
**Alternatives rejected:**
- Direct stderr writes from rayon workers — non-deterministic interleaving, no aggregation
- Channel-based collection (`mpsc`) — more complex than Mutex for low-contention writes
- `RwLock` — all accesses are writes (push), RwLock adds overhead for read/write distinction
- `anyhow` for all errors — loses type information, can't match on error variants in tests
**Reasoning:** `Mutex<Vec<Warning>>` is the simplest correct approach. Warning emission is rare (most files parse successfully), so lock contention is minimal. Sorting after collection guarantees deterministic output (D-006). `thiserror` provides ergonomic error types with `?` operator support and pattern matching in tests.
**Affects:** error-handling.md (new Implementation Architecture section), performance.md (Mutex overhead note), Phase 1a spec D1 (deps: thiserror replaces anyhow), implementation plan Chunk 6.

## D-022: Internal Model vs Output Model Separation

**Date:** 2026-03-18
**Status:** Accepted
**Context:** `ProjectGraph` is used both for in-memory graph operations and as the serialization target. These have different requirements: internal operations benefit from newtypes (`CanonicalPath`) and enums, while JSON output needs string keys and compact tuple edges (D-012). Mixing both in one type forces compromises or complex serde attributes.
**Decision:** Separate internal model (`ProjectGraph` with newtypes, used by pipeline and algorithms) from output model (`GraphOutput` with string keys and tuple edges, used only for serialization). Conversion via `impl From<ProjectGraph> for GraphOutput`. All sort-point enforcement (D-006) happens during conversion — internal model doesn't need to maintain sort order during construction. `ClusterOutput` similarly separated from internal `ClusterMap`.
**Alternatives rejected:**
- Single type with serde attributes — complex, couples internal structure to JSON format
- Single type with BTreeMap everywhere — works but forces serialization concerns into pipeline code
**Reasoning:** Single Responsibility: internal model optimized for programmatic use, output model optimized for JSON. The conversion is a single function — easy to audit for determinism. Adding a new output format (e.g., YAML, binary) means a new output type + conversion, no changes to internal model. HashMap acceptable internally for O(1) lookups; BTreeMap used in output types for sorted keys.
**Affects:** architecture.md Graph Data Model, determinism.md Data Structures, Phase 1a spec D2/D15, implementation plan Chunks 1 and 7.

## D-023: Module Organization — Responsibility-Based Structure

**Date:** 2026-03-18
**Status:** Accepted
**Context:** The initial module layout placed the graph builder, data model, serialization, and clustering all under `src/graph/`. As the architecture matured (D-017 through D-022), these became distinct responsibilities with different dependency directions. `graph/mod.rs` doing both "define types" and "run pipeline" violates SRP.
**Decision:** Reorganize into responsibility-based modules:
- `src/model/` — data types, newtypes, enums (leaf module, depends on nothing)
- `src/parser/` — `LanguageParser` + `ImportResolver` traits, registry, per-language implementations
- `src/pipeline/` — `BuildPipeline`, stage traits (`FileWalker`, `FileReader`), orchestration
- `src/detect/` — file type detection, architectural layer inference
- `src/cluster/` — directory-based clustering
- `src/serial/` — `GraphSerializer` trait, `JsonSerializer`, output types (`GraphOutput`, `ClusterOutput`)
- `src/diagnostic.rs` — `FatalError`, `Warning`, `DiagnosticCollector`
- `src/hash.rs` — xxHash64 wrapper

Dependency rules: `model/` depends on nothing. `parser/` depends on `model/` only. `pipeline/` depends on traits from `parser/` and `serial/`, never concrete implementations. `serial/` depends on `model/` only. `main.rs` depends on everything (Composition Root, D-020).
**Alternatives rejected:**
- Keep everything under `graph/` — growing module with mixed responsibilities
- Feature-based modules (e.g., `build/`, `query/`) — doesn't match the data-flow architecture
- Flat structure (all files in `src/`) — doesn't scale, no clear dependency boundaries
**Reasoning:** Each module has one reason to change. Dependency direction flows downward to `model/`. Adding a language touches only `parser/`. Adding an output format touches only `serial/`. The pipeline can be tested with mocks because it depends on traits, not on `parser/` or `serial/` internals. This structure maps directly to SOLID principles and scales to Phase 2 (new `algo/` module) without reorganization.
**Affects:** architecture.md (module structure), CLAUDE.md (File Structure), Phase 1a spec (Files Created table), implementation plan (all chunk file paths).

## D-024: Pipeline Support Types — FileSet, FileSkipReason, WalkConfig, BuildOutput

**Date:** 2026-03-18
**Status:** Accepted
**Context:** Four types appear in pipeline trait signatures (`FileWalker::walk`, `FileReader::read`, `BuildPipeline::run`) but were never formally defined. These types are prerequisites for implementing the pipeline traits and mock implementations for testing.
**Decision:** Define all four types in `architecture.md` Pipeline Support Types section:
- `FileSet(BTreeSet<CanonicalPath>)` — set of successfully-read files for import resolution. Uses `BTreeSet` for determinism consistency (D-006). Populated from read stage results, not walk results, to prevent dangling edge targets from TOCTOU races. Lives in `model/types.rs` (not `pipeline/`) so `parser/traits.rs` can reference it without `parser/` depending on `pipeline/`.
- `FileSkipReason` — enum with variants `ReadError`, `TooLarge`, `BinaryFile`, `EncodingError`. The pipeline converts these to the appropriate `Warning` via `DiagnosticCollector`.
- `WalkConfig` — struct with `max_files`, `max_file_size`, `exclude_dirs` (always includes `.ariadne`).
- `BuildOutput` — struct with paths to written files, counts, and drained warnings.
**Alternatives rejected:**
- Leave undefined until implementation — would force ad-hoc decisions, inconsistent mocks
- Combine `FileSkipReason` with `Warning` — conflates collection-time decisions with reporting; `FileSkipReason` is internal, `Warning` is for output
**Reasoning:** Explicit type definitions enable parallel implementation of pipeline stages and their test mocks. `FileSet` being `BTreeSet` (not `HashSet`) prevents future non-determinism if iteration is ever needed.
**Affects:** architecture.md Pipeline Support Types, Phase 1a spec D5, implementation plan.

## D-025: arch_depth Placeholder in Phase 1a

**Date:** 2026-03-18
**Status:** Accepted
**Context:** `Node.arch_depth: u32` stores topological depth. Computing it correctly requires topological sort on a DAG, which requires Tarjan SCC to contract cycles into supernodes first. Both algorithms are Phase 2 scope. But the `arch_depth` field is in the Phase 1a data model and appears in `graph.json` output.
**Decision:** Phase 1a sets `arch_depth = 0` for all nodes. This is documented in the `graph.json` schema and the `architecture.md` Node definition. Phase 2 computes correct values via topological sort after SCC contraction. The one-time diff when Phase 2 activates (all `arch_depth` values change from 0 to real values) is an acceptable cost.
**Alternatives rejected:**
- Pull Tarjan + topo sort into Phase 1a — scope creep; these are Phase 2 algorithms
- Simple BFS-based depth without SCC handling — produces incorrect values for files in cycles; worse than a known placeholder because it looks correct but isn't
- Remove `arch_depth` from Phase 1a Node struct — breaks output schema stability; adding a field later is also a large diff
**Reasoning:** A known placeholder (0) is better than an incorrect value. Consumers of `graph.json` can check `arch_depth == 0` to know the field is not yet computed. The Phase 2 diff is one-time and clearly intentional.
**Affects:** architecture.md Node definition, Phase 1a spec D15, graph.json schema.

## D-026: Walk and Read as Separate Pipeline Stages

**Date:** 2026-03-18
**Status:** Accepted
**Context:** error-handling.md described walking and reading as a single "Stage 1." The implementation separates them into `FileWalker::walk()` (returns paths) and `FileReader::read()` (returns content), following D-019's injectable trait design.
**Decision:** Walking and reading are separate stages with separate traits and error handling.
**Reasoning:** Enables independent testing, independent error handling (walk-level vs read-level), and independent resource control.
**Affects:** error-handling.md Stage 1, architecture.md Pipeline Architecture.

## D-027: Workspace Entry Point Preference Order

**Date:** 2026-03-18
**Status:** Accepted
**Context:** Phase 1b adds npm/yarn/pnpm workspace detection. When resolving a workspace member import (`@myapp/auth`), the resolver needs to find the member's entry point. A member's `package.json` may contain `main`, `module`, and/or `exports` fields, each pointing to a different file. The `exports` field can contain conditional exports (`{ "import": "...", "require": "..." }`), adding significant complexity.
**Decision:** Phase 1b uses a simple preference order: `main` → `module` → default probe (`src/index.ts`, `index.ts`). The `exports` field is NOT parsed in Phase 1b. If neither `main` nor `module` exists, fall back to probing standard entry point locations.
**Alternatives rejected:**
- Full `exports` field parsing — significant complexity (conditional exports, subpath exports, pattern matching). Deferred to future phase if needed.
- `module` first — `main` is the canonical Node.js entry point and more universally supported.
- Require explicit entry point — too much friction for a zero-config tool.
**Reasoning:** `main` is the most reliable field — present in virtually all `package.json` files. `module` is a de-facto convention for ESM entry points. The full `exports` spec (Node.js conditional exports) is complex and rarely needed for monorepo cross-package resolution where entry points are straightforward. Pragmatic MVP choice.
**Affects:** Phase 1b spec D4, path-resolution.md §Per-Language Workspace Resolution.

## D-028: WorkspaceInfo Design — npm-Family Only in Phase 1b

**Date:** 2026-03-18
**Status:** Accepted
**Context:** Phase 1b introduces `WorkspaceInfo` and `WorkspaceMember` types for workspace detection. The current design uses `WorkspaceMember { name, path, entry_point }` which maps well to npm/yarn/pnpm packages but not to Go modules (no "entry points") or Cargo workspaces (crate names resolved differently). Future phases will add Go (`go.work`), Cargo (`[workspace]`), Nx, and Turborepo support.
**Decision:** Phase 1b's `WorkspaceInfo` is intentionally npm-family-specific. `WorkspaceKind` enum has variants `Npm`, `Yarn`, `Pnpm`. Future workspace types will require extending the model — either by adding language-specific variants to `WorkspaceMember` or converting `WorkspaceInfo` to a trait with per-type implementations. This is acceptable design debt for a pre-1.0 codebase.
**Alternatives rejected:**
- Trait-based `WorkspaceInfo` from the start — over-engineering for Phase 1b when only npm-family is implemented
- Generic `Box<dyn Any>` for type-specific data — loses type safety
- No workspace support until all types can be supported — delays the most common use case (npm workspaces)
**Reasoning:** npm/yarn/pnpm workspaces are the most common monorepo pattern. Shipping npm-family support early provides value. The `WorkspaceMember` struct is simple and correct for its scope. When Go/Cargo support is added, the required model changes will be informed by real implementation experience with npm workspaces.
**Affects:** Phase 1b spec D3, architecture.md §WorkspaceInfo, path-resolution.md §Monorepo Support.

## D-029: Workspace Member Name Collision Handling

**Date:** 2026-03-18
**Status:** Accepted
**Context:** In an npm workspace, two member directories could have the same `name` field in their `package.json`. This creates ambiguity in the workspace member map used for import resolution. Example: `packages/auth` and `legacy/auth` both declaring `"name": "@myapp/auth"`.
**Decision:** Use first-found member (deterministic by filesystem walk order, which is sorted) and emit W008 warning if a collision is detected. The first member in sorted directory order wins. This is a best-effort approach consistent with Ariadne's philosophy of graceful degradation (D-003).
**Alternatives rejected:**
- Fatal error — too harsh for what may be a transitional state in a monorepo
- Last-found — less predictable, harder to reason about
- Silent first-found — violates transparency principle (D-005)
**Reasoning:** Name collisions in real workspaces are rare and usually indicate a configuration issue. Warning the user while continuing with deterministic behavior is consistent with the best-effort philosophy. Sorted order ensures determinism (D-006).
**Affects:** Phase 1b spec D3, path-resolution.md §Workspace Detection.

## D-030: CLI Flag Interaction Rules

**Date:** 2026-03-18
**Status:** Accepted
**Context:** Phase 1b adds 6 CLI flags. Some flag combinations need explicit behavior definitions: `--strict` with `--warnings json`, `--verbose` with `--warnings json`, and `--max-files` counting semantics.
**Decision:** Three rules:
1. `--strict` and `--warnings` are orthogonal. `--strict` controls exit code (1 if any warnings), `--warnings` controls output format. Both can be set simultaneously: `--strict --warnings json` outputs JSON warnings AND exits with code 1.
2. `--verbose` and `--warnings json` are orthogonal. `--verbose` adds per-stage timing (always stderr, always human-readable) and enables W006 warnings. `--warnings json` controls warning format only. Both can be set: timing is human-format, warnings are JSON.
3. `--max-files` counts files at the walk stage (all files encountered by the walker, regardless of extension). This prevents unbounded filesystem traversal. Files with unrecognized extensions are counted but not parsed.
**Alternatives rejected:**
- `--warnings json` implies non-strict — confusing; flags should be orthogonal
- `--max-files` counts only parseable files — allows unbounded walk on repos with many non-source files
- Mutually exclusive flags — unnecessary restriction
**Reasoning:** Orthogonal flags are easier to reason about. Each flag controls one dimension: `--strict` = exit behavior, `--warnings` = format, `--verbose` = verbosity, `--max-files` = walk limit. Walk-stage counting for `--max-files` is the safer default — it bounds total I/O regardless of file types.
**Affects:** Phase 1b spec D2, error-handling.md §CLI Flags.

## D-031: Feature-Sliced Design (FSD) Architecture Support

**Date:** 2026-03-18
**Status:** Deferred (post-Phase 2)
**Context:** FSD is a frontend architectural methodology with layer hierarchy: app → processes → pages → widgets → features → entities → shared. Each slice has internal segments (ui/, model/, api/, lib/). Current `infer_arch_layer()` uses first-matching-segment strategy, which fails for FSD: `features/auth/ui/Button.tsx` maps to Unknown (stops at `features/`) instead of Component (from `ui/`).
**Decision:** Defer FSD-specific support to post-Phase 2. Current layer detection handles ~3 of 7 FSD layers (shared→Util, pages→Component, widgets→Component) and most inner segments (ui/, api/, lib/, config/). Graphs, edges, and clusters work correctly — only `layer` field is affected.
**What FSD support requires:**
1. New ArchLayer variants or a secondary classification system for FSD layers (features, entities, processes, app)
2. Context-aware matching strategy — FSD needs innermost-segment matching (ui/, model/, api/) rather than first-match, or a two-level scheme: FSD-layer (from outer segment) + functional-layer (from inner segment)
3. Optional FSD detection heuristic (presence of `features/` + `entities/` + `shared/` at root level)
4. Addition of `"model"` (singular) to Data layer matching (currently only `"models"`)
**Alternatives rejected:**
- Adding FSD segments to existing ArchLayer now — would pollute the generic layer model with framework-specific semantics without proper Phase 2 algorithms to use them
- Changing first-match to last-match globally — breaks non-FSD projects where outermost segment is the correct layer
**Reasoning:** FSD support is best done alongside Phase 2's architectural algorithms (depth computation, layer violation detection) which will provide the infrastructure to properly model layered architectures. The graph itself is architecture-agnostic — only the classification metadata needs extension.
**Affects:** detect/layer.rs, model/node.rs (ArchLayer enum), future architecture.md update.
