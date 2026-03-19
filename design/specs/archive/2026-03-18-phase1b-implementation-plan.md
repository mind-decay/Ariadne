# Phase 1b: Hardening — Implementation Plan

**Spec:** `design/specs/2026-03-18-phase1b-hardening.md`

## Overview

10 chunks, organized by dependency. Chunks 1 and 3 can run in parallel (no shared dependencies). The critical path is: Chunk 3 → 4 → 5 → 7.

```
Chunk 1 (warnings) ──→ Chunk 2 (CLI flags) ──→ Chunk 6 (verbose timing)

Chunk 3 (paths) ──────┐
                       ├──→ Chunk 5 (resolver) ──→ Chunk 7 (fixtures)
Chunk 4 (workspace) ──┘                                  │
                                            ┌─────────────┼──────────────┐
                                            ▼             ▼              ▼
                                   Chunk 8 (L3+proptest)  Chunk 9 (bench) Chunk 10 (CI/README)
```

## Chunk 1: Warning Output Formatters (D1)

**Depends on:** nothing
**Files modified:** `src/diagnostic.rs`
**Spec deliverable:** D1

### What

Add two warning output formatters to the existing `DiagnosticCollector`/`DiagnosticReport` system:

1. **Human formatter** — takes `DiagnosticReport`, writes to stderr:
   ```
   warn[W001]: failed to parse src/foo.ts: unexpected token at line 42
   ```

2. **JSON formatter** — takes `DiagnosticReport`, writes JSONL to stderr:
   ```json
   {"level":"warn","code":"W001","file":"src/foo.ts","message":"parse failed","detail":"unexpected token at line 42"}
   ```

3. **Summary formatter** — takes `DiagnosticReport` + build stats, writes to stdout:
   ```
   Built graph: 847 files, 2341 edges, 12 clusters in 1.2s
     3 files skipped (1 parse error, 1 permission denied, 1 too large)
     42 imports unresolved (external packages)
   ```

Add a `WarningFormat` enum (`Human`, `Json`) to `diagnostic.rs`.

Add a `format_warnings(report: &DiagnosticReport, format: WarningFormat, verbose: bool)` function that:
- Filters W006 unless `verbose` is true
- Formats each warning per the selected format
- Returns formatted string (caller writes to stderr)

Add a `format_summary(report: &DiagnosticReport, file_count: usize, edge_count: usize, cluster_count: usize, elapsed: Duration)` function that returns the summary string (caller writes to stdout).

### Existing code context

- `DiagnosticReport` already exists with `warnings: Vec<Warning>` and `counts: DiagnosticCounts`
- `Warning` has `code: WarningCode`, `path: CanonicalPath`, `message: String`, `detail: Option<String>`
- `DiagnosticCollector::drain()` returns sorted `DiagnosticReport`
- Currently `main.rs` prints warnings directly — replace with formatter calls

### Tests

- Unit tests for human formatter: verify format `warn[WXXX]: ...` for each warning code
- Unit tests for JSON formatter: verify valid JSONL, correct field names
- Unit test for summary formatter: verify counts and format
- Test W006 filtering: not shown without verbose, shown with verbose

### Verification

`cargo test` — all existing tests pass + new formatter tests pass.

---

## Chunk 2: CLI Flags (D2)

**Depends on:** Chunk 1 (warning formatters)
**Files modified:** `src/main.rs`, `src/pipeline/mod.rs`, `src/serial/mod.rs`
**Spec deliverable:** D2

### What

Add 6 flags to the `Build` command in `main.rs`:

```rust
Build {
    path: PathBuf,
    #[arg(long, short)]
    output: Option<PathBuf>,
    #[arg(long)]
    verbose: bool,
    #[arg(long, default_value = "human")]
    warnings: String,  // "human" or "json"
    #[arg(long)]
    strict: bool,
    #[arg(long)]
    timestamp: bool,
    #[arg(long, default_value_t = 1_048_576)]
    max_file_size: u64,
    #[arg(long, default_value_t = 50_000)]
    max_files: usize,
}
```

Thread flags through the pipeline:

1. `--max-file-size` and `--max-files` → populate `WalkConfig` (already has these fields)
2. `--verbose` → pass as `bool` to `run_with_output`
3. `--warnings` → parse to `WarningFormat` enum, pass to formatter after pipeline completes
4. `--strict` → after pipeline completes, if `report.warnings.is_empty() == false`, exit with code 1
5. `--timestamp` → pass to serializer; set `GraphOutput.generated` to `Some(chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string())` (or use `time` crate)

