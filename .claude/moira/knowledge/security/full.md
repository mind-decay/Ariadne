<!-- moira:deep-scan security 2026-03-21 -->

# Security Surface Scan — Ariadne

**Date:** 2026-03-21
**Scope:** `/Users/minddecay/Documents/Projects/Ariadne/src/` (all `.rs` files)
**Method:** Static analysis of source code for hardcoded secrets, input validation gaps, unsafe patterns, and sensitive data handling.

---

## 1. Hardcoded Secrets

**No hardcoded secrets found.** No API keys, tokens, passwords, or connection strings were observed in any source file. The codebase does not contain `.env` loading, credential storage, or authentication mechanisms.

---

## 2. Input Validation Gaps

### 2a. MCP Tool Parameters — No Path Sanitization

**Files:** `src/mcp/tools.rs`

MCP tools accept user-supplied file paths (`FileParam.path`, `BlastRadiusParam.path`, `SubgraphParam.paths`, `DependenciesParam.path`) and pass them directly to `CanonicalPath::new()` for graph lookup. These paths are used only for in-memory graph lookups (not filesystem access), so the risk is limited to information disclosure of graph contents. The `CanonicalPath` normalization (line 17-33 of `src/model/types.rs`) strips `..` sequences, which prevents path traversal in the normalized form.

### 2b. MCP views_export — Path Traversal via Cluster Name

**File:** `src/mcp/tools.rs`, lines 651-688

The `ariadne_views_export` tool reads files from disk using the `cluster` parameter:
```
let path = views_dir.join(format!("{}.md", cluster));
std::fs::read_to_string(&path)
```
The `cluster` parameter is user-supplied via MCP and is not sanitized before being used in a filesystem path. A value like `../../etc/passwd` (without `.md` extension) or `../../../etc/shadow.md` could read arbitrary `.md` files outside the views directory. Mitigation: The `std::fs::read_to_string` call would only succeed if the resulting path exists, and the path always gets `.md` appended.

### 2c. Views Generation — Filename Sanitization

**File:** `src/views/mod.rs`, lines 53-55

The `sanitize_filename` function only replaces `/` and `\` with `_`. It does not sanitize other characters that may be problematic on certain filesystems (e.g., `..`, null bytes, very long names). Cluster names are derived from directory paths in the analyzed project, so user control is indirect.

### 2d. CLI Path Inputs — Canonicalization Fallthrough

**File:** `src/main.rs`, line 323

```rust
let abs_project = std::fs::canonicalize(&project).unwrap_or(project);
```
If `canonicalize` fails (e.g., path doesn't exist), the original user-supplied path is used without validation. The pipeline's `run_with_output` subsequently calls `canonicalize` again and returns `FatalError::ProjectNotFound` on failure, so this is handled downstream.

### 2e. No BFS Depth Bounds on Some MCP Tools

**File:** `src/mcp/tools.rs`

- `ariadne_blast_radius`: `depth` is `Option<u32>` — when `None`, unbounded BFS is performed. On large graphs this could be a resource exhaustion vector via MCP.
- `ariadne_subgraph`: `depth` defaults to `2` (line 262), which is bounded.

---

## 3. Unsafe Code

### 3a. Single `unsafe` Block — FFI Call to `libc::kill`

**File:** `src/mcp/lock.rs`, line 112

```rust
let ret = unsafe { libc::kill(pid as i32, 0) };
```

This calls `kill(2)` with signal 0 to check process liveness. The libc binding is defined locally (lines 142-146) as a minimal extern "C" block. The `pid` is cast from `u32` to `i32`, which can overflow for PIDs > `i32::MAX` (2,147,483,647). On all mainstream operating systems, PIDs are well below this limit, so this is a theoretical concern only.

There is no `#![forbid(unsafe_code)]` or `#![deny(unsafe_code)]` attribute at the crate level.

---

## 4. Unwrap/Expect/Panic Paths

### 4a. Production `unwrap()` Calls (Non-Test Code)

