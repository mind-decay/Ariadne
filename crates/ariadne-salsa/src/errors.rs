//! Salsa orchestration error type.

use thiserror::Error;

/// Errors raised by the Salsa orchestrator.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum SalsaError {
    /// Placeholder until tier-04 wires real Salsa error sources.
    #[error("salsa operation failed: {0}")]
    Other(String),
}
