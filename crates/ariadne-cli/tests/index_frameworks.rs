//! `ariadne index` integration test over JS-framework project trees.
//!
//! Each test copies a minimal fixture project (`fixtures/<family>/`) into a
//! tempdir, runs `ariadne init` then `ariadne index`, and asserts the JSON
//! `IndexSummary`: the framework lang tag is present with non-zero symbols,
//! and — for the SFC families — at least one `Renders` edge reaches storage.
//! The fixtures are built so a `Renders` edge is the *only* edge a clean
//! index can emit (the sole resolvable target is the child component), so
//! `edges >= 1` and the redb `Renders` count cross-check each other
//! [src: .claude/plans/js-framework-support/tier-05-cli-detection.md steps 1, 6].

use std::path::Path;
use std::process::Command;

use ariadne_core::{EdgeKind, ReadSnapshot, Storage};
use ariadne_storage::RedbStorage;
use serde_json::Value;

/// Built `ariadne` binary under test [src: ariadne-cli `Cargo.toml` `[[bin]]`].
const BIN: &str = env!("CARGO_BIN_EXE_ariadne");
/// Root of the in-crate framework fixture trees.
const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures");

/// Recursively copy `src` into `dst`, creating `dst` and any sub-dirs.
fn copy_tree(src: &Path, dst: &Path) {
    std::fs::create_dir_all(dst).expect("create destination dir");
    for entry in std::fs::read_dir(src).expect("read fixture dir") {
        let entry = entry.expect("fixture dir entry");
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if entry.file_type().expect("fixture entry file type").is_dir() {
            copy_tree(&from, &to);
        } else {
            std::fs::copy(&from, &to).expect("copy fixture file");
        }
    }
}

/// Run `ariadne <args...>`; fail unless it exits successfully.
fn run_ok(args: &[&str]) {
    let output = Command::new(BIN)
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

/// Number of `Renders` edges persisted in `<root>/.ariadne/index.redb`.
fn renders_edges(root: &Path) -> usize {
    let storage =
        RedbStorage::open(&root.join(".ariadne").join("index.redb")).expect("open redb index");
    let snapshot = storage.snapshot().expect("open read snapshot");
    snapshot
        .iter_edges(1024)
        .expect("stream edges")
        .flat_map(|chunk| chunk.expect("decode edge chunk"))
        .filter(|(key, _)| key.kind == EdgeKind::Renders)
        .count()
}

/// Copy `fixtures/<family>/` into a tempdir, `init` + `index` it, and return
/// the parsed `IndexSummary` plus the persisted `Renders`-edge count.
fn index_fixture(family: &str) -> (Value, usize) {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path();
    copy_tree(&Path::new(FIXTURES).join(family), root);

    run_ok(&["init", root.to_str().expect("utf8 root")]);
    let output = Command::new(BIN)
        .args(["index", root.to_str().expect("utf8 root")])
        .output()
        .expect("spawn `ariadne index`");
    assert!(
        output.status.success(),
        "`ariadne index` exited with {}: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr),
    );

    let stdout = String::from_utf8(output.stdout).expect("utf8 index stdout");
    let line = stdout
        .lines()
        .last()
        .expect("`ariadne index` emits a JSON summary line");
    let summary: Value = serde_json::from_str(line).expect("index summary is JSON");
    let renders = renders_edges(root);
    (summary, renders)
}

/// Assert `summary.langs` contains `tag`.
fn assert_lang(summary: &Value, tag: &str) {
    let langs = summary["langs"]
        .as_array()
        .expect("summary.langs is an array");
    assert!(
        langs.iter().any(|l| l == tag),
        "expected `{tag}` in summary.langs; got {langs:?}",
    );
}

/// Assert `summary.symbols` is a positive count.
fn assert_symbols(summary: &Value) {
    assert!(
        summary["symbols"]
            .as_u64()
            .expect("summary.symbols is a number")
            > 0,
        "expected symbols > 0; got {}",
        summary["symbols"],
    );
}

#[test]
fn index_vue_tree_reports_vue_with_renders_edge() {
    let (summary, renders) = index_fixture("vue");
    assert_lang(&summary, "vue");
    assert_symbols(&summary);
    assert!(
        summary["edges"]
            .as_u64()
            .expect("summary.edges is a number")
            >= 1,
        "expected edges >= 1; got {}",
        summary["edges"],
    );
    assert!(renders >= 1, "expected >=1 persisted `Renders` edge");
}

#[test]
fn index_svelte_tree_reports_svelte_with_renders_edge() {
    let (summary, renders) = index_fixture("svelte");
    assert_lang(&summary, "svelte");
    assert_symbols(&summary);
    assert!(
        summary["edges"]
            .as_u64()
            .expect("summary.edges is a number")
            >= 1,
        "expected edges >= 1; got {}",
        summary["edges"],
    );
    assert!(renders >= 1, "expected >=1 persisted `Renders` edge");
}

#[test]
fn index_astro_tree_reports_astro_with_renders_edge() {
    let (summary, renders) = index_fixture("astro");
    assert_lang(&summary, "astro");
    assert_symbols(&summary);
    assert!(
        summary["edges"]
            .as_u64()
            .expect("summary.edges is a number")
            >= 1,
        "expected edges >= 1; got {}",
        summary["edges"],
    );
    assert!(renders >= 1, "expected >=1 persisted `Renders` edge");
}

#[test]
fn index_react_tsx_tree_reports_tsx() {
    // The TSX grammar re-route: `.tsx` must tag as `tsx`, not `typescript`
    // [src: tier-05 exit_criteria #2].
    let (summary, _renders) = index_fixture("react");
    assert_lang(&summary, "tsx");
    assert_symbols(&summary);
}
