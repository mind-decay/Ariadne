# Phase 14: Stack-Agnostic Script Discovery — Specification

## Goal

`tech_stack()` returns usable `build`/`test`/`lint` commands for any project — not just the three manifest ecosystems (`package.json`, `Cargo.toml`, `go.mod`) it currently recognizes. Discovery is layered (Chain of Responsibility) so new ecosystems are unlocked by adding signal files, not by growing an allow-list of per-ecosystem manifest parsers.

## Problem Restatement (ROADMAP Phase 14)

Theseus (and any other consumer that runs `TechStack.scripts`) cannot verify generated code when `scripts == []`. Today this happens for every ecosystem other than JS/TS, Rust, and Go — Python, Java, Ruby, PHP, Swift, .NET, Elixir, etc. The pipeline runs blind: no build check, no test check, no lint check. Phase 14 closes the gap with four discovery layers tried in priority order, each independent and language-neutral.

| Layer | Source                                        | Confidence | Coverage                |
| ----- | --------------------------------------------- | ---------- | ----------------------- |
| 1     | Manifest-native scripts (existing)            | Highest    | JS/TS, Rust             |
| 2     | Universal task runners (Makefile / justfile / Taskfile) | High       | Any language            |
| 3     | Probe-based ecosystem defaults (signal → cmd) | Medium     | Standard ecosystems     |
| 4     | User declaration (Theseus-side, out of scope) | Highest    | 100% fallback           |

Ariadne owns layers 1–3. Layer 4 is Theseus-side and is not built here; Phase 14 only guarantees that an empty `scripts` result is reachable cleanly so the caller knows to ask.

## Dependencies

| Phase    | Status | What It Provides                                                                                                          |
| -------- | ------ | ------------------------------------------------------------------------------------------------------------------------- |
| Phase 1a | DONE   | `CanonicalPath`, `ProjectGraph`, `FileType::Source` for language detection                                                |
| Phase 12 | DONE   | `src/conventions/` module; `tech_stack()` entry point; `TechStack` / `ScriptInfo` / `ScriptCategory` / `TestConfig` types |
| Phase 13 | DONE   | Nothing directly consumed; listed only because it is the previous landed phase                                            |

No hard dependency on Phase 15 (Phase 14 can land before or after Phase 15 per ROADMAP).

## Risk Classification

**Overall: YELLOW** — The surface area is one Ariadne module (`src/conventions/tech_stack.rs`) plus one additive field and one signature change on `TechStack`/`tech_stack()`. No changes to parser/, pipeline/, model/, or serial/. **Breaking API change**: `tech_stack()` signature gains a `&DiagnosticCollector` parameter to match `temporal::analyze()`. Theseus (`/Users/minddecay/Documents/Projects/theseus`) is an active downstream consumer and must be updated in a coordinated PR — see §Coordinated Landing.

Risk concentrated in three places: (a) keeping probe-based defaults conservative enough that a wrong command never crashes the consumer; (b) splitting the existing 765-line `tech_stack.rs` file cleanly while preserving current semantics (Phase 12 tests must keep passing); (c) landing the Theseus update synchronously so `cargo check -p theseus` does not regress during the release window.

### Per-Deliverable Risk

