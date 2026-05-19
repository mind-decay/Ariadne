//! Codec helpers for redb byte-slice tables.
//!
//! Keys are encoded via [`ariadne_core::IdEncode`] (fixed-width big-endian)
//! so lex byte order matches numeric order — the redb default `&[u8]`
//! comparator is therefore correct without a bespoke `Key` impl.
//!
//! Record bodies are encoded with postcard 1.1
//! (<https://docs.rs/postcard/1.1.3>). Postcard's varint LEB128 is *not*
//! order-preserving across byte-length boundaries
//! (<https://postcard.jamesmunns.com/wire-format>) — that is why keys never
//! travel through postcard.
//!
//! Deviation from tier-02 plan step 5: the plan prescribed `redb::Value` and
//! `redb::Key` trait impls on every record. We keep tables typed as
//! `&[u8] -> &[u8]` and route through free encode/decode helpers instead.
//! Rationale: redb 4.1's `Value::from_bytes` has no error channel, forcing
//! `expect` on corrupt input — these helpers surface
//! [`RedbStorageError::Corrupted`] cleanly, preserving the loud-failure rule
//! ([tier-02 audit I2](../../../../.claude/plans/ariadne-core/audit/tier-02-report.md)).

use ariadne_core::{EdgeKey, EdgeRecord, FileId, FileRecord, IdEncode, SymbolId, SymbolRecord};
use serde::{Deserialize, Serialize};

use crate::errors::RedbStorageError;

/// Encode any postcard-serializable record body.
pub(crate) fn encode_value<T: Serialize>(value: &T) -> Result<Vec<u8>, RedbStorageError> {
    postcard::to_stdvec(value).map_err(RedbStorageError::Postcard)
}

/// Decode any postcard-serializable record body.
pub(crate) fn decode_value<'a, T: Deserialize<'a>>(bytes: &'a [u8]) -> Result<T, RedbStorageError> {
    postcard::from_bytes(bytes).map_err(RedbStorageError::Postcard)
}

/// Encode a [`FileId`] as 8 big-endian bytes.
#[must_use]
pub(crate) fn encode_file_id(id: FileId) -> [u8; 8] {
    id.to_bytes()
}

/// Encode a [`SymbolId`] as 8 big-endian bytes.
#[must_use]
pub(crate) fn encode_symbol_id(id: SymbolId) -> [u8; 8] {
    id.to_bytes()
}

/// Encode an [`EdgeKey`] as 17 big-endian bytes: `[src(8) | kind(1) | dst(8)]`.
#[must_use]
pub(crate) fn encode_edge_key(key: EdgeKey) -> [u8; 17] {
    key.to_bytes()
}

/// Decode an [`EdgeKey`] from a 17-byte slice.
pub(crate) fn decode_edge_key(bytes: &[u8]) -> Result<EdgeKey, RedbStorageError> {
    let arr: [u8; 17] = bytes
        .try_into()
        .map_err(|_| RedbStorageError::Corrupted("EdgeKey wrong length".to_owned()))?;
    EdgeKey::from_bytes(&arr)
        .ok_or_else(|| RedbStorageError::Corrupted("EdgeKey subfield decode failed".to_owned()))
}

/// Encode a [`FileRecord`] body.
pub(crate) fn encode_file_record(rec: &FileRecord) -> Result<Vec<u8>, RedbStorageError> {
    encode_value(rec)
}

/// Decode a [`FileRecord`] body.
pub(crate) fn decode_file_record(bytes: &[u8]) -> Result<FileRecord, RedbStorageError> {
    decode_value(bytes)
}

/// Encode a [`SymbolRecord`] body.
pub(crate) fn encode_symbol_record(rec: &SymbolRecord) -> Result<Vec<u8>, RedbStorageError> {
    encode_value(rec)
}

/// Decode a [`SymbolRecord`] body.
pub(crate) fn decode_symbol_record(bytes: &[u8]) -> Result<SymbolRecord, RedbStorageError> {
    decode_value(bytes)
}

/// Encode an [`EdgeRecord`] body.
pub(crate) fn encode_edge_record(rec: &EdgeRecord) -> Result<Vec<u8>, RedbStorageError> {
    encode_value(rec)
}

/// Decode an [`EdgeRecord`] body.
pub(crate) fn decode_edge_record(bytes: &[u8]) -> Result<EdgeRecord, RedbStorageError> {
    decode_value(bytes)
}
