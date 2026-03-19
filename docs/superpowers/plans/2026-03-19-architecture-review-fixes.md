# Architecture Review Fixes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix all issues identified in the 2026-03-19 architecture review — safety gaps, design doc drift, code quality, and structural improvements.

**Architecture:** Three tiers of fixes: (1) safety-critical code fixes, (2) targeted code improvements, (3) design document updates. Each task is independently committable.

**Tech Stack:** Rust, design docs (Markdown)

---

## Task 1: Fix DiagnosticCollector mutex poison vulnerability (F-3)

**Files:**
- Modify: `src/diagnostic.rs:257,313,319`

- [ ] **Step 1: Fix `warn()` method — replace `.lock().unwrap()` with poison recovery**

In `src/diagnostic.rs:257`, change:
```rust
let mut guard = self.inner.lock().unwrap();
```
to:
```rust
let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
```

- [ ] **Step 2: Fix `increment_unresolved()` — same pattern**

In `src/diagnostic.rs:313`, same change.

- [ ] **Step 3: Fix `drain()` — same pattern**

In `src/diagnostic.rs:319`, change:
```rust
let (mut warnings, counts) = self.inner.into_inner().unwrap();
```
to:
```rust
let (mut warnings, counts) = self.inner.into_inner().unwrap_or_else(|e| e.into_inner());
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p ariadne-graph diagnostic`
Expected: All tests pass

- [ ] **Step 5: Commit**

```bash
git add src/diagnostic.rs
git commit -m "ariadne(core): fix mutex poison panic in DiagnosticCollector"
```

---

## Task 2: Fix serde_json unwrap panics in MCP tools (F-4)

**Files:**
- Modify: `src/mcp/tools.rs` (~25 unwrap sites)

- [ ] **Step 1: Add helper function for safe JSON serialization**

Add at the bottom of `src/mcp/tools.rs` (before `edge_to_json`):
```rust
/// Serialize to pretty JSON, returning error string on failure instead of panicking.
fn to_json<T: serde::Serialize>(value: &T) -> String {
    serde_json::to_string_pretty(value)
        .unwrap_or_else(|e| format!("{{\"error\":\"serialization_failed\",\"reason\":\"{}\"}}", e))
}
```

- [ ] **Step 2: Replace all `serde_json::to_string_pretty(&...).unwrap()` with `to_json(&...)`**

Every tool method return statement that uses `.unwrap()` gets replaced. There are ~25 instances. Pattern:
```rust
// Before:
serde_json::to_string_pretty(&result).unwrap()
// After:
to_json(&result)
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p ariadne-graph --features serve`
Expected: All tests pass

- [ ] **Step 4: Commit**

```bash
git add src/mcp/tools.rs
git commit -m "ariadne(mcp): replace serde_json unwrap with safe fallback in tool handlers"
```

---

## Task 3: Fix Composition Root violation in mcp/server.rs (S-2)

**Files:**
- Modify: `src/mcp/server.rs` — remove `make_pipeline()`, accept pipeline as parameter
- Modify: `src/main.rs` — pass pipeline into `run()`

- [ ] **Step 1: Change `ServeConfig` to include a pipeline**

In `src/mcp/server.rs`, add `pipeline` field to `ServeConfig`:
```rust
pub struct ServeConfig {
    pub project_root: PathBuf,
    pub output_dir: PathBuf,
    pub debounce_ms: u64,
    pub watch_enabled: bool,
    pub pipeline: Arc<BuildPipeline>,
}
```
Add `use std::sync::Arc;` and `use crate::pipeline::BuildPipeline;` imports.

- [ ] **Step 2: Remove `make_pipeline()` function from server.rs**

Delete the `fn make_pipeline()` at the bottom of `server.rs`. Replace all `make_pipeline()` calls with `config.pipeline.clone()` or `Arc::clone(&config.pipeline)`.

