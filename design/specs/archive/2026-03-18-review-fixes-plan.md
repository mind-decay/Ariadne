# Architecture Review Fixes — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix bugs, robustness gaps, and doc inconsistencies identified in `design/reports/2026-03-18-architecture-review.md`. Two targeted improvements are explicitly deferred: moving test naming patterns out of `build.rs` (S-2) and standardizing parser registration patterns (S-4) — both are refactors with no correctness impact, better suited for a dedicated cleanup pass.

**Architecture:** Targeted fixes across pipeline, parser, serial, and diagnostic modules. No structural refactors — each task is a localized change with tests. Doc updates are batched into one final task.

**Tech Stack:** Rust, tree-sitter, `ignore` crate, `thiserror`, `serde`

**Source of truth:** `design/reports/2026-03-18-architecture-review.md` — every task references the specific finding ID it addresses.

---

## File Map

| File | Changes |
|------|---------|
| `src/pipeline/walk.rs` | Fix override builder loop (F-2), emit W002 for walk errors |
| `src/parser/registry.rs` | Replace `expect()` with `Result` return (F-3) |
| `src/main.rs` | Wire `--output` flag to `run_with_output` (S-6) |
| `src/pipeline/read.rs` | Add null-byte scan for binary detection |
| `src/pipeline/resolve.rs` | Add self-import filter (U-5/INV-2) |
| `src/parser/traits.rs` | Add `ImportKind` enum to `RawImport` (F-4) |
| `src/parser/rust_lang.rs` | Use `ImportKind::ModDeclaration` instead of `mod::` sentinel (F-4) |
| `src/parser/typescript.rs` | Set `ImportKind::Regular` on all imports (F-4) |
| `src/parser/go.rs` | Set `ImportKind::Regular` on all imports (F-4) |
| `src/parser/python.rs` | Set `ImportKind::Regular` on all imports (F-4) |
| `src/parser/csharp.rs` | Set `ImportKind::Regular` on all imports (F-4) |
| `src/parser/java.rs` | Set `ImportKind::Regular` on all imports (F-4) |
| `src/pipeline/build.rs` | Add `test_*.py` prefix pattern (U-7), use `HashSet` for edge dedup check (U-8) |
| `src/serial/json.rs` | Clean up `.tmp` file on write failure (U-12) |
| `src/diagnostic.rs` | Merge two `Mutex` into one (U-6) |
| `src/pipeline/mod.rs` | Update `parse_source` call site for new `Result` return, update `DiagnosticCollector` usage |
| `tests/pipeline_tests.rs` | Tests for walk fixes, binary detection, self-import filtering |
| `design/architecture.md` | 6 doc fixes (arch_depth example, dependency table, DiagnosticCollector, resolver limitations, cluster naming, `models/` heuristic) |
| `design/error-handling.md` | Separate walk/read in Stage 1 pseudocode, document W005 gap |
| `design/performance.md` | Reconcile memory estimates |
| `design/decisions/log.md` | D-026: document walk/read separation |

---

### Task 1: Fix override builder loop bug + walk error warnings

**Findings:** F-2, Theme 3 (walk errors silently dropped)

**Files:**
- Modify: `src/pipeline/walk.rs:67-100`
- Test: `tests/pipeline_tests.rs`

- [ ] **Step 1: Write failing test for multiple exclude dirs**

Add to `tests/pipeline_tests.rs`:

```rust
/// F-2: Multiple exclude_dirs should all be excluded, not just the last one.
#[test]
fn walk_excludes_multiple_directories() {
    use ariadne_graph::pipeline::{FsWalker, FileWalker, WalkConfig};

    // Use the typescript-app fixture which has .ariadne/ from previous builds
    let path = helpers::fixture_path("typescript-app");
    let abs_path = std::fs::canonicalize(&path).unwrap();

    let walker = FsWalker::new();
    let config = WalkConfig {
        max_files: 50_000,
        max_file_size: 1_048_576,
        exclude_dirs: vec![".ariadne".to_string(), "node_modules".to_string()],
    };

    let entries = walker.walk(&abs_path, &config).unwrap();

    // No entry should be in .ariadne/ or node_modules/
    for entry in &entries {
        let rel = entry.path.strip_prefix(&abs_path).unwrap();
        let components: Vec<_> = rel.components()
            .map(|c| c.as_os_str().to_str().unwrap())
            .collect();
        assert!(
            !components.contains(&".ariadne"),
            "should exclude .ariadne: {:?}", entry.path
        );
        assert!(
            !components.contains(&"node_modules"),
            "should exclude node_modules: {:?}", entry.path
        );
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test walk_excludes_multiple_directories -- --nocapture`
Expected: FAIL — with current loop-replace bug, only last dir is excluded.

