//! Diff-source reader against a real fixture repository
//! [src: .claude/plans/post-v1-roadmap/tier-14-diff-aware-blast-radius.md step 6].
//!
//! Each test builds a throwaway repo with the system `git` (a real `.git` the
//! `gix` adapter reads — no mocks at the module boundary), then asserts the
//! changed paths + new-side line hunks `diff` derives for each of the three
//! `DiffSpec` kinds, including an uncommitted worktree edit (`WorkingTree`).

use std::path::Path;
use std::process::Command;

use ariadne_core::{DiffSpec, LineHunk};
use ariadne_git::diff;
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

fn write(repo: &Path, path: &str, content: &str) {
    std::fs::write(repo.join(path), content).expect("write fixture");
}

fn commit(repo: &Path, path: &str, content: &str) {
    write(repo, path, content);
    git(repo, &["add", path]);
    git(repo, &["commit", "-m", "c", "--no-gpg-sign"]);
}

/// A three-commit fixture: c1 adds a 4-line `a.txt`; c2 modifies its line 2;
/// c3 (HEAD) adds `b.txt`.
fn three_commit_repo() -> TempDir {
    let tmp = tempfile::tempdir().expect("tempdir");
    let p = tmp.path();
    git(p, &["init", "-b", "main"]);
    commit(p, "a.txt", "l1\nl2\nl3\nl4\n");
    commit(p, "a.txt", "l1\nCHANGED\nl3\nl4\n");
    commit(p, "b.txt", "x\ny\n");
    tmp
}

fn span(h: &LineHunk) -> (&str, u32, u32) {
    (h.path.as_str(), h.start_line, h.end_line)
}

#[test]
fn commit_kind_diffs_against_first_parent() {
    let repo = three_commit_repo();
    // HEAD is c3, which only adds `b.txt`.
    let (hunks, paths) = diff(repo.path(), &DiffSpec::Commit("HEAD".to_owned())).expect("diff");

    assert_eq!(
        paths,
        vec!["b.txt".to_owned()],
        "only b.txt changed in HEAD"
    );
    assert_eq!(hunks.len(), 1, "one addition hunk");
    assert_eq!(span(&hunks[0]), ("b.txt", 1, 2), "whole 2-line addition");
}

#[test]
fn ref_range_diffs_the_two_resolved_trees() {
    let repo = three_commit_repo();
    // c1 (HEAD~2) → HEAD spans the a.txt line-2 modification AND the b.txt add.
    let spec = DiffSpec::RefRange {
        from: "HEAD~2".to_owned(),
        to: "HEAD".to_owned(),
    };
    let (hunks, paths) = diff(repo.path(), &spec).expect("diff");

    assert_eq!(
        paths,
        vec!["a.txt".to_owned(), "b.txt".to_owned()],
        "both files changed across the range",
    );
    // Sorted by (path, start, end): a.txt line 2, then b.txt lines 1-2.
    let spans: Vec<(&str, u32, u32)> = hunks.iter().map(span).collect();
    assert_eq!(spans, vec![("a.txt", 2, 2), ("b.txt", 1, 2)]);
}

#[test]
fn working_tree_kind_reports_uncommitted_edit() {
    let repo = three_commit_repo();
    let p = repo.path();
    // Edit a.txt line 3 in the worktree only — no `git add`, no commit.
    write(p, "a.txt", "l1\nCHANGED\nMODIFIED\nl4\n");

    let (hunks, paths) = diff(p, &DiffSpec::WorkingTree).expect("diff");

    assert_eq!(
        paths,
        vec!["a.txt".to_owned()],
        "the uncommitted worktree edit is the only change",
    );
    assert_eq!(hunks.len(), 1, "single modified line");
    assert_eq!(
        span(&hunks[0]),
        ("a.txt", 3, 3),
        "line 3 is the new-side hunk"
    );
}

#[test]
fn working_tree_kind_reports_staged_change() {
    let repo = three_commit_repo();
    let p = repo.path();
    // Stage an edit to a.txt line 1, then leave the worktree byte-identical to
    // the index (no further edit). The index-vs-worktree leg sees nothing; only
    // the head-vs-index status leg surfaces this path.
    write(p, "a.txt", "STAGED\nCHANGED\nl3\nl4\n");
    git(p, &["add", "a.txt"]);

    let (hunks, paths) = diff(p, &DiffSpec::WorkingTree).expect("diff");

    assert_eq!(
        paths,
        vec!["a.txt".to_owned()],
        "the staged edit is the only change",
    );
    assert_eq!(hunks.len(), 1, "single modified line");
    assert_eq!(
        span(&hunks[0]),
        ("a.txt", 1, 1),
        "line 1 is the new-side hunk (head-vs-index leg)"
    );
}

#[test]
fn working_tree_clean_repo_yields_no_changes() {
    let repo = three_commit_repo();
    // No worktree edits: a clean tree has no uncommitted changeset.
    let (hunks, paths) = diff(repo.path(), &DiffSpec::WorkingTree).expect("diff");
    assert!(paths.is_empty(), "clean worktree has no changed paths");
    assert!(hunks.is_empty(), "clean worktree has no hunks");
}
