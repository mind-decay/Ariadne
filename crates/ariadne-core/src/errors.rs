//! Crate error type. `thiserror` enum per ADR-0001 / folder-layout rule 5.

use thiserror::Error;

/// Errors raised by domain operations.
///
/// Variants are added per-tier as the domain grows; tier-01 ships the bare
/// minimum so the public surface and `thiserror` integration are in place.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum CoreError {
    /// An id was zero or otherwise outside the non-zero domain of its type.
    #[error("invalid id: value must be non-zero")]
    InvalidId,
}
