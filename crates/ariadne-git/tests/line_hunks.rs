//! Per-commit new-side line-hunk extraction against a real fixture repository
//! [src: .claude/plans/post-v1-roadmap/tier-11b-symbol-churn-attribution.md step 2].
//!
//! Each test builds a throwaway repo with the system `git` (a real `.git` the
//! `gix` adapter reads — no mocks at the module boundary), then asserts the
//! new-side changed line ranges `walk_line_hunks` derives via `blob-diff`.

use std::path::Path;
use std::process::Command;

use ariadne_git::walk_line_hunks;
use tempfile::TempDir;

/// Run `git` in `repo`, isolated from any ambient user/global config so the
/// fixture is reproducible on any host. Panics on non-zero exit.
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

fn init_repo() -> TempDir {
    let tmp = tempfile::tempdir().expect("tempdir");
    git(tmp.path(), &["init", "-b", "main"]);
    tmp
}

fn commit(repo: &Path, path: &str, content: &str) {
    std::fs::write(repo.join(path), content).expect("write fixture");
    git(repo, &["add", path]);
    git(repo, &["commit", "-m", "c", "--no-gpg-sign"]);
}

#[test]
fn emits_new_side_line_hunks_for_added_then_modified_blob() {
    let repo = init_repo();
    let p = repo.path();
    // Commit 1 adds a 4-line file; commit 2 changes only line 2.
    commit(p, "a.txt", "l1\nl2\nl3\nl4\n");
    commit(p, "a.txt", "l1\nCHANGED\nl3\nl4\n");

    let hunks = walk_line_hunks(p, None).expect("walk line hunks");
    assert_eq!(hunks.len(), 2, "two commits walked");

    // Newest-first: the edit commit yields one new-side hunk on line 2.
    let edit = &hunks[0];
    assert_eq!(edit.len(), 1, "single modified line");
    assert_eq!(edit[0].path, "a.txt");
    assert_eq!((edit[0].start_line, edit[0].end_line), (2, 2));

    // The root (addition) commit: the whole 4-line file is the new side.
    let add = &hunks[1];
    assert_eq!(add.len(), 1, "one contiguous addition hunk");
    assert_eq!((add[0].start_line, add[0].end_line), (1, 4));
}

#[test]
fn deletion_contributes_no_new_side_hunk() {
    let repo = init_repo();
    let p = repo.path();
    commit(p, "gone.txt", "x\ny\n");
    git(p, &["rm", "gone.txt"]);
    git(p, &["commit", "-m", "rm", "--no-gpg-sign"]);

    let hunks = walk_line_hunks(p, None).expect("walk line hunks");
    assert_eq!(hunks.len(), 2);
    // Newest commit is the deletion — no new-side lines exist for it.
    assert!(
        hunks[0].is_empty(),
        "a deletion has no new-side line to attribute",
    );
}

#[test]
fn depth_bounds_the_walk_to_most_recent_commits() {
    let repo = init_repo();
    let p = repo.path();
    commit(p, "a.txt", "one\n");
    commit(p, "a.txt", "two\n");
    commit(p, "a.txt", "three\n");

    // depth = 1 walks only the newest commit.
    let hunks = walk_line_hunks(p, Some(1)).expect("walk line hunks");
    assert_eq!(hunks.len(), 1, "only the most-recent commit walked");
    assert_eq!(hunks[0].len(), 1);
    assert_eq!((hunks[0][0].start_line, hunks[0][0].end_line), (1, 1));
}
