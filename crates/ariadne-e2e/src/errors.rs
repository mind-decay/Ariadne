//! E2E harness error type. `anyhow::Error` is also permitted in this crate
//! per folder-layout rule 5.

use thiserror::Error;

/// Errors raised by the e2e harness.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum E2eError {
    /// Placeholder until tier-10 wires real harness error sources.
    #[error("e2e operation failed: {0}")]
    Other(String),
}
