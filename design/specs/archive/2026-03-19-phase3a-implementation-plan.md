# Phase 3a: MCP Server — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ariadne runs as a long-lived MCP server (`ariadne serve`) that loads the dependency graph into memory, answers 11 MCP tool queries instantly, and keeps the graph fresh via fs watching and incremental rebuild.

**Architecture:** Single `ariadne` binary gains `serve` subcommand. MCP protocol via `rmcp` (official SDK) over stdio. `ArcSwap<GraphState>` for lock-free reads. Background thread rebuilds graph on fs changes, atomic state swap. Freshness engine with lightweight import re-parse for two-level confidence scoring.

**Tech Stack:** rmcp 0.16, tokio 1 (isolated to serve), arc-swap 1, notify 6, notify-debouncer-full, serde/serde_json (existing)

**Spec:** `design/specs/2026-03-19-phase3a-mcp-server.md`

---

## File Structure

### New Files

| File | Responsibility |
|------|---------------|
| `src/mcp/mod.rs` | Module re-exports |
| `src/mcp/server.rs` | McpServer struct, startup/shutdown, rmcp setup |
| `src/mcp/tools.rs` | 11 MCP tool handlers via #[tool] macro |
| `src/mcp/state.rs` | GraphState, FreshnessState, freshness check logic, index building |
| `src/mcp/watch.rs` | FileWatcher, debounce, incremental rebuild orchestration |
| `src/mcp/lock.rs` | Lock file create/check/remove/stale detection |
| `tests/mcp_tests.rs` | MCP integration tests |

### Modified Files

| File | Change |
|------|--------|
| `Cargo.toml` | Add serve feature flag + optional dependencies |
| `src/main.rs` | Add `Serve` subcommand, dispatch to mcp::server |
| `src/lib.rs` | Re-export mcp module behind feature flag |
| `src/pipeline/mod.rs` | Serialize raw_imports.json; add reparse_imports() method |
| `src/serial/mod.rs` | Add write_raw_imports/read_raw_imports to traits |
| `src/serial/json.rs` | Implement raw imports JSON read/write |
| `src/diagnostic.rs` | Add E010, E011, E012, W014, W015, W016 |

---

## Task 1: Dependencies and Feature Flag

**Files:**
- Modify: `Cargo.toml`

- [ ] **Step 1: Add feature flags and optional dependencies to Cargo.toml**

Add after existing `[dependencies]`:

```toml
# MCP server dependencies (optional, behind "serve" feature)
rmcp = { version = "0.16", features = ["server", "transport-io"], optional = true }
tokio = { version = "1", features = ["rt-multi-thread", "macros", "signal", "sync"], optional = true }
arc-swap = { version = "1", optional = true }
notify = { version = "6", optional = true }
notify-debouncer-full = { version = "0.3", optional = true }

[target.'cfg(unix)'.dependencies]
libc = { version = "0.2", optional = true }

[features]
default = ["serve"]
serve = ["rmcp", "tokio", "arc-swap", "notify", "notify-debouncer-full", "libc"]
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check`
Expected: compiles with no errors

- [ ] **Step 3: Verify no-default-features builds**

Run: `cargo check --no-default-features`
Expected: compiles without tokio/rmcp

- [ ] **Step 4: Commit**

```bash
git add Cargo.toml Cargo.lock
git commit -m "ariadne(core): add serve feature flag with MCP server dependencies"
```

---

## Task 2: New Error and Warning Codes

**Files:**
- Modify: `src/diagnostic.rs`

- [ ] **Step 1: Add new FatalError variants**

In `FatalError` enum (after E009 FileNotInGraph):

```rust
#[error("E010: Failed to start MCP server: {reason}")]
McpServerFailed { reason: String },

#[error("E011: Another ariadne server is running (PID {pid}). Stop it first or remove {}", lock_path.display())]
LockFileHeld { pid: u32, lock_path: PathBuf },

#[error("E012: MCP protocol error: {reason}")]
McpProtocolError { reason: String },
```