Specifically:
- Line 42: `let pipeline = make_pipeline();` → `let pipeline = Arc::clone(&config.pipeline);` (but needs deref for `run_with_output` which takes `&self`) — since `BuildPipeline` methods take `&self`, `Arc<BuildPipeline>` works via deref.
- Line 62: `let pipeline = Arc::new(make_pipeline());` → `let pipeline = config.pipeline.clone();`

- [ ] **Step 3: Update `main.rs` to construct pipeline and pass it in**

In `main.rs`, in the `Commands::Serve` match arm, construct the pipeline:
```rust
let pipeline = Arc::new(BuildPipeline::new(
    Box::new(FsWalker::new()),
    Box::new(FsReader::new()),
    ParserRegistry::with_tier1(),
    Box::new(JsonSerializer),
));
let config = ariadne_graph::mcp::server::ServeConfig {
    project_root: abs_project,
    output_dir,
    debounce_ms: debounce,
    watch_enabled: !no_watch,
    pipeline,
};
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p ariadne-graph --features serve`
Expected: All tests pass

- [ ] **Step 5: Commit**

```bash
git add src/mcp/server.rs src/main.rs
git commit -m "ariadne(mcp): move pipeline construction to main.rs, fixing D-020 Composition Root violation"
```

---

## Task 4: Add SmellSeverity helper to deduplicate filtering (S-3 from review finding D-3)

**Files:**
- Modify: `src/model/smell.rs` — add `severity_level()` method
- Modify: `src/mcp/tools.rs` — use new method
- Modify: `src/main.rs` — use new method

- [ ] **Step 1: Add `severity_level()` to SmellSeverity**

In `src/model/smell.rs`:
```rust
impl SmellSeverity {
    /// Numeric severity level for comparison (High=2, Medium=1, Low=0).
    pub fn level(&self) -> u8 {
        match self {
            Self::High => 2,
            Self::Medium => 1,
            Self::Low => 0,
        }
    }

    /// Parse from string (case-insensitive).
    pub fn from_str_loose(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "high" => Self::High,
            "medium" => Self::Medium,
            _ => Self::Low,
        }
    }
}
```

- [ ] **Step 2: Update mcp/tools.rs smells handler**

Replace the severity filtering block (lines 489-507) with:
```rust
let filtered: Vec<_> = if let Some(ref min_sev) = params.min_severity {
    let min = crate::model::SmellSeverity::from_str_loose(min_sev);
    smells.into_iter().filter(|s| s.severity.level() >= min.level()).collect()
} else {
    smells
};
```

- [ ] **Step 3: Update main.rs smells handler**

Replace the severity filtering block (lines 957-977) with:
```rust
let filtered: Vec<_> = if let Some(ref min_sev) = min_severity {
    let min = ariadne_graph::model::SmellSeverity::from_str_loose(min_sev);
    smells.into_iter().filter(|s| s.severity.level() >= min.level()).collect()
} else {
    smells
};
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p ariadne-graph`
Expected: All tests pass

- [ ] **Step 5: Commit**

```bash
git add src/model/smell.rs src/mcp/tools.rs src/main.rs
git commit -m "ariadne(core): add SmellSeverity::level() to deduplicate severity filtering"
```

---

## Task 5: Remove StatsOutput re-export from serial/mod.rs (S-6)

**Files:**
- Modify: `src/serial/mod.rs` — remove re-export line
- Modify: `src/main.rs` — import StatsOutput from model instead

- [ ] **Step 1: Remove re-export from serial/mod.rs**

Delete line 12: `pub use crate::model::{StatsOutput, StatsSummary};`
And the comment on line 10-11.

- [ ] **Step 2: Fix import in main.rs**

Change line 15:
```rust
use ariadne_graph::serial::{GraphReader, StatsOutput};
```
to:
```rust
use ariadne_graph::serial::GraphReader;
use ariadne_graph::model::StatsOutput;
```

- [ ] **Step 3: Check for other callers**

Run: `rg "serial::\{.*StatsOutput" src/` and `rg "serial::StatsOutput" src/` — fix any other imports.

- [ ] **Step 4: Run tests**

Run: `cargo test -p ariadne-graph`
Expected: All tests pass

