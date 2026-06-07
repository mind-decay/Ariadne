//! Block A, A1 — `affected_tests` end-to-end golden over the real pipeline.
//!
//! Builds a throwaway git repo whose committed `src/lib.rs` differs from the
//! worktree on the line inside `target`, indexes the *worktree* layout with the
//! real `ariadne index`, then drives the real `ariadne affected-tests` binary on
//! the default `working_tree` spec (cold path, daemon auto-spawn off). The
//! uncommitted edit resolves to the `target` seed; its reverse-reachable test
//! `checks_target` (a `#[test]` calling `target`) is the hand-verified affected
//! set. Re-running the query is asserted byte-identical (determinism), mirroring
//! the diff-blast cold golden + the `incremental_history` real-pipeline
//! precedent [src: crates/ariadne-mcp/tests/tools_diff_blast.rs;
//!  crates/ariadne-cli/tests/incremental_history.rs].

use std::path::Path;
use std::process::Command;

use ariadne_e2e::domain::{ariadne_binary, run_init};
use serde_json::Value;
use tempfile::TempDir;

/// Auto-spawn switch the CLI daemon client reads; `"0"` pins the cold path
/// [src: crates/ariadne-cli/src/adapters/daemon_client.rs].
const AUTOSPAWN_ENV: &str = "ARIADNE_CLI_AUTOSPAWN";

/// Committed (HEAD) body of `target` — line 2 is `1`.
const HEAD_LIB: &str = "pub fn target() -> i32 {\n    1\n}\n";
/// Worktree (uncommitted) body of `target` — line 2 is `2`.
const WORKTREE_LIB: &str = "pub fn target() -> i32 {\n    2\n}\n";
/// A test that calls `target` directly (a free, non-macro call the resolver
/// binds cross-file), so a change to `target` reverse-reaches `checks_target`.
const CHECK_RS: &str =
    "use crate::target;\n\n#[test]\nfn checks_target() {\n    let _ = target();\n}\n";

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

/// Build the fixture repo + index: commit the HEAD `lib.rs`/`check.rs`, then
/// apply the uncommitted worktree edit, and index the worktree layout (so the
/// index matches the worktree the daemon/cold path reads).
fn seed_fixture() -> TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path();

    git(root, &["init", "-b", "main"]);
    std::fs::create_dir_all(root.join("src")).expect("mkdir src");
    std::fs::write(root.join("src/lib.rs"), HEAD_LIB).expect("write head lib");
    std::fs::write(root.join("src/check.rs"), CHECK_RS).expect("write check");
    git(root, &["add", "-A"]);
    git(root, &["commit", "-m", "c0", "--no-gpg-sign"]);

    // Uncommitted worktree edit (the change `working_tree` scopes).
    std::fs::write(root.join("src/lib.rs"), WORKTREE_LIB).expect("write worktree lib");

    run_init(root).expect("ariadne init");
    let index = Command::new(ariadne_binary())
        .args(["index", "--no-scip"])
        .arg(root)
        .output()
        .expect("spawn `ariadne index`");
    assert!(
        index.status.success(),
        "ariadne index failed: {}",
        String::from_utf8_lossy(&index.stderr).trim(),
    );
    dir
}

/// Run `ariadne affected-tests working_tree --root <root>` on the cold path,
/// returning stdout. Fails the test on a non-zero exit.
fn affected_tests(root: &Path) -> String {
    let output = Command::new(ariadne_binary())
        .args(["affected-tests", "working_tree", "--root"])
        .arg(root)
        .env(AUTOSPAWN_ENV, "0")
        .output()
        .expect("spawn `ariadne affected-tests`");
    assert!(
        output.status.success(),
        "ariadne affected-tests failed: {}",
        String::from_utf8_lossy(&output.stderr).trim(),
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// Canonical names under `key` in an `affected_tests` JSON report.
fn names(report: &Value, key: &str) -> Vec<String> {
    report[key]
        .as_array()
        .unwrap_or_else(|| panic!("`{key}` is not an array: {report}"))
        .iter()
        .filter_map(|row| row["name"].as_str().map(str::to_owned))
        .collect()
}

#[test]
fn affected_tests_working_tree_returns_the_reachable_test() {
    let dir = seed_fixture();
    let root = dir.path();

    let out = affected_tests(root);
    let report: Value = serde_json::from_str(&out).expect("decode affected_tests report");

    // The hunk inside `target` seeds `target`; its only reverse-reachable test
    // is `checks_target`.
    assert_eq!(
        names(&report, "tests"),
        vec!["checks_target".to_owned()],
        "exactly the reachable test is affected",
    );
    assert_eq!(
        names(&report, "seeds"),
        vec!["target".to_owned()],
        "the changed line resolves to the `target` seed",
    );
    assert!(
        report["unresolved"]
            .as_array()
            .expect("unresolved array")
            .is_empty(),
        "the changed file owns a seed, so nothing is unresolved",
    );

    // Determinism: a second run over the same index is byte-identical.
    assert_eq!(
        out,
        affected_tests(root),
        "affected_tests is deterministic across runs",
    );
}
