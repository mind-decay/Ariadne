//! Incremental Git-history walk: `walk_since` visits only commits newer than a
//! watermark, returns the current HEAD oid to persist, and falls back to a full
//! walk when the watermark is unreachable (force-push / rebase)
//! [src: .claude/plans/post-v1-roadmap/tier-11a-incremental-history.md steps 1, 3].
//!
//! Each test builds a throwaway repo with the system `git` (a real `.git` the
//! `gix` adapter reads — no mocks at the module boundary).

use std::path::Path;
use std::process::Command;

use ariadne_git::{HistoryOptions, walk_history, walk_since};
use tempfile::TempDir;

/// Run `git` with author/committer identity + date pinned, isolated from any
/// ambient config. Panics on non-zero exit.
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

fn init_repo() -> TempDir {
    let tmp = tempfile::tempdir().expect("tempdir");
    git(tmp.path(), "init@x", 0, &["init", "-b", "main"]);
    tmp
}

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

/// Rewrite the current tip in place (`--amend`): the prior tip becomes a
/// dangling commit no longer reachable from HEAD — a force-push / rebase.
fn amend(repo: &Path, email: &str, unix_secs: i64, files: &[(&str, &str)]) {
    for (path, content) in files {
        std::fs::write(repo.join(path), content).expect("write fixture file");
    }
    git(repo, email, unix_secs, &["add", "-A"]);
    git(
        repo,
        email,
        unix_secs,
        &["commit", "--amend", "-m", "c", "--no-gpg-sign"],
    );
}

fn opts() -> HistoryOptions {
    HistoryOptions {
        depth: None,
        max_files_per_commit: 100,
    }
}

#[test]
fn walk_since_none_matches_full_walk() {
    let repo = init_repo();
    let p = repo.path();
    commit(
        p,
        "alice@x",
        1_000_000_000,
        &[("a.txt", "1"), ("b.txt", "1")],
    );
    commit(p, "bob@x", 1_100_000_000, &[("a.txt", "2")]);

    let full = walk_history(p, &opts()).expect("full walk");
    let inc = walk_since(p, &opts(), None).expect("walk_since none");

    assert!(
        !inc.incremental,
        "no watermark -> full walk, not incremental"
    );
    assert_eq!(inc.report.churn, full.churn);
    assert_eq!(inc.report.pairs, full.pairs);
    assert!(
        inc.head_oid.is_some(),
        "HEAD oid returned to persist as the watermark",
    );
}

#[test]
fn walk_since_visits_only_commits_after_watermark() {
    let repo = init_repo();
    let p = repo.path();
    // K = 2 base commits touching a.txt / b.txt.
    commit(
        p,
        "alice@x",
        1_000_000_000,
        &[("a.txt", "1"), ("b.txt", "1")],
    );
    commit(p, "alice@x", 1_100_000_000, &[("a.txt", "2")]);
    let watermark = walk_since(p, &opts(), None)
        .expect("base walk")
        .head_oid
        .expect("base HEAD oid");

    // N = 1 new commit touching only c.txt + d.txt.
    commit(
        p,
        "carol@x",
        1_200_000_000,
        &[("c.txt", "1"), ("d.txt", "1")],
    );
    let inc = walk_since(p, &opts(), Some(&watermark)).expect("incremental walk");

    assert!(inc.incremental, "watermark is an ancestor of HEAD");
    let paths: Vec<&str> = inc.report.churn.iter().map(|c| c.path.as_str()).collect();
    assert_eq!(
        paths,
        ["c.txt", "d.txt"],
        "delta covers only the new commit's files — a.txt/b.txt are pre-watermark",
    );
    assert!(inc.report.churn.iter().all(|c| c.commits == 1));
    let pairs: Vec<(&str, &str, u32)> = inc
        .report
        .pairs
        .iter()
        .map(|p| (p.a.as_str(), p.b.as_str(), p.count))
        .collect();
    assert_eq!(pairs, [("c.txt", "d.txt", 1)]);
    assert_ne!(
        inc.head_oid.as_deref(),
        Some(watermark.as_slice()),
        "HEAD advanced past the watermark",
    );
}

#[test]
fn walk_since_unreachable_watermark_falls_back_to_full() {
    let repo = init_repo();
    let p = repo.path();
    commit(p, "alice@x", 1_000_000_000, &[("a.txt", "1")]);
    commit(
        p,
        "alice@x",
        1_100_000_000,
        &[("a.txt", "2"), ("b.txt", "1")],
    );
    let stale = walk_since(p, &opts(), None)
        .expect("base walk")
        .head_oid
        .expect("base HEAD oid");

    // Rewrite the tip: `stale` is no longer an ancestor of HEAD.
    amend(p, "alice@x", 1_150_000_000, &[("a.txt", "9")]);
    let inc = walk_since(p, &opts(), Some(&stale)).expect("walk_since after rewrite");

    assert!(
        !inc.incremental,
        "rewritten history -> watermark unreachable -> full walk",
    );
    let full = walk_history(p, &opts()).expect("full walk");
    assert_eq!(
        inc.report.churn, full.churn,
        "fallback report equals a full cold walk — no corruption",
    );
    assert_eq!(inc.report.pairs, full.pairs);
}
