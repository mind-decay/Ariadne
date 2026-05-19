//! Crate error types. `thiserror` enums per ADR-0001 / folder-layout rule 5.

use thiserror::Error;

/// Errors raised by domain operations.
///
/// Variants are added per-tier as the domain grows.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum CoreError {
    /// An id was zero or otherwise outside the non-zero domain of its type.
    #[error("invalid id: value must be non-zero")]
    InvalidId,
}

/// Errors raised by [`crate::Storage`] implementations. Adapter crates map
/// their backend errors into these variants via `From`.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum StorageError {
    /// On-disk schema version does not match the binary's expected version.
    /// v1 has no migration policy — the index must be rebuilt from source.
    #[error("storage schema mismatch: found {found}, expected {expected}")]
    SchemaMismatch {
        /// Schema version read from disk.
        found: u64,
        /// Schema version the running binary requires.
        expected: u64,
    },
    /// Lookup hit a missing record at a point the call site expected one.
    #[error("storage record not found")]
    NotFound,
    /// On-disk format is unreadable or violates an invariant.
    #[error("storage corrupted: {0}")]
    Corrupted(String),
    /// Backend IO failure (filesystem, lock, page fault, …).
    #[error("storage io: {0}")]
    Io(String),
}
