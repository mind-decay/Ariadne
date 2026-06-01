//! `Storage::merge_history` additive merge + the HEAD-commit watermark
//! (`last_ingested_commit` / `set_last_ingested_commit`) round-trip and its
//! atomic advance inside `merge_history`
//! [src: .claude/plans/post-v1-roadmap/tier-11a-incremental-history.md steps 2, 4].

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

fn fresh() -> (RedbStorage, tempfile::TempDir) {
    let tmp = tempfile::tempdir().expect("tempdir");
    let storage = RedbStorage::open(&tmp.path().join("index.redb")).expect("open");
    (storage, tmp)
}

#[test]
fn watermark_absent_until_first_write_then_round_trips() {
    let (storage, _tmp) = fresh();
    assert_eq!(
        storage.last_ingested_commit().expect("read watermark"),
        None,
        "no watermark before the first ingest",
    );
    let oid = [0xabu8; 20];
    storage
        .set_last_ingested_commit(&oid)
        .expect("set watermark");
    assert_eq!(
        storage
            .last_ingested_commit()
            .expect("read watermark")
            .as_deref(),
        Some(&oid[..]),
    );
}

#[test]
fn merge_history_adds_counts_unions_authors_maxes_time_and_advances_watermark() {
    let (storage, _tmp) = fresh();
    let a1 = [1u8; 8];
    let a2 = [2u8; 8];

    // Base window: a.txt seen twice by a1 (last 100), b.txt once; pair (a,b)=2.
    storage
        .replace_history(
            &[churn("a.txt", 2, &[a1], 100), churn("b.txt", 1, &[a1], 90)],
            &[pair("a.txt", "b.txt", 2)],
        )
        .expect("replace base");
    storage
        .set_last_ingested_commit(&[0x11u8; 20])
        .expect("set base watermark");

    // Delta window: a.txt +1 by a2 (last 150), new c.txt; pair (a,b)+1, new (a,c)=1.
    let head = [0x22u8; 20];
    storage
        .merge_history(
            &[churn("a.txt", 1, &[a2], 150), churn("c.txt", 1, &[a2], 150)],
            &[pair("a.txt", "b.txt", 1), pair("a.txt", "c.txt", 1)],
            &head,
        )
        .expect("merge delta");

    let all = storage.all_churn().expect("all_churn");
    let a = all.iter().find(|c| c.path == "a.txt").expect("a.txt");
    assert_eq!(a.commits, 3, "commit counts add: 2 + 1");
    assert_eq!(a.author_keys, vec![a1, a2], "author keys union, sorted");
    assert_eq!(a.last_changed_ns, 150, "last-changed takes the max");
    assert_eq!(
        all.iter()
            .find(|c| c.path == "b.txt")
            .expect("b.txt")
            .commits,
        1,
        "untouched file unchanged",
    );
    assert_eq!(
        all.iter()
            .find(|c| c.path == "c.txt")
            .expect("c.txt")
            .commits,
        1,
        "new file inserted",
    );

    let pairs = storage.all_co_change().expect("all_co_change");
    let ab = pairs
        .iter()
        .find(|p| p.a == "a.txt" && p.b == "b.txt")
        .expect("(a,b)");
    assert_eq!(ab.count, 3, "co-change counts add: 2 + 1");
    assert!(
        pairs
            .iter()
            .any(|p| p.a == "a.txt" && p.b == "c.txt" && p.count == 1),
        "new pair inserted",
    );

    assert_eq!(
        storage
            .last_ingested_commit()
            .expect("read watermark")
            .as_deref(),
        Some(&head[..]),
        "watermark advanced atomically with the merge",
    );
}

#[test]
fn merge_history_dedups_repeated_authors() {
    let (storage, _tmp) = fresh();
    let a1 = [7u8; 8];
    storage
        .replace_history(&[churn("x.txt", 1, &[a1], 10)], &[])
        .expect("replace base");
    storage
        .merge_history(&[churn("x.txt", 1, &[a1], 20)], &[], &[0x33u8; 20])
        .expect("merge delta");

    let all = storage.all_churn().expect("all_churn");
    let x = all.iter().find(|c| c.path == "x.txt").expect("x.txt");
    assert_eq!(x.commits, 2);
    assert_eq!(
        x.author_keys,
        vec![a1],
        "the same author seen in both windows is not double-counted",
    );
}
