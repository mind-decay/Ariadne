//! Parser error type.

use thiserror::Error;

/// Errors raised by the parser adapter.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ParserError {
    /// Placeholder until tier-03 wires real tree-sitter error sources.
    #[error("parser operation failed: {0}")]
    Other(String),
}
