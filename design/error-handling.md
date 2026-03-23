# Error Handling & Fault Tolerance

## Principles

1. **Never crash on valid project input.** A project with some broken files is still a valid project. Ariadne must produce the best graph it can.
2. **Every skipped file is reported.** Silent data loss is a bug. The user must know what was excluded and why.
3. **Errors are structured, not just text.** Machine-parseable warnings enable integration with other tools.
4. **Fail fast on infrastructure errors.** If the project root doesn't exist or output can't be written — stop immediately. Don't build a graph that can't be saved.
5. **Resource limits are explicit.** No unbounded memory allocation. No unbounded file reads. Limits are configurable.

## Error Taxonomy

### Fatal Errors (exit code 1, immediate stop)

These prevent any useful work. The binary exits with code 1 and a clear error message.

| Error                     | Cause                                                  | Message                                                                                      |
| ------------------------- | ------------------------------------------------------ | -------------------------------------------------------------------------------------------- |
| `E001: ProjectNotFound`   | Project root path doesn't exist                        | `error: project root not found: {path}`                                                      |
| `E002: NotADirectory`     | Project root is a file, not directory                  | `error: not a directory: {path}`                                                             |
| `E003: OutputNotWritable` | Can't create or write to output directory              | `error: cannot write to output directory: {path}: {reason}`                                  |
| `E004: NoParseableFiles`  | Walk found zero files with recognized extensions       | `error: no parseable files found in {path} (supported: .ts, .js, .go, .py, .rs, .cs, .java, .md, .json, .yaml, .yml)` |
| `E005: WalkFailed`        | Directory walk failed completely (permissions on root) | `error: cannot read project directory: {path}: {reason}`                                     |
| `E006: GraphNotFound`     | `ariadne query` or `ariadne update` when graph.json doesn't exist (Phase 2) | `error: graph not found in {path}. Run 'ariadne build' first.`                              |
| `E007: StatsNotFound`     | `ariadne query stats/layers/cycles` when stats.json doesn't exist (Phase 2) | `error: stats not found in {path}. Run 'ariadne build' first.`                              |
| `E008: GraphCorrupted`    | graph.json or stats.json exists but can't be parsed during query commands (Phase 2) | `error: corrupted file {path}: {reason}`                                                    |
| `E009: FileNotInGraph`    | `ariadne query file` when the specified file is not in the graph (Phase 2) | `error: file not found in graph: {path}`                                                     |
| `E010: McpServerFailed` | MCP server failed to start | Exit 1 |
| `E011: LockFileHeld` | Another ariadne server is running (lock held) | Exit 1 |
| `E012: McpProtocolError` | MCP protocol-level error during serving | Exit 1 |
| `E013: InvalidArgument` | Invalid CLI argument value | Exit 1 |
| `E014: ClustersNotFound` | `ariadne query cluster` or `ariadne update` when clusters.json doesn't exist | `error: clusters not found in {path}. Run 'ariadne build' first.` |

### Recoverable Errors (exit code 0, file skipped, warning emitted)

These affect individual files. The file is excluded from the graph. Build continues.

