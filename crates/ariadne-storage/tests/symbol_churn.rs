//! `Storage::replace_symbol_churn` round-trip + wholesale-replace semantics
//! [src: .claude/plans/post-v1-roadmap/tier-11b-symbol-churn-attribution.md step 5].
//!
//! Asserts the per-symbol churn written by `replace_symbol_churn` reads back
//! identically through `all_symbol_churn` (sorted by `SymbolId`), that a fresh
//! database reads back empty (the bootstrap-created table exists), and that a
//! second `replace_symbol_churn` fully supersedes the first (replace, not
//! merge).

use ariadne_core::{Storage, SymbolChurn, SymbolId};
use ariadne_storage::RedbStorage;

fn sid(n: u64) -> SymbolId {
    SymbolId::new(n).expect("nonzero symbol id")
}

fn churn(symbol: u64, commits: u32) -> SymbolChurn {
    SymbolChurn {
        symbol: sid(symbol),
        commits,
    }
}

#[test]
fn fresh_database_reads_back_empty_symbol_churn() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let storage = RedbStorage::open(&tmp.path().join("index.redb")).expect("open");
    assert!(
        storage
            .all_symbol_churn()
            .expect("all_symbol_churn")
            .is_empty()
    );
}

#[test]
fn replace_symbol_churn_round_trips_records_sorted_by_id() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let storage = RedbStorage::open(&tmp.path().join("index.redb")).expect("open");

    // Insert out of id order; reads come back sorted by the big-endian key.
    let churn_in = vec![churn(3, 5), churn(1, 2), churn(2, 9)];
    storage
        .replace_symbol_churn(&churn_in)
        .expect("replace_symbol_churn");

    let out = storage.all_symbol_churn().expect("all_symbol_churn");
    assert_eq!(
        out,
        vec![churn(1, 2), churn(2, 9), churn(3, 5)],
        "symbol churn round-trips, sorted by SymbolId",
    );
}

#[test]
fn replace_symbol_churn_supersedes_prior_records() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let storage = RedbStorage::open(&tmp.path().join("index.redb")).expect("open");

    storage
        .replace_symbol_churn(&[churn(7, 4), churn(8, 1)])
        .expect("first replace");

    // A second replace must wipe the first set entirely (replace, not merge).
    let churn_in = vec![churn(1, 1)];
    storage
        .replace_symbol_churn(&churn_in)
        .expect("second replace");

    assert_eq!(
        storage.all_symbol_churn().expect("all_symbol_churn"),
        churn_in,
        "prior symbol-churn records cleared",
    );
}
