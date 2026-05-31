//! `Storage::replace_history` round-trip + wholesale-replace semantics
//! [src: .claude/plans/post-v1-roadmap/tier-11-git-history-ingest.md step 8].
//!
//! Asserts the churn + co-change records written by `replace_history` read
//! back identically through `all_churn` / `all_co_change`, that a fresh
//! database reads back empty (the bootstrap-created tables exist), and that a
//! second `replace_history` fully supersedes the first (replace, not merge).

use ariadne_core::{CoChangePair, FileChurn, Storage};
use ariadne_storage::RedbStorage;

fn churn(path: &str, commits: u32, authors: &[[u8; 8]], last_ns: i128) -> FileChurn {
    FileChurn {
        path: path.to_owned(),
        commits,
        author_keys: authors.to_vec(),
        last_changed_ns: last_ns,
    }
}

fn pair(a: &str, b: &str, count: u32) -> CoChangePair {
    CoChangePair {
        a: a.to_owned(),
        b: b.to_owned(),
        count,
    }
}

#[test]
fn fresh_database_reads_back_empty_history() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let storage = RedbStorage::open(&tmp.path().join("index.redb")).expect("open");
    assert!(storage.all_churn().expect("all_churn").is_empty());
    assert!(storage.all_co_change().expect("all_co_change").is_empty());
}

#[test]
fn replace_history_round_trips_records() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let storage = RedbStorage::open(&tmp.path().join("index.redb")).expect("open");

    let churn_in = vec![
        churn("src/a.rs", 3, &[[1u8; 8], [2u8; 8]], 3_000_000_000),
        churn("src/b.rs", 1, &[[1u8; 8]], 1_000_000_000),
        churn("src/c.rs", 1, &[[1u8; 8]], 2_000_000_000),
    ];
    let pairs_in = vec![
        pair("src/a.rs", "src/b.rs", 1),
        pair("src/a.rs", "src/c.rs", 1),
    ];

    storage
        .replace_history(&churn_in, &pairs_in)
        .expect("replace_history");

    // Reads come back deterministically sorted by key (path / pair order).
    let churn_out = storage.all_churn().expect("all_churn");
    assert_eq!(churn_out, churn_in, "churn round-trips byte-identical");

    let pairs_out = storage.all_co_change().expect("all_co_change");
    assert_eq!(pairs_out, pairs_in, "co-change round-trips byte-identical");

    // The `authors()` accessor reports the distinct-author set cardinality.
    assert_eq!(churn_out[0].authors(), 2);
    assert_eq!(churn_out[1].authors(), 1);
}

#[test]
fn replace_history_supersedes_prior_records() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let storage = RedbStorage::open(&tmp.path().join("index.redb")).expect("open");

    storage
        .replace_history(
            &[churn("old.rs", 9, &[[7u8; 8]], 9)],
            &[pair("old.rs", "stale.rs", 4)],
        )
        .expect("first replace");

    // A second replace must wipe the first set entirely (replace, not merge).
    let churn_in = vec![churn("new.rs", 1, &[[1u8; 8]], 1)];
    storage
        .replace_history(&churn_in, &[])
        .expect("second replace");

    assert_eq!(storage.all_churn().expect("all_churn"), churn_in);
    assert!(
        storage.all_co_change().expect("all_co_change").is_empty(),
        "prior co-change pairs cleared",
    );
}
