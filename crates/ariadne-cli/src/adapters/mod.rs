//! Driven adapters for the CLI composition root. The single external IO here
//! is the [`daemon_client`] thin client to the warm daemon (RD6); the query
//! routing that consumes it lives in [`crate::commands::query`].

pub mod daemon_client;