- [ ] **Step 2: Add new WarningCode variants**

In `WarningCode` enum (after W013):

```rust
W014,  // fs watcher failed
W015,  // incremental rebuild failed
W016,  // stale lock file removed
```

- [ ] **Step 3: Update warning formatting**

In `format_warnings()` and related code, add display strings for W014-W016.

- [ ] **Step 4: Run tests**

Run: `cargo test`
Expected: all existing tests pass

- [ ] **Step 5: Commit**

```bash
git add src/diagnostic.rs
git commit -m "ariadne(core): add Phase 3a error codes E010-E012, W014-W016"
```

---

## Task 3: Raw Imports Serialization

**Files:**
- Modify: `src/serial/mod.rs`
- Modify: `src/serial/json.rs`
- Modify: `src/pipeline/mod.rs`
- Test: `tests/pipeline_tests.rs`

- [ ] **Step 1: Write test for raw_imports.json round-trip**

In `tests/pipeline_tests.rs`:

```rust
#[test]
fn test_raw_imports_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let serializer = JsonSerializer;

    let mut imports = BTreeMap::new();
    imports.insert(
        "src/auth/login.ts".to_string(),
        vec![
            RawImportOutput {
                path: "./session".to_string(),
                symbols: vec!["getSession".to_string()],
                is_type_only: false,
            },
        ],
    );

    serializer.write_raw_imports(&imports, dir.path()).unwrap();

    let reader = JsonSerializer;
    let loaded = reader.read_raw_imports(dir.path()).unwrap();
    assert_eq!(loaded, Some(imports));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test test_raw_imports_round_trip`
Expected: FAIL — methods don't exist yet

- [ ] **Step 3: Add RawImportOutput type and trait methods to serial/mod.rs**

Add type:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RawImportOutput {
    pub path: String,
    pub symbols: Vec<String>,
    pub is_type_only: bool,
}
```

Add to `GraphSerializer` trait:

```rust
fn write_raw_imports(
    &self,
    imports: &BTreeMap<String, Vec<RawImportOutput>>,
    dir: &Path,
) -> Result<(), FatalError>;
```

Add to `GraphReader` trait:

```rust
fn read_raw_imports(
    &self,
    dir: &Path,
) -> Result<Option<BTreeMap<String, Vec<RawImportOutput>>>, FatalError>;
```

- [ ] **Step 4: Implement in serial/json.rs**

```rust
fn write_raw_imports(
    &self,
    imports: &BTreeMap<String, Vec<RawImportOutput>>,
    dir: &Path,
) -> Result<(), FatalError> {
    let path = dir.join("raw_imports.json");
    let tmp = dir.join(format!("raw_imports.{}.tmp", std::process::id()));
    let file = std::fs::File::create(&tmp).map_err(|e| FatalError::OutputNotWritable {
        path: tmp.clone(),
        reason: e.to_string(),
    })?;
    let writer = std::io::BufWriter::new(file);
    serde_json::to_writer_pretty(writer, imports).map_err(|e| FatalError::OutputNotWritable {
        path: path.clone(),
        reason: e.to_string(),
    })?;
    std::fs::rename(&tmp, &path).map_err(|e| FatalError::OutputNotWritable {
        path: path.clone(),
        reason: e.to_string(),
    })?;
    Ok(())
}

