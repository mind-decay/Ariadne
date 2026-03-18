# Phase 1b: Hardening — Specification

## Goal

Production-quality error handling, full CLI, workspace support, comprehensive tests, CI/CD.

## Dependencies

**Phase 1a must be complete.** Phase 1b builds on:

- Core data model with newtypes (CanonicalPath, ContentHash, ClusterId, Symbol)
- 6 Tier 1 language parsers with LanguageParser + ImportResolver traits
- Pipeline architecture with injectable stages (FileWalker, FileReader, GraphSerializer)
- DiagnosticCollector for thread-safe warning aggregation
- File type detection + architectural layer inference
- Directory-based clustering
- JSON serialization (deterministic, sorted)
- CLI: `ariadne build <path> [--output <dir>]` and `ariadne info`
- Basic tests: parser snapshots (L1), fixture graph snapshots (L2), invariant checks (L3 basic)

## Risk Classification

**Overall: YELLOW**

Phase 1b is entirely additive — no Phase 1a critical components are rewritten. All YELLOW items have clear designs and mitigations. No RED items exist because the data model, parsing, and pipeline architecture remain unchanged.

### Per-Deliverable Risk

| # | Deliverable | Risk | Rationale |
|---|------------|------|-----------|
| 1 | Structured warning system (W001-W009, human + JSON format) | GREEN | Core warning system exists in 1a. Phase 1b adds JSON formatter — purely additive |
| 2 | CLI flags (--verbose, --warnings, --strict, --timestamp, --max-file-size, --max-files) | YELLOW | Coordination across main.rs + pipeline. Each flag is simple but threading config through stages requires care |
| 3 | npm/yarn/pnpm workspace detection | YELLOW | Must parse package.json, expand glob patterns, build member map. Well-specified in path-resolution.md but more moving parts than greenfield items |
| 4 | Workspace-aware TS/JS import resolution | YELLOW | Extends existing resolver — additive. But `ImportResolver::resolve` signature change ripples across all 6 resolvers (mechanical update, compiler catches misses) |
| 5 | Path normalization + case-insensitive FS detection | GREEN | Well-designed in path-resolution.md. Small, focused utility with clear test cases |
| 6 | Per-stage --verbose timing output | GREEN | Trivial `Instant::now()` wrapping once --verbose flag exists |
| 7 | Property-based tests (proptest) | GREEN | Architecture already testable via injectable traits. Clear scenarios from testing.md |
| 8 | Performance benchmarks (criterion) | GREEN | Targets defined in performance.md. Standard criterion usage |
| 9 | GitHub Actions CI + release workflows | YELLOW | Multi-platform builds (Linux, macOS x64/arm64). Standard Rust CI patterns but operational complexity |
| 10 | install.sh + README.md | GREEN | No code dependencies. Can be written in parallel with everything else |

## Deliverables

### D1: Structured Warning System (W001-W009, Human + JSON Format)

**Files:** `src/diagnostic.rs` (expand existing)

Expand the existing `DiagnosticCollector` and `Warning` types to support two output formats:

- **Human format (default, stderr):** `warn[W001]: failed to parse src/foo.ts: unexpected token at line 42` followed by summary line
- **JSON format (`--warnings json`, stderr):** `{"level":"warn","code":"W001","file":"src/foo.ts","message":"parse failed","detail":"unexpected token at line 42"}` — one JSON object per line (JSONL)

Summary report always printed to stdout:

``` bash
Built graph: 847 files, 2341 edges, 12 clusters in 1.2s
  3 files skipped (1 parse error, 1 permission denied, 1 too large)
  42 imports unresolved (external packages)
```

Warning codes: W001 (ParseFailed), W002 (ReadFailed), W003 (FileTooLarge), W004 (BinaryFile), W005 (reserved), W006 (ImportUnresolved — verbose only), W007 (PartialParse), W008 (ConfigParseFailed), W009 (EncodingError).

**Determinism note:** Cluster cohesion values (`f64`) must be rounded to 4 decimal places before serialization to ensure byte-identical JSON output across platforms (per determinism.md §Floating-Point Determinism).

### D2: CLI Flags

**Files:** `src/main.rs` (clap definitions), `src/pipeline/mod.rs` (config threading)

| Flag | Type | Default | Source |
|------|------|---------|--------|
| `--verbose` | bool | false | error-handling.md, performance.md |
| `--warnings <format>` | enum(human, json) | human | error-handling.md |
| `--strict` | bool | false | error-handling.md |
| `--timestamp` | bool | false | determinism.md |
| `--max-file-size <bytes>` | u64 | 1048576 | error-handling.md |
| `--max-files <count>` | usize | 50000 | error-handling.md |

