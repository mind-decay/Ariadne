//! `ariadne outline <path>` integration test (tier-03).
//!
//! Indexes a multi-symbol Rust fixture and runs the real `ariadne outline`
//! subcommand, asserting it renders the tier-01 folded skeleton: signatures +
//! doc comments kept, bodies folded to a marker, output strictly byte-smaller
//! than the source. Covers `--include-private` (a private symbol reappears),
//! `--json` (the full `Outline` serialized), and a missing path (a typed
//! non-zero exit, not a panic) [src:
//! .claude/plans/context-efficient-read/tier-03-outline-cli.md `<steps>`].

use std::path::Path;
use std::process::{Command, Output};

/// Default binary under test (the workspace `ariadne` build).
fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_ariadne")
}

/// Fixture root crate: two public functions with foldable bodies + leading doc
/// comments, plus a private helper the default skeleton must drop.
const LIB_RS: &str = "\
//! Fixture crate root for the outline CLI test.

/// Adds two integers and returns the sum.
pub fn add(left: i32, right: i32) -> i32 {
    let sum = left + right;
    let checked = sum;
    checked
}

/// Multiplies two integers and returns the product.
pub fn multiply(left: i32, right: i32) -> i32 {
    let product = left * right;
    let checked = product;
    checked
}

fn secret_double(value: i32) -> i32 {
    let doubled = value * 2;
    let checked = doubled;
    checked
}
";

/// Run `<bin> <args...>`; fail unless it exits successfully.
fn run_ok(args: &[&str]) {
    let output = Command::new(bin())
        .args(args)
        .output()
        .unwrap_or_else(|e| panic!("spawn `ariadne {}`: {e}", args.join(" ")));
    assert!(
        output.status.success(),
        "`ariadne {}` exited with {}: {}",
        args.join(" "),
        output.status,
        String::from_utf8_lossy(&output.stderr),
    );
}

/// Run `ariadne outline <args...>` with the working directory set to `root`
/// (the subcommand resolves the index against the current directory).
fn run_outline(root: &Path, args: &[&str]) -> Output {
    Command::new(bin())
        .arg("outline")
        .args(args)
        .current_dir(root)
        .output()
        .expect("spawn `ariadne outline`")
}

/// A source file the indexer records (a `FileRecord`) but extracts no symbols
/// from — drives the zero-symbol note branch.
const COMMENTS_ONLY_RS: &str = "// just a comment\n// no symbols here\n";

/// Index a fresh fixture project (the multi-symbol `lib.rs` plus a
/// comments-only `empty.rs`) and return its temp dir.
fn fixture() -> tempfile::TempDir {
    let project = tempfile::tempdir().expect("create fixture tempdir");
    let root = project.path();
    std::fs::write(root.join("lib.rs"), LIB_RS).expect("write lib.rs");
    std::fs::write(root.join("empty.rs"), COMMENTS_ONLY_RS).expect("write empty.rs");
    let root_str = root.to_str().expect("utf8 root");
    run_ok(&["init", root_str]);
    run_ok(&["index", root_str, "--no-scip"]);
    project
}

#[test]
fn outline_folds_bodies_and_keeps_signatures() {
    let project = fixture();
    let out = run_outline(project.path(), &["lib.rs"]);
    assert!(
        out.status.success(),
        "`ariadne outline lib.rs` failed: {}",
        String::from_utf8_lossy(&out.stderr),
    );
    let skeleton = String::from_utf8_lossy(&out.stdout);

    assert!(
        skeleton.contains("pub fn add(left: i32, right: i32) -> i32"),
        "skeleton dropped the `add` signature:\n{skeleton}",
    );
    assert!(
        skeleton.contains("pub fn multiply(left: i32, right: i32) -> i32"),
        "skeleton dropped the `multiply` signature:\n{skeleton}",
    );
    assert!(
        skeleton.contains("Adds two integers"),
        "skeleton dropped the leading doc comment:\n{skeleton}",
    );
    assert!(
        skeleton.contains("lines }"),
        "skeleton did not fold any body to a marker:\n{skeleton}",
    );
    assert!(
        !skeleton.contains("secret_double"),
        "default skeleton leaked the private helper:\n{skeleton}",
    );

    let file_bytes = LIB_RS.len();
    assert!(
        skeleton.len() < file_bytes,
        "skeleton ({} bytes) is not smaller than the source ({file_bytes} bytes):\n{skeleton}",
        skeleton.len(),
    );
}

#[test]
fn include_private_reveals_the_private_helper() {
    let project = fixture();
    let out = run_outline(project.path(), &["lib.rs", "--include-private"]);
    assert!(
        out.status.success(),
        "`ariadne outline lib.rs --include-private` failed: {}",
        String::from_utf8_lossy(&out.stderr),
    );
    let skeleton = String::from_utf8_lossy(&out.stdout);
    assert!(
        skeleton.contains("fn secret_double(value: i32) -> i32"),
        "`--include-private` did not surface the private helper:\n{skeleton}",
    );
}

#[test]
fn json_emits_the_full_outline() {
    let project = fixture();
    let out = run_outline(project.path(), &["lib.rs", "--json"]);
    assert!(
        out.status.success(),
        "`ariadne outline lib.rs --json` failed: {}",
        String::from_utf8_lossy(&out.stderr),
    );
    let value: serde_json::Value =
        serde_json::from_slice(&out.stdout).expect("`--json` output parses as JSON");
    assert!(
        value
            .get("skeleton")
            .and_then(serde_json::Value::as_str)
            .is_some(),
        "JSON outline missing a string `skeleton` field:\n{value}",
    );
    assert!(
        value
            .get("symbols")
            .and_then(serde_json::Value::as_array)
            .is_some(),
        "JSON outline missing the `symbols` index array:\n{value}",
    );
    assert!(
        value.get("kept_lines").is_some() && value.get("elided_lines").is_some(),
        "JSON outline missing the line-count fields:\n{value}",
    );
}

#[test]
fn zero_symbol_file_prints_a_line_count_note_not_a_dump() {
    let project = fixture();
    let out = run_outline(project.path(), &["empty.rs"]);
    assert!(
        out.status.success(),
        "`ariadne outline empty.rs` failed: {}",
        String::from_utf8_lossy(&out.stderr),
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("no indexed symbols") && stdout.contains("native `Read`"),
        "zero-symbol file did not yield the line-count note:\n{stdout}",
    );
    assert!(
        !stdout.contains("just a comment"),
        "zero-symbol file dumped its source instead of the note:\n{stdout}",
    );
}

#[test]
fn missing_path_exits_nonzero_with_a_message() {
    let project = fixture();
    let out = run_outline(project.path(), &["does_not_exist.rs"]);
    assert!(
        !out.status.success(),
        "`ariadne outline does_not_exist.rs` unexpectedly succeeded",
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("not an indexed file"),
        "missing-path error message was unclear:\n{stderr}",
    );
}
