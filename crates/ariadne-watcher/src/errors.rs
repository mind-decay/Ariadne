//! Watcher driving-adapter error type. Sink failures stay confined to
//! tracing logs (see `ariadne_core::WatcherSink`); these variants surface
//! configuration / start-up failures that block the watcher from running.

use thiserror::Error;

/// Errors raised when constructing or starting the watcher.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum WatcherError {
    /// Building the [`crate::adapters::ignore::Ignore`] matcher failed —
    /// the user-provided `.gitignore` or `.ariadneignore` contains an
    /// unparseable pattern.
    #[error("ignore matcher build failed: {0}")]
    IgnoreBuild(String),
    /// notify-rs / notify-debouncer-full refused to start. Wraps any
    /// platform-specific watcher initialization failure.
    #[error("watcher start failed: {0}")]
    Notify(String),
    /// Filesystem IO failure raised by the reconciliation walk.
    #[error("reconciliation io: {0}")]
    Io(String),
}