fn read_raw_imports(
    &self,
    dir: &Path,
) -> Result<Option<BTreeMap<String, Vec<RawImportOutput>>>, FatalError> {
    let path = dir.join("raw_imports.json");
    if !path.exists() {
        return Ok(None);
    }
    let content = std::fs::read_to_string(&path).map_err(|e| FatalError::GraphCorrupted {
        path: path.display().to_string(),
        reason: e.to_string(),
    })?;
    let imports: BTreeMap<String, Vec<RawImportOutput>> =
        serde_json::from_str(&content).map_err(|e| FatalError::GraphCorrupted {
            path: path.display().to_string(),
            reason: e.to_string(),
        })?;
    Ok(Some(imports))
}
```

- [ ] **Step 5: Integrate into pipeline — serialize raw_imports during build**

In `pipeline/mod.rs`, in `run_with_output()`, after stats serialization, add:

```rust
// Convert ParsedFile raw imports to serializable format
let raw_imports_output: BTreeMap<String, Vec<RawImportOutput>> = parsed_files
    .iter()
    .map(|pf| {
        let key = pf.path.as_str().to_string();
        let imports = pf.imports.iter().map(|ri| RawImportOutput {
            path: ri.path.clone(),
            symbols: ri.symbols.clone(),
            is_type_only: ri.is_type_only,
        }).collect();
        (key, imports)
    })
    .collect();
