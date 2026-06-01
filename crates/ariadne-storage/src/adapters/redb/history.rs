//! Incremental Git-history merge + the HEAD-commit watermark (tier-11a).
//!
//! Split out of `redb/mod.rs` to keep that file inside the project's
//! authoring cap (see `CLAUDE.md`). `merge_history` folds an incremental
//! delta into the `CHURN` / `CO_CHANGE` tables and advances the
//! `KEY_LAST_INGESTED_COMMIT` watermark in one `WriteTransaction`, so a crash
//! never half-applies a delta without recording it (ACID); the cold tier-11
//! full walk continues to use `replace_history`
//! [src: .claude/plans/post-v1-roadmap/tier-11a-incremental-history.md step 4;
//!  <https://docs.rs/redb/4.1.0/redb/struct.WriteTransaction.html>].

use std::collections::BTreeSet;

use ariadne_core::{CoChangePair, FileChurn};
use redb::{Database, ReadableDatabase, ReadableTable};

use super::co_change_key;
use super::tables::{CHURN, CO_CHANGE, HISTORY_META, KEY_LAST_INGESTED_COMMIT};
use crate::adapters::codec::{decode_value, encode_value};
use crate::errors::RedbStorageError;

/// Read the persisted HEAD-commit watermark, or `None` when none is recorded.
pub(super) fn last_ingested_commit(db: &Database) -> Result<Option<Vec<u8>>, RedbStorageError> {
    let txn = db.begin_read()?;
    let table = txn.open_table(HISTORY_META)?;
    Ok(table
        .get(KEY_LAST_INGESTED_COMMIT)?
        .map(|g| g.value().to_vec()))
}

/// Persist `oid` as the watermark in its own transaction (full-replace path).
pub(super) fn set_last_ingested_commit(db: &Database, oid: &[u8]) -> Result<(), RedbStorageError> {
    let txn = db.begin_write()?;
    {
        let mut hist = txn.open_table(HISTORY_META)?;
        hist.insert(KEY_LAST_INGESTED_COMMIT, oid)?;
    }
    txn.commit()?;
    Ok(())
}

/// Merge `churn_delta` / `pair_delta` into the history tables and advance the
/// watermark to `head_oid`, all inside one transaction.
pub(super) fn merge_history(
    db: &Database,
    churn_delta: &[FileChurn],
    pair_delta: &[CoChangePair],
    head_oid: &[u8],
) -> Result<(), RedbStorageError> {
    let txn = db.begin_write()?;
    {
        let mut churn_table = txn.open_table(CHURN)?;
        for delta in churn_delta {
            let key = delta.path.as_bytes();
            // Decode-into-owned before any mutating insert so no read guard
            // borrows the table across the `insert` below.
            let base = churn_table
                .get(key)?
                .map(|g| decode_value::<FileChurn>(g.value()))
                .transpose()?;
            let merged = base.map_or_else(|| delta.clone(), |base| merge_churn(base, delta));
            let value = encode_value(&merged)?;
            churn_table.insert(key, value.as_slice())?;
        }

        let mut pair_table = txn.open_table(CO_CHANGE)?;
        for delta in pair_delta {
            let key = co_change_key(&delta.a, &delta.b);
            let base = pair_table
                .get(key.as_slice())?
                .map(|g| decode_value::<CoChangePair>(g.value()))
                .transpose()?;
            let count = base.map_or(0, |p| p.count) + delta.count;
            let rec = CoChangePair {
                a: delta.a.clone(),
                b: delta.b.clone(),
                count,
            };
            let value = encode_value(&rec)?;
            pair_table.insert(key.as_slice(), value.as_slice())?;
        }

        let mut hist = txn.open_table(HISTORY_META)?;
        hist.insert(KEY_LAST_INGESTED_COMMIT, head_oid)?;
    }
    txn.commit()?;
    Ok(())
}

/// Additive churn merge: commit counts sum, author keys union (deduplicated and
/// re-sorted to match the cold walk's `BTreeSet` ordering), the last-changed
/// time takes the max. Byte-for-byte identical to a full cold walk over the
/// combined window, which is the tier-11a divergence-0 invariant.
fn merge_churn(base: FileChurn, delta: &FileChurn) -> FileChurn {
    let mut authors: BTreeSet<[u8; 8]> = base.author_keys.into_iter().collect();
    authors.extend(delta.author_keys.iter().copied());
    FileChurn {
        path: base.path,
        commits: base.commits + delta.commits,
        author_keys: authors.into_iter().collect(),
        last_changed_ns: base.last_changed_ns.max(delta.last_changed_ns),
    }
}