`--strict` makes build exit code 1 if any warnings occurred. `--timestamp` adds `"generated"` field (ISO 8601 UTC, seconds precision, Z suffix, e.g. `"2026-03-18T14:23:45Z"`) to graph.json output. `--verbose` enables W006 warnings and per-stage timing. `--max-file-size` and `--max-files` populate existing `WalkConfig`.

**Flag interaction rules (D-030):**
- `--strict` and `--warnings` are orthogonal. `--strict --warnings json` outputs JSON warnings AND exits with code 1.
- `--verbose` and `--warnings json` are orthogonal. Per-stage timing is always human-readable stderr; `--warnings` controls warning format only.
- `--max-files` counts files at the walk stage (all files encountered, regardless of extension). This prevents unbounded filesystem traversal.

### D3: npm/yarn/pnpm Workspace Detection

**Files:** `src/model/workspace.rs` (new — pure data types), `src/detect/workspace.rs` (new — detection logic)

Types (live in `model/` per dependency rule — leaf module):

```rust
WorkspaceInfo { kind: WorkspaceKind, members: Vec<WorkspaceMember> }
WorkspaceMember { name: String, path: PathBuf, entry_point: PathBuf }
enum WorkspaceKind { Npm, Yarn, Pnpm }
```

Detection logic:

1. Scan project root for `package.json` with `"workspaces"` field → npm/yarn
2. Scan for `pnpm-workspace.yaml` → pnpm
3. For each workspace member: read its `package.json`, extract `name` and entry point
4. Build map: `package_name → (path, entry_point)`
5. No workspace indicators → return `None` (backward compatible, single-project resolution)
6. If workspace config parsing fails → W008, fall back to non-workspace mode
7. If two members have the same package name → use first-found (sorted directory order), emit W008 warning (D-029)

**Scope:** npm/yarn/pnpm only in Phase 1b. Go/Cargo/Nx/Turbo deferred to later phases per path-resolution.md. `WorkspaceInfo` is intentionally npm-family-specific — future workspace types will require model extension (D-028).

### D4: Workspace-Aware TS/JS Import Resolution

**Files:** `src/parser/typescript.rs` (extend resolver), `src/parser/traits.rs` (signature update)

Update `ImportResolver::resolve` to accept `Option<&WorkspaceInfo>`:

```rust
fn resolve(&self, import: &RawImport, from_file: &CanonicalPath, known_files: &FileSet, workspace: Option<&WorkspaceInfo>) -> Option<CanonicalPath>;
```

All 6 resolver implementations updated (mechanical — non-TS resolvers receive `None` and ignore it). TS/JS resolver checks workspace map for scoped imports (`@scope/name`) before classifying as external.

Resolution for workspace imports:

- `import "@myapp/auth"` → resolve to member's entry point
- `import "@myapp/auth/utils"` → resolve subpath within member directory (extension probing reused)
- Non-workspace scoped package (`@types/react`) → external, skip

Entry point preference (D-027): `main` → `module` → default probe (`src/index.ts`, `index.ts`). Full `exports` field parsing deferred — too complex for Phase 1b, rarely needed for monorepo cross-package resolution.

### D5: Path Normalization + Case-Insensitive FS Detection

**Files:** `src/model/types.rs` (CanonicalPath constructor hardening), new path utilities in `src/detect/`

Path normalization enforces canonical format (per path-resolution.md):

1. Relative to project root
2. Forward slashes only
3. No leading `./`, no `.`/`..` segments, no trailing slash, no double slashes
4. Preserves filesystem case

Case-insensitive FS detection:

- Check once per build (create temp file, test with swapped case)
- Cache result for the build
- On case-insensitive FS: resolution tries exact match first, then case-insensitive fallback
- On case-sensitive FS: exact match only
- Detection failure → assume case-sensitive (safer default)

Path traversal protection: resolved paths must be within project root.

**Dependency:** `dunce` crate for Windows path canonicalization (no-op on Unix).

### D6: Per-Stage --verbose Timing Output

**Files:** `src/pipeline/mod.rs`

When `--verbose` is set, print per-stage timing to stderr (format from performance.md):

``` bash
[walk]      42ms    3,247 files found
[read+hash] 198ms   3,201 files read (46 skipped)
[parse]     2,341ms 3,201 files parsed (12 warnings)
[resolve]   876ms   8,432 edges created (142 unresolved)
[cluster]   34ms    18 clusters
[serialize] 412ms   graph.json (2.1MB) + clusters.json (24KB)
[total]     3,903ms
```

