//! Graph analytics error type.

use thiserror::Error;

/// Errors raised by graph analytics.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum GraphError {
    /// Placeholder until tier-07 wires real analytic error sources.
    #[error("graph operation failed: {0}")]
    Other(String),
}
