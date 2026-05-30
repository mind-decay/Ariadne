//! Driven adapters for the MCP server. The single external IO here is the
//! [`daemon_client`] local-socket transport to the warm daemon (RD6); the
//! per-tool query routing that consumes it lives in [`crate::server`].

pub mod daemon_client;