- [ ] **Step 3: Fix override builder — build all excludes in one builder**

In `src/pipeline/walk.rs`, replace lines 67-100 with:

```rust
        // Add exclude patterns for .ariadne and any configured dirs
        let mut override_builder = ignore::overrides::OverrideBuilder::new(root);
        for dir in &config.exclude_dirs {
            // The '!' prefix tells the ignore crate to exclude matching paths
            let _ = override_builder.add(&format!("!{}/**", dir));
        }
        if let Ok(overrides) = override_builder.build() {
            walker.overrides(overrides);
        }

        let mut entries = Vec::new();

        for result in walker.build() {
            let entry = match result {
                Ok(e) => e,
                Err(err) => {
                    // Emit walk-level errors as structured info on stderr
                    // (W002 warnings require DiagnosticCollector which walk doesn't have access to)
                    eprintln!("walk: skipping entry: {}", err);
                    continue;
                }
            };

            // Skip directories
            if entry.file_type().map_or(true, |ft| ft.is_dir()) {
                continue;
            }

            let path = entry.into_path();
```

This removes the redundant manual path-component check (lines 91-100 in the old code) and consolidates all excludes into a single `OverrideBuilder`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test walk_excludes_multiple_directories -- --nocapture`
Expected: PASS

- [ ] **Step 5: Run full test suite**

Run: `cargo test`
Expected: All tests pass.

- [ ] **Step 6: Commit**

```
ariadne(pipeline): fix override builder loop and walk error reporting

The override builder was creating a new OverrideBuilder per exclude dir,
replacing the previous one each iteration. Only the last exclude dir
was actually effective. Now all excludes are built in a single builder.

Also removed redundant manual path-component exclude check that was
masking the bug, and added stderr output for walk-level errors.

Fixes: F-2 from architecture review 2026-03-18.
```

---

### Task 2: Replace `expect()` panic with `FatalError` in registry

**Finding:** F-3

**Files:**
- Modify: `src/parser/registry.rs:80-111`
- Modify: `src/pipeline/mod.rs:102-127`
- Test: `tests/pipeline_tests.rs`

- [ ] **Step 1: Change `parse_source` to return `Result` instead of `Option`**

In `src/parser/registry.rs`, change `parse_source` signature and body:

```rust
    /// Parse source code with the appropriate parser.
    /// Returns Err if grammar fails to load.
    /// Returns Ok(None) if >50% of top-level nodes are ERROR (W001).
    /// Otherwise extracts imports/exports from valid subtrees.
    pub fn parse_source(
        &self,
        source: &[u8],
        parser: &dyn LanguageParser,
    ) -> Result<Option<(tree_sitter::Tree, Vec<RawImport>, Vec<RawExport>)>, String> {
        let mut ts_parser = tree_sitter::Parser::new();
        ts_parser
            .set_language(&parser.tree_sitter_language())
            .map_err(|e| format!("grammar version mismatch for {}: {}", parser.language(), e))?;

        let tree = match ts_parser.parse(source, None) {
            Some(t) => t,
            None => return Ok(None),
        };

        // Check error rate
        let root = tree.root_node();
        if root.has_error() {
            let total = root.child_count();
            if total > 0 {
                let error_count = (0..total)
                    .filter(|&i| root.child(i).map_or(false, |n| n.is_error()))
                    .count();
                if error_count * 2 > total {
                    // >50% ERROR nodes → skip file entirely
                    return Ok(None);
                }
            }
        }

        let imports = parser.extract_imports(&tree, source);
        let exports = parser.extract_exports(&tree, source);

        Ok(Some((tree, imports, exports)))
    }
