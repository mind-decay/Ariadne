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

## D-004: Separate Project — Consumer-Agnostic Design

**Date:** 2026-03-17
**Updated:** 2026-03-18 (Phase 3 planning — generalized from Moira-specific to consumer-agnostic)
**Status:** Accepted (updated)
**Context:** Ariadne was originally designed as `moira-graph`, a component of the Moira orchestration framework. However, the tool has no dependency on Moira's infrastructure and uses a completely different tech stack (Rust vs shell/markdown). Phase 3 introduces MCP server capabilities — the integration boundary must be clearly defined.
**Decision:** Ariadne is a standalone project with its own repository, CI/CD, versioning, and release cycle. Ariadne provides **generic, consumer-agnostic APIs** (CLI commands and MCP tools). Consumer-specific adapters live in the consumer's codebase:
- Moira: wraps Ariadne MCP tools into knowledge bridge, agent context injection, bootstrap acceleration — all on Moira's side
- IDEs: wrap Ariadne MCP tools into editor UI — on IDE plugin's side
- CI tools: wrap Ariadne CLI/MCP into pipeline checks — on CI config's side
Ariadne has **zero knowledge of any specific consumer**. No Moira-specific formats, no agent role mappings, no consumer-specific export modes.
**Alternatives rejected:**
- Subdirectory in Moira repo — GitHub Actions doesn't work from nested `.github/`, `cargo install` doesn't work from subdirectory, Rust toolchain not needed for core Moira
- Git submodule — worst of both approaches
- Consumer-specific export formats in Ariadne (e.g., `format: "moira"`) — violates separation, requires Ariadne to know consumer schemas, creates coupling that prevents independent releases
**Reasoning:** Clean separation enables: standard `cargo install ariadne`, native CI/CD, independent releases. The tool is useful beyond any single consumer. Generic MCP tools let any MCP-compatible system benefit. Consumer-specific logic belongs in the consumer.

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
**Decision:** Split into two traits. `LanguageParser` (6 methods): `language`, `extensions`, `tree_sitter_language`, `tree_sitter_language_for_ext` (default impl, override when one parser covers multiple grammars — e.g. TS vs TSX), `extract_imports`, `extract_exports`. `ImportResolver` (1 method): `resolve(import, from_file, known_files) -> Option<CanonicalPath>`. Both traits require `Send + Sync`. A single struct can implement both traits. Parsers return `RawImport`/`RawExport` (unresolved), resolution produces `ResolvedImport` (with `CanonicalPath`).
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
**Decision:** Separate internal model (`ProjectGraph` with newtypes, used by pipeline and algorithms) from output model (`GraphOutput` with string keys and tuple edges, used only for serialization). Conversion via `project_graph_to_output(graph, project_root)` free function (requires `project_root` parameter, so `From` impl is not suitable). All sort-point enforcement (D-006) happens during conversion — internal model doesn't need to maintain sort order during construction. `ClusterOutput` similarly separated from internal `ClusterMap`.
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

Dependency rules: `model/` depends on nothing. `parser/` depends on `model/` only. `pipeline/` depends on traits from `parser/` and `serial/`, never concrete implementations. `detect/` depends on `model/` (and may also depend on `diagnostic.rs` for W008 warnings during workspace detection). `serial/` depends on `model/` only. `main.rs` depends on everything (Composition Root, D-020).
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
**Status:** Accepted (implemented 2026-03-22)
**Context:** FSD is a frontend architectural methodology with layer hierarchy: app → processes → pages → widgets → features → entities → shared. Each slice has internal segments (ui/, model/, api/, lib/). Current `infer_arch_layer()` uses first-matching-segment strategy, which loses FSD layer context for files like `features/auth/index.ts` (no inner segment → Unknown).
**Decision:** Implement two-level FSD classification with automatic project detection:
1. New `FsdLayer` enum (App, Processes, Pages, Widgets, Features, Entities, Shared) + `fsd_layer: Option<FsdLayer>` on Node — secondary classification system, keeping ArchLayer generic
2. Two-pass matching when FSD detected: outer segment → FsdLayer, innermost inner segment → ArchLayer (e.g., `features/auth/ui/Button.tsx` → FsdLayer::Features + ArchLayer::Component)
3. 2-of-3 detection heuristic: if at least 2 of {features/, entities/, shared/} exist at root or src/ level → FSD project
4. `"model"` singular was already present in Data layer matching — no action needed
**Alternatives rejected:**
- Extending ArchLayer with FSD variants — pollutes generic enum with framework-specific semantics (7+ new variants)
- Metadata map on Node — loses type safety, no compile-time exhaustiveness
- Always last-match globally — breaks non-FSD projects where outermost segment is correct
- Single directory trigger for detection — too many false positives (BDD `features/`, common `shared/`)
**Reasoning:** Flat optional field on Node follows existing patterns and preserves backward compatibility via `skip_serializing_if = "Option::is_none"`. Non-FSD projects produce byte-identical output. Detection heuristic follows the workspace detection pattern (structural indicators from file paths).
**Affects:** detect/layer.rs, model/node.rs (FsdLayer enum, Node.fsd_layer), pipeline/build.rs, serial/mod.rs, serial/convert.rs, pipeline/mod.rs.

## D-032: GraphReader Trait — Separate from GraphSerializer

**Date:** 2026-03-18
**Status:** Accepted
**Context:** Phase 2 needs to load graph.json/stats.json from disk for query commands and delta computation. The obvious approach — adding `read_*` methods to `GraphSerializer` — violates D-019's original intent (write-only stage abstractions) and creates a misleading API. Read and write have different error semantics: a missing file is acceptable during reads (fall back to full build) but never during writes.
**Decision:** Introduce a separate `GraphReader` trait with `read_graph`, `read_clusters`, `read_stats` methods. `GraphSerializer` remains write-only, gaining only `write_stats` for Phase 2. `JsonSerializer` implements both traits. Test mocks implement each independently. `BuildPipeline` accepts `Box<dyn GraphReader>` as an additional parameter (or via a combined `Box<dyn GraphSerializer + GraphReader>` where needed).
**Alternatives rejected:**
- Add read methods to `GraphSerializer` — violates SRP, misleading trait name, breaks all existing mock implementations
- Free functions (no trait) — loses testability, can't mock reads in pipeline tests
- Combined `GraphIO` trait — better than extending `GraphSerializer`, but the separate concern argument still holds
**Reasoning:** Follows the same Interface Segregation principle as D-018 (LanguageParser/ImportResolver split). Read and write are separate responsibilities with different lifecycles and error handling. Mocking is simpler when traits are narrow.
**Affects:** Phase 2 spec D1, serial/mod.rs, pipeline/mod.rs, main.rs (wiring).

## D-033: Module Dependency Rules — algo/, views/, analysis/, mcp/

**Date:** 2026-03-18
**Updated:** 2026-03-18 (Phase 3 planning — added analysis/ and mcp/ modules)
**Status:** Accepted (extends D-023)
**Context:** Phase 2 adds `src/algo/` (graph algorithms) and `src/views/` (markdown view generation). Phase 3 adds `src/analysis/` (architectural intelligence) and `src/mcp/` (MCP server). Their dependency rules must be defined to maintain the module boundary discipline established in D-023.
**Decision:**
- `algo/` depends on `model/` only. Pure functions on `ProjectGraph`. No I/O, no pipeline, no serialization. Delta diff logic lives in `algo/delta.rs` but receives pre-loaded data — orchestration (walk/read/re-parse) stays in `pipeline/`.
- `views/` depends on `model/` and `serial/` (output types like `StatsOutput` only — never serialization methods). Receives pre-computed algorithm results. Does not depend on `algo/`.
- `analysis/` depends on `model/` and `algo/`. Composes algorithm results into higher-level insights (Martin metrics, smell detection, structural diffs). Never depends on `serial/`, `pipeline/`, or `parser/`. Shared data types (`StructuralDiff`, `ArchSmell`) live in `model/` (see D-048).
- `mcp/` depends on `model/`, `algo/`, `analysis/`, `serial/`, `pipeline/`. Never depends on `parser/` directly. This is the widest dependency set — justified because MCP server orchestrates all capabilities.
- `pipeline/` gains dependency on `algo/` for invoking algorithms after build. This is additive — existing dependencies unchanged.
- `SubgraphResult` lives in `model/query.rs` (pure data type) so both `algo/` and CLI code can reference it without circular dependencies.

Full dependency table:

