//! Git-history extraction against a real fixture repository
//! [src: .claude/plans/post-v1-roadmap/tier-11-git-history-ingest.md step 1].
//!
//! Each test builds a throwaway repo with the system `git` (a real `.git` the
//! `gix` adapter reads — no mocks at the module boundary) committing a known
//! sequence, then asserts the per-file commit counts, distinct-author counts,
//! last-changed times, and unordered co-change pairs.

use std::path::Path;
use std::process::Command;

use ariadne_git::{HistoryOptions, HistoryReport, walk_history};
use tempfile::TempDir;

const NANOS_PER_SEC: i128 = 1_000_000_000;

/// Run `git` in `repo` with author/committer identity + date pinned via env,
/// isolated from any ambient user/global config. Panics on non-zero exit.
fn git(repo: &Path, email: &str, unix_secs: i64, args: &[&str]) {
    let ident_date = format!("@{unix_secs} +0000");
    let output = Command::new("git")
        .current_dir(repo)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .env("GIT_AUTHOR_NAME", email)
        .env("GIT_AUTHOR_EMAIL", email)
        .env("GIT_COMMITTER_NAME", email)
        .env("GIT_COMMITTER_EMAIL", email)
        .env("GIT_AUTHOR_DATE", &ident_date)
        .env("GIT_COMMITTER_DATE", &ident_date)
        .args(args)
        .output()
        .expect("spawn git");
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr),
    );
}

/// Initialise an empty repo on branch `main` in a fresh tempdir.
fn init_repo() -> TempDir {
    let tmp = tempfile::tempdir().expect("tempdir");
    git(tmp.path(), "init@x", 0, &["init", "-b", "main"]);
    tmp
}

/// Write files (path, content) then `git add -A` + commit as `email` at
/// `unix_secs`.
fn commit(repo: &Path, email: &str, unix_secs: i64, files: &[(&str, &str)]) {
    for (path, content) in files {
        std::fs::write(repo.join(path), content).expect("write fixture file");
    }
    git(repo, email, unix_secs, &["add", "-A"]);
    git(
        repo,
        email,
        unix_secs,
        &["commit", "-m", "c", "--no-gpg-sign"],
    );
}

fn report(repo: &Path, depth: Option<u32>, max_files_per_commit: u32) -> HistoryReport {
    walk_history(
        repo,
        &HistoryOptions {
            depth,
            max_files_per_commit,
        },
    )
    .expect("walk history")
}

#[test]
fn extracts_file_churn_and_co_change() {
    let repo = init_repo();
    let p = repo.path();
    // alice touches a + b; alice touches a + c; bob touches a.
    commit(
        p,
        "alice@x",
        1_000_000_000,
        &[("a.txt", "1"), ("b.txt", "1")],
    );
    commit(
        p,
        "alice@x",
        1_100_000_000,
        &[("a.txt", "2"), ("c.txt", "1")],
    );
    commit(p, "bob@x", 1_200_000_000, &[("a.txt", "3")]);

    let report = report(p, None, 100);

    // Churn comes back sorted by path: a.txt, b.txt, c.txt.
    let paths: Vec<&str> = report.churn.iter().map(|c| c.path.as_str()).collect();
    assert_eq!(paths, ["a.txt", "b.txt", "c.txt"]);

    let a = &report.churn[0];
    assert_eq!(a.commits, 3, "a.txt touched by all three commits");
    assert_eq!(a.authors(), 2, "a.txt distinct authors: alice + bob");
    assert_eq!(a.last_changed_ns, 1_200_000_000 * NANOS_PER_SEC);

    let b = &report.churn[1];
    assert_eq!((b.commits, b.authors()), (1, 1));
    assert_eq!(b.last_changed_ns, 1_000_000_000 * NANOS_PER_SEC);

    let c = &report.churn[2];
    assert_eq!((c.commits, c.authors()), (1, 1));
    assert_eq!(c.last_changed_ns, 1_100_000_000 * NANOS_PER_SEC);

    // Co-change: (a,b) from commit 1, (a,c) from commit 2; commit 3 is a
    // single-file commit and contributes no pair.
    let pairs: Vec<(&str, &str, u32)> = report
        .pairs
        .iter()
        .map(|p| (p.a.as_str(), p.b.as_str(), p.count))
        .collect();
    assert_eq!(
        pairs,
        [("a.txt", "b.txt", 1), ("a.txt", "c.txt", 1)],
        "unordered co-change pairs, sorted",
    );
}

#[test]
fn excludes_large_commits_from_co_change() {
    let repo = init_repo();
    let p = repo.path();
    // A small 2-file commit, then a 5-file commit exceeding max_files=4.
    commit(
        p,
        "alice@x",
        1_000_000_000,
        &[("x.txt", "1"), ("y.txt", "1")],
    );
    commit(
        p,
        "alice@x",
        1_100_000_000,
        &[
            ("f1", "1"),
            ("f2", "1"),
            ("f3", "1"),
            ("f4", "1"),
            ("f5", "1"),
        ],
    );

    let report = report(p, None, 4);

    // Churn still counts every file in the large commit.
    assert_eq!(report.churn.len(), 7, "2 + 5 files all counted for churn");
    assert!(report.churn.iter().all(|c| c.commits == 1));

    // Co-change keeps only the small commit's pair; the 5-file commit (10
    // pairs) is excluded as coupling noise.
    let pairs: Vec<(&str, &str, u32)> = report
        .pairs
        .iter()
        .map(|p| (p.a.as_str(), p.b.as_str(), p.count))
        .collect();
    assert_eq!(pairs, [("x.txt", "y.txt", 1)]);
}

#[test]
fn depth_bounds_the_walk_to_most_recent_commits() {
    let repo = init_repo();
    let p = repo.path();
    commit(
        p,
        "alice@x",
        1_000_000_000,
        &[("a.txt", "1"), ("b.txt", "1")],
    );
    commit(
        p,
        "alice@x",
        1_100_000_000,
        &[("a.txt", "2"), ("c.txt", "1")],
    );
    commit(p, "bob@x", 1_200_000_000, &[("a.txt", "3")]);

    // depth = 1 walks only the newest commit (bob's single-file a.txt change).
    let report = report(p, Some(1), 100);
    assert_eq!(report.churn.len(), 1, "only the most-recent commit walked");
    assert_eq!(report.churn[0].path, "a.txt");
    assert_eq!(report.churn[0].commits, 1);
    assert!(
        report.pairs.is_empty(),
        "single-file commit yields no pairs"
    );
}

#[test]
fn records_full_blob_paths_and_skips_directory_entries() {
    let repo = init_repo();
    let p = repo.path();
    std::fs::create_dir_all(p.join("pkg/sub")).expect("mkdir");
    // Nested files in a new directory tree: the recursive diff also emits
    // tree (directory) entries, which must NOT be counted as churned files.
    commit(
        p,
        "alice@x",
        1_000_000_000,
        &[("top.txt", "1"), ("pkg/a.rs", "1"), ("pkg/sub/b.rs", "1")],
    );

    let report = report(p, None, 100);
    let paths: Vec<&str> = report.churn.iter().map(|c| c.path.as_str()).collect();
    assert_eq!(
        paths,
        ["pkg/a.rs", "pkg/sub/b.rs", "top.txt"],
        "only blob leaves with full paths — no `pkg` / `pkg/sub` directory entries",
    );
}