| Warning                   | Cause                                                                                                  | Handling                                                                                                                                                                            |
| ------------------------- | ------------------------------------------------------------------------------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `W001: ParseFailed`       | Tree-sitter can't parse the file at all                                                                | Skip file, emit warning                                                                                                                                                             |
| `W002: ReadFailed`        | File can't be read (permissions, encoding)                                                             | Skip file, emit warning                                                                                                                                                             |
| `W003: FileTooLarge`      | File exceeds size limit (default: 1MB)                                                                 | Skip file, emit warning                                                                                                                                                             |
| `W004: BinaryFile`        | File contains null bytes (binary, not source)                                                          | Skip file, emit warning                                                                                                                                                             |
| `W005: MaxFilesReached`   | Walk hit the `--max-files` limit; graph is partial                                                     | Emit warning, stop walk, build graph from files collected so far                                                                                                                     |
| `W006: ImportUnresolved`  | Import path can't be resolved to a project file                                                        | No edge created, emit warning (only in verbose mode — too noisy otherwise)                                                                                                          |
| `W007: PartialParse`      | Tree-sitter parsed with ERROR nodes (>50% of top-level nodes → W001; otherwise extract valid subtrees) | Extract what we can, emit warning                                                                                                                                                   |
| `W008: ConfigParseFailed` | Language config file (go.mod, tsconfig.json) can't be parsed                                           | Fall back to heuristic resolution, emit warning                                                                                                                                     |
| `W009: EncodingError`     | File is not valid UTF-8                                                                                | Skip file, emit warning                                                                                                                                                             |
| `W010: GraphVersionMismatch` | graph.json `version` field doesn't match current code (Phase 2)                                     | Fall back to full rebuild, emit warning                                                                                                                                             |
| `W011: GraphCorrupted`    | graph.json exists but can't be parsed (Phase 2)                                                        | Fall back to full rebuild, emit warning                                                                                                                                             |
| `W012: AlgorithmFailed`   | Algorithm failed (e.g., Louvain didn't converge) (Phase 2)                                             | Skip that output, continue. Fall back to directory clusters if Louvain fails or reduces clusters below 50% of directory count (D-073)                                               |
| `W013: StaleStats`        | stats.json modification time older than graph.json (Phase 2)                                           | Recompute stats, emit warning                                                                                                                                                       |
| `W014: FsWatcherFailed` | File system watcher failed to start | Fall back to 30s polling |
| `W015: IncrementalRebuildFailed` | Auto-rebuild during MCP serving failed | Serve stale data with freshness warning |
| `W016: StaleLockRemoved` | Removed stale lock file from dead process | Continue normally |
| `W017: SmellDetectionSkipped` | Smell detection skipped due to missing data | Return partial results |
| `W018: BlastRadiusTimeout` | Blast radius computation exceeded limit | Skip file in shotgun surgery detection |

### Design Decisions

**Partial parse (W007):** Tree-sitter is error-tolerant — it always produces a tree, even for broken syntax. Some nodes will be `ERROR` type. Decision: **extract imports from valid subtrees, skip ERROR subtrees.** This is better than skipping the entire file. Threshold: if >50% of top-level nodes are ERROR, treat as full parse failure (W001).

**Unresolved imports (W006):** Most unresolved imports are external packages (npm, pip, go modules). These are expected and not errors. Decision: **only warn in verbose mode (`--verbose` flag).** In normal mode, unresolved imports are silently skipped — they're the common case. Summary line at the end: `"N imports unresolved (external packages)"`.

**Binary files (W004):** The `ignore` crate skips most binary files via .gitignore, but some may slip through. Decision: **check for null bytes in the first 8KB.** If found, skip as binary.

## Implementation Architecture (D-021)

### Fatal Errors — `FatalError` enum via `thiserror`

Fatal errors stop the pipeline and return via `Result`. Defined as a `thiserror` enum for ergonomic `?` operator use and pattern matching in tests:

```rust
#[derive(Debug, thiserror::Error)]
pub enum FatalError {
    #[error("E001: project root not found: {path}")]
    ProjectNotFound { path: PathBuf },
    #[error("E002: not a directory: {path}")]
    NotADirectory { path: PathBuf },
    #[error("E003: cannot write to output directory: {path}: {reason}")]
    OutputNotWritable { path: PathBuf, reason: String },
    #[error("E004: no parseable files found in {path}")]
    NoParseableFiles { path: PathBuf },
    #[error("E005: cannot read project directory: {path}: {reason}")]
    WalkFailed { path: PathBuf, reason: String },
    #[error("E006: graph not found in {path}. Run 'ariadne build' first.")]
    GraphNotFound { path: PathBuf },
    #[error("E007: stats not found in {path}. Run 'ariadne build' first.")]
    StatsNotFound { path: PathBuf },
    #[error("E008: corrupted file {path}: {reason}")]
    GraphCorrupted { path: PathBuf, reason: String },
    #[error("E009: file not found in graph: {path}")]
    FileNotInGraph { path: String },
    #[error("E010: failed to start MCP server: {reason}")]
    McpServerFailed { reason: String },
    #[error("E011: another ariadne server is running (PID {pid})")]
    LockFileHeld { pid: u32, lock_path: PathBuf },
    #[error("E012: MCP protocol error: {reason}")]
    McpProtocolError { reason: String },
    #[error("E013: invalid argument: {reason}")]
    InvalidArgument { reason: String },
    #[error("E014: clusters not found in {path}. Run 'ariadne build' first.")]
    ClustersNotFound { path: PathBuf },
}
```

### Recoverable Warnings — `DiagnosticCollector`

Warnings are collected during parallel processing (rayon) and reported after all stages complete. Direct stderr writes from parallel workers would produce non-deterministic, interleaved output.

```rust
pub struct Warning {
    pub code: WarningCode,         // W001-W018 enum
    pub path: CanonicalPath,       // affected file
    pub message: String,           // human-readable description
    pub detail: Option<String>,    // additional context
}

pub struct DiagnosticCollector {
    inner: Mutex<(Vec<Warning>, DiagnosticCounts)>,
}
```

A single `Mutex` wrapping both the warning list and counts reduces lock acquisitions — one lock per `warn()` call instead of two. The `warn()` method updates the appropriate `DiagnosticCounts` fields (incrementing `files_skipped` plus the specific reason counter) and pushes the `Warning` in a single critical section.

```rust
pub struct DiagnosticCounts {
    pub files_skipped: u32,
    pub parse_errors: u32,
    pub read_errors: u32,
    pub too_large: u32,
    pub binary_files: u32,
    pub encoding_errors: u32,
    pub imports_unresolved: u32,
    pub partial_parses: u32,
    pub graph_load_warnings: u32,   // W010, W011 (Phase 2)
    pub algorithm_failures: u32,    // W012 (Phase 2)
    pub stale_stats: u32,           // W013 (Phase 2)
}
```

**Thread safety:** `Mutex<(Vec<Warning>, DiagnosticCounts)>` is shared across rayon workers via `&DiagnosticCollector`. Lock contention is minimal — warnings are rare (most files parse successfully), and lock hold time is short (one `push` + counter increment).

**Deterministic output:** `drain()` sorts warnings by `(path, code)` before reporting. This guarantees identical warning order across runs regardless of rayon scheduling (D-006).

**Reporting is separate from collection:** `DiagnosticCollector` only collects. Formatting (human/JSON) is handled by a separate module that consumes the sorted `DiagnosticReport`.

### Dependency Choice

`thiserror` for `FatalError` — derive macro for `Display` and `Error`, compile-time checked, works with `?`. No `anyhow` — concrete error types throughout enable pattern matching in tests and explicit error handling.

## Warning Output Format

Warnings go to stderr. Two formats:

**Human format (default):**

```
warn[W001]: failed to parse src/legacy/old-code.ts: unexpected token at line 42
warn[W002]: cannot read src/secrets/.env: permission denied
warn[W003]: skipping src/generated/huge-bundle.js: file too large (4.2MB, limit 1MB)
warn[W009]: skipping data/binary.dat: not valid UTF-8

Built graph: 847 files, 2341 edges, 12 clusters in 1.2s
  3 files skipped (1 parse error, 1 read error, 1 too large)
  42 imports unresolved (external packages)
```

**JSON format (`--warnings json`):**

```json
{"level":"warn","code":"W001","file":"src/legacy/old-code.ts","message":"parse failed","detail":"unexpected token at line 42"}
{"level":"warn","code":"W002","file":"src/secrets/.env","message":"read failed","detail":"permission denied"}
```

JSON format enables machine consumption — CI tools, editors, integration systems can parse warnings programmatically.

## Resource Limits

| Resource      | Default Limit              | Flag                      | Rationale                                                                                          |
| ------------- | -------------------------- | ------------------------- | -------------------------------------------------------------------------------------------------- |
| Max file size | 1MB                        | `--max-file-size <bytes>` | Generated/bundled files shouldn't be in the graph                                                  |
| Max files     | 50,000                     | `--max-files <count>`     | Memory protection. 50k files with edges and overhead ≈ 250MB (see performance.md Memory Estimates) |
| Max depth     | 64 directories             | (not configurable)        | Prevents symlink loops and pathological nesting                                                    |
| Parse timeout | None (tree-sitter is fast) | —                         | Tree-sitter parses ~10MB/s. 1MB limit makes timeout unnecessary                                    |

**Memory estimation:** A 50k file project with edges, path interning, and all per-file overhead ≈ 250MB (see performance.md Memory Estimates). Comfortable for any modern machine.

**If max-files exceeded:** Emit a single warning and stop file collection. Build graph from files collected so far. This is partial but useful.

```
warn: file limit reached (50000). Graph is partial. Use --max-files to increase.
```

## File System Edge Cases

### Symlinks

- `ignore` crate does NOT follow symlinks by default. This is the correct behavior.
- If `--follow-symlinks` is added in the future, symlink loop detection is required (track visited inodes).
- For now: symlinks are skipped. Files reachable only through symlinks are not in the graph.

### Non-UTF-8 Filenames

- Rust's `Path` handles non-UTF-8 via `OsStr`. File walking works fine.
- For graph.json serialization: non-UTF-8 paths are lossy-converted (`to_string_lossy()`).
- Warning W009 is for file contents, not filenames.

### Concurrent Modification

- Files may change during a build (IDE auto-save, git operations).
- Decision: **no locking, no consistency guarantees.** Ariadne reads each file once. If a file changes between being walked and being parsed, the result may be inconsistent. This is acceptable — rerun `ariadne build` for a consistent snapshot.
- Content hashes capture the state at read time. Delta updates (Phase 2) handle this correctly.

### Empty Files

- An empty source file is valid. It produces a Node with 0 lines, empty exports, no outgoing edges.
- It may still have incoming edges (other files import from it — they'll fail resolution, but the node exists).

### Empty Directories

- Ignored. Directories don't produce nodes.

## Error Handling by Pipeline Stage

### Stage 1: File Walking and Reading

**Note:** Walking and reading are separate pipeline stages (D-026). Walking produces `Vec<FileEntry>` (paths only), reading produces `Vec<FileContent>` (with bytes). This separation enables independent error handling: walk-level errors (E001, E002, E005) are fatal, while read-level errors (W002, W003, W004, W009) are per-file and recoverable.

```
walk(project_root) → Vec<FileEntry>:
  IF !exists(project_root) → E001
  IF !is_dir(project_root) → E002

  for each entry from ignore::Walk:
    IF entry.is_error:
      IF is_permission_error → W002, skip
      ELSE → W002, skip
    IF entry.is_dir → continue
    IF entry.is_symlink AND !follow_symlinks → skip
    IF file_count >= max_files → warn, stop walk

    yield FileEntry(path, extension)

read(entries) → Vec<FileContent>:
  for each FileEntry:
    read file bytes:
      IF read error → W002, skip
      IF size > max_file_size → W003, skip
      IF contains null bytes in first 8KB → W004, skip
      IF not valid UTF-8 → W009, skip

    yield FileContent(path, bytes, hash, lines)
```

### Stage 2: Parsing

```
parse(path, content, parser):
  tree = tree_sitter_parse(content)

  IF tree.root_node().has_error():
    error_rate = count_error_nodes(tree) / count_top_level_nodes(tree)
    IF error_rate > 0.50 → W001, skip file entirely
    ELSE → W007, extract from valid subtrees only

  imports = parser.extract_imports(tree, content)
  exports = parser.extract_exports(tree, content)

  return (imports, exports)
```

### Stage 3: Path Resolution

```
resolve(import, from_file, known_files, resolver, diagnostics):
  resolved = resolver.resolve(import, from_file, known_files)

  IF resolved.is_none():
    // Unresolved — likely external package
    diagnostics.increment_unresolved()
    IF verbose → diagnostics.warn(W006)
    return None

  return Some(resolved)
```

### Stage 4: Config File Parsing

Some parsers need config files for path resolution:

- Go: `go.mod` for module path
- TypeScript: `tsconfig.json` for paths (deferred to future)
- Python: `pyproject.toml` for src layout
- Java: `pom.xml` / `build.gradle` for source roots

```
parse_config(config_path):
  IF !exists → fall back to heuristic, no warning (many projects don't have config)

  content = read(config_path)
  IF read error → W008, fall back to heuristic

  parsed = parse_format(content)  // JSON, TOML, XML
  IF parse error → W008, fall back to heuristic

  return parsed or default_config
```

**Heuristic fallbacks:**

- Go without go.mod: treat all `.go` files as same module, skip external imports
- Python without pyproject.toml: use directory structure for resolution
- Java without build config: try `src/main/java/` and `src/` as source roots

### Stage 5: Clustering

```
cluster(graph):
  IF clustering fails (malformed data, unexpected structure):
    log warning, output unclustered graph
    // Clustering failure is non-fatal — the graph is still valid
```

### Stage 6: Sorting

```
sort(graph):
  // Deterministic sorting should not fail under normal conditions.
  // If it does, treat as E005 (internal error) — indicates a bug.
  IF sort error → E005
```

### Stage 7: Output Writing

```
write_output(graph, clusters, output_dir):
  IF !can_create_dir(output_dir) → E003

  write graph.json:
    IF write error → E003 (disk full, permissions)

  write clusters.json:
    IF write error → E003

  // Both files written atomically — write to .tmp first, rename
  // This prevents partial writes if interrupted
```

**Atomic writes:** Write to `graph.json.tmp` then rename to `graph.json`. If the process is killed mid-write, the old `graph.json` remains intact. No corruption.

## Summary Report

Every build ends with a summary on stdout:

```
Built graph: 847 files, 2341 edges, 12 clusters in 1.2s
```

If warnings occurred:

```
Built graph: 844 files, 2298 edges, 12 clusters in 1.3s
  3 files skipped (1 parse error, 1 read error, 1 too large)
  42 imports unresolved (external packages)
```

If build was partial (max-files reached):

```
Built graph: 50000 files (PARTIAL — limit reached), 142301 edges, 87 clusters in 8.4s
  Use --max-files to increase the limit.
```

## CLI Flags for Error Control

```
ariadne build <path> [options]

Error control:
  --max-file-size <bytes>   Max file size to parse (default: 1048576 = 1MB)
  --max-files <count>       Max files to include (default: 50000)
  --verbose                 Show all warnings including unresolved imports
  --warnings <format>       Warning output format: human (default), json
  --strict                  Exit with code 1 if ANY warnings occurred
```

`--strict` mode is useful in CI: fail the build if the graph isn't perfect. Not default because most projects have some unresolvable imports.

## Testing Error Handling

Covered in `design/testing.md` via the `edge-cases/` fixture:

| Test case                         | Errors exercised                            |
| --------------------------------- | ------------------------------------------- |
| `syntax-error.ts`                 | W001 (parse failed) or W007 (partial parse) |
| `empty-file.ts`                   | No error — valid file with 0 imports        |
| `circular-a.ts` ↔ `circular-b.ts` | No error — circular imports are valid edges |
| `deeply-nested/a/b/c/d/e/f.ts`    | No error unless depth > 64                  |
| `unicode-path/файл.ts`            | No error — UTF-8 filename is valid          |

**Additional test cases to add:**

| Test case                                          | Errors exercised                                        |
| -------------------------------------------------- | ------------------------------------------------------- |
| `binary-file.png` (with .ts extension)             | W004 (binary file detected)                             |
| `huge-file.ts` (>1MB)                              | W003 (file too large)                                   |
| `no-permission.ts` (chmod 000)                     | W002 (read failed) — platform-dependent, may skip in CI |
| `non-utf8.ts` (Latin-1 encoded)                    | W009 (encoding error)                                   |
| `partial-error.ts` (valid imports + broken syntax) | W007 (partial parse — imports still extracted)          |

These should be added to the `edge-cases/` fixture in the implementation plan.
