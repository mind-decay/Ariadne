//! End-to-end incremental Git-history ingest at the composition root: a
//! watermarked incremental walk merged into redb yields `CHURN`/`CO_CHANGE`
//! byte-identical to a full cold walk over the whole history (divergence 0),
//! and a force-pushed history falls back to a full replace
//! [src: .claude/plans/post-v1-roadmap/tier-11a-incremental-history.md
//!  `exit_criteria` 1-3].
//!
//! Composes the real `ariadne-git` walker with the real `ariadne-storage` redb
//! adapter (no mocks at the module boundary) — `ariadne-cli` is the only crate
//! depending on both, mirroring `commands::index::refresh_history`.

use std::path::Path;
use std::process::Command;

use ariadne_core::Storage;
use ariadne_git::{HistoryOptions, walk_history, walk_since};
use ariadne_storage::RedbStorage;
use tempfile::TempDir;

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

fn open(dir: &TempDir) -> RedbStorage {
    RedbStorage::open(&dir.path().join("index.redb")).expect("open redb")
}

/// Mirror of `commands::index::refresh_history`: full replace on an invalid
/// watermark, incremental merge on a valid one.
fn refresh(storage: &RedbStorage, root: &Path) {
    let watermark = storage.last_ingested_commit().expect("read watermark");
    let walk = walk_since(root, &opts(), watermark.as_deref()).expect("walk_since");
    let Some(head) = walk.head_oid else {
        return;
    };
    if walk.incremental {
        storage
            .merge_history(&walk.report.churn, &walk.report.pairs, &head)
            .expect("merge");
    } else {
        storage
            .replace_history(&walk.report.churn, &walk.report.pairs)
            .expect("replace");
        storage
            .set_last_ingested_commit(&head)
            .expect("set watermark");
    }
}

/// Open a fresh redb and cold-walk the whole history into it in one shot.
fn full_cold(dir: &TempDir, root: &Path) -> RedbStorage {
    let storage = open(dir);
    let report = walk_history(root, &opts()).expect("full walk");
    storage
        .replace_history(&report.churn, &report.pairs)
        .expect("replace full");
    storage
}

#[test]
fn incremental_ingest_equals_full_cold_walk() {
    let repo = init_repo();
    let p = repo.path();
    commit(
        p,
        "alice@x",
        1_000_000_000,
        &[("a.txt", "1"), ("b.txt", "1")],
    );
    commit(p, "alice@x", 1_100_000_000, &[("a.txt", "2")]);

    // Incremental store: cold-ingest the first K, then merge N new commits.
    let inc_dir = tempfile::tempdir().expect("tempdir");
    let inc = open(&inc_dir);
    refresh(&inc, p);

    commit(p, "bob@x", 1_200_000_000, &[("a.txt", "3"), ("c.txt", "1")]);
    commit(p, "carol@x", 1_300_000_000, &[("c.txt", "2")]);
    refresh(&inc, p);

    // Full store: a single cold walk over all K + N commits.
    let full_dir = tempfile::tempdir().expect("tempdir");
    let full = full_cold(&full_dir, p);

    assert_eq!(
        inc.all_churn().expect("inc churn"),
        full.all_churn().expect("full churn"),
        "churn divergence 0",
    );
    assert_eq!(
        inc.all_co_change().expect("inc co_change"),
        full.all_co_change().expect("full co_change"),
        "co-change divergence 0",
    );

    let head = walk_since(p, &opts(), None)
        .expect("head walk")
        .head_oid
        .expect("head oid");
    assert_eq!(
        inc.last_ingested_commit().expect("watermark"),
        Some(head),
        "watermark advanced to the current HEAD",
    );
}

#[test]
fn force_pushed_history_falls_back_to_full_replace() {
    let repo = init_repo();
    let p = repo.path();
    commit(p, "alice@x", 1_000_000_000, &[("a.txt", "1")]);
    commit(
        p,
        "alice@x",
        1_100_000_000,
        &[("a.txt", "2"), ("b.txt", "1")],
    );

    let dir = tempfile::tempdir().expect("tempdir");
    let storage = open(&dir);
    refresh(&storage, p);

    // Rewrite the tip: the stored watermark is no longer an ancestor of HEAD.
    amend(p, "alice@x", 1_150_000_000, &[("a.txt", "9")]);
    refresh(&storage, p);

    let expected_dir = tempfile::tempdir().expect("tempdir");
    let expected = full_cold(&expected_dir, p);
    assert_eq!(
        storage.all_churn().expect("churn"),
        expected.all_churn().expect("expected churn"),
        "no corruption: equals a full cold walk over the rewritten history",
    );
    assert_eq!(
        storage.all_co_change().expect("co_change"),
        expected.all_co_change().expect("expected co_change"),
    );
}
