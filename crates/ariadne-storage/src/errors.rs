//! Storage error type.

use thiserror::Error;

/// Errors raised by the storage adapter.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum StorageError {
    /// Placeholder until tier-02 wires real redb error sources.
    #[error("storage operation failed: {0}")]
    Other(String),
}