The `GraphOutput.generated` field already exists with `skip_serializing_if = "Option::is_none"`. Just populate it when `--timestamp` is set.

### Existing code context

- `main.rs` has `Commands::Build { path, output }` — extend with new fields
- `WalkConfig::default()` already sets max_files=50000, max_file_size=1MB
- `GraphOutput.generated` is `Option<String>`, already skip-serialized if None

### New dependency

Add `chrono` crate (or use `time` crate) for ISO 8601 timestamp generation. Alternatively, use `humantime` or manual formatting to avoid heavy dependency — `chrono` is large. Consider `time` crate (lighter) or manual UTC formatting via `std::time::SystemTime`.

### Tests

- CLI parsing tests: verify each flag is parsed correctly
- `--strict` test: build fixture with known warnings → verify exit code 1
- `--timestamp` test: build fixture → verify `generated` field present in output
- `--max-file-size` test: create fixture with large file → verify W003 + file skipped
- `--max-files` test: create fixture exceeding limit → verify partial build + warning
- Flag combination test: `--strict --warnings json` → JSON output + exit code 1

### Verification

`cargo test` — all existing tests pass + new CLI tests pass.

---

## Chunk 3: Path Normalization + Case-Insensitive FS Detection (D5)

**Depends on:** nothing
**Files modified:** `src/model/types.rs`, `src/detect/mod.rs` (add re-export), new `src/detect/case_sensitivity.rs`
**Spec deliverable:** D5

### What

1. **Harden CanonicalPath::new()** — the constructor already normalizes (replaces backslashes, removes `./`, resolves `..`). Verify it handles all edge cases from path-resolution.md:
   - Double slashes (`src//auth//login.ts` → `src/auth/login.ts`)
   - Trailing slashes stripped
   - Path traversal (sequences of `..` that would escape root) → the constructor currently resolves `..` by popping segments. Verify it handles `../../..` gracefully (empty path or error).

2. **Add case-insensitive FS detection** — new function in `src/detect/case_sensitivity.rs`:
   ```rust
   pub fn is_case_insensitive(root: &Path) -> bool
   ```
   Creates temp file in `root`, checks if swapped-case variant exists. Returns `false` on detection failure (assume case-sensitive).

3. **Add case-insensitive resolution helper** — used during import resolution in Chunk 5. The function takes a `CanonicalPath` that didn't match exactly in `FileSet` and tries case-insensitive matching:
   ```rust
   pub fn find_case_insensitive(target: &CanonicalPath, known_files: &FileSet) -> Option<CanonicalPath>
   ```

### Existing code context

- `CanonicalPath::new()` already does normalization (backslash → `/`, strip `./`, resolve `..`)
- `FileSet` is `BTreeSet<CanonicalPath>` — iteration is sorted, case-insensitive search is O(n) worst case but files are typically <50k

### New dependency

`dunce` crate — for Windows path canonicalization. No-op on Unix. Add to `Cargo.toml`.

### Tests

- Unit tests for CanonicalPath edge cases: double slashes, trailing slashes, deep `..` sequences, empty path
- Unit tests for case-insensitive detection: verify on current platform (macOS = true, Linux = false)
- Unit tests for case-insensitive resolution: mock FileSet, verify matching with different casing

### Verification

`cargo test` — all existing tests pass + new path tests pass.

---

## Chunk 4: Workspace Types + Detection (D3)

**Depends on:** nothing (workspace types use `PathBuf`, not `CanonicalPath`; can run in parallel with Chunk 3)
**Files created:** `src/model/workspace.rs`, `src/detect/workspace.rs`
**Files modified:** `src/model/mod.rs` (add re-export), `src/detect/mod.rs` (add re-export)
**Spec deliverable:** D3

### What