| #   | Deliverable                                           | Risk   | Rationale                                                                                                                                                   |
| --- | ----------------------------------------------------- | ------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------- |
| D1  | Layered discovery pipeline orchestration              | GREEN  | Chain-of-responsibility is straightforward; winner-takes-all merge produces one `ScriptInfo` per category at most                                            |
| D2  | Universal task runner layer (Makefile/justfile/Taskfile) | YELLOW | Makefile target-line parser must stay minimal — no recursive include handling, no variable expansion, no pattern rules                                     |
| D3  | Probe-based ecosystem defaults table                  | YELLOW | Defaults have to be safe-on-failure; must not overreach (e.g. don't claim `mvn test` if no tests dir or surefire config)                                   |
| D4  | `structured_diagnostics_cmd` field on `TechStack`     | GREEN  | Replaces Theseus `execute.rs:53` `starts_with("cargo")` hardcode with a field read; Cargo-only in Phase 14                                                   |
| D5  | `categorize_script` cleanup                           | GREEN  | Drop framework-specific names (jest/vitest/mocha/eslint); keep only generic patterns (test/lint/build/check/format/etc.)                                   |
| D6  | File split of `tech_stack.rs` (765 → ≤300 per file)   | YELLOW | Must keep public API (`pub use tech_stack::tech_stack`), no behavior change, update `no_god_modules` allowlist                                               |
| D7  | `tech_stack()` signature extension + W056-W058        | YELLOW | Breaking API change: adds `&DiagnosticCollector` parameter matching `temporal::analyze`. Theseus must update `gather.rs:91-92` in the coordinated PR        |
| D8  | Tests + fixtures for each new layer                   | YELLOW | 16 fixtures (13 happy-path per ecosystem + 3 adversarial for Makefile/justfile/Taskfile); `ariadne-on-ariadne` self-test in SIGN-OFF                       |
| D9  | Theseus coordinated PR                                | YELLOW | Updates `gather.rs` (add collector), `execute.rs:51-58` (read field instead of hardcoding cargo), 6+ synthetic fixtures across test modules                 |

## Design Sources

| Deliverable | Primary Source                          | Supporting Sources                                                                                  |
| ----------- | --------------------------------------- | --------------------------------------------------------------------------------------------------- |
| D1          | ROADMAP.md Phase 14 (layered table)     | D-047, Theseus `verify.rs:106` (`.find()` contract), Theseus `verify.rs:99-101` (explicit "Ariadne decides" comment) |
| D2          | ROADMAP.md Phase 14 (Layer 2 subsection)| CLAUDE.md Locked Constraints (`<=300 lines per file`)                                               |
| D3          | ROADMAP.md Phase 14 (probe table)       | error-handling.md §Principles + §Recoverable Errors (safe-on-failure handling for per-file errors)  |
| D4          | ROADMAP.md Phase 14 (also-in-scope)     | Theseus `execute.rs:40-58` (`parse_structured_build_errors` hardcodes `starts_with("cargo")` today) |
| D5          | ROADMAP.md Phase 14 (also-in-scope)     | `src/conventions/tech_stack.rs:222-241` (current impl)                                              |
| D6          | CLAUDE.md Locked Constraints            | `tests/constraints.rs::no_god_modules`                                                              |
| D7          | error-handling.md Warning Code Table    | `src/temporal/mod.rs:17-20` (nearest sibling signature), Theseus `gather.rs:196-197` (consumer already uses the pattern) |
| D8          | testing.md §Test Levels (L1, L2)        | Theseus test modules (30+ synthetic `TechStack` fixtures, zero real-repo tests for this code path) |
| D9          | —                                       | Theseus `gather.rs:91-92`, `execute.rs:51-58`, synthetic fixture call sites                         |

## Deliverables

### D1: Layered Discovery Pipeline (`src/conventions/tech_stack/mod.rs`)

**What:** Replace the current sequential `if pkg_json { … } else if cargo_toml { … } else if go_mod { … }` in `tech_stack.rs:20-60` with a layered pipeline that always runs layers 1 → 2 → 3 and merges results via **winner-takes-all by category**.

**Entry point (breaking API change — see D7):**

```rust
pub fn tech_stack(
    project_root: &Path,
    graph: &ProjectGraph,
    collector: &DiagnosticCollector,
) -> Result<TechStack, std::io::Error>
```

**New internal contract:**

```rust
// Each layer is a pure function returning at most one ScriptInfo per category.
fn discover_manifest_scripts(
    project_root: &Path,
    language: &str,
    collector: &DiagnosticCollector,
) -> Option<TechStack>;

fn discover_task_runner_scripts(
    project_root: &Path,
    collector: &DiagnosticCollector,
) -> Vec<ScriptInfo>;

fn discover_probe_defaults(
    project_root: &Path,
    collector: &DiagnosticCollector,
) -> Vec<ScriptInfo>;
```

**Merge rule — winner-takes-all by category:** Layers run in order Manifest → TaskRunner → Probe. For each `ScriptCategory` (`Build`, `Test`, `Lint`), the first layer that produces a command wins; lower-priority layers are **skipped for that category**. The final `TechStack.scripts` contains at most one entry per `ScriptCategory::{Build,Test,Lint}` — no duplicates, no alternates.

**Rationale (consumer-coherent):** Theseus `verify.rs:106` uses `tech_stack.scripts.iter().find(|s| s.category == category)` — first match wins. `verify.rs:99-101` comment makes the contract explicit: "Ariadne provides ready-to-run commands … Theseus needs no knowledge of package managers or build tools." Ariadne owns the priority decision; Theseus blindly runs the first matching script. Returning alternates would either be dead weight (Theseus ignores them) or a behavioral regression (if sort order puts a probe command before a manifest command). Winner-takes-all is the only option that matches the live consumer contract.

**`ScriptCategory::Dev` and `ScriptCategory::Other`** are only produced by Layer 1 — Layers 2/3 never synthesize them, because defaults for "dev" and "other" are too ambiguous to be safe.

**Deterministic output:** Final `scripts: Vec<ScriptInfo>` is sorted by `(category, name)` before return (see determinism.md §Summary of Sort Points — new row added for `TechStack.scripts`).

**Empty result is a valid outcome** — if no layer produces anything, `TechStack.scripts == []`, and the caller (Theseus) handles Layer 4 (user declaration). Ariadne must not error.

### D2: Universal Task Runner Layer (`src/conventions/tech_stack/task_runners.rs`)

**What:** Parse `Makefile`, `justfile`, `Taskfile.yml` target/task names and map them to `ScriptCategory` using the same generic pattern matching as `categorize_script` (D5). The runner file existence is a signal; target names drive category selection.

**Makefile parser:**

- Match lines of the form `^([A-Za-z0-9_./-]+):\s*(.*)$` (BSD+GNU compatible).
- Skip `.PHONY:`, `.DEFAULT:`, and any target starting with `.` (internal).
- Skip targets containing `%` (pattern rules) — ambiguous, not runnable as-is.
- Skip targets containing `$` in the name (variable expansion) — same reason.
- For each surviving target name, produce `ScriptInfo { name: target, command: format!("make {target}"), category: categorize_script(&target) }`.
- Discard `ScriptCategory::Other` and `ScriptCategory::Dev` results (only Build/Test/Lint/Check bubble up from this layer — "other" is noise, "dev" requires knowing whether the target is a server).

**justfile parser:**

- Match lines of the form `^([a-zA-Z0-9_-]+)(\s+[a-zA-Z0-9_-]+)*\s*:\s*.*$` at column 0.
- Skip lines starting with `#`, whitespace, or `set ` (just settings).
- Skip recipes whose names start with `_` (hidden just recipes).
- Produce `ScriptInfo { name, command: format!("just {name}"), category }` with the same Build/Test/Lint/Check filter.

**Taskfile parser:**

- YAML parse via **new direct dependency** `serde_yaml = "0.9"` added to `Cargo.toml`. Verified 2026-04-12: neither `serde_yaml` nor `serde_yml` is in the current workspace `Cargo.toml`/`Cargo.lock`; only `tree-sitter-yaml` and `insta` (with its `yaml` feature) ship YAML-adjacent code. Plan step must add the dep before the Taskfile parser lands.
- Read the top-level `tasks:` map; each key is a task name.
- Produce `ScriptInfo { name, command: format!("task {name}"), category }` with the same Build/Test/Lint/Check filter.
- If YAML parse fails → emit `W056 TaskfileParseError`, return empty vec, DO NOT fail.

**Error handling:**

- Read failures on any runner file → `W057 TaskRunnerReadFailed`, skip that runner, continue.
- Malformed Makefile (no target lines) → empty vec, no warning (common for include-only Makefiles).
- Malformed justfile → `W058 JustfileParseError`, empty vec.
- Probe order is deterministic: Makefile → justfile → Taskfile.yml. Result is the concatenation in that order before the merge step in D1 deduplicates by category.

**Out of scope for Makefile:** include directives, variable expansion, conditional blocks, automatic variables, recursive make. If a real project needs any of these to get its scripts detected, Layer 3 (probe defaults) or Layer 4 (user declaration) takes over — that is the explicit fall-through model.

### D3: Probe-Based Ecosystem Defaults (`src/conventions/tech_stack/probes.rs`)

**What:** Map signal files to default build/test/lint commands without reading language fields from `TechStack`. Signal file existence is necessary but not sufficient — additional content guards keep the defaults safe.

**Static probe table** (from ROADMAP.md Phase 14 — table is the authoritative source):

| Signal file                      | Build                        | Test                                        | Lint                                           | Content guard                                                             |
| -------------------------------- | ---------------------------- | ------------------------------------------- | ---------------------------------------------- | ------------------------------------------------------------------------- |
| `go.mod`                         | `go build ./...`             | `go test ./...`                             | `go vet ./...`                                 | none (file existence is sufficient)                                       |
| `pyproject.toml`                 | —                            | `pytest` (only if pytest marker present)    | —                                              | file must contain `pytest` substring (matches existing `file_mentions`)   |
| `conftest.py`                    | —                            | `pytest`                                    | —                                              | none                                                                      |
| `Gemfile`                        | —                            | `bundle exec rake test`                     | `bundle exec rubocop` (only if `.rubocop.yml`) | Lint only if `.rubocop.yml` or `.rubocop.yaml` also exists                |
| `pom.xml`                        | `mvn compile`                | `mvn test`                                  | —                                              | none                                                                      |
| `build.gradle` / `build.gradle.kts` | `./gradlew build` (only if `gradlew` exists, else `gradle build`) | `./gradlew test` / `gradle test`     | —                                              | Prefer wrapper when present                                               |
| `composer.json`                  | —                            | `composer test` (only if `scripts.test` set) | —                                              | JSON must contain a `scripts.test` entry                                  |
| `mix.exs`                        | `mix compile`                | `mix test`                                  | —                                              | none                                                                      |
| `*.csproj` (first match, sorted) | `dotnet build`               | `dotnet test`                               | —                                              | Glob under project root, top-level only — no recursive walk in this layer |

**Layer 1 precedence:** If Layer 1 already produced a `Build`/`Test`/`Lint` command (e.g. Cargo.toml), Layer 3 is skipped for that category. For example, a Rust repo with a `Makefile` and `pyproject.toml` still gets `cargo build`/`cargo test`/`cargo clippy` from Layer 1; Layer 3 never overrides Layer 1 — it only fills gaps.

**go.mod parity with Phase 12 behavior (DP-5 resolution):** Today `tech_stack.rs:37-49` short-circuits on `go.mod` and returns `TechStack { test_framework: Some("go test"), scripts: Vec::new(), … }`. Phase 14 moves this into Layer 3 but **preserves the `test_framework = Some("go test")` side-effect**: when the `go.mod` probe row fires, the probe layer sets both `scripts` (three go commands per the table) AND `test_framework = Some("go test")`. Losing this parity would break Theseus `test_gen.rs:52`, which reads `test_framework.is_none()` as its Go-skip signal. `test_framework` is the **only** Layer 3 probe that touches a non-scripts field — all other probe rows leave `test_framework`/`linter`/`bundler` as whatever Layer 1 set them to (or `None`).

**Forward-compat note for Phase 16a (`ROADMAP.md:2176-2206`):** Phase 16a plans to promote `test_framework` from `Option<String>` to a richer `TestFrameworkInfo` struct. When Phase 16a lands, it must preserve `name: "go test"` semantic on the new struct, and Theseus `test_gen.rs:52` `is_none()` check becomes `info.is_none() || info.name.is_empty()`. Phase 14 raises the cost of the Phase 16a refactor slightly (probe layer now populates `test_framework` too), but this is an explicit trade-off — the parity is load-bearing for Theseus today. Recorded in D-153 as a coupling marker; Phase 16a spec must reference this entry.

**csproj sort key (determinism):** When multiple `*.csproj` files exist at the project-root top level (unusual but legal), the probe picks the first one by **lexicographic sort of file name, case-sensitive on Unix, case-insensitive on Windows** — matching the existing path normalization rules in `design/path-resolution.md`. This is recorded in D-153 as part of the layered discovery decision.

**Layer 2 runner priority (determinism tie-breaker):** If both a `Makefile` and a `justfile` exist in the same project, Layer 2 probes them in order **Makefile → justfile → Taskfile.yml**. First runner producing a command for a given category wins within Layer 2, before the result bubbles up to the D1 merge step. Also recorded in D-153.

**csproj monorepo limitation (acknowledged gap):** The probe is **top-level only** — it does not recurse into `services/api/Api.csproj`-style layouts. This is a deliberate Phase 14 scope decision: recursive csproj discovery belongs to Phase 11's `src/parser/config/csproj.rs`, which already ships full csproj/sln handling. Phase 14 does not call into Phase 11's walker because that would cross module boundaries from `conventions/` into `parser/config/`, violating the architecture.md dependency rules table. Monorepo .NET projects fall through to Layer 4 (user declaration) in Phase 14. Recorded in D-153 as a known limitation; Phase 16 or later may integrate the Phase 11 walker under a new decision entry.

**Explicit non-goals for Layer 3:**

- No per-ecosystem manifest *parsing*. `pom.xml`, `build.gradle`, `mix.exs`, `composer.json` are treated as signal files, not as configuration to extract from. (ROADMAP.md Phase 14 "Out of scope".)
- No `dotnet` solution discovery. First `*.csproj` found at project-root top level is enough; `.sln` handling stays with the C# parser side of Ariadne (Phase 11).
- No bash/python/ruby version detection. Command strings are static.

**Fault tolerance:** Wrong defaults are safe by construction. The consumer runs the command and gets a shell exit code; non-zero → consumer retries or asks user. Ariadne does not execute these commands itself.

### D4: `structured_diagnostics_cmd` Field (`src/conventions/types.rs`)

**What:** Add one field to `TechStack`:

```rust
pub struct TechStack {
    // … existing fields unchanged …
    /// Shell command emitting JSON/NDJSON compiler diagnostics. Consumers
    /// execute this verbatim and parse the output without knowing which tool
    /// it is. Only populated for ecosystems that ship a stable machine-parseable
    /// diagnostics flag.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub structured_diagnostics_cmd: Option<String>,
}
```

**Consumer motivation (load-bearing, not speculative):** Theseus `execute.rs:40-58` today hardcodes:

```rust
let structured_cmd: Vec<&str> = if build_script.command.starts_with("cargo") {
    vec!["cargo", "build", "--message-format=json"]
} else {
    return None;  // tool doesn't support JSON
};
```

The `starts_with("cargo")` string-match is a consumer antipattern waiting for exactly this field. After Phase 14 lands, Theseus replaces the branch with a direct field read (see §Coordinated Landing → Theseus changes).

**Populated per ecosystem:**

| Ecosystem signal | `structured_diagnostics_cmd`        | Rationale                                       |
| ---------------- | ----------------------------------- | ----------------------------------------------- |
| `Cargo.toml`     | `cargo build --message-format=json` | Stable since Rust 1.0, Theseus already consumes |
| all others       | `None`                              | No stable JSON-diagnostic CLI flag in Phase 14  |

**Strict-JSON contract:** The field is populated **only** for tools that emit JSON/NDJSON. Text output (e.g. `tsc --pretty false`) would crash Theseus `execute.rs:68` `serde_json::from_str(line)` path. TypeScript has no stable JSON diagnostics mode as of TS 5.4 (`--generateTrace` is build-graph trace, not diagnostics; `--explainFiles` is module explanation). ESLint has `--format json` but is a linter, belongs under a hypothetical future `structured_lint_cmd`, not this field. Phase 14 ships Cargo only. Future phases add fields per ecosystem when each has a verifiably stable JSON flag.

**Shell-string contract (not argv):** `structured_diagnostics_cmd` is `Option<String>` — a complete shell command line — not `Option<Vec<String>>` argv. Consumers must execute it via `sh -c "<cmd>"` (see Theseus `execute.rs:562-566` in the post-Phase-14 state). This deliberately sacrifices argv precision for the simpler contract "run this string, parse stdout". The field MUST NOT contain untrusted interpolation — Phase 14 only populates it from static strings in the Ariadne source, never from project-supplied input, so shell-injection risk is zero. Recorded in D-154 as the field contract.

### D5: `categorize_script` Cleanup (`src/conventions/tech_stack/categorize.rs`)

**What:** Remove framework-specific substring checks. Current implementation at `tech_stack.rs:222-241`:

```rust
// BEFORE
if name_lower.contains("test") || name_lower.contains("jest")
    || name_lower.contains("vitest") || name_lower.contains("mocha")
{ … }
if name_lower.contains("lint") || name_lower.contains("eslint")
    || name_lower.contains("check") || name_lower.contains("format")
{ … }
```

**After:**

```rust
fn categorize_script(name: &str) -> ScriptCategory {
    let n = name.to_lowercase();
    if n.contains("test")    { return ScriptCategory::Test; }
    if n.contains("lint")    { return ScriptCategory::Lint; }
    if n.contains("check")   { return ScriptCategory::Lint; }
    if n.contains("format")  { return ScriptCategory::Lint; }
    if n.contains("fmt")     { return ScriptCategory::Lint; }
    if n.contains("build")   { return ScriptCategory::Build; }
    if n.contains("compile") { return ScriptCategory::Build; }
    if n.contains("dev")     { return ScriptCategory::Dev; }
    if n.contains("start")   { return ScriptCategory::Dev; }
    if n.contains("serve")   { return ScriptCategory::Dev; }
    if n.contains("watch")   { return ScriptCategory::Dev; }
    ScriptCategory::Other
}
```

**Rationale:** framework names (jest, vitest, mocha, eslint) are already matched by the generic `test`/`lint` substring — the explicit framework checks were dead weight and coupled Ariadne to the JS ecosystem. Keep it stack-agnostic. `format`/`fmt` added because Rust/Go use `fmt` and not `lint`.

**Compatibility test:** The existing `script_categorization` test (`tech_stack.rs:500-511`) still passes without modification. Add boundary cases for `fmt`, `precommit`, `deploy`, `generate` to ensure none of them leak into the wrong category.

### D6: File Split (`src/conventions/tech_stack/…`)

**What:** `src/conventions/tech_stack.rs` is 765 lines today. Phase 14 must not push it higher. The CLAUDE.md constraint is `<=300 lines per file`; the file is already in the `no_god_modules` allowlist (Phase 12 legacy) but Phase 14 is the right moment to split it since we're touching every major function.

**Target layout:**

```
src/conventions/tech_stack/
├── mod.rs              # pub fn tech_stack() — orchestration only, ≤ 150 LOC
├── layers.rs           # Layer 1 merge logic, detect_language() ≤ 200 LOC
├── manifest.rs         # parse_package_json, parse_cargo_toml, detect_js_runner ≤ 300 LOC
├── task_runners.rs     # Makefile/justfile/Taskfile parsers ≤ 250 LOC
├── probes.rs           # probe table + go.mod row + dotnet glob ≤ 250 LOC
├── categorize.rs       # categorize_script ≤ 80 LOC
├── test_config.rs      # discover_test_config, normalize_framework_stem, probe_config_stem ≤ 200 LOC
└── tests.rs            # unit tests moved from tech_stack.rs ≤ 300 LOC (split further if needed)
```

**Public API unchanged:** `src/conventions/mod.rs:10` still says `pub use tech_stack::tech_stack;` — the submodule boundary is internal. `TechStack` and `TestConfig` remain in `src/conventions/types.rs`.

**Allowlist update:** Remove the old `src/conventions/tech_stack.rs` entry from the `no_god_modules` allowlist in `tests/constraints.rs`. Each new file must compile under 300 LOC or earn a fresh allowlist entry with justification (prefer splitting).

**No_hashmap constraint:** None of the new files introduce `HashMap` (see `tests/constraints.rs::no_hashmap_in_model`) — Phase 14 stays on `BTreeMap` where maps are needed.

### D7: Signature Extension + Warning Codes W056–W058

**What:** Extend `tech_stack()` to take a `&DiagnosticCollector` parameter (matching the `temporal::analyze()` sibling API) and register three new warning codes for task-runner parse/IO errors.

**Breaking API change (`src/conventions/tech_stack/mod.rs`):**

```rust
// BEFORE (Phase 12)
pub fn tech_stack(
    project_root: &Path,
    graph: &ProjectGraph,
) -> Result<TechStack, io::Error>

// AFTER (Phase 14)
pub fn tech_stack(
    project_root: &Path,
    graph: &ProjectGraph,
    collector: &DiagnosticCollector,
) -> Result<TechStack, io::Error>
```

**Why `&DiagnosticCollector` (shared ref, not `&mut`):** `DiagnosticCollector` uses interior mutability via `std::sync::Mutex` (see `src/diagnostic.rs:3`). The shared-ref signature matches `temporal::analyze()` in `src/temporal/mod.rs:17-20`:

```rust
pub fn analyze(
    project_root: &Path,
    graph: &ProjectGraph,
    collector: &DiagnosticCollector,
)
```

Theseus already knows this pattern — `gather.rs:196-197` creates an ephemeral collector for exactly this signature:

```rust
let diagnostics = ariadne_graph::diagnostic::DiagnosticCollector::new();
ariadne_graph::temporal::analyze(project_path, graph, &diagnostics)
```

The Theseus update for `tech_stack()` is a two-line copy of that pattern (see §Coordinated Landing).

**Warning codes (Phase 15a claims W050–W055, next free range is W056+):**

| Code | Name                   | Triggered when                                                                      | Emit policy                                    |
| ---- | ---------------------- | ----------------------------------------------------------------------------------- | ---------------------------------------------- |
| W056 | `TaskfileParseError`   | `Taskfile.yml` present but YAML parse fails                                         | Always emit (rare, actionable)                 |
| W057 | `TaskRunnerReadFailed` | Runner file present but read errors (permission, EACCES, etc.)                      | Always emit (usually a user setup issue)       |
| W058 | `JustfileParseError`   | `justfile` non-empty but yields zero valid recipes (likely malformed)               | Always emit                                    |

**W049 gap rationale:** Current `diagnostic.rs:44-90` enum jumps from `W048NextConfigParseError` directly to the (upcoming) `W050`..`W055` Phase 15a range — `W049` is an unallocated gap. Phase 14 deliberately does NOT reclaim W049; it starts at W056 to stay contiguous with Phase 15a's reservation and to keep the sequential-allocation convention visible. Rationale: re-using an intentionally-skipped code years later breaks `git log`/`grep` searches that assume "W049 never existed" semantics. The gap is preserved, not filled. Recorded explicitly in D-155.

Codes are registered in the `WarningCode` enum (`diagnostic.rs:44-90`), given an `as_str()` mapping, and a no-op branch in the `emit` match. They use the same `DiagnosticCollector` pattern as W044–W048.

**Emit policy differs from W006** (unresolved imports are verbose-only because they are the common case for external packages). Task-runner parse errors are **rare and always actionable** — a malformed `Taskfile.yml` is a real bug the user should fix. Emit by default; add `--quiet` suppression later if telemetry shows noise.

### D8: Tests and Fixtures

**L1 (unit tests in-module):**

- `categorize_script` boundary cases: `fmt`, `format`, `precommit`, `deploy`, `generate`, empty string, unicode name.
- Makefile parser: single target, multi-target, `.PHONY` skip, pattern-rule skip, variable-in-name skip, empty file, include-only file.
- justfile parser: single recipe, multi-recipe, hidden `_recipe` skip, `set` line skip, comment-only file.
- Taskfile parser: valid `tasks:` map, malformed YAML, missing `tasks:` key.
- Probe table: each row has a positive and a negative test (signal present → command; signal absent → nothing).
- **Layer merge rule (winner-takes-all):** Layer 1 wins over Layer 2; Layer 2 wins over Layer 3; each category appears at most once in the returned `Vec<ScriptInfo>`.
- `structured_diagnostics_cmd`: cargo project yields `cargo build --message-format=json`; every other ecosystem yields `None`.
- Warning emission: malformed `Taskfile.yml` emits W056 into the passed `DiagnosticCollector`; unreadable `Makefile` emits W057; empty-recipe `justfile` emits W058.

**L2 (fixture tests in `tests/fixtures/`) — 16 total:**

*Happy-path per ecosystem (Layer 3 detection):*

| Fixture                                    | Signal files                                      | Expected `scripts` (after merge)                    |
| ------------------------------------------ | ------------------------------------------------- | --------------------------------------------------- |
| `phase14-python-pytest/`                   | `pyproject.toml` + `conftest.py`                  | `pytest`                                            |
| `phase14-java-maven/`                      | `pom.xml`, `src/main/java/App.java`               | `mvn compile`, `mvn test`                           |
| `phase14-java-gradle/`                     | `build.gradle`, `gradlew`, `settings.gradle`      | `./gradlew build`, `./gradlew test` (wrapper-aware) |
| `phase14-ruby-bundler/`                    | `Gemfile`, `.rubocop.yml`                         | `bundle exec rake test`, `bundle exec rubocop`     |
| `phase14-elixir-mix/`                      | `mix.exs`, `lib/app.ex`, `test/app_test.exs`      | `mix compile`, `mix test`                           |
| `phase14-dotnet-csproj/`                   | `App.csproj`, `Program.cs`                        | `dotnet build`, `dotnet test`                       |
| `phase14-php-composer/`                    | `composer.json` with `scripts.test`               | `composer test`                                     |

*Layer priority tests:*

| Fixture                                    | Signal files                                                | Expected                                                       |
| ------------------------------------------ | ----------------------------------------------------------- | -------------------------------------------------------------- |
| `phase14-makefile-overrides-probe/`        | `Makefile` with `build:`/`test:`/`lint:`, `pyproject.toml`  | Layer 2 wins Build+Test+Lint; Layer 3 `pytest` suppressed      |
| `phase14-justfile-overrides-probe/`        | `justfile` with `test`/`lint`, `Gemfile`, `.rubocop.yml`    | Layer 2 wins Test+Lint                                         |
| `phase14-taskfile-overrides-probe/`        | `Taskfile.yml` with `build`/`test`, `go.mod`                | Layer 2 wins Build+Test; Layer 3 `go test` suppressed          |
| `phase14-manifest-beats-runner/`           | `Cargo.toml` + `Makefile` with `test:` target               | Layer 1 wins all categories; Layer 2 suppressed                |
| `phase14-empty/`                           | Just `README.md`                                            | `scripts == []`, clean success, zero warnings                  |

*Adversarial (parser robustness):*

| Fixture                               | Adversarial content                                                                                               |
| ------------------------------------- | ----------------------------------------------------------------------------------------------------------------- |
| `phase14-makefile-adversarial/`       | Continuation lines (`\`), `@` command prefix, `.PHONY` with multi-line deps, `include` directive, `$(VAR)` in target name (skip), pattern rule `%.o:` (skip), tabs vs spaces, comment-heavy sections, recipes with `if/else` |
| `phase14-justfile-adversarial/`       | `[private]` recipe attribute, `alias` declarations, `set shell := […]`, parameterized recipes (`build target:`), shebang recipes, hidden `_recipe`                   |
| `phase14-taskfile-adversarial/`       | `includes:` directive, `vars:` section, `cmd:` (single) vs `cmds:` (list), `deps:`, nested task groups, `preconditions:`                                             |

Each adversarial fixture must:
1. Yield at least one correctly-detected script (parser extracts what it can).
2. Skip unknown-shape targets silently (no W-code emission — just ignore).
3. Never crash or hang.

**L3 (invariants, `tests/invariants.rs`):** three new assertions, numbered sequentially after the existing INV-1 through INV-18.

- **INV-11 extension:** `tech_stack()` produces byte-identical output across two consecutive calls on the same fixture (existing INV-11 covered graph determinism; extended to include `TechStack`).
- **INV-19 (one-per-category):** `TechStack.scripts` contains at most one entry per `ScriptCategory::{Build,Test,Lint}` (winner-takes-all invariant). Enforced on every Phase 14 fixture.
- **INV-20 (sort order):** `TechStack.scripts` is sorted by `(category, name)` on return. `ScriptCategory` must derive `Ord` (tracked in Files Modified → `src/conventions/types.rs`). Enforced on every Phase 14 fixture.

**L4 (benchmarks):** None. `tech_stack()` is called once per gather and the IO is a handful of `fs::read_to_string` calls — no benchmark target needed.

**SIGN-OFF self-test (real-project smoke):** Run `ariadne build` on Ariadne itself (`/Users/minddecay/Documents/Projects/Ariadne`). Expected:

- `tech_stack.scripts` contains exactly `cargo build`, `cargo test`, `cargo clippy -- -D warnings` (Layer 1 manifest output, unchanged from Phase 12).
- `tech_stack.structured_diagnostics_cmd == Some("cargo build --message-format=json")`.
- Zero W056-W058 warnings.
- `cargo check -p theseus` still compiles against the updated `ariadne-graph` (see §Coordinated Landing).

## Files Created

| File                                               | Type    | Description                                                        |
| -------------------------------------------------- | ------- | ------------------------------------------------------------------ |
| `src/conventions/tech_stack/mod.rs`                | Source  | Orchestration + public entry point (winner-takes-all merge)        |
| `src/conventions/tech_stack/layers.rs`             | Source  | Layer merge logic, `detect_language`                               |
| `src/conventions/tech_stack/manifest.rs`           | Source  | `package.json`, `Cargo.toml` parsers (Layer 1)                     |
| `src/conventions/tech_stack/task_runners.rs`       | Source  | Makefile / justfile / Taskfile parsers (Layer 2)                   |
| `src/conventions/tech_stack/probes.rs`             | Source  | Probe defaults table (Layer 3), including the go.mod row           |
| `src/conventions/tech_stack/categorize.rs`         | Source  | Stack-agnostic `categorize_script`                                 |
| `src/conventions/tech_stack/test_config.rs`        | Source  | Moved from current `tech_stack.rs`                                 |
| `src/conventions/tech_stack/tests.rs`              | Test    | Existing + new unit tests (split if >300 LOC)                      |
| `tests/fixtures/phase14-python-pytest/`            | Fixture | Layer 3 pytest detection                                           |
| `tests/fixtures/phase14-java-maven/`               | Fixture | Layer 3 mvn compile/test                                           |
| `tests/fixtures/phase14-java-gradle/`              | Fixture | Layer 3 wrapper-aware gradle                                       |
| `tests/fixtures/phase14-ruby-bundler/`             | Fixture | Layer 3 rake + rubocop                                             |
| `tests/fixtures/phase14-elixir-mix/`               | Fixture | Layer 3 mix compile/test                                           |
| `tests/fixtures/phase14-dotnet-csproj/`            | Fixture | Layer 3 dotnet build/test                                          |
| `tests/fixtures/phase14-php-composer/`             | Fixture | Layer 3 composer scripts.test                                      |
| `tests/fixtures/phase14-makefile-overrides-probe/` | Fixture | Layer 2 wins over Layer 3                                          |
| `tests/fixtures/phase14-justfile-overrides-probe/` | Fixture | Layer 2 wins over Layer 3                                          |
| `tests/fixtures/phase14-taskfile-overrides-probe/` | Fixture | Layer 2 wins over Layer 3                                          |
| `tests/fixtures/phase14-manifest-beats-runner/`    | Fixture | Layer 1 wins over Layer 2 (regression guard for current Phase 12 behavior) |
| `tests/fixtures/phase14-empty/`                    | Fixture | No manifest, no runner → empty scripts, clean success              |
| `tests/fixtures/phase14-taskfile-broken/`          | Fixture | Malformed `Taskfile.yml` → W056 emitted into collector, fall-through works |
| `tests/fixtures/phase14-makefile-adversarial/`     | Fixture | Parser robustness: continuations, includes, pattern rules, vars    |
| `tests/fixtures/phase14-justfile-adversarial/`     | Fixture | Parser robustness: attributes, aliases, shebangs, parameters       |
| `tests/fixtures/phase14-taskfile-adversarial/`     | Fixture | Parser robustness: includes, vars, nested tasks, preconditions     |

## Files Modified

### Ariadne repo

| File                                 | Change                                                                                                                                                        |
| ------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `src/conventions/mod.rs`             | `mod tech_stack;` stays; re-export unchanged                                                                                                                  |
| `src/conventions/tech_stack.rs`      | Deleted (replaced by submodule layout in D6)                                                                                                                  |
| `src/conventions/types.rs`           | `TechStack` gains `structured_diagnostics_cmd: Option<String>`. **`ScriptCategory` gains `PartialOrd, Ord` derives** to enable `(category, name)` sorting; variant order becomes load-bearing for deterministic output (`Build`, `Test`, `Lint`, `Dev`, `Other` — preserve current enum order) |
| `src/diagnostic.rs`                  | Register `W056TaskfileParseError`, `W057TaskRunnerReadFailed`, `W058JustfileParseError` in the `WarningCode` enum and `as_str()` map                          |
| `tests/invariants.rs`                | Add INV-19 (one-per-category), INV-20 (sort order), extend INV-11 (byte-identical determinism on `TechStack`); wire each new Phase 14 fixture into invariant runner |
| `tests/constraints.rs`               | Remove `src/conventions/tech_stack.rs` from `no_god_modules` allowlist                                                                                        |
| `Cargo.toml`                         | Add `serde_yaml = "0.9"` direct dependency (required by Taskfile parser in D2)                                                                                |
| `design/architecture.md`             | Add `conventions/` row to the Dependency Rules table (line 555-572); pre-existing gap also includes `temporal/` — both rows added for completeness            |
| `design/error-handling.md`           | Append W056–W058 rows to the Warning Code Table                                                                                                               |
| `design/determinism.md`              | Add row to §Summary of Sort Points: `TechStack.scripts` sorted by `(category, name)` before return                                                            |
| `design/decisions/log.md`            | Add D-153 (layered script discovery + winner-takes-all + Layer 2 runner priority + csproj sort key + Phase 16a forward link), D-154 (`structured_diagnostics_cmd` shell-string contract, strict-JSON consumption), D-155 (W056-W058 allocation + `tech_stack()` signature extension matching `temporal::analyze`, W049 reserved-gap rationale) |
| `design/ROADMAP.md`                  | Mark Phase 14 complete in Progress Tracking                                                                                                                   |

### Theseus repo (coordinated PR — see §Coordinated Landing)

| File                                 | Change                                                                                                                               |
| ------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------ |
| `src/gather.rs:91-92`                | Create local `DiagnosticCollector::new()` and pass to `tech_stack()` — 2-line addition matching existing `temporal::analyze` pattern |
| `src/execute.rs:40-58`               | Replace `starts_with("cargo")` hardcode with `tech_stack.structured_diagnostics_cmd.as_ref()` field read; execute via `sh -c`        |
| `src/verify.rs` tests                | Add `structured_diagnostics_cmd: None` to all synthetic `TechStack` fixtures (except Cargo-shaped which get `Some(...)`)             |
| `src/verify_preflight.rs` tests      | Add field to `stack_with_scripts` helper                                                                                             |
| `src/failure_class.rs` tests         | Add field to `empty_tech_stack` helper                                                                                               |
| `src/test_gen.rs` tests              | Add field to fixtures                                                                                                                |
| `src/discuss.rs` tests               | Add field to inline literal                                                                                                          |
| `src/prompt.rs` tests                | Add field to inline literal                                                                                                          |
| `src/retry.rs` tests                 | Add field to inline literal                                                                                                          |
| `Cargo.toml`                         | Update `ariadne-graph` dep pin per its current form (git tag/commit, local `[patch]`, or crates.io version — see §Coordinated Landing step 4) |

## Success Criteria

**Ariadne-side:**

1. `cargo test` passes — all existing `tech_stack` tests plus all new Phase 14 tests.
2. `cargo test no_god_modules` passes with the old `tech_stack.rs` allowlist entry removed and no new file in `src/conventions/tech_stack/` exceeds 300 LOC.
3. `cargo test no_hashmap_in_model` still passes — Phase 14 does not introduce `HashMap`.
4. `cargo test deterministic_output` still passes — two consecutive `tech_stack()` calls on the same fixture produce identical `TechStack`.
5. `TechStack` serializes cleanly via `serde_json::to_string` with the new `structured_diagnostics_cmd` field present only when populated (`skip_serializing_if = "Option::is_none"`).
6. **Fixture test — Python:** `phase14-python-pytest/` yields `scripts` containing `pytest`. Previously empty.
7. **Fixture test — Java (Maven):** `phase14-java-maven/` yields `mvn compile` and `mvn test`. Previously empty.
8. **Fixture test — Manifest priority:** `phase14-manifest-beats-runner/` (Cargo.toml + Makefile with `test:`) still yields `cargo build` / `cargo test` / `cargo clippy` — Layer 1 is not clobbered by Layer 2.
9. **Fixture test — Empty:** `phase14-empty/` yields `scripts == []`, zero warnings, exit code 0, caller can invoke Layer 4 (user declaration).
10. **Fixture test — Warning path:** `phase14-taskfile-broken/` with garbage YAML emits `W056 TaskfileParseError` into the passed `DiagnosticCollector`; `scripts` populated from remaining layers; no crash.
11. **Fixture test — `structured_diagnostics_cmd`:** Cargo-shaped fixture yields `Some("cargo build --message-format=json")`; all other ecosystem fixtures yield `None`.
12. **Invariants:** INV-11 extension, INV-19, INV-20 all pass on every Phase 14 fixture.
13. Warning codes W056–W058 appear in `design/error-handling.md` Warning Code Table.
13a. **D5 `categorize_script` cleanup verified by boundary tests:** new L1 unit tests assert that `jest`, `vitest`, `mocha`, `eslint` (as bare names without `test`/`lint`/`build` substring) no longer match any specific category — they fall into `ScriptCategory::Other`. `fmt`, `format`, `precommit`, `deploy`, `generate` also have explicit tests per D5's post-state (`fmt` → `Lint`, others → `Other`).

**Ariadne self-test (SIGN-OFF only, not automated):**

14. `ariadne build /Users/minddecay/Documents/Projects/Ariadne` produces `TechStack` with `scripts = [cargo build, cargo test, cargo clippy -- -D warnings]` and `structured_diagnostics_cmd = Some("cargo build --message-format=json")`. Zero warnings emitted.

**Theseus-side (coordinated PR, see §Coordinated Landing):**

15. `cargo check -p theseus` compiles against the new `ariadne-graph` API.
16. `cargo test -p theseus` passes with updated fixtures.
17. Theseus `execute.rs::parse_structured_build_errors` no longer contains the `starts_with("cargo")` string-match — replaced with field read.
18. Theseus `gather.rs` creates a local `DiagnosticCollector::new()` before the `tech_stack()` call.
19. Manual smoke test — run Theseus against a real Python project (any pytest repo): preflight detects `pytest`, `verify_tests` actually executes it, command runs.

## Testing Requirements (from testing.md)

- **L1 (unit):** cover every branch of every new function. See D8 for the exhaustive list.
- **L2 (fixture):** **16 new fixture projects** per the table in D8 — 7 happy-path ecosystem detection, 5 layer-priority, 1 empty, 1 warning-path (`phase14-taskfile-broken/`), and 3 adversarial (Makefile/justfile/Taskfile). Each fixture is a complete mini-project on disk so the tests exercise the real file-system IO path.
- **L3 (invariant):** extend `tests/invariants.rs` with three new invariants — INV-19 (one-per-category), INV-20 (sort order), and an INV-11 extension (byte-identical determinism across two consecutive `tech_stack()` calls). All run on every Phase 14 L2 fixture.
- **L4 (bench):** not required — see D8.

## Resolved Discussion Points (2026-04-12)

All DPs from the initial draft have been resolved after auditing the live consumer (Theseus) code. Each answer is now backed by specific `file:line` evidence.

| DP  | Question                              | Answer                                                     | Evidence                                                                                                                 |
| --- | ------------------------------------- | ---------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------ |
| 1   | Merge rule                            | **(A) Winner-takes-all**, Manifest > TaskRunner > Probe    | Theseus `verify.rs:106` — `.find(\|s\| s.category == category)` first match. `verify.rs:99-101` comment — "Ariadne provides ready-to-run commands". |
| 2   | Warning surface                       | **(A) Extend signature** with `&DiagnosticCollector`       | Theseus `gather.rs:196-197` already uses the same pattern for `temporal::analyze` (`src/temporal/mod.rs:17-20`).         |
| 3   | `structured_diagnostics_cmd` for TS   | **(C) Strict JSON only**, Cargo-only in Phase 14           | Theseus `execute.rs:68` parses via `serde_json::from_str` and filters `reason == "compiler-message"` — text would crash. |
| 4   | E2E targets                           | Internal fixtures (16 total, incl. 3 adversarial) + self-test on Ariadne | Theseus has 30+ synthetic `TechStack` fixtures, zero real-repo tests for this code path.                      |
| 5   | go.mod `test_framework` parity        | **(A) Keep** `test_framework = Some("go test")`            | Theseus `test_gen.rs:52` reads `test_framework.is_none()` for Go skip logic — breaking parity regresses test_gen.        |
| 6   | Split `tech_stack.rs` in Phase 14     | **(A) Split now**                                          | CLAUDE.md `<=300 lines` constraint + `tests/constraints.rs::no_god_modules` CI enforcement — not negotiable.             |

No open discussion points remain.

## Coordinated Landing

Phase 14 is a **breaking API change** on `ariadne_graph::conventions::tech_stack`. Theseus (`/Users/minddecay/Documents/Projects/theseus`) consumes this API today and must update in a synchronized PR. The Ariadne PR cannot land in isolation without regressing `cargo check -p theseus`.

### Ariadne changes (this spec)

All D1–D8 above. Landing order:
1. Feature branch with all D1–D8 changes.
2. `cargo test` green + self-test passes.
3. PR open, review, merge.
4. Bump `ariadne-graph` minor version in `Cargo.toml` (breaking API per semver).

### Theseus coordinated PR

A parallel Theseus PR must land synchronously. Required changes:

**1. `src/gather.rs:91-92` — add collector to `tech_stack()` call**

```rust
// BEFORE
let tech_stack = conventions::tech_stack(project_path, graph)
    .map_err(|e| anyhow::anyhow!("tech_stack failed: {e}"))?;

// AFTER (copies pattern from gather.rs:196-197)
let tech_stack_diagnostics = ariadne_graph::diagnostic::DiagnosticCollector::new();
let tech_stack = conventions::tech_stack(project_path, graph, &tech_stack_diagnostics)
    .map_err(|e| anyhow::anyhow!("tech_stack failed: {e}"))?;
// Optional: drain into telemetry or log. Safe to discard for initial landing.
```

**2. `src/execute.rs:40-58` — replace cargo hardcode with field read**

```rust
// BEFORE
pub fn parse_structured_build_errors(
    project_path: &Path,
    tech_stack: &ariadne_graph::conventions::types::TechStack,
) -> Option<Vec<StructuredBuildError>> {
    use ariadne_graph::conventions::types::ScriptCategory;
    let build_script = tech_stack.scripts.iter().find(|s| s.category == ScriptCategory::Build)?;
    let structured_cmd: Vec<&str> = if build_script.command.starts_with("cargo") {
        vec!["cargo", "build", "--message-format=json"]
    } else {
        return None;
    };
    let output = std::process::Command::new(structured_cmd[0])
        .args(&structured_cmd[1..])
        .current_dir(project_path)
        .output().ok()?;
    // …
}

// AFTER
pub fn parse_structured_build_errors(
    project_path: &Path,
    tech_stack: &ariadne_graph::conventions::types::TechStack,
) -> Option<Vec<StructuredBuildError>> {
    let structured_cmd = tech_stack.structured_diagnostics_cmd.as_ref()?;
    let output = std::process::Command::new("sh")
        .arg("-c")
        .arg(structured_cmd)
        .current_dir(project_path)
        .output().ok()?;
    // … rest unchanged (JSON parsing logic)
}
```

**3. Synthetic `TechStack` fixtures — add `structured_diagnostics_cmd: None`**

All synthetic `TechStack { … }` literals in Theseus tests must gain the new field. Known call sites:

| File                             | Fixture name(s)                                                      |
| -------------------------------- | -------------------------------------------------------------------- |
| `src/verify.rs`                  | `make_ts_tech_stack`, `make_rust_tech_stack`, `make_empty_tech_stack` |
| `src/verify_preflight.rs`        | `stack_with_scripts`                                                  |
| `src/failure_class.rs`           | `empty_tech_stack`                                                    |
| `src/test_gen.rs`                | `make_empty_tech_stack`, `make_ts_tech_stack`                         |
| `src/discuss.rs`                 | inline `TechStack { … }` in `test_build_qa_prompt_includes_tech_stack` |
| `src/prompt.rs`                  | inline `TechStack { … }` in doctests/unit tests                       |
| `src/retry.rs`                   | inline `TechStack { … }`                                              |

For Cargo-shaped fixtures (`make_rust_tech_stack`), set `structured_diagnostics_cmd: Some("cargo build --message-format=json".into())` to reflect production behavior. All others stay `None`.

**4. `Cargo.toml` — update `ariadne-graph` dep pin**

Theseus consumes `ariadne-graph` via a git dependency with a local `[patch]` override, not via a versioned crates.io release. The exact action depends on the current pin state at landing time:

- If the pin is a git tag or commit SHA → bump to the Phase 14 commit (or new tag).
- If the pin is a path `[patch]` override → no direct edit needed; `cargo check` picks up the local Ariadne working tree automatically during the review window.
- If `ariadne-graph` is ever published to crates.io → bump minor version (breaking API change per semver).

No assumption about which form is active — the plan step must check and act accordingly.

**5. `cargo test -p theseus` must pass** against the updated `ariadne-graph` before merging either PR.

### Landing sequence

1. Ariadne Phase 14 PR opens and is reviewed.
2. Theseus coordinated PR opens referencing the Ariadne PR, using a local `[patch]` or git dep during review.
3. Both PRs pass `cargo test` individually (Theseus with the local patch).
4. Merge Ariadne first.
5. Publish new `ariadne-graph` version (if published, otherwise path dep).
6. Rebase Theseus PR against latest master + published version, re-run tests, merge.
7. Both PRs reference each other in the description for traceability.
