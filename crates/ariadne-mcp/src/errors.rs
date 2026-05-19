//! MCP server error type.

use thiserror::Error;

/// Errors raised by the MCP server and the per-tool handlers.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum McpError {
    /// Storage-port failure (redb open, snapshot read, etc.).
    #[error("storage: {0}")]
    Storage(#[from] ariadne_core::StorageError),
    /// Graph build / analytics failure.
    #[error("graph: {0}")]
    Graph(#[from] ariadne_graph::GraphError),
    /// JSON serialization failure when shaping tool output.
    #[error("serialize: {0}")]
    Serialize(#[from] serde_json::Error),
    /// Caller asked about an unknown symbol or path.
    #[error("not found: {0}")]
    NotFound(String),
    /// Catch-all for paths surfaced as plain strings.
    #[error("mcp: {0}")]
    Other(String),
}

impl McpError {
    /// Convert into the rmcp wire error type so `#[tool]` handlers can
    /// return `Result<CallToolResult, rmcp::ErrorData>`.
    #[must_use]
    pub fn into_rmcp(self) -> rmcp::ErrorData {
        rmcp::ErrorData::internal_error(self.to_string(), None)
    }
}
