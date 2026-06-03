//! Driven adapters for the MCP server. The external IO here is the
//! [`daemon_client`] local-socket transport to the warm daemon (RD6) and the
//! [`source`] filesystem read backing `read_symbol`; the per-tool query
//! routing that consumes them lives in [`crate::server`].

pub mod daemon_client;
pub mod source;
