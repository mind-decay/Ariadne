# Security Surface Scan

**Date:** 2026-04-04
**Agent:** Hermes (explorer)
**Scope:** Full source tree at `/Users/minddecay/Documents/Projects/Ariadne/src/`
**Files examined:** ~50 source files across all modules

---

## 1. Hardcoded Secrets

**No hardcoded secrets found.** Searched all source files for patterns matching `password`, `secret`, `token`, `api_key`, `PRIVATE_KEY` (case-insensitive). All matches are semantic uses (e.g., `CancellationToken`, `token_estimate`, `"unexpected token at line 42"` in test fixtures). No API keys, credentials, or connection strings are present in source code.

No `.env` files or `credentials.*` files exist in the repository.

---

## 2. Unsafe Blocks

Two `unsafe` blocks exist, both in `src/mcp/lock.rs`:

- **Line 133:** `unsafe { libc::kill(pid as i32, 0) }` -- used in `is_process_alive()` to check if a process exists via `kill(pid, 0)`.
- **Line 173:** `unsafe { libc::kill(pid as i32, 15) }` -- used in `terminate_process()` to send SIGTERM.

Both call into a minimal hand-rolled libc binding defined at lines 194-198:
```rust
mod libc {
    extern "C" {
        pub fn kill(pid: i32, sig: i32) -> i32;
    }
}
```

**Observations:**
- The `terminate_process` function (line 164) guards against dangerous PID values: rejects `pid <= 1` and `pid > i32::MAX as u32`.
- The `is_process_alive` function (line 128) does **not** have the same PID guard. A PID of 0 would check the entire process group; a PID of 1 would check init. The `pid` parameter is `u32`, so negative values are not possible.
- No `#![forbid(unsafe_code)]` or `#![deny(unsafe_code)]` attribute is set at crate level.

---

## 3. External Process Execution

All external process execution is confined to the `src/temporal/` module and invokes only `git`:

- `src/temporal/git.rs` lines 37, 48, 66, 89: `Command::new("git")` with fixed argument lists (`--version`, `-C`, `rev-parse`, `log`, `--numstat`, `-M`, `--format=...`, `--since=1 year ago`).
- `src/temporal/mod.rs` line 121: `Command::new("git")` with `rev-parse --is-shallow-repository`.

**Observations:**
- The `project_root` path is passed via `-C` flag using `.to_string_lossy()`, not interpolated into a shell string. This uses `Command` (direct exec, no shell), so shell injection is not possible.
- No user-controlled strings are interpolated into command arguments. The `--format` and `--since` values are compile-time constants.

---

## 4. Path Traversal

### 4a. CanonicalPath normalization (`src/model/types.rs`)

`CanonicalPath::new()` normalizes `..` segments by clamping at root. Tests at lines 250-270 confirm:
- `"../../../etc/passwd"` becomes `"etc/passwd"` (leading `..` stripped)
- `"src/../../escape.ts"` becomes `"escape.ts"`

This means `CanonicalPath` treats paths as project-relative and prevents traversal above project root.

### 4b. Views filename sanitization (`src/views/mod.rs` line 53)