self.serializer.write_raw_imports(&raw_imports_output, output_dir)?;
```

- [ ] **Step 6: Run tests**

Run: `cargo test`
Expected: all tests pass, including round-trip test

- [ ] **Step 7: Verify raw_imports.json is produced by build**

Run: `cargo test test_raw_imports_round_trip -- --nocapture`
Expected: PASS

- [ ] **Step 8: Commit**

```bash
git add src/serial/mod.rs src/serial/json.rs src/pipeline/mod.rs tests/pipeline_tests.rs
git commit -m "ariadne(serial): add raw_imports.json serialization for freshness engine"
```

---

## Task 4: Pipeline Extensions for MCP

**Files:**
- Modify: `src/parser/registry.rs`
- Modify: `src/pipeline/mod.rs`
- Test: `tests/pipeline_tests.rs`

This task adds `reparse_imports()` as a low-level method on `ParserRegistry`, then exposes it through `BuildPipeline` as a public API. The freshness engine in `mcp/` calls `pipeline.reparse_imports()`, preserving the dependency rule that `mcp/` never depends on `parser/` directly.

- [ ] **Step 1: Write test for reparse_imports**

```rust
#[test]
fn test_reparse_imports_returns_imports_for_known_extension() {
    let pipeline = BuildPipeline::new(
        Box::new(FsWalker::new()),
        Box::new(FsReader::new()),
        ParserRegistry::with_tier1(),
        Box::new(JsonSerializer),
    );
    let source = b"import { foo } from './bar';";
    let result = pipeline.reparse_imports("ts", source);
    assert!(result.is_some());
    let imports = result.unwrap();
    assert!(!imports.is_empty());
    assert_eq!(imports[0].path, "./bar");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test test_reparse_imports`
Expected: FAIL

- [ ] **Step 3: Add reparse_imports to ParserRegistry (private helper)**

In `src/parser/registry.rs`:

```rust
/// Re-parse imports from source bytes for a given file extension.
/// Called by BuildPipeline — not intended for direct use from mcp/.
pub fn reparse_imports(&self, extension: &str, source: &[u8]) -> Option<Vec<RawImport>> {
    let parser = self.parser_for(extension)?;
    let ts_lang = parser.tree_sitter_language();
    let mut ts_parser = tree_sitter::Parser::new();
    ts_parser.set_language(&ts_lang).ok()?;
    let tree = ts_parser.parse(source, None)?;
    Some(parser.extract_imports(&tree, source))
}
```

- [ ] **Step 4: Add reparse_imports wrapper to BuildPipeline**

In `src/pipeline/mod.rs`, add method to `BuildPipeline`:

```rust
/// Re-parse imports from source bytes for a given file extension.
/// Used by the freshness engine for lightweight import change detection.
/// This method preserves the dependency boundary: mcp/ → pipeline/ → parser/.
pub fn reparse_imports(&self, extension: &str, source: &[u8]) -> Option<Vec<RawImport>> {
    self.registry.reparse_imports(extension, source)
}
```

- [ ] **Step 5: Run test**

Run: `cargo test test_reparse_imports`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add src/parser/registry.rs src/pipeline/mod.rs tests/pipeline_tests.rs
git commit -m "ariadne(pipeline): add reparse_imports for freshness engine import change detection"
```

---

## Task 5: Lock File Management

**Files:**
- Create: `src/mcp/mod.rs`
- Create: `src/mcp/lock.rs`
- Modify: `src/lib.rs`
- Test: `tests/mcp_tests.rs`

- [ ] **Step 1: Create mcp module skeleton**

`src/mcp/mod.rs`:

```rust
#[cfg(feature = "serve")]
pub mod lock;
#[cfg(feature = "serve")]
pub mod state;
#[cfg(feature = "serve")]
pub mod tools;
#[cfg(feature = "serve")]
pub mod watch;
#[cfg(feature = "serve")]
pub mod server;
```

Update `src/lib.rs` — add:

```rust
#[cfg(feature = "serve")]
pub mod mcp;
```

- [ ] **Step 2: Write lock file tests**

`tests/mcp_tests.rs`:

```rust
#[cfg(feature = "serve")]
mod lock_tests {
    use ariadne_graph::mcp::lock::{acquire_lock, release_lock, check_lock};
    use tempfile::tempdir;

    #[test]
    fn test_acquire_and_release_lock() {
        let dir = tempdir().unwrap();
        let lock_path = dir.path().join(".lock");

        acquire_lock(&lock_path).unwrap();
        assert!(lock_path.exists());

        let status = check_lock(&lock_path).unwrap();
        assert!(status.is_held_by_us());

        release_lock(&lock_path).unwrap();
        assert!(!lock_path.exists());
    }

    #[test]
    fn test_check_lock_no_file() {
        let dir = tempdir().unwrap();
        let lock_path = dir.path().join(".lock");

        let status = check_lock(&lock_path).unwrap();
        assert!(status.is_free());
    }

    #[test]
    fn test_stale_lock_detection() {
        let dir = tempdir().unwrap();
        let lock_path = dir.path().join(".lock");

        // Write a lock with a fake PID that doesn't exist
        let content = serde_json::json!({
            "pid": 999999999u32,
            "started_at": "2026-01-01T00:00:00Z"
        });
        std::fs::write(&lock_path, serde_json::to_string(&content).unwrap()).unwrap();

        let status = check_lock(&lock_path).unwrap();
        assert!(status.is_stale());
    }
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test lock_tests`
Expected: FAIL — module doesn't exist

- [ ] **Step 4: Implement lock.rs**

`src/mcp/lock.rs`:

```rust
use crate::diagnostic::FatalError;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Serialize, Deserialize)]
struct LockContent {
    pid: u32,
    started_at: String,
}

#[derive(Debug)]
pub enum LockStatus {
    Free,
    HeldByUs,
    HeldByOther { pid: u32 },
    Stale { pid: u32 },
}

impl LockStatus {
    pub fn is_free(&self) -> bool { matches!(self, LockStatus::Free) }
    pub fn is_held_by_us(&self) -> bool { matches!(self, LockStatus::HeldByUs) }
    pub fn is_stale(&self) -> bool { matches!(self, LockStatus::Stale { .. }) }
}

pub fn acquire_lock(lock_path: &Path) -> Result<(), FatalError> {
    let status = check_lock(lock_path)?;
    match status {
        LockStatus::Free | LockStatus::Stale { .. } => {
            if matches!(status, LockStatus::Stale { pid } if pid > 0) {
                // Stale lock — remove and log (W016 handled by caller)
            }
            let content = LockContent {
                pid: std::process::id(),
                started_at: current_timestamp(),
            };
            let json = serde_json::to_string_pretty(&content).unwrap();
            if let Some(parent) = lock_path.parent() {
                std::fs::create_dir_all(parent).ok();
            }
            std::fs::write(lock_path, json).map_err(|e| FatalError::OutputNotWritable {
                path: lock_path.to_path_buf(),
                reason: e.to_string(),
            })?;
            Ok(())
        }
        LockStatus::HeldByUs => Ok(()), // Already held by us
        LockStatus::HeldByOther { pid } => Err(FatalError::LockFileHeld {
            pid,
            lock_path: lock_path.to_path_buf(),
        }),
    }
}

pub fn release_lock(lock_path: &Path) -> Result<(), FatalError> {
    if lock_path.exists() {
        std::fs::remove_file(lock_path).map_err(|e| FatalError::OutputNotWritable {
            path: lock_path.to_path_buf(),
            reason: e.to_string(),
        })?;
    }
    Ok(())
}

pub fn check_lock(lock_path: &Path) -> Result<LockStatus, FatalError> {
    if !lock_path.exists() {
        return Ok(LockStatus::Free);
    }
    let content = std::fs::read_to_string(lock_path).map_err(|e| FatalError::GraphCorrupted {
        path: lock_path.to_path_buf(),
        reason: e.to_string(),
    })?;
    let lock: LockContent = serde_json::from_str(&content).map_err(|e| FatalError::GraphCorrupted {
        path: lock_path.to_path_buf(),
        reason: e.to_string(),
    })?;

    let current_pid = std::process::id();
    if lock.pid == current_pid {
        return Ok(LockStatus::HeldByUs);
    }

    if is_process_alive(lock.pid) {
        Ok(LockStatus::HeldByOther { pid: lock.pid })
    } else {
        Ok(LockStatus::Stale { pid: lock.pid })
    }
}

#[cfg(unix)]
fn is_process_alive(pid: u32) -> bool {
    unsafe { libc::kill(pid as i32, 0) == 0 }
}

#[cfg(not(unix))]
fn is_process_alive(_pid: u32) -> bool {
    // Conservative: assume alive on non-Unix
    true
}

fn current_timestamp() -> String {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| "unknown".to_string())
}
```

**Note:** `libc` dependency is already added in Task 1 under the `serve` feature flag.

- [ ] **Step 5: Run tests**

Run: `cargo test lock_tests`
Expected: all 3 lock tests pass

- [ ] **Step 7: Commit**

```bash
git add src/mcp/ src/lib.rs tests/mcp_tests.rs Cargo.toml Cargo.lock
git commit -m "ariadne(mcp): implement lock file management with stale detection"
```

---

## Task 6: GraphState and Freshness Engine

**Files:**
- Create: `src/mcp/state.rs`
- Test: `tests/mcp_tests.rs`

- [ ] **Step 1: Write freshness tests**

```rust
#[cfg(feature = "serve")]
mod freshness_tests {
    use ariadne_graph::mcp::state::{GraphState, FreshnessState};
    // ... test that freshness check detects hash mismatch
    // ... test that body-only change keeps structural_confidence high
    // ... test confidence score computation
}
```

- [ ] **Step 2: Implement GraphState struct**

`src/mcp/state.rs` — the core state container with derived indices and freshness:

```rust
use crate::model::*;
use crate::serial::RawImportOutput;
use crate::parser::traits::RawImport;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::time::SystemTime;

pub struct GraphState {
    pub graph: ProjectGraph,
    pub stats: StatsOutput,
    pub clusters: ClusterMap,
    pub reverse_index: BTreeMap<CanonicalPath, Vec<Edge>>,
    pub layer_index: BTreeMap<u32, Vec<CanonicalPath>>,
    pub file_hashes: BTreeMap<CanonicalPath, ContentHash>,
    pub raw_imports: BTreeMap<CanonicalPath, Vec<RawImportOutput>>,
    pub freshness: FreshnessState,
    pub loaded_at: SystemTime,
}

pub struct FreshnessState {
    pub stale_files: BTreeSet<CanonicalPath>,
    pub structurally_changed: BTreeSet<CanonicalPath>,
    pub new_files: Vec<PathBuf>,
    pub removed_files: Vec<CanonicalPath>,
    pub hash_confidence: f64,
    pub structural_confidence: f64,
    pub last_full_check: SystemTime,
}
```

Include functions:
- `GraphState::from_loaded_data()` — builds derived indices from loaded graph/stats/clusters
- `GraphState::build_reverse_index()` — precompute reverse adjacency
- `GraphState::build_layer_index()` — precompute layer → files mapping
- `FreshnessState::new()` — fresh state with 1.0 confidence
- `check_file_freshness()` — hash + import re-parse for a single file

- [ ] **Step 3: Run tests**

Run: `cargo test freshness_tests`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add src/mcp/state.rs tests/mcp_tests.rs
git commit -m "ariadne(mcp): implement GraphState with two-level freshness engine"
```

---

## Task 7: MCP Tool Handlers

**Files:**
- Create: `src/mcp/tools.rs`
- Test: `tests/mcp_tests.rs`

This is the largest task — 11 tool handlers. Each is a thin wrapper.

- [ ] **Step 1: Write unit tests for tool logic**

Test the core logic functions (not MCP transport). For each tool, test the data transformation:

```rust
#[cfg(feature = "serve")]
mod tool_logic_tests {
    // test_overview_aggregation
    // test_file_lookup_found
    // test_file_lookup_not_found
    // test_centrality_filter
    // test_dependencies_direction_filter
    // test_freshness_response
}
```

- [ ] **Step 2: Implement tool handler struct with rmcp**

`src/mcp/tools.rs`:

```rust
use arc_swap::ArcSwap;
use rmcp::prelude::*;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use crate::mcp::state::GraphState;

#[derive(Clone)]
pub struct AriadneTools {
    state: Arc<ArcSwap<GraphState>>,
    rebuilding: Arc<AtomicBool>,
    project_root: std::path::PathBuf,
}
```

- [ ] **Step 3: Implement T1-T4 (overview, file, blast_radius, subgraph)**

Each tool uses `#[tool]` macro from rmcp. Load state via `self.state.load()`, call existing algo functions, return JSON.

- [ ] **Step 4: Implement T5-T8 (centrality, cycles, layers, cluster)**

Filter/lookup operations on cached state.

- [ ] **Step 5: Implement T9-T11 (dependencies, freshness, views_export)**

T9: filter edges by direction. T10: return FreshnessState. T11: read markdown files from `.ariadne/views/`.

- [ ] **Step 6: Run tests**

Run: `cargo test tool_logic_tests`
Expected: all pass

- [ ] **Step 7: Commit**

```bash
git add src/mcp/tools.rs tests/mcp_tests.rs
git commit -m "ariadne(mcp): implement 11 MCP tool handlers"
```

---

## Task 8: File Watcher and Auto-Update

**Files:**
- Create: `src/mcp/watch.rs`
- Test: `tests/mcp_tests.rs`

- [ ] **Step 1: Write test for file pattern filtering**

```rust
#[test]
fn test_should_trigger_rebuild() {
    let extensions: HashSet<String> = ["ts", "js", "rs", "go", "py"]
        .iter().map(|s| s.to_string()).collect();

    assert!(should_trigger_rebuild(Path::new("src/foo.ts"), &extensions));
    assert!(should_trigger_rebuild(Path::new("src/bar.rs"), &extensions));
    assert!(!should_trigger_rebuild(Path::new("README.md"), &extensions));
    assert!(!should_trigger_rebuild(Path::new("image.png"), &extensions));
}
```

- [ ] **Step 2: Implement FileWatcher**

`src/mcp/watch.rs`:

```rust
use arc_swap::ArcSwap;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use crate::mcp::state::GraphState;
use crate::pipeline::BuildPipeline;

pub struct FileWatcher {
    pub debounce_ms: u64,
    pub state: Arc<ArcSwap<GraphState>>,
    pub rebuilding: Arc<AtomicBool>,
    pub pipeline: Arc<BuildPipeline>,
    pub known_extensions: HashSet<String>,
    pub project_root: PathBuf,
    pub output_dir: PathBuf,
}

pub fn should_trigger_rebuild(path: &Path, known_extensions: &HashSet<String>) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|ext| known_extensions.contains(ext))
        .unwrap_or(false)
}
```

Implement `FileWatcher::start()` — spawns notify watcher with debounce, filters events, triggers rebuild on `tokio::spawn_blocking`.

- [ ] **Step 3: Implement rebuild logic**

The rebuild function:
1. Compute delta via `algo::delta::compute_delta()` (changed/added/removed sets)
2. If `requires_full_recompute` (>5% changes) → full rebuild via `pipeline.run_with_output()`
3. Else → still do full rebuild via `pipeline.run_with_output()` for correctness in v1
4. Build new `GraphState` from results via `GraphState::from_loaded_data()`
5. `state.store(Arc::new(new_state))`

**Note:** The spec defines a true incremental path (re-parse only changed files, patch graph). For v1, all paths use full rebuild — the delta detection provides the no-op fast path (no changes → no rebuild) which is the most valuable optimization. True incremental re-parsing can be added later as a performance optimization within this same architecture — the `pipeline.reparse_imports()` and `algo/delta.rs` scaffolding are in place for it.

- [ ] **Step 4: Run tests**

Run: `cargo test`
Expected: all pass

- [ ] **Step 5: Commit**

```bash
git add src/mcp/watch.rs tests/mcp_tests.rs
git commit -m "ariadne(mcp): implement file watcher with debounce and auto-rebuild"
```

---

## Task 9: MCP Server Core

**Files:**
- Create: `src/mcp/server.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: Implement McpServer**

`src/mcp/server.rs`:

```rust
use crate::diagnostic::FatalError;
use crate::mcp::lock::{acquire_lock, release_lock};
use crate::mcp::state::GraphState;
use crate::mcp::tools::AriadneTools;
use crate::mcp::watch::FileWatcher;
use crate::pipeline::BuildPipeline;
use crate::parser::ParserRegistry;
use crate::serial::json::JsonSerializer;
use crate::serial::GraphReader;
use arc_swap::ArcSwap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

pub struct ServeConfig {
    pub project_root: PathBuf,
    pub output_dir: PathBuf,
    pub debounce_ms: u64,
    pub watch_enabled: bool,
}

pub async fn run(
    pipeline: BuildPipeline,
    registry: ParserRegistry,
    config: ServeConfig,
) -> Result<(), FatalError> {
    // 1. Acquire lock
    let lock_path = config.output_dir.join(".lock");
    acquire_lock(&lock_path)?;

    // 2. Load or build graph
    let reader = JsonSerializer;
    let graph_state = load_or_build(&pipeline, &reader, &config)?;

    // 3. Setup ArcSwap state
    let state = Arc::new(ArcSwap::from_pointee(graph_state));
    let rebuilding = Arc::new(AtomicBool::new(false));

    // 4. Start file watcher (if enabled)
    if config.watch_enabled {
        // spawn FileWatcher
    }

    // 5. Register signal handlers for graceful shutdown
    let lock_for_shutdown = lock_path.clone();
    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.ok();
        release_lock(&lock_for_shutdown).ok();
        std::process::exit(0);
    });

    // 6. Start MCP server via rmcp
    let tools = AriadneTools::new(state.clone(), rebuilding.clone(), config.project_root.clone());
    // rmcp server setup on stdio...

    // 7. Cleanup on shutdown (normal exit)
    release_lock(&lock_path)?;
    Ok(())
}
```

- [ ] **Step 2: Add Serve subcommand to main.rs**

Add to `Commands` enum:

```rust
/// Start MCP server for instant graph queries
Serve {
    /// Project root to serve
    #[arg(long, default_value = ".")]
    project: PathBuf,

    /// Debounce milliseconds for file watcher
    #[arg(long, default_value = "2000")]
    debounce: u64,

    /// Disable file system watcher
    #[arg(long)]
    no_watch: bool,
},
```

Add match arm that constructs pipeline, registry, config, and calls `mcp::server::run()` inside a tokio runtime.

- [ ] **Step 3: Verify compilation**

Run: `cargo check`
Expected: compiles

- [ ] **Step 4: Run all tests**

Run: `cargo test`
Expected: all pass

- [ ] **Step 5: Commit**

```bash
git add src/mcp/server.rs src/main.rs
git commit -m "ariadne(mcp): implement MCP server core with serve subcommand"
```

---

## Task 10: CLI Lock Check Integration

**Files:**
- Modify: `src/main.rs`

- [ ] **Step 1: Add lock check to build and update commands**

Before running `pipeline.run_with_output()` in the `Build` and `Update` arms:

```rust
// Check if MCP server is running
let lock_path = output_dir.join(".lock");
if let Ok(status) = crate::mcp::lock::check_lock(&lock_path) {
    if let crate::mcp::lock::LockStatus::HeldByOther { pid } = status {
        return Err(FatalError::LockFileHeld {
            pid,
            lock_path: lock_path.display().to_string(),
        });
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test`
Expected: all pass

- [ ] **Step 3: Commit**

```bash
git add src/main.rs
git commit -m "ariadne(cli): check lock file before build/update to prevent conflicts with MCP server"
```

---

## Task 11: Integration Tests

**Files:**
- Modify: `tests/mcp_tests.rs`

- [ ] **Step 1: Write MCP integration test**

Test that starts server subprocess, sends JSON-RPC, verifies response:

```rust
#[cfg(feature = "serve")]
#[test]
fn test_mcp_server_initialize() {
    // Build the fixture first
    let fixture = fixture_path("typescript_simple");
    // ... run ariadne build on fixture
    // ... spawn ariadne serve --project <fixture> as subprocess
    // ... send initialize JSON-RPC request via stdin
    // ... read response from stdout
    // ... verify capabilities include tools
    // ... kill subprocess
}
```

- [ ] **Step 2: Write tool response tests**

Test ariadne_overview, ariadne_file, ariadne_freshness via subprocess.

- [ ] **Step 3: Run integration tests**

Run: `cargo test mcp_integration`
Expected: all pass

- [ ] **Step 4: Commit**

```bash
git add tests/mcp_tests.rs
git commit -m "ariadne(test): add MCP server integration tests"
```

---

## Task 12: Final Verification

- [ ] **Step 1: Run full test suite**

Run: `cargo test`
Expected: all tests pass

- [ ] **Step 2: Run with no-default-features**

Run: `cargo check --no-default-features`
Expected: compiles without serve feature

- [ ] **Step 3: Manual smoke test**

```bash
cargo build --release
./target/release/ariadne build tests/fixtures/typescript_simple
./target/release/ariadne serve --project tests/fixtures/typescript_simple --no-watch
# Send initialize request manually, verify response
```

- [ ] **Step 4: Verify raw_imports.json is produced**

```bash
ls tests/fixtures/typescript_simple/.ariadne/graph/raw_imports.json
```

- [ ] **Step 5: Update decision log**

Add D-051 through D-055 to `design/decisions/log.md` with date, status, context, decision, and rationale as per existing format.

- [ ] **Step 6: Create performance benchmarks**

Add to `benches/mcp_bench.rs`:
- `bench_mcp_overview` on 3k-node graph: target <5ms
- `bench_mcp_blast_radius` on 3k-node graph: target <10ms
- `bench_freshness_check` (10 files): target <20ms
- `bench_server_startup` (load 3k-node graph): target <1s

- [ ] **Step 7: Final commit if any cleanup needed**

```bash
git add -A
git commit -m "ariadne(mcp): Phase 3a complete — MCP server with 11 tools, freshness engine, auto-update"
```
