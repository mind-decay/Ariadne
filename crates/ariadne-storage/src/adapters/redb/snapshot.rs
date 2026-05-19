//! Read-snapshot accessors. Each function opens its target table on demand
//! against the owning `ReadTransaction` so the snapshot stays cheap when an
//! upstream query touches only a subset of the schema.
//!
//! Chunked full-table scanners (`iter_files`, `iter_symbols`, `iter_edges`)
//! live in [`super::scan`] so this file stays inside the project's
//! 200-line authored cap (CLAUDE.md `<rules>`).

use ariadne_core::{EdgeKey, EdgeRecord, FileId, FileRecord, IdEncode, SymbolId, SymbolRecord};
use redb::{ReadTransaction, ReadableTable};

use super::tables::{EDGES, EDGES_BY_FILE, FILES, SYMBOLS};
use crate::adapters::codec::{
    decode_edge_key, decode_edge_record, decode_file_record, decode_symbol_record, encode_file_id,
};
use crate::errors::RedbStorageError;

pub(super) fn decode_file_id(bytes: &[u8]) -> Result<FileId, RedbStorageError> {
    let arr: [u8; 8] = bytes
        .try_into()
        .map_err(|_| RedbStorageError::Corrupted("FileId wrong length".to_owned()))?;
    FileId::from_bytes(arr)
        .ok_or_else(|| RedbStorageError::Corrupted("FileId zero or out-of-range".to_owned()))
}

pub(super) fn decode_symbol_id(bytes: &[u8]) -> Result<SymbolId, RedbStorageError> {
    let arr: [u8; 8] = bytes
        .try_into()
        .map_err(|_| RedbStorageError::Corrupted("SymbolId wrong length".to_owned()))?;
    SymbolId::from_bytes(arr).ok_or_else(|| RedbStorageError::Corrupted("SymbolId zero".to_owned()))
}

pub(super) fn file(
    txn: &ReadTransaction,
    id: FileId,
) -> Result<Option<FileRecord>, RedbStorageError> {
    let files = txn.open_table(FILES)?;
    Ok(match files.get(&encode_file_id(id)[..])? {
        Some(g) => Some(decode_file_record(g.value())?),
        None => None,
    })
}

pub(super) fn symbols_in_file(
    txn: &ReadTransaction,
    id: FileId,
) -> Result<Vec<SymbolRecord>, RedbStorageError> {
    let symbols = txn.open_table(SYMBOLS)?;
    let mut out = Vec::new();
    for entry in symbols.iter()? {
        let (_, vg) = entry?;
        let rec = decode_symbol_record(vg.value())?;
        if rec.defining_file == id {
            out.push(rec);
        }
    }
    Ok(out)
}

pub(super) fn outgoing(
    txn: &ReadTransaction,
    src: SymbolId,
) -> Result<Vec<(EdgeKey, EdgeRecord)>, RedbStorageError> {
    let edges = txn.open_table(EDGES)?;
    let mut lo = [0u8; 17];
    lo[..8].copy_from_slice(&src.to_bytes());
    let mut hi = [0u8; 17];
    hi[..8].copy_from_slice(&src.to_bytes());
    hi[8..].fill(0xFF);
    let mut out = Vec::new();
    for entry in edges.range::<&[u8]>(&lo[..]..=&hi[..])? {
        let (kg, vg) = entry?;
        out.push((
            decode_edge_key(kg.value())?,
            decode_edge_record(vg.value())?,
        ));
    }
    Ok(out)
}

pub(super) fn incoming(
    txn: &ReadTransaction,
    dst: SymbolId,
) -> Result<Vec<(EdgeKey, EdgeRecord)>, RedbStorageError> {
    let edges = txn.open_table(EDGES)?;
    let mut out = Vec::new();
    for entry in edges.iter()? {
        let (kg, vg) = entry?;
        let key = decode_edge_key(kg.value())?;
        if key.dst == dst {
            out.push((key, decode_edge_record(vg.value())?));
        }
    }
    Ok(out)
}

pub(super) fn edges_in_file(
    txn: &ReadTransaction,
    file: FileId,
) -> Result<Vec<EdgeKey>, RedbStorageError> {
    let ebf = txn.open_multimap_table(EDGES_BY_FILE)?;
    let mut out = Vec::new();
    for entry in ebf.get(&encode_file_id(file)[..])? {
        out.push(decode_edge_key(entry?.value())?);
    }
    Ok(out)
}
