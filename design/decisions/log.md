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
**Status:** Accepted
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
**Decision:** Ariadne always produces one graph per invocation (entire repo). Workspace detection scans for root-level indicators (package.json workspaces, go.work, Cargo.toml workspace, nx.json, pnpm-workspace.yaml). Detected workspace members are mapped (package name → path → entry point). Import resolution checks workspace map before classifying an import as external. Phase 1 covers npm/yarn/pnpm workspaces; other workspace types added incrementally. No workspace indicators → simple single-project resolution (fully backward compatible).
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
