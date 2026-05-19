//! Chunked full-table scanners feeding the streaming `ReadSnapshot`
//! API (tier-07 step 4). Each scanner owns the `ReadOnlyTable` it
//! walks; `ReadOnlyTable<K, V>` is owned (no lifetime tied to the
//! originating transaction), so the iterator survives the borrow on
//! `&ReadTransaction` used to open it
//! [src: <https://docs.rs/redb/4.1.0/redb/struct.ReadTransaction.html#method.open_table>].
//!
//! Per `next()`, the scanner opens a fresh `range` from after
//! `last_key` (exclusive), drains up to `chunk_size` decoded records,
//! then drops the range. Re-seeking is O(log n) per chunk — negligible
//! against the per-chunk decode cost.

use std::ops::Bound;

use ariadne_core::{
    ChunkStream, EdgeKey, EdgeRecord, FileId, FileRecord, StorageError, SymbolId, SymbolRecord,
};
use redb::{ReadOnlyTable, ReadTransaction};

use super::snapshot::{decode_file_id, decode_symbol_id};
use super::tables::{EDGES, FILES, SYMBOLS};
use crate::adapters::codec::{
    decode_edge_key, decode_edge_record, decode_file_record, decode_symbol_record,
};
use crate::errors::RedbStorageError;

const DEFAULT_CHUNK: usize = 4096;

fn clamp(n: usize) -> usize {
    if n == 0 { DEFAULT_CHUNK } else { n }
}

type DecodeFn<K, V> = fn(&[u8], &[u8]) -> Result<(K, V), RedbStorageError>;

pub(super) struct ChunkedScan<K, V> {
    table: ReadOnlyTable<&'static [u8], &'static [u8]>,
    last_key: Option<Vec<u8>>,
    chunk_size: usize,
    done: bool,
    decode: DecodeFn<K, V>,
}

impl<K, V> Iterator for ChunkedScan<K, V> {
    type Item = Result<Vec<(K, V)>, StorageError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }
        let bounds: (Bound<&[u8]>, Bound<&[u8]>) = match &self.last_key {
            Some(k) => (Bound::Excluded(k.as_slice()), Bound::Unbounded),
            None => (Bound::Unbounded, Bound::Unbounded),
        };
        let range = match self.table.range::<&[u8]>(bounds) {
            Ok(r) => r,
            Err(e) => {
                self.done = true;
                return Some(Err(RedbStorageError::from(e).into()));
            }
        };
        let mut chunk = Vec::with_capacity(self.chunk_size);
        let mut new_last: Option<Vec<u8>> = None;
        for entry in range {
            let (kg, vg) = match entry {
                Ok(p) => p,
                Err(e) => {
                    self.done = true;
                    return Some(Err(RedbStorageError::from(e).into()));
                }
            };
            let k_bytes = kg.value();
            let v_bytes = vg.value();
            match (self.decode)(k_bytes, v_bytes) {
                Ok(pair) => {
                    new_last = Some(k_bytes.to_vec());
                    chunk.push(pair);
                    if chunk.len() >= self.chunk_size {
                        break;
                    }
                }
                Err(e) => {
                    self.done = true;
                    return Some(Err(e.into()));
                }
            }
        }
        if chunk.len() < self.chunk_size {
            self.done = true;
        }
        match new_last {
            Some(k) => {
                self.last_key = Some(k);
                Some(Ok(chunk))
            }
            None => None,
        }
    }
}

pub(super) fn iter_files(
    txn: &ReadTransaction,
    chunk_size: usize,
) -> Result<ChunkStream<'static, (FileId, FileRecord)>, StorageError> {
    let table = txn
        .open_table(FILES)
        .map_err(|e| StorageError::from(RedbStorageError::from(e)))?;
    Ok(Box::new(ChunkedScan {
        table,
        last_key: None,
        chunk_size: clamp(chunk_size),
        done: false,
        decode: |k, v| Ok((decode_file_id(k)?, decode_file_record(v)?)),
    }))
}

pub(super) fn iter_symbols(
    txn: &ReadTransaction,
    chunk_size: usize,
) -> Result<ChunkStream<'static, (SymbolId, SymbolRecord)>, StorageError> {
    let table = txn
        .open_table(SYMBOLS)
        .map_err(|e| StorageError::from(RedbStorageError::from(e)))?;
    Ok(Box::new(ChunkedScan {
        table,
        last_key: None,
        chunk_size: clamp(chunk_size),
        done: false,
        decode: |k, v| Ok((decode_symbol_id(k)?, decode_symbol_record(v)?)),
    }))
}

pub(super) fn iter_edges(
    txn: &ReadTransaction,
    chunk_size: usize,
) -> Result<ChunkStream<'static, (EdgeKey, EdgeRecord)>, StorageError> {
    let table = txn
        .open_table(EDGES)
        .map_err(|e| StorageError::from(RedbStorageError::from(e)))?;
    Ok(Box::new(ChunkedScan {
        table,
        last_key: None,
        chunk_size: clamp(chunk_size),
        done: false,
        decode: |k, v| Ok((decode_edge_key(k)?, decode_edge_record(v)?)),
    }))
}
