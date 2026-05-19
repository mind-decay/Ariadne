//! Graph analytics error type.

use ariadne_core::StorageError;
use thiserror::Error;

/// Errors raised by graph analytics.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum GraphError {
    /// Failure reading from the underlying `Storage` port.
    #[error("storage error during graph build: {0}")]
    Storage(#[from] StorageError),
    /// The caller addressed a symbol that does not exist in the graph.
    #[error("unknown symbol id: {0}")]
    UnknownSymbol(u64),
}
