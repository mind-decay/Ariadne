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
    /// Caller supplied a malformed argument (e.g. an invalid `regex` query or
    /// `path` glob) — distinct from a server fault so it maps to the JSON-RPC
    /// `invalid_params` code rather than `internal_error`.
    #[error("invalid input: {0}")]
    InvalidInput(String),
    /// Catch-all for paths surfaced as plain strings.
    #[error("mcp: {0}")]
    Other(String),
}

impl McpError {
    /// Convert into the rmcp wire error type so `#[tool]` handlers can
    /// return `Result<CallToolResult, rmcp::ErrorData>`.
    ///
    /// Caller-input faults ([`Self::InvalidInput`]) map to JSON-RPC
    /// `invalid_params` so a client can tell a bad argument from a server
    /// fault; every other variant maps to `internal_error`
    /// [src: rmcp-1.7.0 model.rs:556 `ErrorData::invalid_params`].
    #[must_use]
    pub fn into_rmcp(self) -> rmcp::ErrorData {
        let message = self.to_string();
        match self {
            Self::InvalidInput(_) => rmcp::ErrorData::invalid_params(message, None),
            _ => rmcp::ErrorData::internal_error(message, None),
        }
    }
}
