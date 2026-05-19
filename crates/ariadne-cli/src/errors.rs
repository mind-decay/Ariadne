//! CLI entrypoint error type. `anyhow::Error` is also permitted in this
//! crate per folder-layout rule 5 — this `thiserror` enum exists so the
//! crate matches the canonical layout shape across all `ariadne-*` crates.

use thiserror::Error;

/// Errors raised by the `ariadne` CLI entrypoint.
#[allow(dead_code)] // Tier-10 wires real variants; stub keeps shape canonical.
#[derive(Debug, Error)]
#[non_exhaustive]
pub(crate) enum CliError {
    /// Placeholder until tier-10 wires real subcommand error sources.
    #[error("cli operation failed: {0}")]
    Other(String),
}
