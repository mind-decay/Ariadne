//! Block A, A2 — `api-diff` end-to-end golden over the real pipeline.
//!
//! Builds a throwaway two-commit git repo whose base commit exposes three public
//! items across two files (`removed_item`, `changed_item`, `stable`) and whose
//! head commit removes one, signature-changes one, adds one, and leaves one
//! stable. Drives the real `ariadne api-diff HEAD~1..HEAD` binary — which runs
//! entirely in-process (git diff → base/head blob reads → parser surface
//! extraction → pure classify, no index, no daemon; D6 / ADR-0027) — and asserts
//! the verdict is `major` with the exact added/removed/changed lists. The
//! changeset spans two files (BR3 multi-file bound), the report is asserted
//! byte-identical across two runs (determinism), and the run is asserted under
//! the 500ms incremental budget. Mirrors the A1 `affected_tests` golden + the
//! `incremental_history` real-pipeline precedent [src:
//! crates/ariadne-e2e/tests/affected_tests.rs;
//! crates/ariadne-cli/tests/incremental_history.rs].

use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

use ariadne_e2e::domain::ariadne_binary;
use serde_json::Value;
use tempfile::TempDir;

/// The 500ms incremental-update budget the multi-file diff must clear (BR3).
const INCREMENTAL_BUDGET: Duration = Duration::from_millis(500);

/// Base (HEAD~1) `src/lib.rs`: a public item that will be removed + a stable one.
const BASE_LIB: &str = "pub fn removed_item() {}\npub fn stable() {}\n";
/// Head (HEAD) `src/lib.rs`: `removed_item` gone, `stable` kept, `added_item` new.
const HEAD_LIB: &str = "pub fn stable() {}\npub fn added_item() {}\n";
/// Base (HEAD~1) `src/util.rs`: a public item whose signature will change.
const BASE_UTIL: &str = "pub fn changed_item(x: u32) -> u32 {\n    x\n}\n";
/// Head (HEAD) `src/util.rs`: the same item with a changed signature.
const HEAD_UTIL: &str = "pub fn changed_item(x: u64) -> u64 {\n    x\n}\n";

/// Run `git` in `repo`, isolated from ambient config; panics on non-zero exit.
fn git(repo: &Path, args: &[&str]) {
    let output = Command::new("git")
        .current_dir(repo)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@x")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@x")
        .args(args)
        .output()
        .expect("spawn git");
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr),
    );
}

/// Write the two source files, stage, and commit with message `msg`.
fn commit(root: &Path, msg: &str, lib: &str, util: &str) {
    std::fs::write(root.join("src/lib.rs"), lib).expect("write lib");
    std::fs::write(root.join("src/util.rs"), util).expect("write util");
    git(root, &["add", "-A"]);
    git(root, &["commit", "-m", msg, "--no-gpg-sign"]);
}

/// Build the two-commit fixture repo: base commit (HEAD~1) then head commit
/// (HEAD), each touching both files.
fn seed_fixture() -> TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path();
    git(root, &["init", "-b", "main"]);
    std::fs::create_dir_all(root.join("src")).expect("mkdir src");
    commit(root, "base", BASE_LIB, BASE_UTIL);
    commit(root, "head", HEAD_LIB, HEAD_UTIL);
    dir
}

/// Run `ariadne api-diff HEAD~1..HEAD --root <root>`, returning stdout. Fails the
/// test on a non-zero exit.
fn api_diff(root: &Path) -> String {
    let output = Command::new(ariadne_binary())
        .args(["api-diff", "HEAD~1..HEAD", "--root"])
        .arg(root)
        .output()
        .expect("spawn `ariadne api-diff`");
    assert!(
        output.status.success(),
        "ariadne api-diff failed: {}",
        String::from_utf8_lossy(&output.stderr).trim(),
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// Canonical `name` values under `key` (an array of symbol rows) in the report.
fn names(report: &Value, key: &str) -> Vec<String> {
    report[key]
        .as_array()
        .unwrap_or_else(|| panic!("`{key}` is not an array: {report}"))
        .iter()
        .filter_map(|row| row["name"].as_str().map(str::to_owned))
        .collect()
}

#[test]
fn api_diff_classifies_a_breaking_two_commit_change() {
    let dir = seed_fixture();
    let root = dir.path();

    // First (cold) run: capture the report and warm the binary. This spawn
    // pays the one-time process-start cost (process creation + dynamic linker +
    // binary page-in) so the timed run below measures the api-diff computation,
    // not a cold subprocess spawn — mirroring the slo.rs warm-path precedent
    // [src: crates/ariadne-e2e/tests/slo.rs measure_query].
    let out = api_diff(root);
    let report: Value = serde_json::from_str(&out).expect("decode api-diff report");

    // A removed and a signature-changed public item make the verdict major.
    assert_eq!(
        report["verdict"].as_str(),
        Some("major"),
        "removed + signature-changed ⇒ major: {report}",
    );

    // Exact lists: one added, one removed, one changed.
    assert_eq!(names(&report, "added"), vec!["added_item".to_owned()]);
    assert_eq!(names(&report, "removed"), vec!["removed_item".to_owned()]);

    let changed = report["changed"].as_array().expect("changed array");
    assert_eq!(changed.len(), 1, "exactly one signature change: {report}");
    let change = &changed[0];
    assert_eq!(change["name"].as_str(), Some("changed_item"));
    assert_eq!(
        change["base_signature"].as_str(),
        Some("pub fn changed_item(x: u32) -> u32"),
    );
    assert_eq!(
        change["head_signature"].as_str(),
        Some("pub fn changed_item(x: u64) -> u64"),
    );

    // Determinism: a second run is byte-identical. Time *this* warm run so the
    // BR3 budget measures the re-parse cost over the multi-file diff, not the
    // cold process-start cost paid by the first spawn above.
    let started = Instant::now();
    let rerun = api_diff(root);
    let elapsed = started.elapsed();
    assert_eq!(out, rerun, "api-diff is deterministic across runs");

    // BR3: the multi-file diff clears the 500ms incremental budget.
    assert!(
        elapsed < INCREMENTAL_BUDGET,
        "api-diff took {elapsed:?}, over the {INCREMENTAL_BUDGET:?} budget",
    );
}
