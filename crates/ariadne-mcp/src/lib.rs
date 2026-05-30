//! MCP driving adapter. Tier-08 wires `rmcp` 1.7.0 tool routers exposing
//! Ariadne's analytics over stdio.

#![deny(missing_docs)]

pub mod adapters;
pub mod catalog;
pub mod domain;
pub mod errors;
pub mod serve;
pub mod server;
pub mod tools;
pub mod types;

pub use adapters::daemon_client::DaemonClient;
pub use catalog::Catalog;
pub use errors::McpError;
pub use serve::{ServeOpts, build_server, serve_stdio};
pub use server::AriadneServer;