`sanitize_filename()` replaces `/` and `\` with `_`. Test at line 87 confirms `"../etc/passwd"` becomes `".._etc_passwd"`. This prevents directory traversal in generated view filenames.

### 4c. MCP `views_export` tool (`src/mcp/tools.rs` line 1940)

The `cluster` parameter from MCP clients is used directly in a `format!("{}.md", cluster)` joined to the clusters directory path. The `cluster` value comes from untrusted MCP client input. A crafted cluster name like `"../../etc/passwd"` would resolve to a path outside the views directory. The `std::fs::read_to_string` call would then read arbitrary files. However, this reads file content and returns it as a string response -- it does not write.

### 4d. tsconfig extends resolution (`src/parser/config/tsconfig.rs` line 211)

`resolve_extends_path()` joins `config_dir` with the user-provided `extends` string from `tsconfig.json`. The path is then passed to `std::fs::canonicalize()` and `std::fs::read_to_string()`. This reads files from disk based on content found in `tsconfig.json` files within the project. The `extends` value could reference paths outside the project root (e.g., `"extends": "../../../etc/some-file"`). However, the input comes from files already within the scanned project directory, not from external user input.

### 4e. Pipeline root canonicalization (`src/pipeline/mod.rs` line 148, `src/main.rs` line 444)

The CLI `build` command takes a user-provided path and calls `std::fs::canonicalize()` on it, resolving symlinks to absolute paths. This is standard behavior for CLI tools.

---

## 5. Input Validation

### 5a. CLI input (via clap)

CLI argument parsing uses `clap` with derive macros (`src/main.rs`). Type constraints are enforced at the framework level:
- `max_file_size: u64` with default `1_048_576`
- `max_files: usize` with default `50_000`
- `debounce: u64` with default `2000`

No custom validation logic beyond clap's type parsing.

### 5b. MCP tool parameters

MCP tool parameters are deserialized via `serde` + `schemars` (JSON Schema). Parameter types are defined as typed structs (e.g., `FileParam`, `BlastRadiusParam`). String fields like `path`, `direction`, `level` are validated at the tool implementation level with match statements returning error JSON for invalid values.

The `direction` parameter in `DependenciesParam` (line 183) is a free `String` -- validation happens at match time in the tool handler.

### 5c. File size limits

`src/pipeline/read.rs` line 64: Files exceeding `max_file_size` are rejected with `FileSkipReason::TooLarge`.

### 5d. File count limits

`src/pipeline/walk.rs` line 175: Walking stops at `max_files` with a `W005MaxFilesReached` warning.

### 5e. Directory depth limit

`src/pipeline/walk.rs` line 69: Hardcoded `MAX_DEPTH: usize = 64` for directory traversal.

### 5f. tsconfig extends depth limit

`src/parser/config/tsconfig.rs` line 117: `MAX_EXTENDS_DEPTH: u32 = 10` with circular reference detection via `HashSet<PathBuf>`.

---

## 6. File I/O Patterns

### 6a. Atomic writes

`src/serial/json.rs` line 152: All graph output uses atomic write (write to `{filename}.{pid}.tmp`, then `fs::rename`). Temp file is cleaned up on error.

`src/mcp/persist.rs` line 74: `JsonStore` also uses atomic writes (`.tmp` suffix + rename).

### 6b. Lock file

`src/mcp/lock.rs`: File-based lock using PID + timestamp JSON. Stale lock detection via `kill(pid, 0)`. Lock guard uses RAII (`Drop` impl removes file). Race condition window exists between `check_lock()` and `write_lock()` -- no `O_EXCL`/`O_CREAT` atomic creation.

### 6c. Hidden files included in walk

`src/pipeline/walk.rs` line 102: `walker.hidden(false)` -- the `ignore` crate's `hidden(false)` means "do not skip hidden files." Hidden files (including `.env` if present) will be walked and potentially parsed if they have a recognized extension.

---

## 7. Symlinks

No explicit symlink handling found in the codebase. The `ignore` crate's `WalkBuilder` follows symlinks by default. `std::fs::canonicalize()` resolves symlinks. No symlink loop detection is implemented beyond what the `ignore` crate provides internally.

---

## 8. Error Messages and Logging

- Error messages in `src/diagnostic.rs` include file paths in all `FatalError` variants (e.g., `ProjectNotFound { path }`, `GraphCorrupted { path, reason }`).
- `eprintln!` is used for server status messages in `src/mcp/server.rs` (lines 43, 84, 97-99, 168-172, 240, 244).
- Git command stderr is captured and included in warning detail (`src/temporal/git.rs` line 111): `detail: Some(stderr.trim().to_string())`.
- No PII or credentials are logged. Path information in errors reflects project structure only.

---

## 9. Dependency Surface

From `Cargo.toml`:
- **tree-sitter** (0.24) + language grammars: C-based parsers compiled via cc. These contain native code.
- **rayon** (1): Thread pool for parallel parsing.
- **ignore** (0.4): File walking with gitignore support.
- **rmcp** (1.2, optional): MCP protocol implementation over stdio transport.
- **notify** (8, optional): Filesystem event watching.
- **tokio** (1, optional): Async runtime for MCP server.

No network-fetching dependencies (no HTTP client crates). The MCP server uses stdio transport only (`rmcp::transport::io::stdio()` at `src/mcp/server.rs` line 174), not TCP/HTTP.

---

## 10. Summary of Observed Concerns

| # | Area | File | Observation |
|---|------|------|-------------|
| 1 | Path traversal via MCP | `src/mcp/tools.rs:1940` | `cluster` parameter from MCP client used in `format!("{}.md", cluster)` joined to views directory; can read files outside views dir |
| 2 | PID guard inconsistency | `src/mcp/lock.rs:128-140` | `is_process_alive()` lacks the PID <= 1 guard that `terminate_process()` has |
| 3 | Lock file race condition | `src/mcp/lock.rs:56-70` | TOCTOU gap between `check_lock()` and `write_lock()` -- no atomic create |
| 4 | Hidden files walked | `src/pipeline/walk.rs:102` | `walker.hidden(false)` includes hidden files; `.env` files with recognized extensions would be parsed |
| 5 | No `#![forbid(unsafe_code)]` | crate root | Two `unsafe` blocks exist but no crate-level policy is declared |
| 6 | `unwrap()` in non-test code | `src/views/*.rs`, `src/serial/convert.rs:181` | Multiple `unwrap()` calls on `writeln!` and `try_into()` -- panics on write failure |
