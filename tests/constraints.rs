//! Architectural constraint enforcement tests.
//!
//! These tests scan the source code to prevent introduction of patterns
//! that violate Ariadne's architectural invariants. They complement the
//! graph invariants in `invariants.rs` by checking the Rust source itself.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

fn src_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src")
}

/// Collect all .rs files under a directory, recursively.
fn collect_rs_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    collect_rs_files_recursive(dir, &mut files);
    files.sort();
    files
}

fn collect_rs_files_recursive(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_rs_files_recursive(&path, out);
        } else if path.extension().map_or(false, |ext| ext == "rs") {
            out.push(path);
        }
    }
}

/// Return only production lines with their original line numbers.
/// Everything from `#[cfg(test)]` to end-of-file is considered test code.
/// This is simpler and more robust than brace-depth tracking, which breaks
/// on raw strings containing braces (e.g., JSON in test fixtures).
fn production_lines(content: &str) -> Vec<(usize, &str)> {
    let mut result = Vec::new();
    for (i, line) in content.lines().enumerate() {
        if line.trim().starts_with("#[cfg(test)]") {
            break;
        }
        result.push((i + 1, line));
    }
    result
}

fn relative_path(path: &Path) -> String {
    let src = src_dir();
    path.strip_prefix(&src)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

// ---- Constraint: no_hashmap_in_model ----
// HashMap is forbidden in model/ and serial/ — determinism requires BTreeMap.

#[test]
fn no_hashmap_in_model() {
    let mut violations = Vec::new();

    for subdir in &["model", "serial"] {
        let dir = src_dir().join(subdir);
        for file in collect_rs_files(&dir) {
            let content = std::fs::read_to_string(&file).unwrap();
            for (i, line) in content.lines().enumerate() {
                let trimmed = line.trim();
                // Skip comments
                if trimmed.starts_with("//") || trimmed.starts_with("*") {
                    continue;
                }
                if line.contains("HashMap") {
                    violations.push(format!(
                        "  {}:{} — {}",
                        relative_path(&file),
                        i + 1,
                        trimmed
                    ));
                }
            }
        }
    }

    assert!(
        violations.is_empty(),
        "no_hashmap_in_model: HashMap forbidden in model/ and serial/ (use BTreeMap for determinism):\n{}",
        violations.join("\n")
    );
}

// ---- Constraint: no_god_modules ----
// No NEW file in src/ may exceed 300 lines. Known legacy violations are allowlisted.
// To fix: split the file, then remove it from the allowlist.

#[test]
fn no_god_modules() {
    // Known violations as of 2026-04-10. Each entry is (relative path, current max lines).
    // When you split a file below 300, remove it from here.
    let allowlist: HashSet<&str> = [
        "mcp/tools.rs",
        "parser/typescript.rs",
        "recommend/refactor.rs",
        "recommend/placement.rs",
        "recommend/split.rs",
        "parser/rust_lang.rs",
        "parser/csharp.rs",
        "parser/python.rs",
        "parser/java.rs",
        "semantic/http.rs",
        "analysis/smells.rs",
        "pipeline/mod.rs",
        "conventions/tech_stack.rs",
        "mcp/prompts.rs",
        "semantic/java.rs",
        "parser/config/mod.rs",
        "algo/louvain.rs",
        "algo/context.rs",
        "semantic/events.rs",
        "detect/workspace.rs",
        "analysis/diff.rs",
        "parser/go.rs",
        "parser/config/tsconfig.rs",
        "diagnostic.rs",
        "mcp/annotations.rs",
        "algo/compress.rs",
        "temporal/git.rs",
        "detect/layer.rs",
        "mcp/resources.rs",
        "analysis/metrics.rs",
        "parser/config/csproj.rs",
        "parser/markdown.rs",
        "algo/spectral.rs",
        "semantic/edges.rs",
        "detect/framework.rs",
        "parser/registry.rs",
        "temporal/coupling.rs",
        "mcp/bookmarks.rs",
        "algo/impact.rs",
        "conventions/naming.rs",
        "parser/config/gradle.rs",
        "algo/callgraph.rs",
        "pipeline/build.rs",
        "algo/pagerank.rs",
        "detect/filetype.rs",
        "temporal/churn.rs",
        "model/types.rs",
        "algo/test_map.rs",
        "mcp/watch.rs",
        "temporal/hotspot.rs",
        "views/index.rs",
        "parser/config/maven.rs",
        "conventions/trends.rs",
        "conventions/imports.rs",
        "parser/config/bundler.rs",  // Phase 13a: Vite+Webpack parsing + tests (468 code + 306 tests)
        "parser/config/nextjs.rs",   // Phase 13a: Next.js route discovery + tests (227 code + 179 tests)
        "detect/js_framework.rs",    // Phase 13a: React/Next.js detection + tests (290 code + 283 tests)
    ]
    .iter()
    .copied()
    .collect();

    let mut violations = Vec::new();

    for file in collect_rs_files(&src_dir()) {
        let rel = relative_path(&file);
        if allowlist.contains(rel.as_str()) {
            continue;
        }
        let content = std::fs::read_to_string(&file).unwrap();
        let line_count = content.lines().count();
        if line_count > 300 {
            violations.push(format!("  {} — {} lines", rel, line_count));
        }
    }

    assert!(
        violations.is_empty(),
        "no_god_modules: new files exceeding 300 lines (split or add to allowlist with justification):\n{}",
        violations.join("\n")
    );
}

// ---- Constraint: no_hashmap_leaks ----
// HashMap is allowed in some modules for performance, but must not leak into
// the output path (model/ and serial/ are checked above). This test ensures
// HashMap imports don't spread into new modules without awareness.
// Currently allowed: mcp/prompts.rs, parser/registry.rs

#[test]
fn no_new_hashmap_imports() {
    let allowed: HashSet<&str> = [
        "mcp/prompts.rs",
        "parser/registry.rs",
        // Intermediate computation — not serialized, determinism not affected
        "semantic/mod.rs",
        "temporal/churn.rs",
        "temporal/coupling.rs",
        "temporal/ownership.rs",
    ]
    .iter()
    .copied()
    .collect();

    let mut violations = Vec::new();

    for file in collect_rs_files(&src_dir()) {
        let rel = relative_path(&file);
        if allowed.contains(rel.as_str()) {
            continue;
        }
        let content = std::fs::read_to_string(&file).unwrap();
        for (i, line) in content.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("//") {
                continue;
            }
            // Check for HashMap import (not string literals in test data)
            if trimmed.starts_with("use ") && trimmed.contains("HashMap") {
                violations.push(format!("  {}:{} — {}", rel, i + 1, trimmed));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "no_new_hashmap_imports: HashMap import found in new location (use BTreeMap, or add to allowlist):\n{}",
        violations.join("\n")
    );
}

// ---- Constraint: no_unwrap_in_production ----
// `.unwrap()` is forbidden in production code. Use `?`, `.expect()` with justification,
// or `.unwrap_or()` / `.unwrap_or_default()` / `.unwrap_or_else()` instead.

#[test]
fn no_unwrap_in_production() {
    let mut violations = Vec::new();

    for file in collect_rs_files(&src_dir()) {
        let rel = relative_path(&file);
        let content = std::fs::read_to_string(&file).unwrap();
        for (line_no, line) in production_lines(&content) {
            let trimmed = line.trim();
            if trimmed.starts_with("//") || trimmed.starts_with("*") {
                continue;
            }
            // Match `.unwrap()` but not `.unwrap_or`, `.unwrap_or_default`, `.unwrap_or_else`
            if trimmed.contains(".unwrap()")
                && !trimmed.contains(".unwrap_or(")
                && !trimmed.contains(".unwrap_or_default(")
                && !trimmed.contains(".unwrap_or_else(")
            {
                violations.push(format!("  {}:{} — {}", rel, line_no, trimmed));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "no_unwrap_in_production: `.unwrap()` forbidden in production code \
         (use `?`, `.expect()`, or `.unwrap_or*()` instead):\n{}",
        violations.join("\n")
    );
}

// ---- Constraint: no_silent_errors ----
// `let _ = expr` on Result types silently discards errors.
// Allowed in test code. Existing production uses are allowlisted.

#[test]
fn no_silent_errors() {
    let allowed: HashSet<&str> = [
        // Filesystem probe cleanup — failure is harmless
        "detect/case_sensitivity.rs",
        // Lock file cleanup in Drop — can't propagate errors
        "mcp/lock.rs",
        // Glob override builder — infallible in practice
        "pipeline/walk.rs",
        // Intentionally unused bindings
        "recommend/placement.rs",
        "semantic/java.rs",
        // Existence check, not error suppression
        "parser/typescript.rs",
        // Atomic write cleanup — failure on tmp removal is harmless
        "serial/json.rs",
    ]
    .iter()
    .copied()
    .collect();

    let mut violations = Vec::new();

    for file in collect_rs_files(&src_dir()) {
        let rel = relative_path(&file);
        if allowed.contains(rel.as_str()) {
            continue;
        }
        let content = std::fs::read_to_string(&file).unwrap();
        for (line_no, line) in production_lines(&content) {
            let trimmed = line.trim();
            if trimmed.starts_with("//") {
                continue;
            }
            if trimmed.starts_with("let _ = ") {
                violations.push(format!("  {}:{} — {}", rel, line_no, trimmed));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "no_silent_errors: `let _ = ...` in production code silently discards errors. \
         Handle the Result or add to allowlist with justification:\n{}",
        violations.join("\n")
    );
}