| Module | Depends on | Never depends on |
|--------|-----------|-----------------|
| `model/` | (nothing) | everything |
| `algo/` | `model/` | `serial/`, `pipeline/`, `parser/`, `views/`, `analysis/`, `mcp/` |
| `views/` | `model/`, `serial/` (types only) | `parser/`, `pipeline/`, `algo/`, `analysis/`, `mcp/` |
| `analysis/` | `model/`, `algo/` | `serial/`, `pipeline/`, `parser/`, `views/`, `mcp/` |
| `mcp/` | `model/`, `algo/`, `analysis/`, `serial/`, `pipeline/` | `parser/` |
| `pipeline/` | `model/`, `parser/` (traits), `serial/` (traits), `algo/`, `detect/`, `cluster/` | `views/`, `analysis/`, `mcp/` |

**Alternatives rejected:**
- `algo/` depending on `serial/` for delta deserialization — violates leaf-module design, makes algo/ impure
- `views/` depending on `algo/` — tighter coupling, harder to test views without full algo infrastructure
- Analysis logic inside `algo/` — would require `algo/` to depend on `serial/` (for StatsOutput), violating the pure-computation principle
- All algorithm types in `algo/` — forces CLI/views code to depend on `algo/` for type definitions
**Reasoning:** Preserves D-023's principle: each module has one reason to change. Algorithm logic is pure computation. View generation is pure template rendering. Pipeline orchestrates both. Data types shared across boundaries live in `model/`.
**Affects:** Phase 2 spec (module structure), architecture.md (module dependency table — to be updated).

## D-034: Phase 2 Algorithm Parameters and Policies

