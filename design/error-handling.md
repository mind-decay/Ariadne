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

| Error | Cause | Message |
|-------|-------|---------|
| `E001: ProjectNotFound` | Project root path doesn't exist | `error: project root not found: {path}` |
| `E002: NotADirectory` | Project root is a file, not directory | `error: not a directory: {path}` |
| `E003: OutputNotWritable` | Can't create or write to output directory | `error: cannot write to output directory: {path}: {reason}` |
| `E004: NoParseableFiles` | Walk found zero files with recognized extensions | `error: no parseable files found in {path} (supported: .ts, .js, .go, .py, .rs, .cs, .java)` |
| `E005: WalkFailed` | Directory walk failed completely (permissions on root) | `error: cannot read project directory: {path}: {reason}` |

### Recoverable Errors (exit code 0, file skipped, warning emitted)

These affect individual files. The file is excluded from the graph. Build continues.

| Warning | Cause | Handling |
|---------|-------|----------|
| `W001: ParseFailed` | Tree-sitter can't parse the file at all | Skip file, emit warning |
| `W002: ReadFailed` | File can't be read (permissions, encoding) | Skip file, emit warning |
| `W003: FileTooLarge` | File exceeds size limit (default: 1MB) | Skip file, emit warning |
| `W004: BinaryFile` | File contains null bytes (binary, not source) | Skip file, emit warning |
| `W005: SymlinkLoop` | Symlink resolves to ancestor directory | Skip path, emit warning |
| `W006: ImportUnresolved` | Import path can't be resolved to a project file | No edge created, emit warning (only in verbose mode — too noisy otherwise) |
| `W007: PartialParse` | Tree-sitter parsed with ERROR nodes (>50% of top-level nodes → W001; otherwise extract valid subtrees) | Extract what we can, emit warning |
| `W008: ConfigParseFailed` | Language config file (go.mod, tsconfig.json) can't be parsed | Fall back to heuristic resolution, emit warning |
| `W009: EncodingError` | File is not valid UTF-8 | Skip file, emit warning |

### Design Decisions

**Partial parse (W007):** Tree-sitter is error-tolerant — it always produces a tree, even for broken syntax. Some nodes will be `ERROR` type. Decision: **extract imports from valid subtrees, skip ERROR subtrees.** This is better than skipping the entire file. Threshold: if >50% of top-level nodes are ERROR, treat as full parse failure (W001).

**Unresolved imports (W006):** Most unresolved imports are external packages (npm, pip, go modules). These are expected and not errors. Decision: **only warn in verbose mode (`--verbose` flag).** In normal mode, unresolved imports are silently skipped — they're the common case. Summary line at the end: `"N imports unresolved (external packages)"`.

**Binary files (W004):** The `ignore` crate skips most binary files via .gitignore, but some may slip through. Decision: **check for null bytes in the first 8KB.** If found, skip as binary.

## Warning Output Format

Warnings go to stderr. Two formats:

**Human format (default):**
```
warn[W001]: failed to parse src/legacy/old-code.ts: unexpected token at line 42
warn[W002]: cannot read src/secrets/.env: permission denied
warn[W003]: skipping src/generated/huge-bundle.js: file too large (4.2MB, limit 1MB)
warn[W009]: skipping data/binary.dat: not valid UTF-8

Built graph: 847 files, 2341 edges, 12 clusters in 1.2s
  3 files skipped (1 parse error, 1 permission denied, 1 too large)
  42 imports unresolved (external packages)
```

**JSON format (`--warnings json`):**
```json
{"level":"warn","code":"W001","file":"src/legacy/old-code.ts","message":"parse failed","detail":"unexpected token at line 42"}
{"level":"warn","code":"W002","file":"src/secrets/.env","message":"read failed","detail":"permission denied"}
```

JSON format enables machine consumption — CI tools, editors, integration systems can parse warnings programmatically.

## Resource Limits

| Resource | Default Limit | Flag | Rationale |
|----------|--------------|------|-----------|
| Max file size | 1MB | `--max-file-size <bytes>` | Generated/bundled files shouldn't be in the graph |
| Max files | 50,000 | `--max-files <count>` | Memory protection. 50k files × ~500 bytes/node = ~25MB |
| Max depth | 64 directories | (not configurable) | Prevents symlink loops and pathological nesting |
| Parse timeout | None (tree-sitter is fast) | — | Tree-sitter parses ~10MB/s. 1MB limit makes timeout unnecessary |

**Memory estimation:** Each Node is ~500 bytes (path + metadata). Each Edge is ~200 bytes. A 50k file project with 150k edges ≈ 55MB. Comfortable for any modern machine.

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

### Stage 1: File Walking

```
walk(project_root):
  IF !exists(project_root) → E001
  IF !is_dir(project_root) → E002

  for each entry from ignore::Walk:
    IF entry.is_error:
      IF is_permission_error → W002, skip
      IF is_loop → W005, skip
      ELSE → W002, skip
    IF entry.is_dir → continue
    IF entry.is_symlink AND !follow_symlinks → skip
    IF file_count >= max_files → warn, stop walk

    read file bytes:
      IF read error → W002, skip
      IF size > max_file_size → W003, skip
      IF contains null bytes in first 8KB → W004, skip
      IF not valid UTF-8 → W009, skip

    yield (path, content)
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
resolve(import, file, root, parser):
  resolved = parser.resolve_import_path(import, file, root)

  IF resolved.is_none():
    // Unresolved — likely external package
    increment unresolved_count
    IF verbose → W006
    return None

  IF !resolved.exists_in_graph():
    // Resolves to a path that wasn't walked (outside project, gitignored)
    increment unresolved_count
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

### Stage 5: Output Writing

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
  3 files skipped (1 parse error, 1 permission denied, 1 too large)
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

| Test case | Errors exercised |
|-----------|-----------------|
| `syntax-error.ts` | W001 (parse failed) or W007 (partial parse) |
| `empty-file.ts` | No error — valid file with 0 imports |
| `circular-a.ts` ↔ `circular-b.ts` | No error — circular imports are valid edges |
| `deeply-nested/a/b/c/d/e/f.ts` | No error unless depth > 64 |
| `unicode-path/файл.ts` | No error — UTF-8 filename is valid |

**Additional test cases to add:**

| Test case | Errors exercised |
|-----------|-----------------|
| `binary-file.png` (with .ts extension) | W004 (binary file detected) |
| `huge-file.ts` (>1MB) | W003 (file too large) |
| `no-permission.ts` (chmod 000) | W002 (read failed) — platform-dependent, may skip in CI |
| `non-utf8.ts` (Latin-1 encoded) | W009 (encoding error) |
| `partial-error.ts` (valid imports + broken syntax) | W007 (partial parse — imports still extracted) |

These should be added to the `edge-cases/` fixture in the implementation plan.