- [ ] **Step 5: Commit**

```bash
git add src/serial/mod.rs src/main.rs
git commit -m "ariadne(serial): remove StatsOutput re-export, import directly from model"
```

---

## Task 6: Fix update() warning routing bypass (S-5)

**Files:**
- Modify: `src/pipeline/mod.rs` — remove direct `eprintln!` in `update()`, propagate warnings via BuildOutput

- [ ] **Step 1: Remove local DiagnosticCollector + eprintln pattern in update()**

In `src/pipeline/mod.rs`, the `update()` method has 3 blocks (around lines 368-419) that create a local `DiagnosticCollector`, push a warning, drain it, and `eprintln!` the warnings. Instead, emit a verbose log and fall back to full build (which has its own proper diagnostic pipeline).

Replace the pattern:
```rust
let diagnostics = DiagnosticCollector::new();
diagnostics.warn(Warning { ... });
let report = diagnostics.drain();
for w in &report.warnings {
    eprintln!("warn[{}]: {}: {}", w.code, w.path, w.message);
}
```

With just a verbose log before the full build fallback:
```rust
if verbose {
    eprintln!("[delta]     corrupted graph: {} — falling back to full build", reason);
}
```

The full build's own `DiagnosticCollector` will handle warnings properly through the normal `format_warnings()` path in `main.rs`.

- [ ] **Step 2: Run tests**

Run: `cargo test -p ariadne-graph`
Expected: All tests pass

- [ ] **Step 3: Commit**

```bash
git add src/pipeline/mod.rs
git commit -m "ariadne(pipeline): remove warning routing bypass in update(), use verbose logging instead"
```

---

## Task 7: Update design/decisions/log.md — stale decisions

**Files:**
- Modify: `design/decisions/log.md`

- [ ] **Step 1: Mark D-047 as superseded by D-051**

Change D-047's status line from:
```
**Status:** Accepted
```
to:
```
**Status:** Partially superseded by D-051
```

Add a note at the end of D-047:
```
**Note (updated 2026-03-19):** The `serve` subcommand uses tokio due to `rmcp` crate requirements (D-051). The "no async runtime" principle still applies to all non-serve code paths (build, query, update, info).
```

- [ ] **Step 2: Update D-022 to reference free function**

In D-022, change:
```
**Decision:** Separate internal model ... Conversion via `impl From<ProjectGraph> for GraphOutput`.
```
to:
```
**Decision:** Separate internal model ... Conversion via `project_graph_to_output(graph, project_root)` free function (requires `project_root` parameter, so `From` impl is not possible).
```

- [ ] **Step 3: Commit**

```bash
git add design/decisions/log.md
git commit -m "ariadne(design): update D-047 supersession status and D-022 conversion description"
```

---

## Task 8: Update design/error-handling.md — add missing error codes

**Files:**
- Modify: `design/error-handling.md`

- [ ] **Step 1: Add E010-E013 to fatal error table**

Add after the existing E009 entry:
```
| E010 | McpServerFailed | MCP server failed to start | Exit 1 |
| E011 | LockFileHeld | Another ariadne server is running | Exit 1 |
| E012 | McpProtocolError | MCP protocol-level error | Exit 1 |
| E013 | InvalidArgument | Invalid CLI argument | Exit 1 |
```

- [ ] **Step 2: Add W014-W018 to warning table**

Add after the existing W013 entry:
```
| W014 | FsWatcherFailed | File system watcher failed to start | Fall back to 30s polling |
| W015 | IncrementalRebuildFailed | Auto-rebuild during MCP serving failed | Serve stale data |
| W016 | StaleLockRemoved | Removed stale lock file from dead process | Continue normally |
| W017 | SmellDetectionSkipped | Smell detection skipped (e.g., missing stats) | Return partial results |
| W018 | BlastRadiusTimeout | Blast radius computation exceeded timeout | Skip file in shotgun surgery detection |
```

- [ ] **Step 3: Commit**

