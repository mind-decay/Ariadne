//! Watcher error type.

use thiserror::Error;

/// Errors raised by the watcher adapter.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum WatcherError {
    /// Placeholder until tier-06 wires real notify-rs error sources.
    #[error("watcher operation failed: {0}")]
    Other(String),
}
