//! Single-txn write path. Sequenced per
//! [tier-02 plan step 7](../../../../../.claude/plans/ariadne-core/tier-02-storage.md).

use ariadne_core::Changeset;
use redb::{ReadableTable, WriteTransaction};

use super::tables::{EDGES, EDGES_BY_FILE, FILES, KEY_REVISION, META, SYMBOLS};
use crate::adapters::codec::{
    decode_edge_record, decode_symbol_record, encode_edge_key, encode_edge_record, encode_file_id,
    encode_file_record, encode_symbol_id, encode_symbol_record,
};
use crate::errors::RedbStorageError;

pub(super) fn apply_writes(
    txn: &WriteTransaction,
    cs: &Changeset,
) -> Result<u64, RedbStorageError> {
    let mut files = txn.open_table(FILES)?;
    let mut symbols = txn.open_table(SYMBOLS)?;
    let mut edges = txn.open_table(EDGES)?;
    let mut ebf = txn.open_multimap_table(EDGES_BY_FILE)?;
    let mut meta = txn.open_table(META)?;

    for &fid in &cs.file_deletes {
        let fid_bytes = encode_file_id(fid);
        let mut edge_keys: Vec<Vec<u8>> = Vec::new();
        for entry in ebf.remove_all(&fid_bytes[..])? {
            edge_keys.push(entry?.value().to_vec());
        }
        for key in edge_keys {
            edges.remove(&key[..])?;
        }
        let mut symbols_to_drop: Vec<Vec<u8>> = Vec::new();
        for entry in symbols.iter()? {
            let (kg, vg) = entry?;
            if decode_symbol_record(vg.value())?.defining_file == fid {
                symbols_to_drop.push(kg.value().to_vec());
            }
        }
        for key in symbols_to_drop {
            symbols.remove(&key[..])?;
        }
        files.remove(&fid_bytes[..])?;
    }

    for (fid, rec) in &cs.file_upserts {
        files.insert(&encode_file_id(*fid)[..], &encode_file_record(rec)?[..])?;
    }

    for (sid, rec) in &cs.symbol_upserts {
        symbols.insert(&encode_symbol_id(*sid)[..], &encode_symbol_record(rec)?[..])?;
    }
    for &sid in &cs.symbol_deletes {
        symbols.remove(&encode_symbol_id(sid)[..])?;
    }

    for ekey in &cs.edges_removed {
        let key = encode_edge_key(*ekey);
        if let Some(old) = edges.remove(&key[..])? {
            let file_bytes = encode_file_id(decode_edge_record(old.value())?.source_span.file);
            drop(old);
            ebf.remove(&file_bytes[..], &key[..])?;
        }
    }
    for (ekey, rec) in &cs.edges_added {
        let key = encode_edge_key(*ekey);
        let file_bytes = encode_file_id(rec.source_span.file);
        edges.insert(&key[..], &encode_edge_record(rec)?[..])?;
        ebf.insert(&file_bytes[..], &key[..])?;
    }

    let current = meta.get(KEY_REVISION)?.map_or(0, |g| g.value());
    let next = current.saturating_add(1);
    meta.insert(KEY_REVISION, &next)?;
    Ok(next)
}