```bash
git add design/error-handling.md
git commit -m "ariadne(design): add E010-E013 and W014-W018 to error taxonomy"
```

---

## Task 9: Update design/architecture.md — module table, storage format, pipeline types

**Files:**
- Modify: `design/architecture.md`

- [ ] **Step 1: Add analysis/ and mcp/ to the module dependency table**

Find the dependency rules table (around line 438) and add rows:
```
| `analysis/` | `model/`, `algo/` | `serial/`, `pipeline/`, `parser/`, `views/`, `mcp/` |
| `mcp/` | `model/`, `algo/`, `analysis/`, `serial/`, `pipeline/` | `parser/` |
```

- [ ] **Step 2: Add raw_imports.json to Storage Format section**

In the `.ariadne/graph/` file listing (around line 503), add:
```
│   ├── raw_imports.json # per-file import data for freshness engine (D-054)
```

- [ ] **Step 3: Add raw_imports.json to Git Tracking Policy**

In the git tracking policy section (around line 866), add a bullet:
```
- `.ariadne/graph/raw_imports.json`: **not committed** (internal data for freshness engine, regenerated on every build)
```

- [ ] **Step 4: Update BuildOutput in Pipeline Support Types**

Find the `BuildOutput` definition (around line 206) and add the missing fields:
```rust
BuildOutput {
    graph_path: PathBuf,
    clusters_path: PathBuf,
    stats_path: PathBuf,           // added Phase 2
    file_count: usize,
    edge_count: usize,
    cluster_count: usize,
    warnings: Vec<Warning>,
    counts: DiagnosticCounts,      // added Phase 2
}
```

- [ ] **Step 5: Add note about arch_depth ambiguity in graph.json schema**

In the graph.json example (around line 529), add a comment after `"arch_depth": 0`:
```
Note: `arch_depth` is computed via topological sort after SCC contraction. A value of 0 means
the file has no outgoing architectural dependencies (Layer 0 — foundations/utilities).
```

- [ ] **Step 6: Update pipeline flow to say "sequential" for read stage**

At line 339, change:
```
→ read_files() → Vec<FileContent>           (parallel via rayon on sorted list)
```
to:
```
→ read_files() → Vec<FileContent>           (sequential, I/O-bound)
```

- [ ] **Step 7: Commit**

```bash
git add design/architecture.md
git commit -m "ariadne(design): update architecture.md with Phase 3 modules, raw_imports.json, BuildOutput fields"
```

---

## Task 10: Update design/performance.md — fix stale delta claims

**Files:**
- Modify: `design/performance.md`

- [ ] **Step 1: Update the delta computation description**

Find the delta computation entry in the performance table and update to match D-050:
```
Delta computation: When changes are detected, performs a full rebuild (D-050).
The no-op fast path (zero changes) is the primary optimization — skips rebuild entirely.
True incremental re-parsing is deferred to Phase 3 MCP server.
```

Remove or update any claims of "O(changed files)" or "only re-parses changed files."

- [ ] **Step 2: Commit**

```bash
git add design/performance.md
git commit -m "ariadne(design): fix stale delta computation claims in performance.md per D-050"
```

---

## Task 11: Update design/determinism.md — add raw_imports.json sort points

**Files:**
- Modify: `design/determinism.md`

- [ ] **Step 1: Add raw_imports.json to Sort Points table**

Add entry:
```
| raw_imports.json | File keys | BTreeMap<String, Vec<RawImportOutput>> — outer keys sorted lexicographically |
| raw_imports.json | Import entries per file | Vec order matches parsed_files iteration (sorted by CanonicalPath before parsing) |
```

- [ ] **Step 2: Commit**

```bash
git add design/determinism.md
git commit -m "ariadne(design): add raw_imports.json sort points to determinism.md"
```

---

## Task 12: Run full test suite and verify

- [ ] **Step 1: Run full test suite**

Run: `cargo test --all-features`
Expected: All tests pass

- [ ] **Step 2: Run clippy**

Run: `cargo clippy --all-features -- -D warnings`
Expected: No warnings