1. **Add workspace types to model/** — pure data, no behavior:
   ```rust
   // src/model/workspace.rs
   pub struct WorkspaceInfo { pub kind: WorkspaceKind, pub members: Vec<WorkspaceMember> }
   pub struct WorkspaceMember { pub name: String, pub path: PathBuf, pub entry_point: PathBuf }
   pub enum WorkspaceKind { Npm, Yarn, Pnpm }
   ```

2. **Add workspace detection to detect/** — reads filesystem:
   ```rust
   // src/detect/workspace.rs
   pub fn detect_workspace(root: &Path) -> Option<WorkspaceInfo>
   ```

   Detection logic:
   - Read `root/package.json` → parse JSON → check for `"workspaces"` field
   - If found: expand glob patterns (use `glob` crate), read each member's `package.json`, extract `name` field
   - Entry point: check `main` → `module` → probe `src/index.ts`, `index.ts` (D-027)
   - Check `root/pnpm-workspace.yaml` → parse YAML → extract `packages` globs
   - Name collision: first-found + W008 (D-029)
   - Parse failure: W008, return `None`
   - No indicators: return `None`

### New dependencies

- `glob` crate — for expanding workspace glob patterns (`packages/*`)
- `serde_yaml` crate — for parsing `pnpm-workspace.yaml` (lightweight; or use manual YAML parsing for simple structure)

Consider: pnpm-workspace.yaml is trivially simple (`packages:\n  - 'packages/*'`). Could parse manually to avoid serde_yaml dependency. Decision during implementation.

### Tests

- Unit tests for npm workspace detection: create temp dir with package.json + workspace members
- Unit tests for pnpm workspace detection: create temp dir with pnpm-workspace.yaml
- Test glob expansion: `packages/*` matches correctly
- Test entry point preference: `main` → `module` → default probe
- Test name collision: two members with same name → first-found + warning
- Test missing/malformed package.json → None (graceful degradation)
- Test no workspace indicators → None

### Verification

`cargo test` — all existing tests pass + new workspace detection tests pass.

---

## Chunk 5: ImportResolver Signature + Workspace-Aware Resolution (D4)

**Depends on:** Chunks 3 (case-insensitive resolution functions) and 4 (workspace types)
**Files modified:** `src/parser/traits.rs`, `src/parser/typescript.rs`, `src/parser/go.rs`, `src/parser/python.rs`, `src/parser/rust_lang.rs`, `src/parser/csharp.rs`, `src/parser/java.rs`, `src/pipeline/build.rs`, `src/pipeline/resolve.rs`
**Spec deliverable:** D4

### What

1. **Update ImportResolver trait** in `src/parser/traits.rs`:
   ```rust
   fn resolve(
       &self,
       import: &RawImport,
       from_file: &CanonicalPath,
       known_files: &FileSet,
       workspace: Option<&WorkspaceInfo>,  // NEW
   ) -> Option<CanonicalPath>;
   ```

2. **Update all 6 resolver implementations** — mechanical change. Each resolver adds `_workspace: Option<&WorkspaceInfo>` parameter and ignores it. Only TypeScript resolver uses it.

3. **Implement workspace resolution in TypeScript resolver** — before the existing relative/external classification:
   ```
   If workspace is Some AND import path starts with workspace member name:
     → resolve to member entry point (or subpath within member directory)
   Else:
     → existing resolution logic (relative, external skip)
   ```

   For subpath imports (`@myapp/auth/utils`):
   - Strip package name prefix → get subpath (`utils`)
   - Join with member path → probe with extensions (reuse existing extension probing)

4. **Update pipeline call sites** — `pipeline/build.rs` and `pipeline/resolve.rs` pass `Option<&WorkspaceInfo>` to resolver calls. Workspace detection runs in `pipeline/mod.rs` before the resolve stage.

5. **Integrate case-insensitive resolution** (from Chunk 3) into the resolve flow — after exact match fails, try case-insensitive if `is_case_insensitive` is true.

### Existing code context

- `ImportResolver::resolve` currently has 3 params
- `resolve_import()` in `pipeline/resolve.rs` calls `resolver.resolve(import, from_file, &file_set)`
- `resolve_and_build()` in `pipeline/build.rs` orchestrates resolution
- TypeScript resolver already handles relative paths (`./`, `../`), external package skip, extension probing, index file probing

### Tests

- Verify all 6 parsers still work with `workspace: None` (regression)
- TypeScript workspace resolution tests:
  - `@myapp/auth` → member entry point
  - `@myapp/auth/utils` → subpath within member
  - `@types/react` (non-workspace scoped) → None
  - Bare specifier (`lodash`) → None (unchanged)
  - Relative import (`./foo`) → unchanged behavior
- Case-insensitive resolution test: import `'./Utils'` with file `utils.ts` on case-insensitive FS

### Verification

`cargo test` — all existing tests pass + new resolver tests pass. Critical: all Phase 1a fixture graph snapshots must remain unchanged.

---

## Chunk 6: Per-Stage Verbose Timing (D6)

**Depends on:** Chunk 2 (--verbose flag threaded through pipeline)
**Files modified:** `src/pipeline/mod.rs`
**Spec deliverable:** D6

### What

Add `Instant::now()` timing around each pipeline stage in `BuildPipeline::run_with_output()`. When `verbose` is true, print per-stage timing to stderr.

Update the `run_with_output` signature to accept `verbose: bool` (or a config struct). Wrap each stage:

```rust
let walk_start = Instant::now();
let entries = self.walker.walk(root, &config)?;
if verbose {
    eprintln!("[walk]      {:>6}ms  {:>5} files found", walk_start.elapsed().as_millis(), entries.len());
}
// ... repeat for read+hash, parse, resolve, cluster, serialize, total
```

Format matches performance.md §Built-in Timing exactly.

### Existing code context

- `run_with_output` already calls stages sequentially: walk → read → parse → resolve_and_build → cluster → serialize
- Each stage already returns results used by the next

### Tests

- Integration test: build fixture with verbose=true → verify timing output appears in stderr
- Verify timing format matches expected pattern (regex check)

### Verification

`cargo test` — all existing tests pass + timing test passes.

---

## Chunk 7: Workspace Fixture + Edge-Cases Expansion (L2)

**Depends on:** Chunk 5 (workspace resolution working)
**Files created:** `tests/fixtures/workspace-project/` (new fixture), expanded `tests/fixtures/edge-cases/`
**Files modified:** `tests/graph_tests.rs`
**Spec deliverable:** Testing Requirements §L2

### What

1. **Create workspace-project fixture** — npm workspace with 3 packages:
   ```
   tests/fixtures/workspace-project/
   ├── package.json               {"name": "workspace-root", "workspaces": ["packages/*"]}
   ├── packages/
   │   ├── auth/
   │   │   ├── package.json       {"name": "@myapp/auth", "main": "src/index.ts"}
   │   │   └── src/
   │   │       ├── index.ts       export { login } from './login';
   │   │       └── login.ts       export function login() {}
   │   ├── api/
   │   │   ├── package.json       {"name": "@myapp/api", "main": "src/index.ts"}
   │   │   └── src/
   │   │       ├── index.ts       export { router } from './router';
   │   │       └── router.ts      import { login } from '@myapp/auth';
   │   └── shared/
   │       ├── package.json       {"name": "@myapp/shared", "main": "src/index.ts"}
   │       └── src/
   │           ├── index.ts       export { format } from './format';
   │           └── format.ts      export function format() {}
   ```

   Expected edges: `api/src/router.ts --[imports]--> auth/src/index.ts`

2. **Expand edge-cases fixture:**
   - Add binary file with `.ts` extension (null bytes) → W004
   - Add oversized file (>1MB) → W003
   - Add non-UTF-8 file → W009
   - Add partial parse file (valid + invalid syntax) → W007

3. **Add graph test** for workspace-project fixture in `tests/graph_tests.rs`

4. **Generate and commit `.ariadne/graph/` snapshots** for the workspace-project fixture

### Tests

- `test_workspace_project` — verifies cross-package import edges exist
- Updated edge-cases test — verifies warnings for new edge case files

### Verification

`cargo test` — all fixture tests pass including new workspace fixture.

---

## Chunk 8: Full L3 Invariants + Property-Based Tests (D7)

**Depends on:** Chunk 7 (all fixtures ready)
**Files modified:** `tests/invariants.rs`, new `tests/properties.rs`
**Files modified:** `Cargo.toml` (add proptest dev-dependency)
**Spec deliverable:** D7

### What

1. **Add remaining invariants to `tests/invariants.rs`:**
   - INV-3: Test edges connect test → source/type_def
   - INV-4: Every node belongs to a cluster
   - INV-5: Cluster file lists are complete
   - INV-6: Cluster edge counts are correct
   - INV-7: Cohesion is correctly computed (check 4 decimal place rounding)
   - INV-10: Content hashes are deterministic (hash same file twice)
   - INV-12: Type-only imports produce type_imports edges
   - INV-13: Re-export edges have is_reexport source

   Run all 13 invariants on every fixture graph (parametrized test).

2. **Add proptest-based property tests in `tests/properties.rs`:**
   - Random TypeScript source generation → parse → verify RawImport validity
   - Random file content → hash → verify determinism (INV-10)
   - Random graph construction → verify all invariants hold
   - Random directory paths → CanonicalPath normalization → verify invariants (no `..`, no `./`, forward slashes)

### New dependency

`proptest = "1"` in `[dev-dependencies]`

### Tests

- 8 new invariant tests (INV-3, 4, 5, 6, 7, 10, 12, 13)
- 4+ proptest property tests
- All run as part of `cargo test`

### Verification

`cargo test` — all 13 invariants pass on all fixtures + proptest passes.

---

## Chunk 9: Performance Benchmarks (D8)

**Depends on:** Chunks 1-7 (all code complete)
**Files created:** `benches/build_bench.rs`, `benches/parser_bench.rs`, `benches/helpers.rs`
**Files modified:** `Cargo.toml` (add criterion dev-dependency + `[[bench]]` sections)
**Spec deliverable:** D8

### What

1. **Add benchmark helpers** — `benches/helpers.rs`:
   - `generate_synthetic_project(file_count, dir_count, imports_per_file, language) -> TempDir`
   - Generates valid source files with import statements pointing to other generated files

2. **Add build benchmarks** — `benches/build_bench.rs`:
   - `bench_build_100` — 100 TS files
   - `bench_build_1000` — 1000 TS files
   - `bench_build_3000` — 3000 mixed files

3. **Add parser benchmarks** — `benches/parser_bench.rs`:
   - `bench_parse_typescript` — single file, 50 imports
   - `bench_parse_go` — single file, 30 imports
   - `bench_parse_python` — single file, 40 imports
   - `bench_hash_1mb` — xxHash64 on 1MB content
   - `bench_clustering_3000` — assign_clusters on 3000-node graph
   - `bench_serialization_3000` — JSON serialization of 3000-node graph

### New dependency

```toml
[dev-dependencies]
criterion = { version = "0.5", features = ["html_reports"] }

[[bench]]
name = "build_bench"
harness = false

[[bench]]
name = "parser_bench"
harness = false
```

### Verification

`cargo test` — all existing tests still pass (no regressions from benchmark code).
`cargo bench` — all benchmarks run without errors. Check targets from performance.md are met.

---

## Chunk 10: CI/CD + install.sh + README (D9, D10)

**Depends on:** Chunks 1-9 (everything else)
**Files created:** `.github/workflows/ci.yml`, `.github/workflows/release.yml`, `install.sh`, `README.md`
**Spec deliverable:** D9, D10

### What

1. **CI workflow** — `.github/workflows/ci.yml`:
   ```yaml
   on: [push, pull_request] to main/master
   jobs:
     test: cargo test + cargo insta test --check
     lint: cargo fmt --check + cargo clippy -D warnings
     bench: cargo bench (main branch only)
   ```

2. **Release workflow** — `.github/workflows/release.yml`:
   ```yaml
   on: push tags v*
   jobs:
     build: linux-x64 + darwin-x64 + darwin-arm64
     release: GitHub Release with artifacts
     publish: cargo publish
   ```

3. **install.sh** — platform-detecting bash script:
   - Detect OS (Darwin/Linux) and architecture (x86_64/arm64)
   - Download binary from GitHub Releases
   - Install to `$INSTALL_PATH` or `/usr/local/bin`
   - `chmod +x`

4. **README.md** — quick-start documentation:
   - What Ariadne is (1-2 sentences)
   - Installation (3 methods: cargo, binary, install.sh)
   - Usage (`ariadne build .`, `ariadne info`, flags)
   - Supported languages table
   - Output format overview
   - Limitations
   - Performance table
   - License (MIT/Apache-2.0)

### Verification

`cargo test` — all existing tests still pass.
- CI: push branch, verify workflow runs
- install.sh: run locally, verify binary downloads
- README: review for accuracy

---

## Summary

| Chunk | Deliverable | Files | Depends on | Risk |
|-------|------------|-------|------------|------|
| 1 | Warning formatters (D1) | diagnostic.rs | — | GREEN |
| 2 | CLI flags (D2) | main.rs, pipeline/mod.rs, serial/mod.rs | 1 | YELLOW |
| 3 | Path normalization + case sensitivity (D5) | model/types.rs, detect/ | — | GREEN |
| 4 | Workspace types + detection (D3) | model/workspace.rs, detect/workspace.rs | — | YELLOW |
| 5 | Resolver signature + workspace resolution (D4) | parser/traits.rs, all 6 parsers, pipeline/ | 3, 4 | YELLOW |
| 6 | Verbose timing (D6) | pipeline/mod.rs | 2 | GREEN |
| 7 | Fixtures (L2) | tests/fixtures/, graph_tests.rs | 5 | GREEN |
| 8 | L3 invariants + proptest (D7) | invariants.rs, properties.rs | 7 | GREEN |
| 9 | Benchmarks (D8) | benches/ | 1-7 | GREEN |
| 10 | CI/CD + install.sh + README (D9, D10) | .github/, install.sh, README.md | 1-9 | YELLOW |

**Parallel opportunities:**
- Chunks 1, 3, and 4 can all run in parallel (no shared dependencies)
- Chunks 8, 9, and 10 can run in parallel after Chunk 7
- Chunk 6 can run in parallel with Chunks 3-5 (different dependency chain)

**Critical path:** Chunk 3 + 4 (parallel) → 5 → 7 → 8

**`cargo test` after each chunk** — no chunk should break existing tests.