```

- [ ] **Step 2: Update call site in `pipeline/mod.rs`**

In `src/pipeline/mod.rs`, replace the `match self.registry.parse_source(...)` block inside the existing `filter_map` closure (lines 109-126). Only the inner `match` expression changes — the surrounding `par_iter().filter_map(|fc| { ... }).collect()` structure remains intact:

```rust
                match self.registry.parse_source(&fc.bytes, parser) {
                    Ok(Some((_tree, imports, exports))) => Some(ParsedFile {
                        path: fc.path.clone(),
                        imports,
                        exports,
                    }),
                    Ok(None) => {
                        // Parse failed (>50% ERROR nodes)
                        diagnostics.warn(Warning {
                            code: WarningCode::W001ParseFailed,
                            path: fc.path.clone(),
                            message: "parse failed: too many errors".to_string(),
                            detail: None,
                        });
                        None
                    }
                    Err(msg) => {
                        // Grammar version mismatch — treat as parse failure
                        diagnostics.warn(Warning {
                            code: WarningCode::W001ParseFailed,
                            path: fc.path.clone(),
                            message: msg,
                            detail: None,
                        });
                        None
                    }
                }
```

- [ ] **Step 3: Run full test suite**

Run: `cargo test`
Expected: All tests pass. No panic on grammar version mismatch — it's now a warning.

- [ ] **Step 4: Commit**

```
ariadne(parser): replace expect() panic with Result in parse_source

Grammar ABI version mismatch previously panicked via expect().
In a rayon parallel context this would crash the binary. Now returns
a Result with descriptive error message, converted to W001 warning
by the pipeline.

Fixes: F-3 from architecture review 2026-03-18.
```

---

### Task 3: Wire `--output` flag

**Finding:** S-6

**Files:**
- Modify: `src/main.rs:36-37`
- Test: `tests/pipeline_tests.rs`

- [ ] **Step 1: Write test that --output produces files in custom dir**

```rust
/// S-6: --output flag should write to the specified directory.
#[test]
fn pipeline_output_dir_is_respected() {
    let path = helpers::fixture_path("typescript-app");
    let custom_dir = tempfile::tempdir().expect("create tempdir");
    let custom_path = custom_dir.path().join("custom-output");

    let pipeline = make_pipeline();
    let config = WalkConfig::default();

    let output = pipeline
        .run_with_output(&path, config, Some(&custom_path))
        .expect("build should succeed");

    assert_eq!(output.graph_path, custom_path.join("graph.json"));
    assert!(output.graph_path.exists(), "graph.json should exist in custom dir");
}
```

- [ ] **Step 2: Run test to verify it passes (run_with_output already works)**

Run: `cargo test pipeline_output_dir_is_respected -- --nocapture`
Expected: PASS — `run_with_output` already supports custom dirs. The bug is only in `main.rs` wiring.

- [ ] **Step 3: Wire the flag in `main.rs`**

In `src/main.rs`, change line 36-37:

```rust
        Commands::Build { path, output } => {
            run_build(&path, output.as_deref());
        }