| File | Line | Context |
|------|------|---------|
| `src/mcp/lock.rs` | 96 | `serde_json::to_string_pretty(&content).unwrap()` — serializing a known-good struct; panic only on serialization bugs |
| `src/algo/scc.rs` | 66 | `stack.pop().unwrap()` — Tarjan's algorithm invariant guarantees non-empty stack |
| `src/algo/centrality.rs` | 57, 74, 78 | `get_mut(w).unwrap()` / `get_mut(v).unwrap()` — Brandes' algorithm invariant guarantees keys exist |
| `src/algo/topo_sort.rs` | 95 | `out_degree.get_mut(&pred).unwrap()` — algorithm invariant |
| `src/views/cluster.rs` | 13-169 (many) | `writeln!(out, ...).unwrap()` — writing to `String` buffer, which cannot fail |
| `src/views/impact.rs` | 13-93 (many) | `writeln!(out, ...).unwrap()` — writing to `String` buffer, which cannot fail |
| `src/views/index.rs` | 9-97 (many) | `writeln!(out, ...).unwrap()` — writing to `String` buffer, which cannot fail |

### 4b. Production `expect()` Calls (Non-Test Code)

No `expect()` calls found in non-test production code.

### 4c. No `panic!`, `todo!`, `unimplemented!`, or `unreachable!` Macros

No instances found in production code.

---

## 5. Sensitive Data Handling

### 5a. File Paths in Error Messages

**Files:** `src/diagnostic.rs`, `src/mcp/tools.rs`

Error messages and MCP tool responses include full file paths from the analyzed project. This is by design (the tool's purpose is code analysis), but error messages propagated via MCP could reveal directory structure to MCP clients.

Examples:
- `FatalError::ProjectNotFound` includes the full path (line 12)
- `FatalError::OutputNotWritable` includes path and OS error reason (line 16)
- `FatalError::GraphCorrupted` includes path and parse error detail (line 26)
- MCP tool `ariadne_file` returns file path in "not_found" error response with freshness info (tools.rs lines 193-199)

### 5b. Verbose Logging to stderr

**Files:** `src/pipeline/mod.rs`, `src/mcp/server.rs`, `src/mcp/watch.rs`

When verbose mode is enabled, timing information and file counts are printed to stderr. The MCP server always prints to stderr (lines 39-41, 80, 88, 113, 125-129 of server.rs). This includes:
- Output directory paths
- File/edge counts
- Error details from failed rebuilds

### 5c. Content Hashes Exposed via MCP

**File:** `src/mcp/tools.rs`, line 224

The `ariadne_file` tool returns the content hash (`node.hash.as_str()`) for each file. xxHash64 is not cryptographic; the hash alone does not leak file contents but confirms whether specific content exists.

---

## 6. Concurrency Safety

### 6a. Lock File Race Condition

**File:** `src/mcp/lock.rs`, lines 35-49

The lock acquisition (`acquire_lock`) performs a check-then-act sequence: `check_lock` reads the file, then `write_lock` writes it. Between these two operations, another process could acquire the lock (TOCTOU race). This is a standard limitation of file-based locks without `flock`/`fcntl` advisory locking.

### 6b. Mutex Poisoning Recovery

**File:** `src/diagnostic.rs`, lines 263, 319, 325

The `DiagnosticCollector` uses `unwrap_or_else(|e| e.into_inner())` on `Mutex::lock()`, which recovers from poisoned mutexes. This is a deliberate design choice to avoid panics propagating across threads.

---

## 7. Dependency Surface

**File:** `Cargo.toml`

Key dependencies with security relevance:
- `tree-sitter` 0.24 + language grammars 0.23 — C-based parsers invoked via FFI (tree-sitter generates C code). Not auditable from Rust source alone.
- `rmcp` 1.2 — MCP protocol implementation (stdio transport, not network).
- `notify` 8 — filesystem watching (platform-native APIs).
- `ignore` 0.4 — respects `.gitignore` patterns for file walking.
- `serde_json` 1 — JSON deserialization of untrusted graph files.
- `tokio` 1 — async runtime for MCP server.

The MCP server communicates via stdio only (`rmcp::transport::io::stdio()` at `src/mcp/server.rs` line 131), not TCP/HTTP. The attack surface is limited to whoever controls the stdio pipe.

---

## 8. File System Operations Summary

All filesystem write operations are confined to:
1. The `.ariadne/graph/` output directory (graph.json, clusters.json, stats.json, raw_imports.json)
2. The `.ariadne/views/` directory (markdown views)
3. The `.ariadne/graph/.lock` lock file
4. Case-sensitivity probe file at `.ariadne/.case_probe` (`src/detect/case_sensitivity.rs` line 17)

Filesystem reads occur on:
1. The user-specified project root (recursive walk + file read)
2. The output directory (loading existing graph data)
3. The views directory (MCP views_export tool)

Atomic writes use temp files with PID suffix, then `fs::rename` (`src/serial/json.rs` lines 116-147).