**Date:** 2026-03-18
**Status:** Accepted
**Context:** Phase 2 introduces 5+ graph algorithms with parameters that are not fully specified in architecture.md. These include edge type filtering, centrality normalization, Louvain convergence, orphan definition, and build scope changes.
**Decision:** Bundled policy decisions for Phase 2 algorithms:
1. **Edge type filtering:** All algorithms (SCC, centrality, BFS, topo sort) use `imports` + `re_exports` + `type_imports`, excluding `tests` edges. Tests are not architectural dependencies. **Exception:** subgraph extraction (`extract_subgraph`) includes ALL edge types including `tests` — tests are relevant for scoped development views and impact reports. This exception applies only to subgraph, not to blast radius or other algorithms.
2. **Centrality normalization:** Brandes raw BC divided by `(V-1)(V-2)` for directed graphs → values in [0.0, 1.0]. Bottleneck threshold 0.7 applies to normalized values. Rounded to 4 decimal places (same as cohesion in Phase 1).
3. **Louvain parameters:** 100 iteration limit, ΔQ < 1e-6 convergence threshold. Directed graph converted to undirected weights (edge count between two nodes, ignoring direction). Cluster naming: directory-based name of plurality of files; lexicographic tie-break. **Updated:** Resolution parameter (gamma) added via D-074; directory retention guard via D-073.
4. **Orphan definition:** `source` or `test` file with zero incoming edges AND zero outgoing `imports`/`type_imports`/`re_exports` edges. Config, style, asset files are excluded (they naturally have no import edges).
5. **Build scope:** `ariadne build` always produces stats.json (runs all algorithms). ~720ms overhead on 3000-file project. Avoids "forgot --stats" UX problem.
6. **Louvain default:** On by default. `--no-louvain` flag to disable. No existing consumers means no breaking change.
**Alternatives rejected:**
- Per-algorithm edge type flags — over-engineering for Phase 2; can add `--include-tests` later if needed
- Optional stats generation (`--stats` flag) — creates fragile dependency: query commands fail if build was run without `--stats`
**Reasoning:** Batching these decisions enables consistent behavior across all algorithms. Edge type policy is the most impactful — excluding `tests` from structural algorithms matches the conceptual model (tests verify, they don't define architecture).
**Affects:** Phase 2 spec D2-D9, stats.json schema, CLI behavior.

## D-035: Subgraph Cluster Expansion Limit

**Date:** 2026-03-18
**Status:** Accepted
**Context:** architecture.md §Algorithms §7 says subgraph extraction should "add all files in f.cluster to result_nodes." In large projects, a single cluster can contain hundreds of files. Including all of them defeats the purpose of scoped extraction — the subgraph balloons to near-full-graph size.
**Decision:** Cap cluster expansion at 100 files. If a cluster has >100 files, include only BFS-reachable files within that cluster, not all files. 100 is large enough for real modules but prevents pathological expansion. Hardcoded for Phase 2; configurable flag deferred.
**Alternatives rejected:**
- No limit — subgraph can become uselessly large on projects with big clusters
- 50 files — too conservative for medium-large modules
- Configurable from the start — premature; 100 is a reasonable default until real usage data says otherwise
**Reasoning:** Subgraph extraction is meant to provide focused context. A 100-file cluster included in full is still useful. A 500-file cluster included in full is noise. The BFS-reachable fallback ensures relevant files are still included.
**Affects:** Phase 2 spec D7, algo/subgraph.rs.

## D-036: Phase 2 Split into 2a and 2b

**Date:** 2026-03-18
**Status:** Accepted
**Context:** Phase 2 contains 11 deliverables with varying risk. Two (Louvain clustering and delta computation) are ORANGE risk — Louvain modifies existing cluster behavior and involves iterative convergence; delta touches the core pipeline with incremental correctness concerns. The remaining 9 deliverables are GREEN/YELLOW risk and deliver 90% of user value (queryable graph).
**Decision:** Split Phase 2 into:
- **Phase 2a (YELLOW):** D1 (deserialization), D2 (SCC), D3 (blast radius), D4 (centrality), D5 (topo sort), D7 (subgraph), D8 (stats.json), D10 (views), D11 (CLI queries + views generate). This makes the graph queryable.
- **Phase 2b (ORANGE):** D6 (Louvain), D9 (delta computation). Optimization and refinement.
Phase 2b depends on 2a. Phase 2a can ship independently.
**Alternatives rejected:**
- Ship everything at once — delays query functionality behind ORANGE-risk items
- Defer Louvain to Phase 3 — separates related graph algorithm work too far; better to keep it in Phase 2 scope
**Reasoning:** Same principle as D-011 (Phase 1a/1b split). Deliver working value first, then enhance. Delta computation benefits from real usage patterns of full builds before optimizing. Louvain benefits from seeing whether directory clusters are "good enough" in practice.
**Affects:** Phase 2 spec, ROADMAP.md (may need update to reflect sub-phases).

---

## Phase 3 Decisions

## D-037: MCP Server over CLI for Integration

**Date:** 2026-03-18
**Status:** Accepted
**Context:** Phase 3 introduces integration with external consumers (AI orchestrators, IDEs, CI). Three options: MCP server (long-running process with in-memory graph), CLI invocation (consumers call `ariadne query ...` per request), or direct file reading (consumers parse `.ariadne/graph/graph.json` themselves).
**Decision:** Ariadne provides an MCP server via `ariadne serve`. The server loads the graph into memory on startup and answers queries via MCP tools over stdio JSON-RPC. CLI commands remain available for one-shot usage.
**Alternatives rejected:**
- CLI-only integration — cold start (load + parse graph.json) per query adds 100-500ms latency. Unacceptable for interactive agent workflows requiring multiple queries per task
- Direct file reading by consumers — no query API (consumers would reimplement blast radius, subgraph extraction, etc.), no freshness tracking, no auto-update
- HTTP server — more complex than stdio, requires port management, firewall considerations. MCP over stdio is the standard for Claude Code integration
**Reasoning:** In-memory graph enables <10ms query responses. MCP is the native integration protocol for Claude Code and compatible tools. The server lifecycle aligns with development sessions. CLI remains for scripts and CI.
**Affects:** Phase 3 spec, src/mcp/ module, main.rs (new `serve` subcommand).

## D-038: File System Watcher with Debounced Delta Rebuild

**Date:** 2026-03-18
**Status:** Accepted
**Context:** When the MCP server is running and agents modify source files, the in-memory graph becomes stale. Users should not have to manually run `ariadne update` — the graph should stay fresh automatically.
**Decision:** Use the `notify` crate for OS-native file system watching (kqueue on macOS, inotify on Linux). Collect file change events with a 2-second debounce window (configurable via `--debounce`). After debounce expires, run delta computation (Phase 2b D9) on changed files and hot-swap the in-memory `GraphState`. `--no-watch` disables the watcher for environments where it's unavailable or undesirable.
**Alternatives rejected:**
- Manual `ariadne update` after each task — requires user discipline, easy to forget, breaks seamless agent integration
- Poll-based checking (no watcher) — higher latency (30s+ vs 2s), wastes CPU on repeated hash comparisons. Used only as fallback when watcher is unavailable
- Immediate rebuild on every file write — thrashing during multi-file saves (e.g., `git checkout`, batch writes by code generation agents)
**Reasoning:** 2-second debounce balances freshness vs rebuild cost. OS-native watchers are efficient (no polling). Delta computation (Phase 2b) is fast (<2s for typical changes). The fallback to polling ensures graceful degradation on unsupported filesystems.
**Affects:** Phase 3 spec D4, src/mcp/state.rs, Cargo.toml (notify dependency).

## D-039: Hash-Based Freshness with Confidence Scoring

**Date:** 2026-03-18
**Status:** Accepted
**Context:** Between auto-updates, the graph may be partially stale. Consumers need to know how much to trust query results. A binary fresh/stale signal is too coarse — if 3 out of 3000 files changed, the graph is 99.9% reliable for structural queries.
**Decision:** Freshness engine computes a confidence score: `confidence = 1 - (stale_files / total_files)`. Additionally tracks structural confidence: if stale files have the same import structure (only body changes), structural queries (dependencies, layers, cycles) remain fully valid even though content hashes differ. Every MCP tool response includes freshness metadata. Thresholds: ≥0.95 (fresh), 0.80-0.95 (reliable), 0.50-0.80 (degraded), <0.50 (auto-rebuild triggered).
**Alternatives rejected:**
- Binary fresh/stale — too coarse; a single file change would mark entire graph as stale
- Timestamp-based freshness — unreliable (clock skew, git checkout changes timestamps without content change)
- No freshness tracking — consumers can't assess result reliability
**Reasoning:** Hash-based comparison is precise (same mechanism as Phase 2b delta detection). Confidence score gives consumers a quantitative signal. Structural confidence distinguishes "file body edited" (common, graph still valid) from "imports changed" (structural staleness).
**Affects:** Phase 3 spec D3, src/mcp/state.rs (FreshnessState).

## D-040: Martin Metrics at Cluster Level

**Date:** 2026-03-18
**Status:** Accepted
**Context:** Robert C. Martin's Instability/Abstractness metrics can be computed at any granularity (file, directory, cluster, package). File-level metrics are noisy (a single utility file has I=1.0 — meaningless). Package-level requires package boundary detection not available in Ariadne.
**Decision:** Compute Martin metrics at the cluster level (directory-based or Louvain). Instability I = Ce/(Ca+Ce), Abstractness A = Na/Nc, Distance from Main Sequence D = |A+I-1|. Classify clusters into zones: Main Sequence (D < 0.3), Zone of Pain (low A, low I, high D), Zone of Uselessness (high A, high I, high D).
**Alternatives rejected:**
- File-level metrics — too noisy, every leaf file is I=1.0
- Package-level metrics — requires explicit package boundaries; Ariadne works with implicit clustering
- Skip metrics entirely — significant value for architectural quality assessment
**Reasoning:** Cluster-level aligns with how developers think about modules. Directory-based clusters map to logical boundaries. The metrics are cheap to compute (single pass over edges) and provide actionable insights ("this module is in the Zone of Pain — high coupling, low abstraction").
**Affects:** Phase 3 spec D5, src/analysis/metrics.rs.

## D-041: Hierarchical Graph Compression

**Date:** 2026-03-18
**Status:** Accepted
**Context:** For large codebases (10k+ files), full graph data in MCP tool responses can exceed token budgets. A 10k-node graph serialized as JSON is 50k+ tokens — too much for any single agent context.
**Decision:** Three compression levels: L0 (cluster-level graph, ~200-500 tokens), L1 (per-cluster file detail, ~500-2000 tokens), L2 (per-file neighborhood, ~200-1000 tokens). MCP tool `ariadne_compressed(level, focus?)` returns the appropriate view. Token estimates are advisory — actual counts depend on naming conventions.
**Alternatives rejected:**
- Always send full graph — blows token budget on large projects
- Fixed truncation (top-N files) — loses structural information, arbitrary cutoff
- No compression (consumers handle it) — pushes complexity to every consumer
**Reasoning:** Hierarchical views match natural zoom levels: project overview → module detail → file context. L0 is always small enough for any agent. Consumers request the level they need.
**Affects:** Phase 3 spec D9, src/algo/ or src/analysis/.

## D-042: PageRank + Centrality Combined Ranking

**Date:** 2026-03-18
**Status:** Accepted
**Context:** Brandes betweenness centrality (Phase 2) identifies "bridge" files on many shortest paths. But files that are heavily depended upon (foundations) may have low centrality if they're leaves in the DAG. PageRank captures this — it measures recursive importance (files depended on by important files score higher).
**Decision:** Compute both metrics. Combined score: `0.5 * normalized_centrality + 0.5 * normalized_pagerank`. Both normalized to [0.0, 1.0]. Exposed via `ariadne_importance` MCP tool and `ariadne query importance` CLI command.
**Alternatives rejected:**
- PageRank only — misses bridge files (high centrality, low PageRank)
- Centrality only — misses foundational files (low centrality, high PageRank)
- Configurable weights — premature; 50/50 is a reasonable default
**Reasoning:** The two metrics are complementary. Combined ranking gives the most complete picture of file importance. Fixed 50/50 weight avoids configuration complexity.
**Affects:** Phase 3 spec D8, src/algo/pagerank.rs (new).

## D-043: Spectral Analysis as Optional (ORANGE Risk)

**Date:** 2026-03-18
**Status:** Accepted
**Context:** Fiedler vector (second eigenvector of graph Laplacian) provides natural graph bisection — revealing module boundaries that community detection (Louvain) might miss. However, sparse eigensolvers involve iterative floating-point computation where accumulation order affects results. Cross-platform determinism (D-006) may be impossible without fixed-precision arithmetic.
**Decision:** Implement spectral analysis as a best-effort feature. If cross-platform f64 determinism cannot be achieved at reasonable cost, either: (a) mark spectral results as advisory (not included in deterministic outputs), or (b) defer entirely. Evaluate during implementation.
**Alternatives rejected:**
- Fixed-precision arithmetic — significant performance penalty, complex implementation
- Skip entirely at design time — the insight value is high enough to attempt
- Require determinism — may be technically infeasible for eigensolver convergence
**Reasoning:** Spectral analysis provides unique value (monolith detection, natural refactoring boundaries) that no other metric provides. Attempting implementation with a clear fallback plan is better than premature exclusion.
**Affects:** Phase 3 spec D10, Cargo.toml (nalgebra-sparse or sprs dependency).

## D-044: Consumer-Agnostic MCP Tools

**Date:** 2026-03-18
**Status:** Accepted
**Context:** Phase 3 was initially designed with Moira-specific features: `ariadne_knowledge_export(format: "moira")`, Moira freshness metadata tags (`<!-- moira:freshness ... -->`), agent-specific access matrices in Ariadne's MCP registry. This violates D-004 (Ariadne has zero knowledge of consumers).
**Decision:** All MCP tools return generic, structured JSON. No consumer-specific formatting, export modes, or metadata. Consumer-specific adapters (Moira knowledge bridge, IDE plugins, CI integrations) live entirely in the consumer's codebase. The `ariadne_views_export` tool returns pre-generated markdown views (Phase 2 D10) as-is — consumers transform them into their own format. Agent access matrices, context injection protocols, and bootstrap acceleration logic belong to the consumer.
**Alternatives rejected:**
- Moira-specific `format: "moira"` parameter — couples Ariadne releases to Moira schema changes
- Plugin system for consumer adapters — over-engineering for Phase 3; can add later if multiple consumers emerge with complex needs
- Separate Moira-specific MCP tools alongside generic ones — maintenance burden, version drift
**Reasoning:** Follows the principle of D-004. Ariadne is a general-purpose structural analysis tool. Consumer-specific intelligence should leverage Ariadne's generic capabilities, not depend on Ariadne knowing consumer internals. This enables independent release cycles and prevents scope creep.
**Affects:** Phase 3 spec (removes Phase 3b Moira Knowledge Bridge from Ariadne — moved to Moira project), all MCP tool definitions.

## D-045: Single Binary with `serve` Subcommand

**Date:** 2026-03-18
**Status:** Accepted
**Context:** Phase 3 introduces a long-running MCP server alongside existing one-shot CLI commands. Two options: add `serve` as a subcommand to the existing `ariadne` binary, or create a separate `ariadne-mcp` binary target.
**Decision:** Single `ariadne` binary with `serve` subcommand. `main.rs` remains the sole Composition Root (D-020) — it dispatches to either one-shot mode (build/query/info) or long-running mode (serve) based on the subcommand. Shared code (graph loading, algorithm execution, serialization) is reused.
**Alternatives rejected:**
- Separate `ariadne-mcp` binary — duplicates Composition Root, binary distribution complexity, users must install two binaries
- Library + two binaries — splits wiring logic, harder to keep consistent
**Reasoning:** One binary to install, one binary to manage. `main.rs` is slightly more complex (dispatches to two modes) but all wiring is in one place. Users run `cargo install ariadne-graph` once and get both CLI and MCP server. Standard pattern (rustup, cargo, git all have subcommands with different lifecycles).
**Affects:** Phase 3 spec D1, main.rs, Cargo.toml (no new [[bin]] target).

## D-046: Lock File for Graph Write Exclusion

**Date:** 2026-03-18
**Status:** Accepted
**Context:** When the MCP server is running, it owns `.ariadne/graph/` — reading, computing delta updates, and writing updated files. If a user simultaneously runs `ariadne build` or `ariadne update` in a terminal, two processes would write to the same files concurrently, causing corruption or race conditions.
**Decision:** The MCP server creates `.ariadne/graph/.lock` on startup (containing PID and timestamp). CLI `build` and `update` commands check for this lock before proceeding. If lock exists and the PID is alive → refuse with message: `"error: MCP server (PID {pid}) is running and owns .ariadne/graph/. Stop the server first, or let it handle updates automatically."` If lock exists but PID is dead → remove stale lock and proceed. Lock is released on server shutdown (normal exit, SIGTERM, SIGINT).
**Alternatives rejected:**
- No locking — race conditions, corrupted graph files
- File-level locks (per graph.json, per stats.json) — complex, doesn't prevent semantic races (partial state updates)
- MCP server delegates to CLI (spawns `ariadne update` subprocess) — unnecessarily complex, no benefit over in-process delta computation
**Reasoning:** Directory-level lock is simple and sufficient. The MCP server is the sole writer during its lifetime. CLI commands work normally when no server is running. Stale lock detection handles crash recovery.
**Affects:** Phase 3 spec D4, src/mcp/state.rs, src/pipeline/mod.rs (lock check).

## D-047: Thread-Based Architecture — No Async Runtime

**Date:** 2026-03-18
**Status:** Partially superseded by D-051
**Context:** Phase 3 MCP server needs concurrency: fs watching, delta rebuilds, and MCP request handling. Two approaches: async runtime (tokio) or OS threads.
**Decision:** Thread-based architecture. `notify` crate uses OS-native file watching with a dedicated watcher thread. MCP JSON-RPC runs on the main thread (stdio is inherently sequential — one request at a time). Delta rebuilds run on a background thread, communicate via `Arc<RwLock<GraphState>>`. No tokio or async-std dependency.
**Alternatives rejected:**
- tokio async runtime — adds ~1.5MB to binary, 30+ transitive dependencies, compile time increase. Async IO provides no benefit for stdio (sequential) or file watching (OS-native). The only concurrent operation (delta rebuild) is CPU-bound, not IO-bound
- Single-threaded with blocking — delta rebuild would block MCP responses for 1-2 seconds
**Reasoning:** The concurrency model is simple: main thread handles MCP requests, background thread handles delta rebuilds, watcher thread handles fs events. `Arc<RwLock<GraphState>>` is the only synchronization point. No async complexity. Binary stays small. Build stays fast. Phase 1-2 have zero async dependencies — Phase 3 should not introduce them without clear benefit.
**Affects:** Phase 3 spec D1/D4, Cargo.toml (no tokio), src/mcp/.

**Note (updated 2026-03-19):** The `serve` subcommand uses tokio due to `rmcp` crate requirements (D-051). The "no async runtime" principle still applies to all non-serve code paths (build, query, update, info).

## D-048: `analysis/` Module Separate from `algo/`

**Date:** 2026-03-18
**Status:** Accepted
**Context:** Phase 3 introduces architectural analysis features: Martin metrics, smell detection, structural diffs. These could live in `algo/` (which contains all Phase 2 algorithms) or in a new module.
**Decision:** New `src/analysis/` module, separate from `algo/`. Key distinction:
- `algo/` contains **pure graph algorithms** — takes `ProjectGraph`, returns computed results. Depends on `model/` only (D-033). No knowledge of stats, metrics, or higher-level concepts.
- `analysis/` contains **architectural analysis** — composes algorithm results, graph data, and stats into higher-level insights (Martin metrics, smell detection, structural diffs). Depends on `model/` and `algo/`.
Shared data types (`StructuralDiff`, `ArchSmell`, `SmellSeverity`) live in `model/` (pure data, no computation) so both `analysis/` and `mcp/` can reference them.
**Alternatives rejected:**
- Everything in `algo/` — `algo/` would need to depend on `serial/` (for `StatsOutput` in smell detection), violating D-033
- Everything in `mcp/` — mixes analysis logic with transport/protocol code
- Smell detection depends on `serial/StatsOutput` directly — move needed stats fields into `model/` types instead
**Reasoning:** Preserves D-033 (`algo/` depends on `model/` only). Analysis logic is a higher abstraction layer — it consumes algorithm outputs rather than implementing algorithms. Separate module enables independent testing and clear dependency boundaries.
**Affects:** Phase 3 spec D5-D7, src/analysis/, model/diff.rs, model/smell.rs.

## D-049: Unified Float Determinism Strategy

**Date:** 2026-03-18
**Status:** Accepted
**Context:** Phase 2 introduces Brandes centrality with f64 values (4 decimal rounding per D-034). Phase 3 adds PageRank, and potentially spectral analysis — more iterative f64 algorithms. Each needs deterministic output across platforms. Without a unified strategy, each algorithm handles determinism ad-hoc.
**Decision:** All iterative floating-point algorithms share a common determinism strategy:
1. **Rounding:** Final results rounded to 4 decimal places via `fn round4(v: f64) -> f64 { (v * 10000.0).round() / 10000.0 }`
2. **Iteration order:** Nodes processed in `BTreeMap` key order (lexicographic path order). This ensures deterministic floating-point accumulation across platforms.
3. **Fixed parameters:** Iteration limits and convergence tolerances are hardcoded (not configurable) to ensure identical convergence behavior.
4. **Intermediate rounding:** No rounding during iteration (would slow convergence). Only final output is rounded.
Applies to: Brandes centrality (Phase 2), Louvain modularity (Phase 2b), PageRank (Phase 3c), cohesion (Phase 1 — already uses round4).
**Alternatives rejected:**
- Per-algorithm determinism decisions — inconsistent, easy to miss an algorithm
- Fixed-precision arithmetic throughout — severe performance penalty
- No intermediate determinism (round only at output) — insufficient; accumulation order matters for f64
**Reasoning:** Deterministic iteration order + final rounding is the minimum sufficient strategy. BTreeMap guarantees lexicographic order. Hardcoded parameters prevent user-introduced non-determinism. This approach has zero performance cost (BTreeMap already used everywhere per D-006).
**Affects:** All f64 algorithms in algo/ and analysis/. Utility function in algo/mod.rs or a shared location.

## D-050: `ariadne update` Full-Rebuild-Always Behavior

**Date:** 2026-03-19
**Status:** Accepted
**Context:** Phase 2b delivers `ariadne update` with delta computation. The original design (architecture.md §Algorithms §6) described selective re-parsing of only changed files. During implementation, a simpler approach was chosen: detect changes → if zero changes, return immediately (no-op fast path); if any changes, full rebuild. The algorithms are fast enough (<1s for 3k files) that incremental re-parsing provides negligible benefit without Phase 3's in-memory graph.
**Decision:** `ariadne update` performs a full rebuild when any changes are detected. The no-op fast path (zero changes) is the primary optimization. The delta module (`algo/delta.rs`) correctly computes changed/added/removed sets and the 5% threshold — this scaffolding is preserved for Phase 3's auto-update mechanism, which will benefit from incremental re-parsing due to its in-memory `GraphState`.
**Alternatives rejected:**
- Implement true incremental re-parse now — premature optimization; algorithms are fast, and the in-memory graph (Phase 3) is the correct place for incremental updates
- Remove `ariadne update` and use `ariadne build` — loses the no-op fast path, which is valuable for CI idempotency checks
**Reasoning:** Correctness over optimization. Full rebuild guarantees correct results. The no-op fast path provides the most common performance win (checking if anything changed). True incrementality is Phase 3 scope where the MCP server's in-memory graph makes partial updates worthwhile.
**Affects:** `pipeline/mod.rs` update(), `algo/delta.rs`, architecture.md §Algorithms §6.

## D-051: Tokio Isolated to Serve Subcommand

**Date:** 2026-03-19
**Status:** Accepted
**Context:** The MCP server requires an async runtime for rmcp's stdio transport and signal handling. All CLI commands (build, query, update) are synchronous.
**Decision:** Tokio runtime is created only inside the `Serve` match arm in `main.rs` via `Runtime::new().block_on()`. All other commands remain fully synchronous. The `serve` feature flag gates all async dependencies.
**Affects:** `src/main.rs`, `Cargo.toml` feature flags.

## D-052: ArcSwap for Lock-Free Graph State Reads

**Date:** 2026-03-19
**Status:** Accepted
**Context:** The MCP server needs concurrent read access to the graph state while background rebuilds update it.
**Decision:** Use `arc-swap` crate's `ArcSwap<GraphState>` for lock-free reads. Tool handlers call `state.load()` for a consistent snapshot. Background rebuilds construct a new `GraphState` and swap atomically via `state.store(Arc::new(new_state))`.
**Affects:** `src/mcp/state.rs`, `src/mcp/tools.rs`, `src/mcp/watch.rs`.

## D-053: Two-Level Freshness Confidence Scoring

**Date:** 2026-03-19
**Status:** Accepted
**Context:** Binary fresh/stale is too coarse. A file can change content without changing imports (body-only edit).
**Decision:** Two confidence levels: `hash_confidence` (any content change) and `structural_confidence` (import changes only). Body-only edits reduce hash confidence but not structural confidence. This lets consumers decide trust level.
**Affects:** `src/mcp/state.rs` FreshnessState.

## D-054: raw_imports.json Persistence

**Date:** 2026-03-19
**Status:** Accepted
**Context:** The freshness engine needs to compare current imports against stored imports for structural confidence.
**Decision:** Serialize raw imports to `raw_imports.json` during every build. `RawImportOutput` type in `serial/mod.rs` with path, symbols, and is_type_only fields.
**Affects:** `src/serial/mod.rs`, `src/serial/json.rs`, `src/pipeline/mod.rs`.

## D-055: rmcp 1.2 as MCP SDK

**Date:** 2026-03-19
**Status:** Accepted
**Context:** Need a Rust MCP server implementation for `ariadne serve`.
**Decision:** Use rmcp 1.2 (official Anthropic Rust SDK) with `#[tool_router]` / `#[tool_handler]` macros, `Parameters<T>` for structured tool inputs, and stdio transport. Server implements `ServerHandler` trait.
**Affects:** `src/mcp/tools.rs`, `src/mcp/server.rs`, `Cargo.toml`.

## D-056: Abstract File Classification

**Date:** 2026-03-19
**Status:** Accepted
**Context:** Martin metrics require classifying files as abstract or concrete to compute Abstractness (A).
**Decision:** A file is abstract if: (1) `FileType::TypeDef` (e.g., `.d.ts`, `.pyi`), OR (2) barrel file with >80% re-export ratio (`re_export_edges / exports > 0.8`). All other files are concrete. Simple, deterministic, avoids AST-level abstract/interface detection.
**Affects:** `src/analysis/metrics.rs`.

## D-057: Louvain Noise Filtering in Structural Diff

**Date:** 2026-03-19
**Status:** Accepted
**Context:** Louvain community detection is non-deterministic — cluster assignments can change without any structural change.
**Decision:** `changed_clusters` in StructuralDiff only includes files where the cluster assignment changed AND at least one edge was also added or removed. Pure Louvain re-assignments are filtered out.
**Affects:** `src/analysis/diff.rs`.

## D-058: StructuralDiff is MCP-Only

**Date:** 2026-03-19
**Status:** Accepted
**Context:** Structural diff requires a "before" snapshot to compare against.
**Decision:** `ariadne_diff` is MCP-only (no CLI equivalent). The MCP server holds the pre-update `Arc<GraphState>` in memory. CLI `ariadne update` doesn't persist previous state. `GraphState.last_diff` stores the diff from the last auto-update.
**Affects:** `src/mcp/state.rs`, `src/mcp/watch.rs`, `src/mcp/tools.rs`.

## D-059: ChangeClassification Heuristic

**Date:** 2026-03-19
**Status:** Accepted
**Context:** Need to classify structural changes for human-readable diff summaries.
**Decision:** Four categories: Breaking (removed edges + new cycles), Additive (only additions), Refactor (balanced add/remove, small magnitude), Migration (more removed than added). Default to Refactor for ambiguous cases. Heuristic, not authoritative.
**Affects:** `src/analysis/diff.rs`, `src/model/diff.rs`.

## D-060: Fiedler Vector Sign Convention

**Date:** 2026-03-19
**Status:** Accepted
**Context:** Eigenvectors are unique only up to sign. Need deterministic partition assignment.
**Decision:** The lexicographically first node (by `CanonicalPath` in BTreeMap order) always gets a positive Fiedler vector component. If the raw vector gives it a negative component, flip all signs. Zero additional computation cost.
**Affects:** `src/algo/spectral.rs`.

## D-061: PageRank on Original Import Graph

**Date:** 2026-03-19
**Status:** Accepted (revised from spec)
**Context:** The spec originally called for PageRank on the *reversed* import graph. Testing revealed this ranks importers high rather than foundations.
**Decision:** Run standard PageRank on the original import graph (A→B where A imports B). This naturally ranks B (the dependency/foundation) highest, matching the stated goal of "authority ranking: files that important files depend on." The spec's reversal was based on a web-PageRank analogy that doesn't apply to import graphs where edges already point toward dependencies.
**Affects:** `src/algo/pagerank.rs`.

## D-062: Token Estimation Heuristic

**Date:** 2026-03-19
**Status:** Accepted
**Context:** Need approximate token count for compressed graph views.
**Decision:** Simple heuristic: serialized JSON bytes / 4 ≈ tokens. Exact tokenization depends on the model's tokenizer and is not worth computing. Order-of-magnitude guidance is sufficient.
**Affects:** `src/algo/compress.rs`.

## D-063: PageRank Edge Filter

**Date:** 2026-03-19
**Status:** Accepted
**Context:** Not all edge types represent structural dependencies for importance ranking.
**Decision:** PageRank includes only `imports` and `re_exports` edges. Excludes `tests` (not runtime dependencies) and `type_imports` (type-only imports don't represent runtime dependency weight). This prevents test utility files and type definition files from having inflated importance.
**Affects:** `src/algo/pagerank.rs`.

## D-064: Spectral Analysis Without External Sparse Matrix Library

**Date:** 2026-03-19
**Status:** Accepted
**Context:** Spec proposed using `sprs` crate for Lanczos iteration. Decision gate: evaluate dependency cost.
**Decision:** Implemented spectral analysis using hand-rolled power iteration with deflation on the graph Laplacian. No external dependency needed — the Laplacian matrix-vector product is computed directly from adjacency lists. This avoids binary size increase and dependency management complexity. Deterministic via fixed initial vector and BTreeMap iteration order.
**Affects:** `src/algo/spectral.rs`, `Cargo.toml` (no new dependency).

## D-065: Extension-Aware Grammar Selection for TSX/JSX

**Date:** 2026-03-22
**Status:** Accepted
**Context:** `TypeScriptParser` used `LANGUAGE_TYPESCRIPT` grammar for all extensions including `.tsx`/`.jsx`. The TypeScript grammar doesn't parse JSX syntax — `tree-sitter-typescript` ships a separate `LANGUAGE_TSX` grammar for that. This caused W001 (full parse failure) on inline JSX arrow returns and W007 (partial parse) on `{{ }}` JSX props with adjacent text content.
**Decision:** Added `tree_sitter_language_for_ext(ext: &str)` method to `LanguageParser` trait with a default implementation delegating to `tree_sitter_language()`. `TypeScriptParser` overrides it to return `LANGUAGE_TSX` for `.tsx`/`.jsx` extensions. `ParserRegistry::parse_source()` and `reparse_imports()` now pass the file extension through to select the correct grammar.
**Alternatives rejected:**
- Separate `TsxParser` struct — duplicates all extraction logic, only the grammar differs
- Always use `LANGUAGE_TSX` — TSX grammar accepts non-JSX TypeScript but may have different performance/behavior characteristics
**Affects:** `src/parser/traits.rs`, `src/parser/typescript.rs`, `src/parser/registry.rs`, `src/pipeline/mod.rs`. Updates D-018 (6 methods on LanguageParser, not 5).

---

## D-066: FileType::Doc for Markdown Files

**Date:** 2026-03-23
**Status:** Accepted
**Context:** Markdown files contain cross-references to other project files via links. Need a FileType variant to classify `.md` files distinctly from Source/Asset.
**Decision:** Add `FileType::Doc` variant. Serializes as `"doc"`. Detection at Priority 4.5 (between Style and Asset). Extension match: `.md`.
**Affects:** `src/model/node.rs`, `src/detect/filetype.rs`.

---

## D-067: tree-sitter-md for Markdown Parsing

**Date:** 2026-03-23
**Status:** Accepted
**Context:** Need AST-based parsing for Markdown to reliably extract link references. tree-sitter-md 0.3 compatible with tree-sitter 0.24.
**Decision:** Add `tree-sitter-md = "0.3"` dependency. Use block grammar (`LANGUAGE`) only — inline grammar not needed for link extraction. Inline links extracted via pattern matching on `inline` node text.
**Affects:** `Cargo.toml`, `src/parser/markdown.rs`.

---

## D-068: ImportKind::Link for Markdown References

**Date:** 2026-03-23
**Status:** Accepted
**Context:** Markdown link references are semantically different from code imports. Need a distinct ImportKind to classify them.
**Decision:** Add `ImportKind::Link` variant. Used in `RawImport.kind` when extracting markdown links.
**Affects:** `src/parser/traits.rs`, `src/parser/markdown.rs`.

---

## D-069: EdgeType::References for Non-Architectural Links

**Date:** 2026-03-23
**Status:** Accepted
**Context:** Markdown cross-references are informational, not architectural dependencies. They should appear in the graph but not influence architectural analysis.
**Decision:** Add `EdgeType::References` variant. `is_architectural()` returns `false` for this type. Serializes as `"references"`.
**Affects:** `src/model/edge.rs`, pipeline edge construction.

---

## D-070: Rust Parser — `collect_path_segments` Priority Fix

**Date:** 2026-03-23
**Status:** Accepted
**Context:** All `use crate::` imports were silently dropped during Rust parsing. Root cause: in `collect_path_segments`, the or_else chain checked `find_child_by_kind("identifier")` before `find_child_by_kind("crate")`. For `scoped_identifier "crate::model"` (children: `crate`, `::`, `identifier "model"`), the `identifier` match fired first, `crate` was never found, producing segments `["model", "model"]` which `is_skip_import` classified as an external crate.
**Decision:** Reorder the or_else chain to check `crate`/`super`/`self` before `identifier`. This ensures Rust keyword nodes are matched before generic identifiers.
**Affects:** `src/parser/rust_lang.rs` — `collect_path_segments`.

---

## D-071: Rust Resolver — Walk-Back Path Probing

**Date:** 2026-03-23
**Status:** Accepted
**Context:** `use crate::model::CanonicalPath` resolves to fs path `src/model/CanonicalPath`. But `CanonicalPath` is a type exported from `src/model/mod.rs`, not a file. The resolver tried `src/model/CanonicalPath.rs` and `src/model/CanonicalPath/mod.rs`, both non-existent, and returned None.
**Decision:** Add `probe_rust_path_with_walkback`: after exact probe fails, trim the last path segment (likely a symbol name) and retry, repeating until a file is found or segments are exhausted. For `src/model/CanonicalPath` → trims to `src/model` → finds `src/model/mod.rs`.
**Affects:** `src/parser/rust_lang.rs` — `RustResolver`.

---

## D-072: Rust Crate Name Detection for Self-Referencing Imports

**Date:** 2026-03-23
**Status:** Accepted
**Context:** `src/main.rs` uses `use ariadne_graph::foo` (the crate's own name from Cargo.toml), not `use crate::foo`. The parser treated `ariadne_graph` as an external crate and skipped these imports, leaving `main.rs` as an orphan node with zero edges.
**Decision:** Three-part fix:
1. `detect_rust_crate_name(root)` reads Cargo.toml `[package] name`, replaces `-` with `_` (Rust convention).
2. `RustParser::with_crate_name(name)` stores the crate name; `is_skip_import` treats it as internal (like `crate`/`super`/`self`).
3. Pipeline rewrites matching imports before resolution: `ariadne_graph::foo` → `crate::foo`.
**Alternatives rejected:**
- Modifying the `LanguageParser` trait to accept crate name — too invasive, changes all language parsers.
- Removing `is_skip_import` entirely — would count all external crate imports as "unresolved", inflating diagnostics.
**Affects:** `src/detect/workspace.rs`, `src/parser/rust_lang.rs`, `src/parser/registry.rs`, `src/pipeline/build.rs`, `src/main.rs`.

---

## D-073: Louvain Guard — Directory Cluster Retention

**Date:** 2026-03-23
**Status:** Accepted
**Context:** Louvain community detection on hub-heavy codebases (Rust mod.rs pattern, lib.rs fan-out) merges all src/ directories into a single mega-cluster. This is mathematically correct (modularity maximization) but unhelpful for navigation — loses the directory-level structure that developers use.
**Decision:** After Louvain runs, compare result cluster count to directory-based count. If Louvain produced fewer than 50% of the directory clusters, discard Louvain results and keep directory-based clusters. This preserves navigable structure while still using Louvain when it genuinely finds better communities.
**Alternatives rejected:**
- Changing Louvain default to `--no-louvain` — throws away Louvain for all codebases, even those where it helps.
- High default gamma (resolution parameter) — doesn't help; even gamma=50 fails to split hub-connected clusters.
**Affects:** `src/pipeline/mod.rs` (Stage 5b).

---

## D-074: Louvain Resolution Parameter (Gamma)

**Date:** 2026-03-23
**Status:** Accepted
**Context:** Standard Louvain uses gamma=1.0 in the modularity formula. Higher gamma penalizes merging more, producing finer-grained communities. Different codebases benefit from different values.
**Decision:** Add `--resolution <gamma>` CLI flag for `build` and `update` commands. Default: 1.0 (standard modularity). Threaded through pipeline to `louvain_clustering_with_resolution()`. Formula: `Q = (1/2m) * Σ [A_ij - γ * k_i*k_j/(2m)] * δ(c_i, c_j)`.
**Affects:** `src/algo/louvain.rs`, `src/pipeline/mod.rs`, `src/main.rs` (CLI).

## D-075: FileType::Data Variant for Structured Data Files

**Date:** 2026-03-24
**Status:** Accepted
**Context:** JSON and YAML files were previously classified as `FileType::Asset`, lumping them with binary assets like images and fonts. Structured data files are semantically distinct — they are parseable text files that other source files may import.
**Decision:** Add `FileType::Data` variant (appended after `Doc` to minimize Ord impact). Detection priority 4.75 — between Doc (4.5) and Asset (5). Config filenames (e.g., `package.json`, `docker-compose.yml`) remain `FileType::Config` at priority 1. Removed `json`, `yaml`, `yml` from `is_asset_file()`.
**Affects:** `src/model/node.rs`, `src/detect/filetype.rs`, `src/serial/convert.rs`.

## D-076: JSON and YAML Tree-Sitter Parsers with No-Dependency Semantics

**Date:** 2026-03-24
**Status:** Accepted
**Context:** JSON and YAML files should appear as nodes in the dependency graph so that edges from source files importing them are visible. However, JSON/YAML files themselves have no import/export semantics.
**Decision:** Add `json_lang.rs` and `yaml.rs` no-op parsers following the `markdown.rs` pattern. Both return empty import/export vectors. No new `ImportKind` or `EdgeType` variants. Registered in `with_tier1_config()`. Dependencies: `tree-sitter-json = "0.24"`, `tree-sitter-yaml = "0.7"`. `json_lang.rs` naming follows `rust_lang.rs` precedent for disambiguation.
**Affects:** `src/parser/json_lang.rs` (new), `src/parser/yaml.rs` (new), `src/parser/mod.rs`, `src/parser/registry.rs`, `Cargo.toml`.

## D-077: SymbolExtractor as Separate Trait from LanguageParser

**Date:** 2026-03-24
**Status:** Accepted
**Context:** Phase 4 introduces symbol-level extraction (functions, classes, structs, interfaces, etc.) from source files. This could be added to the existing `LanguageParser` trait or kept separate.
**Decision:** `SymbolExtractor` is a separate trait (`src/parser/symbols.rs`) mirroring D-018's pattern. Receives the same `tree_sitter::Tree` already parsed by `LanguageParser` — no re-parsing. Registered in `ParserRegistry` via `register_symbol_extractor()` parallel to `register()`. Implemented for TypeScript/JS, Rust, and Go. Languages without extractors produce empty symbol lists.
**Alternatives rejected:**
- Adding methods to `LanguageParser` — violates single-responsibility, forces all language parsers to implement symbol extraction even when not supported.
- Separate pass with re-parsing — wasteful; tree-sitter trees are cheap to reuse.
**Reasoning:** Keeps existing parsers unchanged, enables incremental rollout per language, and follows the established trait separation pattern.
**Affects:** `src/parser/symbols.rs` (new), `src/parser/typescript.rs`, `src/parser/rust_lang.rs`, `src/parser/go.rs`, `src/parser/registry.rs`, `src/pipeline/mod.rs`.

## D-081: Language-Native Signature Format with 200-Char Truncation

**Date:** 2026-03-24
**Status:** Accepted
**Context:** Symbol definitions benefit from a signature field showing the declaration's source text (function signature, struct definition, etc.). Signatures can be arbitrarily long.
**Decision:** Signature is the first line of the node's source text, in the language's native syntax. Truncated to 200 characters with `...` suffix. Stored as `Option<String>` — `None` when not applicable (e.g., module declarations). Serialized with `skip_serializing_if = "Option::is_none"`.
**Affects:** `src/model/symbol.rs`, all `SymbolExtractor` implementations.

## D-078: SymbolIndex Built at Load Time (Not Persisted)

**Date:** 2026-03-25
**Status:** Accepted
**Context:** Phase 4b introduces a SymbolIndex for cross-project symbol lookup. The index could be persisted to disk or rebuilt at load time.
**Decision:** `SymbolIndex` is built in `GraphState::from_loaded_data()` from `Node.symbols` and `edges`, following the same pattern as `reverse_index`, `forward_index`, and `layer_index`. Not persisted to a separate file. Rebuilt on auto-update alongside other derived indices. Uses `BTreeMap` for deterministic iteration.
**Alternatives rejected:**
- Persisted index file — adds I/O complexity, version management, and staleness concerns for marginal benefit since the build is O(n) on symbols.
- Lazy build on first query — complicates concurrency model and adds latency to first MCP tool call.
**Reasoning:** Consistent with existing GraphState pattern. Build time is negligible compared to PageRank/spectral which already run at load time. Eliminates cache invalidation concerns.
**Affects:** `src/model/symbol_index.rs` (new), `src/mcp/state.rs`.

## D-080: Symbol Search Uses Case-Insensitive Substring (Not Regex)

**Date:** 2026-03-25
**Status:** Accepted
**Context:** The `ariadne_symbol_search` MCP tool needs a query mechanism. Options: exact match, substring, glob, regex.
**Decision:** Case-insensitive substring match. No regex support. Early termination at 100 results. Optional kind filter (parse SymbolKind from string). Results sorted deterministically via BTreeMap iteration.
**Alternatives rejected:**
- Regex — complexity and potential DoS with pathological patterns. Not needed for symbol lookup.
- Exact match — too restrictive for exploratory use.
- Fuzzy match — complex to implement deterministically, overkill for structural analysis.
**Reasoning:** Substring match covers 95% of use cases (finding all "Service" symbols, all "auth" functions). Case-insensitivity matches developer expectations. 100-result cap prevents unbounded output.
**Affects:** `src/model/symbol_index.rs`, `src/mcp/tools.rs`.

## D-079: Cross-File Call Graph via Import Matching

**Date:** 2026-03-25
**Status:** Accepted
**Context:** Phase 4c requires a cross-file call graph for symbol-level dependency analysis. Two approaches: (1) analyze function bodies for call sites, (2) match imported symbol names against SymbolDefs in the target file.
**Decision:** Cross-file via imports ONLY. For each architectural edge (Imports/TypeImports/ReExports), match imported symbol names against `SymbolIndex.symbols_for_file(target)`. If found, create bidirectional `CallEdge` entries (caller and callee maps). `CallGraph` is a leaf module in `algo/` — only imported by `mcp/state.rs` and `mcp/tools.rs`. Built at load time alongside `SymbolIndex`. Uses `BTreeMap` for deterministic output.
**Alternatives rejected:**
- Intra-file body analysis — requires tree-sitter re-parsing at load time, significant complexity, and breaks the architectural-edges-only principle.
- Persisted call graph — unnecessary given O(n) build time from edges + symbol index.
**Reasoning:** Leverages existing edge symbols and SymbolIndex. Consistent with Ariadne's structural (not semantic) analysis philosophy. Avoids false positives from intra-file analysis.
**Affects:** `src/algo/callgraph.rs` (new), `src/mcp/state.rs`, `src/mcp/tools.rs`.

## D-082: Token-Budget-Aware Context Assembly

**Date:** 2026-03-25
**Status:** Accepted
**Context:** Phase 5 introduces `ariadne_context`, which must assemble file context within a token budget. Naive approaches either exceed the budget or leave it underutilized.
**Decision:** Two-phase approach: (1) BFS from seed files to generate all candidates within the expansion depth, scoring each by relevance (distance, centrality, task weight). (2) Greedy selection sorted by tier then relevance/tokens ratio, filling the budget. Target files are always included (even if they exceed the budget). Budget default: 8000 tokens. Token estimation: lines * 8 (D-062). Exposed via `ariadne_context` MCP tool with `budget_tokens` parameter.
**Alternatives rejected:**
- Knapsack optimization — unnecessary complexity for marginal improvement over greedy selection.
- Fixed file count — ignores file size variance; a 10-line util and a 500-line service should not count equally.
**Reasoning:** Greedy with tier-first ordering ensures high-priority context (targets, direct deps) is always included. Token budget maps directly to LLM context window constraints.
**Affects:** `src/algo/context.rs`, `src/mcp/tools.rs`, `src/mcp/tools_context.rs`.

## D-083: Task-Type-Aware Relevance Scoring

**Date:** 2026-03-25
**Status:** Accepted
**Context:** Different development tasks benefit from different context. A bug fix needs test files; a refactor needs interface files; understanding needs high-centrality files.
**Decision:** Five task types: `AddField`, `Refactor`, `FixBug`, `AddFeature`, `Understand`. Each applies a weight multiplier to relevance scores: FixBug boosts test files (1.5x), Refactor boosts tests (1.3x), AddField boosts interfaces/type definitions (1.5x), Understand boosts high-centrality (1.3x), AddFeature is neutral (1.0x). Task type is optional — defaults to `Understand`. Parsed case-insensitively from string via `TaskType::from_str`.
**Alternatives rejected:**
- Per-task BFS strategies — too complex, unclear benefit over simple weight multipliers.
- User-defined weights — premature customization; the 5 built-in types cover common workflows.
**Reasoning:** Simple multipliers are deterministic, composable with distance and centrality scores, and easy to tune. The five task types align with common agent workflow patterns.
**Affects:** `src/algo/context.rs`, `src/mcp/tools.rs`, `src/mcp/tools_context.rs`.

## D-084: Phase 5 Spec Simplification — Implementation Over Spec

**Date:** 2026-03-25
**Status:** Accepted
**Context:** Phase 5 (Agent Context Engine) spec defined a rich output schema with nested `interfaces`, `tests`, `related_configs`, `warnings` arrays, `suggested_review_files` in plan_impact, and conditional task-type weight logic. The implementation simplified all of these to a flat tier-based candidate model with uniform weight application.
**Decision:** Update specs to match implementation rather than adding unproven complexity. Specific simplifications:
1. Context output schema uses flat `ContextEntry` list with tier/relevance/tokens instead of nested category arrays
2. `plan_impact` omits `suggested_review_files` — blast radius already covers this use case
3. `TaskType` weight multipliers apply unconditionally (simplified from conditional per-tier logic)
**Alternatives rejected:**
- Implementing the full spec schema — adds complexity without proven benefit; simpler schema is consumed successfully by MCP clients
- Keeping spec as aspirational target — creates confusion between spec and reality
**Reasoning:** Simplified implementations are in production, consumed by MCP clients, and working well. Adding complexity without proven benefit is premature optimization. Specs should reflect reality.
**Affects:** Phase 5 spec (archived), `src/algo/context.rs`, `src/algo/impact.rs`, `src/mcp/tools.rs`.

## D-085: RAII Lock Guard Pattern for MCP Server

**Date:** 2026-03-25
**Status:** Accepted
**Context:** MCP server must prevent concurrent instances on the same project. Lock files must be reliably released even on panics or unexpected exits.
**Decision:** `LockGuard` struct in `src/mcp/lock.rs` acquires a `.lock` file on creation, releases on `Drop`. Server acquires lock at startup; RAII guarantees cleanup. Stale locks from dead processes are detected and removed with W016 warning.
**Alternatives rejected:**
- Manual lock/unlock calls — error-prone, risks orphaned locks on panics
- Advisory file locking (flock) — not portable across all platforms
**Reasoning:** RAII is idiomatic Rust for resource management. Drop-based cleanup handles all exit paths including panics.
**Affects:** `src/mcp/lock.rs`, `src/mcp/server.rs`.

## D-086: Inline Test Detection via Symbol Data

**Date:** 2026-03-25
**Status:** Accepted
**Context:** Rust uses `#[cfg(test)] mod tests` inline in source files. Test detection based solely on `FileType::Test` misses inline tests. Phase 4 symbols provide `SymbolKind::Test` markers.
**Decision:** Context assembly (`src/algo/context.rs`) uses `node.file_type == FileType::Test` for test file boosting in task-aware scoring. The `tests_for` tool (`src/algo/test_map.rs`) additionally checks symbol data for inline test detection. Both approaches coexist — file-level for context scoring, symbol-level for precise test mapping.
**Alternatives rejected:**
- Only symbol-based detection — requires symbol extraction to have succeeded; file-type-based detection is more robust as a fallback
**Reasoning:** Layered approach: fast file-type check for scoring, precise symbol check for test mapping.
**Affects:** `src/algo/context.rs`, `src/algo/test_map.rs`.

## D-087: Unchanged Diff Classification Variant

**Date:** 2026-03-25
**Status:** Accepted
**Context:** `ChangeClassification` in `src/model/diff.rs` needs to handle the case where a structural diff detects zero changes (no added/removed nodes, no added/removed edges, no new cycles).
**Decision:** Added `ChangeClassification::Unchanged` variant. Returned when all diff vectors are empty. This is distinct from `Additive` (which requires at least one addition). Enables MCP `ariadne_diff` tool to distinguish "no changes" from "changes happened."
**Alternatives rejected:**
- Returning `None`/null for unchanged — loses type information, complicates client handling
- Omitting classification when unchanged — inconsistent API surface
**Reasoning:** Explicit variant follows Rust's "make invalid states unrepresentable" principle.
**Affects:** `src/model/diff.rs`, `src/analysis/diff.rs`.

## D-088: Depth Sentinel Null Mapping in JSON API

**Date:** 2026-03-25
**Status:** Accepted
**Context:** MCP tool `ariadne_diff` returns `None` when no diff has occurred since last auto-update. JSON serialization must distinguish "no diff computed yet" from "diff computed but empty."
**Decision:** `None` maps to JSON `"null"` string literal in the `diff()` MCP tool handler. `Some(diff)` is serialized via `to_json()`. This preserves the distinction at the API level.
**Alternatives rejected:**
- Returning empty JSON object `{}` — ambiguous, could be confused with an empty diff result
- Using `Option<String>` in the MCP tool return — MCP tool framework requires `String` return type
**Reasoning:** Simple and unambiguous. MCP clients can check for the literal `"null"` string to know no diff is available.
**Affects:** `src/mcp/tools.rs`.

## D-089: Resource and Prompt Registration via Manual ServerHandler Override

**Date:** 2026-03-25
**Status:** Accepted
**Context:** MCP resources and prompts need to be served alongside tools. rmcp's `#[tool_handler]` macro only generates tool-related methods on the `ServerHandler` impl. No equivalent macro exists for resources or prompts.
**Decision:** Resources and prompts are registered via manual `list_resources`/`read_resource`/`list_prompts`/`get_prompt` method overrides in the `ServerHandler` impl. The `#[tool_handler]` macro continues to generate tool methods; resource/prompt methods are manually overridden in the same impl block.
**Alternatives rejected:**
- Separate ServerHandler impls — Rust's orphan rules and rmcp's design require a single ServerHandler per server
- Custom macro for resources/prompts — over-engineering for a small number of resources
**Reasoning:** rmcp has no resource/prompt router macros; single ServerHandler constraint means manual overrides are the only option. Straightforward and explicit.
**Affects:** `src/mcp/server.rs`, `src/mcp/tools.rs`.

## D-090: Annotation and Bookmark State Separate from GraphState

**Date:** 2026-03-25
**Status:** Accepted
**Context:** Annotations and bookmarks are user-created data. GraphState is derived data rebuilt on every file watcher trigger. These have fundamentally different lifecycles.
**Decision:** Separate `UserState` stored in `Arc<ArcSwap<UserState>>` alongside the existing `Arc<ArcSwap<GraphState>>`. UserState is not rebuilt on file watcher triggers; it has an independent lifecycle managed by annotation/bookmark tool handlers.
**Alternatives rejected:**
- Embedding user data in GraphState — would be destroyed on every auto-rebuild
- Separate Mutex-guarded state — ArcSwap is already proven in the codebase and provides lock-free reads
**Reasoning:** User-created data (annotations, bookmarks) has a different lifecycle from derived graph data. Keeping them separate prevents accidental data loss during rebuilds.
**Affects:** `src/mcp/state.rs`, `src/mcp/annotations.rs`, `src/mcp/bookmarks.rs`.

## D-091: Shared Persistence Module `JsonStore<T>`

**Date:** 2026-03-25
**Status:** Accepted
**Context:** Both annotation and bookmark stores need to persist data as JSON files with atomic writes. Duplicating file I/O logic across stores violates DRY.
**Decision:** Generic `JsonStore<T>` in `src/mcp/persist.rs` for atomic JSON file I/O. Provides `load()` and `save()` methods. Atomic write uses temp file + rename pattern to prevent corruption on crashes.
**Alternatives rejected:**
- Per-store file I/O — duplicated logic, inconsistent error handling
- SQLite — overkill for simple key-value JSON persistence
**Reasoning:** DRY — both annotation and bookmark stores need identical atomic JSON I/O. Generic type parameter keeps it reusable for future persistent state.
**Affects:** `src/mcp/persist.rs`, `src/mcp/annotations.rs`, `src/mcp/bookmarks.rs`.

## D-092: Annotation/Bookmark Types in `src/model/`

**Date:** 2026-03-25
**Status:** Accepted
**Context:** Annotation and bookmark data types need a home. They could live in `src/mcp/` (near their handlers) or `src/model/` (with other data types).
**Decision:** Pure data types (`Annotation`, `AnnotationTarget`, `Bookmark`) defined in `src/model/` following the precedent of `ArchSmell`, `SymbolDef`, and other domain types. MCP-specific logic (tool handlers, persistence) stays in `src/mcp/`.
**Alternatives rejected:**
- Types in `src/mcp/` — would create MCP dependency for any module that needs annotation/bookmark types
**Reasoning:** Consistent with project pattern of keeping pure data types in `model/` with no infrastructure dependencies.
**Affects:** `src/model/annotation.rs`, `src/model/bookmark.rs`, `src/model/mod.rs`.

## D-093: Resource Refresh via `list_changed` Capability

**Date:** 2026-03-25
**Status:** Accepted
**Context:** MCP resources should reflect the current graph state. When the graph is rebuilt (file watcher trigger), clients need to know resources have changed.
**Decision:** Enable `list_changed` flag in `ServerCapabilities`. This signals to MCP clients that the resource list may change over time. Push notification (active `notifications/resources/list_changed` messages) deferred; clients poll on the capability flag.
**Alternatives rejected:**
- Full push notification system — requires subscription tracking and notification dispatch; can be added incrementally
- No capability flag — clients have no way to know resources can change
**Reasoning:** One-line capability flag signals clients that resources are dynamic. Full push notification can be added incrementally without API changes.
**Affects:** `src/mcp/server.rs`.

## D-094: Query-Time Bookmark Path Expansion

**Date:** 2026-03-25
**Status:** Accepted
**Context:** Bookmarks store directory prefixes (e.g., `src/auth/`). These need to be expanded to actual file paths for subgraph extraction and context assembly.
**Decision:** Expand directory prefixes at each query, not at bookmark creation time. When a bookmark is used (e.g., in `ariadne_subgraph` or `ariadne_context`), its paths are expanded against the current graph state to find all matching files.
**Alternatives rejected:**
- Expand at creation time and store full file list — becomes stale as files are added/removed
- Re-expand on every graph rebuild — unnecessary work for bookmarks that may never be queried
**Reasoning:** Dynamic expansion means bookmarks track the living codebase. New files under a bookmarked directory are automatically included; deleted files are automatically excluded.
**Affects:** `src/mcp/bookmarks.rs`, `src/algo/compress.rs`, `src/algo/context.rs`.