### D7: Property-Based Tests (proptest)

**Files:** `tests/properties.rs` (new), `tests/invariants.rs` (enhance)

Property-based tests via `proptest` crate:
- Generate random valid TypeScript files → parse → verify RawImport/RawExport validity
- Generate random directory structures → cluster → verify INV-4 through INV-7
- Generate random file contents → hash → verify determinism (INV-10)
- Build random graphs → verify all 13 invariants (INV-1 through INV-13)

Full L3 invariant suite (Phase 1a had basic INV-1, 2, 8, 9, 11; Phase 1b adds INV-3 through INV-7, INV-10, INV-12, INV-13).

**Dependency:** `proptest` crate (dev-dependency).

### D8: Performance Benchmarks (criterion)

**Files:** `benches/build_bench.rs`, `benches/parser_bench.rs`, `benches/helpers.rs`

Benchmarks with targets from performance.md:

| Benchmark | Target | Regression threshold |
|-----------|--------|---------------------|
| 100 files build | <200ms | >20% slower |
| 1000 files build | <3s | >20% slower |
| 3000 files build | <10s | >20% slower |
| Single file parse (TS, 50 imports) | <5ms | >50% slower |
| Single file parse (Go, 30 imports) | <3ms | >50% slower |
| Single file parse (Python, 40 imports) | <3ms | >50% slower |
| xxHash64 1MB file | <1ms | >100% slower |
| Clustering 3000 nodes | <100ms | >50% slower |
| JSON serialization 3000 nodes | <500ms | >50% slower |

Synthetic project generation helper: `generate_synthetic_project(file_count, dir_count, imports_per_file, language) -> TempDir`.

**Dependency:** `criterion` crate with `html_reports` feature (dev-dependency).

### D9: GitHub Actions CI + Release Workflows

**Files:** `.github/workflows/ci.yml`, `.github/workflows/release.yml`

**CI workflow (on push/PR to main):**
- `cargo test --lib --tests` (L1 + L2 + L3)
- `cargo insta test --check` (snapshot validation)
- `cargo fmt -- --check`
- `cargo clippy -- -D warnings`
- `cargo bench` (on main only, store baseline)

**Release workflow (on tag push `v*`):**
- Build release binaries: linux-x64, darwin-x64, darwin-arm64
- Create GitHub Release with artifacts
- Publish to crates.io as `ariadne-graph`

### D10: install.sh + README.md

**Files:** `install.sh` (project root), `README.md` (project root)

**install.sh:** Platform-detecting shell script that downloads the correct binary from GitHub Releases and installs to `/usr/local/bin` (or `$INSTALL_PATH`).

**README.md:** Quick-start documentation covering what Ariadne is, installation (cargo install, binary download, install.sh), usage examples, supported languages, output format, limitations, performance characteristics, and license (MIT/Apache-2.0).

## Design Sources

| Deliverable | Authoritative Sources |
|-------------|----------------------|
| D1: Warning system | error-handling.md (full spec), D-005, D-021 |
| D2: CLI flags | error-handling.md §CLI Flags, architecture.md §CLI Interface, determinism.md §Timestamp, D-030 |
| D3: Workspace detection | path-resolution.md §Monorepo Support, architecture.md §WorkspaceInfo, D-008, D-028, D-029 |
| D4: Workspace-aware resolution | path-resolution.md §Per-Language Workspace Resolution, D-008, D-018, D-027 |
| D5: Path normalization | path-resolution.md §Path Normalization + §Case Sensitivity, D-007 |
| D6: Verbose timing | performance.md §Built-in Timing |
| D7: Property-based tests | testing.md §L3: Graph Invariant Tests |
| D8: Performance benchmarks | testing.md §L4: Performance Tests, performance.md §Benchmark Targets |
| D9: CI/CD | testing.md §CI Integration |
| D10: install.sh + README | architecture.md §Installation |

## Success Criteria

