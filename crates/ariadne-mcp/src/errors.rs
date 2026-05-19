//! MCP server error type.

use thiserror::Error;

/// Errors raised by the MCP server.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum McpError {
    /// Placeholder until tier-08 wires real rmcp error sources.
    #[error("mcp operation failed: {0}")]
    Other(String),
}
