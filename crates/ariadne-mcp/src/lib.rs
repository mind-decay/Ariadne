//! MCP driving adapter. Tier-08 wires `rmcp` 1.7.0 tool routers exposing
//! Ariadne's analytics over stdio.

#![deny(missing_docs)]

pub mod domain;
pub mod errors;

pub use errors::McpError;
