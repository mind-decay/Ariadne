//! SCIP error type.

use thiserror::Error;

/// Errors raised by the SCIP adapter.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ScipError {
    /// Placeholder until tier-05 wires real subprocess/protobuf error
    /// sources.
    #[error("scip operation failed: {0}")]
    Other(String),
}