```

Update `run_build` signature and body:

```rust
fn run_build(path: &PathBuf, output: Option<&std::path::Path>) {
    let start = Instant::now();

    // Composition Root (D-020)
    let pipeline = BuildPipeline::new(
        Box::new(FsWalker::new()),
        Box::new(FsReader::new()),
        ParserRegistry::with_tier1(),
        Box::new(JsonSerializer),
    );

    let config = WalkConfig::default();

    match pipeline.run_with_output(path, config, output) {
```

The rest of `run_build` stays the same.

- [ ] **Step 4: Run full test suite**

Run: `cargo test`
Expected: All pass.

- [ ] **Step 5: Commit**

```
ariadne(cli): wire --output flag to pipeline

The --output flag was accepted by clap but silently discarded.
Now passes the value to run_with_output so output goes to the
user-specified directory.

Fixes: S-6 from architecture review 2026-03-18.
```

---

### Task 4: Add binary file detection (null-byte scan)

**Finding:** Theme 3 (missing binary detection)

**Files:**
- Modify: `src/pipeline/read.rs:60-66`
- Test: `tests/pipeline_tests.rs`

- [ ] **Step 1: Write test that binary file with source extension is detected**

```rust
/// Binary file with .ts extension should be skipped with W004, not passed to parser.
#[test]
fn binary_file_detected_by_null_bytes() {
    use ariadne_graph::pipeline::{FsReader, FileReader, FileEntry};

    let temp_dir = tempfile::tempdir().expect("create tempdir");
    let file_path = temp_dir.path().join("binary.ts");
    // Write binary content: valid UTF-8 with embedded null bytes
    std::fs::write(&file_path, b"import foo\x00\x00 from 'bar';\n").unwrap();

    let reader = FsReader::new();
    let entry = FileEntry {
        path: file_path,
        extension: "ts".to_string(),
    };

    let result = reader.read(&entry, temp_dir.path(), 1_048_576);
    assert!(result.is_err(), "binary file should be rejected");

    match result.unwrap_err() {
        ariadne_graph::pipeline::FileSkipReason::BinaryFile { .. } => {},
        other => panic!("expected BinaryFile, got {:?}", other),
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test binary_file_detected_by_null_bytes -- --nocapture`
Expected: FAIL — currently passes as UTF-8 check succeeds (null bytes are valid UTF-8).

- [ ] **Step 3: Add null-byte scan before UTF-8 check**

In `src/pipeline/read.rs`, after reading bytes (line 59) and before the UTF-8 check (line 62), add:

```rust
        // Check for binary content (null bytes in first 8KB)
        let check_len = bytes.len().min(8192);
        if bytes[..check_len].contains(&0) {
            return Err(FileSkipReason::BinaryFile {
                path: entry.path.clone(),
            });
        }
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test binary_file_detected_by_null_bytes -- --nocapture`
Expected: PASS

- [ ] **Step 5: Run full test suite**

Run: `cargo test`
Expected: All pass.

- [ ] **Step 6: Commit**

```
ariadne(pipeline): add null-byte binary file detection

Binary files that happen to be valid UTF-8 were previously passed to
tree-sitter. Now checks the first 8KB for null bytes and emits W004
(BinaryFile) skip reason, as specified in error-handling.md.

Fixes: binary detection gap from architecture review 2026-03-18.
```

---

### Task 5: Add self-import filter

**Finding:** Theme 3 (self-imports not filtered, violates INV-2)

**Files:**
- Modify: `src/pipeline/resolve.rs:7-23`
- Test: `tests/pipeline_tests.rs`

- [ ] **Step 1: Write INV-2 integration test across all fixtures**

This tests the invariant across all fixture projects. The test verifies the guard works for any future fixture that might contain self-imports.

```rust
/// INV-2: No edge should have the same from and to path.
#[test]
fn inv2_no_self_import_edges() {
    for fixture in &["typescript-app", "go-service", "python-package", "rust-crate", "mixed-project"] {
        let output = helpers::build_fixture(fixture);
        let graph_json = std::fs::read_to_string(&output.graph_path).unwrap();
        let graph: serde_json::Value = serde_json::from_str(&graph_json).unwrap();

        if let Some(edges) = graph["edges"].as_array() {
            for edge in edges {
                let from = edge[0].as_str().unwrap();
                let to = edge[1].as_str().unwrap();
                assert_ne!(from, to, "INV-2 violation in {}: self-edge {} -> {}", fixture, from, to);
            }
        }
    }
}
```

- [ ] **Step 2: Run test — should pass (no fixtures have self-imports)**

Run: `cargo test inv2_no_self_import_edges -- --nocapture`
Expected: PASS. This is a safety-net test — it guards against regressions even though no current fixture triggers it.

- [ ] **Step 3: Add self-import check in `resolve.rs`**

In `src/pipeline/resolve.rs`, modify `resolve_import`:

```rust
pub fn resolve_import(
    import: &RawImport,
    from_file: &CanonicalPath,
    known_files: &FileSet,
    resolver: &dyn ImportResolver,
    diagnostics: &DiagnosticCollector,
) -> Option<CanonicalPath> {
    match resolver.resolve(import, from_file, known_files) {
        Some(resolved) => {
            // INV-2: filter self-imports
            if resolved == *from_file {
                return None;
            }
            Some(resolved)
        }
        None => {
            diagnostics.increment_unresolved();
            None
        }
    }
}
```

- [ ] **Step 4: Run full test suite**

Run: `cargo test`
Expected: All pass.

- [ ] **Step 5: Commit**

```
ariadne(pipeline): filter self-import edges (INV-2)

A file importing itself would produce a self-referencing edge,
violating INV-2. Now resolve_import returns None when the resolved
path equals the source file path.

Fixes: self-import gap from architecture review 2026-03-18.
```

---

### Task 6: Add `ImportKind` to `RawImport`

**Finding:** F-4

**Files:**
- Modify: `src/parser/traits.rs` — add `ImportKind` enum, add field to `RawImport`
- Modify: `src/parser/rust_lang.rs` — use `ImportKind::ModDeclaration`
- Modify: `src/parser/typescript.rs` — set `ImportKind::Regular`
- Modify: `src/parser/go.rs` — set `ImportKind::Regular`
- Modify: `src/parser/python.rs` — set `ImportKind::Regular`
- Modify: `src/parser/csharp.rs` — set `ImportKind::Regular`
- Modify: `src/parser/java.rs` — set `ImportKind::Regular`
- Modify: `src/pipeline/build.rs` — update `RawImport` construction for re-exports

- [ ] **Step 1: Add `ImportKind` enum and field to `RawImport`**

In `src/parser/traits.rs`:

```rust
/// Discriminant for import origin — allows language-specific import types
/// without encoding them as sentinel values in `RawImport.path`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ImportKind {
    /// Standard import statement (import/require/use/using)
    Regular,
    /// Rust `mod` declaration — path is the module name, not a filesystem path
    ModDeclaration,
}

/// Raw import extracted from AST (unresolved).
#[derive(Clone, Debug)]
pub struct RawImport {
    pub path: String,
    pub symbols: Vec<String>,
    pub is_type_only: bool,
    pub kind: ImportKind,
}
```

- [ ] **Step 2: Update re-exports and imports for `ImportKind`**

In `src/parser/mod.rs`, update the re-export line to include `ImportKind`:

```rust
pub use traits::{ImportKind, ImportResolver, LanguageParser, RawExport, RawImport};
```

In each parser file, add `ImportKind` to the import statement. For files using `use super::traits::{...}`, add `ImportKind` to the list. In `src/pipeline/build.rs`, update the import:

```rust
use crate::parser::{ParserRegistry, RawImport, ImportKind};
```

- [ ] **Step 3: Add `kind: ImportKind::Regular` to all `RawImport` constructions**

In every parser file (`typescript.rs`, `go.rs`, `python.rs`, `csharp.rs`, `java.rs`), add `kind: ImportKind::Regular` to every `RawImport { ... }` construction. In `pipeline/build.rs` line 105-108 (re-export RawImport), add the same.

In `rust_lang.rs`, change the `mod` declaration import (around line 300-303):

```rust
                            imports.push(RawImport {
                                path: name.to_string(),
                                symbols: vec![name.to_string()],
                                is_type_only: false,
                                kind: ImportKind::ModDeclaration,
                            });
```

And change all `use` statement imports to use `kind: ImportKind::Regular`.

- [ ] **Step 4: Update `RustResolver` to check `kind` instead of `mod::` prefix**

In `src/parser/rust_lang.rs`, in `RustResolver::resolve()`, replace the `mod::` sentinel check:

```rust
// Old:
// if path.starts_with("mod::") { ... }
// New:
if import.kind == ImportKind::ModDeclaration {
    // mod declaration — resolve module name to file path
    let mod_name = &import.path;
    // ... (rest of mod resolution logic, using mod_name instead of stripping "mod::" prefix)
}
```

- [ ] **Step 5: Run full test suite**

Run: `cargo test`
Expected: All pass. The Rust parser snapshot tests should still pass since the resolution behavior is identical.

- [ ] **Step 6: Commit**

```
ariadne(parser): add ImportKind enum to RawImport

Replaces the Rust-specific "mod::" sentinel encoding in
RawImport.path with a proper ImportKind::ModDeclaration
discriminant. This prevents language-specific semantics from
leaking into the shared RawImport type.

Fixes: F-4 from architecture review 2026-03-18.
```

---

### Task 7: Add `test_*.py` prefix pattern + HashSet edge dedup

**Findings:** U-7, U-8

**Files:**
- Modify: `src/pipeline/build.rs:171-222` (test edge inference)
- Test: existing invariant tests cover this via fixture builds

- [ ] **Step 1: Add prefix pattern support and HashSet dedup**

In `src/pipeline/build.rs`, modify `infer_test_edges_by_naming`:

```rust
fn infer_test_edges_by_naming(
    nodes: &BTreeMap<CanonicalPath, Node>,
    file_set: &FileSet,
    edges: &mut Vec<Edge>,
) {
    // Suffix-based patterns: (test_suffix, source_suffix)
    let suffix_patterns = [
        (".test.ts", ".ts"),
        (".spec.ts", ".ts"),
        (".test.tsx", ".tsx"),
        (".spec.tsx", ".tsx"),
        (".test.js", ".js"),
        (".spec.js", ".js"),
        (".test.jsx", ".jsx"),
        (".spec.jsx", ".jsx"),
        ("_test.go", ".go"),
        ("_test.py", ".py"),
    ];

    // Prefix-based patterns: (test_prefix, source_ext)
    // e.g., test_auth.py → auth.py
    let prefix_patterns = [
        ("test_", ".py"),
    ];

    // Build HashSet for O(1) edge existence check (U-8)
    let existing_edges: std::collections::HashSet<(CanonicalPath, CanonicalPath, EdgeType)> = edges
        .iter()
        .map(|e| (e.from.clone(), e.to.clone(), e.edge_type))
        .collect();

    let mut new_edges = Vec::new();

    for (path, node) in nodes {
        if node.file_type != FileType::Test {
            continue;
        }

        let path_str = path.as_str();
        let file_name = path.file_name();

        // Try suffix patterns
        for (test_suffix, source_suffix) in &suffix_patterns {
            if path_str.ends_with(test_suffix) {
                let source_path_str =
                    format!("{}{}", &path_str[..path_str.len() - test_suffix.len()], source_suffix);
                let source_path = CanonicalPath::new(&source_path_str);

                if file_set.contains(&source_path) {
                    let key = (path.clone(), source_path.clone(), EdgeType::Tests);
                    if !existing_edges.contains(&key) {
                        new_edges.push(Edge {
                            from: path.clone(),
                            to: source_path,
                            edge_type: EdgeType::Tests,
                            symbols: vec![],
                        });
                    }
                    break;
                }
            }
        }

        // Try prefix patterns (e.g., test_auth.py → auth.py in same dir)
        for (test_prefix, source_ext) in &prefix_patterns {
            if file_name.starts_with(test_prefix) && file_name.ends_with(source_ext) {
                let source_name = &file_name[test_prefix.len()..];
                let source_path = match path.parent() {
                    Some(parent) => CanonicalPath::new(format!("{}/{}", parent, source_name)),
                    None => CanonicalPath::new(source_name),
                };

                if file_set.contains(&source_path) {
                    let key = (path.clone(), source_path.clone(), EdgeType::Tests);
                    if !existing_edges.contains(&key) {
                        new_edges.push(Edge {
                            from: path.clone(),
                            to: source_path,
                            edge_type: EdgeType::Tests,
                            symbols: vec![],
                        });
                    }
                    break;
                }
            }
        }
    }

    edges.extend(new_edges);
}
```

- [ ] **Step 2: Run full test suite**

Run: `cargo test`
Expected: All pass. No existing fixtures use `test_*.py` prefix, so this is additive only.

- [ ] **Step 3: Commit**

```
ariadne(pipeline): add test_*.py prefix pattern and O(1) edge dedup

Added Python test_*.py → *.py prefix pattern to naming-convention
test edge inference (was only handling suffix patterns).

Replaced O(n*e) edges.iter().any() duplicate check with a HashSet
for O(1) lookups.

Fixes: U-7, U-8 from architecture review 2026-03-18.
```

---

### Task 8: Clean up `.tmp` files on write failure

**Finding:** U-12

**Files:**
- Modify: `src/serial/json.rs:32-53`

- [ ] **Step 1: Add cleanup in error paths**

In `src/serial/json.rs`, modify `atomic_write`:

```rust
fn atomic_write<T: serde::Serialize>(dir: &Path, filename: &str, value: &T) -> Result<(), FatalError> {
    let final_path = dir.join(filename);
    let tmp_path = dir.join(format!("{}.{}.tmp", filename, std::process::id()));

    let file = fs::File::create(&tmp_path).map_err(|e| FatalError::OutputNotWritable {
        path: final_path.clone(),
        reason: e.to_string(),
    })?;

    let writer = BufWriter::new(file);
    if let Err(e) = serde_json::to_writer_pretty(writer, value) {
        // Clean up tmp file on serialization failure
        let _ = fs::remove_file(&tmp_path);
        return Err(FatalError::OutputNotWritable {
            path: final_path,
            reason: e.to_string(),
        });
    }

    fs::rename(&tmp_path, &final_path).map_err(|e| {
        // Clean up tmp file on rename failure
        let _ = fs::remove_file(&tmp_path);
        FatalError::OutputNotWritable {
            path: final_path,
            reason: e.to_string(),
        }
    })?;

    Ok(())
}
```

- [ ] **Step 2: Run full test suite**

Run: `cargo test`
Expected: All pass.

- [ ] **Step 3: Commit**

```
ariadne(serial): clean up tmp files on write failure

Stale .tmp files accumulated when serialization or rename failed.
Now the error path removes the tmp file before returning the error.

Fixes: U-12 from architecture review 2026-03-18.
```

---

### Task 9: Merge `DiagnosticCollector` into single `Mutex`

**Finding:** U-6

**Files:**
- Modify: `src/diagnostic.rs:72-131`
- Test: `tests/pipeline_tests.rs` (existing tests cover `DiagnosticCollector`)

- [ ] **Step 1: Combine warnings + counts into single Mutex**

Replace the `DiagnosticCollector` implementation in `src/diagnostic.rs`:

```rust
/// Thread-safe warning collector for use during parallel pipeline stages.
pub struct DiagnosticCollector {
    inner: Mutex<(Vec<Warning>, DiagnosticCounts)>,
}

impl DiagnosticCollector {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new((Vec::new(), DiagnosticCounts::default())),
        }
    }

    /// Record a warning.
    pub fn warn(&self, warning: Warning) {
        let mut guard = self.inner.lock().unwrap();
        match warning.code {
            WarningCode::W001ParseFailed
            | WarningCode::W002ReadFailed
            | WarningCode::W003FileTooLarge
            | WarningCode::W004BinaryFile
            | WarningCode::W009EncodingError => {
                guard.1.files_skipped += 1;
            }
            WarningCode::W006ImportUnresolved => {
                guard.1.imports_unresolved += 1;
            }
            WarningCode::W007PartialParse => {
                guard.1.partial_parses += 1;
            }
            WarningCode::W008ConfigParseFailed => {}
        }
        guard.0.push(warning);
    }

    /// Increment unresolved import count without recording a warning
    /// (used when not in verbose mode).
    pub fn increment_unresolved(&self) {
        let mut guard = self.inner.lock().unwrap();
        guard.1.imports_unresolved += 1;
    }

    /// Consume the collector and return a sorted diagnostic report.
    pub fn drain(self) -> DiagnosticReport {
        let (mut warnings, counts) = self.inner.into_inner().unwrap();
        // Sort by (path, code) for deterministic output (D-006)
        warnings.sort_by(|a, b| a.path.cmp(&b.path).then(a.code.cmp(&b.code)));
        DiagnosticReport { warnings, counts }
    }
}
```

- [ ] **Step 2: Run full test suite**

Run: `cargo test`
Expected: All pass. The existing `diagnostic_collector_*` tests verify the behavior is unchanged.

- [ ] **Step 3: Commit**

```
ariadne(core): merge DiagnosticCollector into single Mutex

Previously used two separate Mutex instances (warnings + counts),
acquired sequentially in warn(). Now uses a single
Mutex<(Vec<Warning>, DiagnosticCounts)> — one lock acquisition
per operation, eliminating the double-lock pattern.

Fixes: U-6 from architecture review 2026-03-18.
```

---

### Task 10: Documentation fixes (all quick wins)

**Findings:** Quick Wins 1-8 from recommendations, plus D-026 missing decision

**Files:**
- Modify: `design/architecture.md`
- Modify: `design/error-handling.md`
- Modify: `design/performance.md`
- Modify: `design/decisions/log.md`

- [ ] **Step 1: Fix `arch_depth` in architecture.md graph.json example**

In `design/architecture.md`, find the graph.json example node and change `"arch_depth": 2` to `"arch_depth": 0`. Add a comment line or note near the field: `// Phase 1: always 0 (D-025); computed via topological sort in Phase 2`.

- [ ] **Step 2: Update architecture.md dependency table**

In the "Dependency rules" table, change the `serial/` row:

| `serial/` | `model/`, `diagnostic.rs` (for `FatalError`) | `parser/`, `pipeline/`, `detect/`, `cluster/` |

- [ ] **Step 3: Update architecture.md `BuildPipeline` struct**

Change the `BuildPipeline` code block to remove `diagnostics: DiagnosticCollector` field. Add a note: "DiagnosticCollector is created per `run_with_output()` call, not stored as a struct field — this prevents state leakage between pipeline runs."

- [ ] **Step 4: Add resolver limitation notes to architecture.md**

After the Tier 1 language table, add a note:

```markdown
**Phase 1a resolution limitations:**
- **Go:** Stdlib-only resolution. Module-qualified imports (e.g., `github.com/org/repo/internal/handler`) require reading `go.mod`, which is deferred to Phase 1b. Go projects will have zero inter-package edges in Phase 1a.
- **C#:** Namespace-to-path heuristic. C# namespaces do not map to filesystem paths, so resolution accuracy is low for typical C# projects.
- **Java:** Package-to-path heuristic with hardcoded `src/main/java/` prefix. Accuracy depends on project following Maven/Gradle conventions.
```

- [ ] **Step 5: Document `src/`-centric cluster naming assumption**

In the Cluster Interface section of `design/architecture.md`, add:

```markdown
**Cluster naming heuristic:** If a file path starts with `src/`, the cluster name uses the segment after `src/` (e.g., `src/auth/login.ts` → cluster `auth`). Otherwise, the first path segment is used. This convention works well for TypeScript/Rust projects with a `src/` root. Go projects (`cmd/`, `pkg/`, `internal/`) and Java Maven projects (`src/main/java/com/...`) may produce less meaningful cluster names. Language-aware prefix stripping is a Phase 1b consideration.
```

- [ ] **Step 6: Update error-handling.md Stage 1 pseudocode**

In `design/error-handling.md`, update the Stage 1 section to clearly separate walking and reading into two sub-stages, matching the actual pipeline architecture (walk returns `Vec<FileEntry>`, read is a separate loop producing `Vec<FileContent>`).

- [ ] **Step 7: Document W005 gap in error-handling.md**

Add a note in the warning codes table: "W005 is reserved (unassigned). Warning codes jump from W004 to W006 intentionally to allow future insertion."

- [ ] **Step 8: Reconcile performance.md memory estimates**

In `design/performance.md`, update the Scaling Characteristics table note for 50,000 files to clarify: "~250MB for graph data structures, ~400MB peak including file bytes retained during the parse-and-build phase."

- [ ] **Step 9: Add D-026 to decision log**

```markdown
## D-026: Walk and Read as Separate Pipeline Stages

**Date:** 2026-03-18
**Status:** Accepted
**Context:** The original error-handling.md described file walking and file reading as a single "Stage 1" operation. The implementation (following D-019 injectable pipeline traits) separates them into `FileWalker::walk()` (returns paths) and `FileReader::read()` (returns content). This separation was implicit — no decision documented it.
**Decision:** Walking and reading are separate pipeline stages with separate traits, separate error handling, and separate responsibilities. Walking produces `Vec<FileEntry>` (path + extension). Reading produces `Vec<FileContent>` (path + bytes + hash + lines). Walking errors are logged to stderr. Reading errors produce structured `FileSkipReason` values converted to warnings.
**Reasoning:** Separation enables independent testing (mock walker, mock reader), independent error handling (walk-level vs read-level), and independent resource control (max_files in walk, max_file_size in read).
**Affects:** error-handling.md Stage 1, architecture.md Pipeline Architecture.
```

- [ ] **Step 10: Remove `models/ (if sibling to services/)` heuristic from architecture.md**

In the Architectural Layer Heuristics table, change the `data` row to remove the parenthetical "(if sibling to services/)" from `models/`, since this context-sensitive rule cannot be implemented by a pure path-matching function. Simply list `models/` under `data` unconditionally and add a note: "The `models/` directory is always classified as `data`. In frameworks where `models` is co-located with views (e.g., Django), this may be imprecise."

- [ ] **Step 11: Run full test suite to confirm no code breakage**

Run: `cargo test`
Expected: All pass (doc-only changes).

- [ ] **Step 12: Commit**

```
ariadne(design): fix doc inconsistencies from architecture review

- Fix arch_depth example in architecture.md (2 → 0, per D-025)
- Update dependency table: serial/ depends on diagnostic.rs
- Update BuildPipeline to show DiagnosticCollector per-run scoping
- Add Phase 1a resolver limitation notes (Go, C#, Java)
- Document src/-centric cluster naming assumption
- Separate walk/read in error-handling.md Stage 1 pseudocode
- Document W005 gap as intentionally reserved
- Reconcile memory estimates in performance.md (250MB vs 400MB)
- Add D-026: Walk/Read separation decision
- Fix models/ heuristic (remove unimplementable sibling check)

Fixes: Quick Wins 1-8 from architecture review 2026-03-18.
```