1. `ariadne build . --verbose` prints per-stage timing to stderr
2. `ariadne build . --warnings json` outputs warnings as JSONL to stderr
3. `ariadne build . --strict` exits with code 1 if any warnings occurred
4. `ariadne build . --timestamp` includes `"generated"` field in graph.json
5. `ariadne build . --max-file-size 1000 --max-files 100` respects both limits
6. `ariadne build` on `workspace-project/` fixture resolves cross-package imports (`@scope/name`) correctly
7. Path normalization produces canonical format for all input variations
8. Case-insensitive FS detection correctly identifies macOS/Linux behavior
9. All 13 graph invariants (INV-1 through INV-13) pass on all fixture graphs
10. Property-based tests (`proptest`) pass with default case count
11. All 9 criterion benchmarks run and are within target thresholds
12. `cargo test` passes all L1-L3 tests
13. `cargo bench` runs without errors
14. CI workflow runs successfully on GitHub Actions
15. `install.sh` downloads and installs the correct binary for the current platform
16. README.md provides accurate quick-start instructions

## Testing Requirements

### L1: Parser Snapshots
- All existing Phase 1a snapshots continue to pass (no regressions)
- New path resolution snapshots: workspace package resolution, case-insensitive matching

### L2: Fixture Graph Tests
- All existing fixtures continue to produce correct graphs
- **New fixture:** `tests/fixtures/workspace-project/` — npm workspace with 3 packages, cross-package imports. Expected edges from consumer → producer entry points.
- Edge-cases fixture expanded: binary file with source extension (W004), oversized file (W003), non-UTF-8 file (W009), partial parse file (W007)

### L3: Graph Invariants (Full Suite)
- Phase 1a basic: INV-1 (edge referential integrity), INV-2 (no self-imports), INV-8 (counts match), INV-9 (no duplicates), INV-11 (byte-identical determinism)
- Phase 1b additions: INV-3 (test edges connect test→source), INV-4 (every node has cluster), INV-5 (cluster file lists complete), INV-6 (cluster edge counts correct), INV-7 (cohesion correctly computed), INV-10 (hash determinism), INV-12 (type-only imports → type_imports edges), INV-13 (re-export edges have source)
- Property-based testing via proptest: random source files, random graphs → verify all invariants

### L4: Performance Benchmarks
- All 9 benchmarks from performance.md table
- Synthetic project generation for build benchmarks
- Regression tracking via criterion historical results

### Additional Phase 1b Tests
- Warning output format tests: human format matches expected pattern, JSON format is valid JSONL
- CLI flag parsing: each flag individually, flag combinations
- Workspace detection: npm, pnpm, missing workspace config, malformed package.json (→ W008)
- Case sensitivity detection: verify behavior on current platform
- Path normalization: `./foo` → `foo`, `foo/../bar` → `bar`, backslashes → forward slashes, trailing slashes stripped

## Resolved Design Decisions

The following design gaps were identified during spec review and resolved via decision log entries D-027 through D-030.

### Resolved (with decision log entries)

| # | Question | Resolution | Decision |
|---|----------|------------|----------|
| 1 | ImportResolver signature change scope | Add `workspace` parameter directly to the trait method. All 6 resolvers updated mechanically (receive `None`). Compiler enforces correctness. | D-018 (existing) |
| 2 | TS/JS workspace entry point preference | `main` → `module` → default probe. Full `exports` field parsing deferred. | D-027 |
| 3 | Workspace member name collisions | First-found (sorted directory order) + W008 warning. | D-029 |
| 4 | `--strict` + `--warnings json` interaction | Orthogonal flags. Both allowed simultaneously. `--strict` controls exit code, `--warnings` controls format. | D-030 |
| 5 | `--max-files` counting basis | Counts at walk stage (all files encountered, regardless of extension). | D-030 |
| 6 | Timestamp format | ISO 8601, seconds precision, always UTC with Z suffix: `"2026-03-18T14:23:45Z"` | D-030 |
| 7 | WorkspaceInfo forward compatibility | Intentionally npm-family-specific. Future workspace types will require model extension. | D-028 |

### Open (can be decided during implementation)

1. **CI matrix specifics:** Which OS versions, Rust versions (MSRV?), and architectures for CI testing vs release builds. (Source: testing.md §CI Integration — example shown but matrix undefined)

2. **Warning JSON schema completeness:** Generic `{code, file, message, detail}` schema is sufficient for Phase 1b. Code-specific fields can be added later if needed. (Source: error-handling.md §Warning Output Format)

3. **install.sh scope:** Download-only for Phase 1b. No checksums, no version pinning. Bash script. (Source: ROADMAP)

4. **README.md content scope:** Quick-start focused. Link to design docs for comprehensive reference. (Source: ROADMAP)

5. **Proptest case count:** Default 256 cases. TypeScript generators primary, basic generators for other languages. (Source: testing.md)

6. **Benchmark CI storage:** GitHub Actions artifacts for Phase 1b. Can migrate to committed baselines later. (Source: testing.md §L4)
