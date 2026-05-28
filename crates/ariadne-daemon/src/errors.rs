//! Daemon driving-adapter error type. Failures from the `interprocess`
//! transport, the pidfile/socket filesystem state, and the detached-process
//! spawn are stringified at the boundary so neither `interprocess` nor
//! `std::io` types leak into the public API [src: docs/folder-layout.md
//! rule 4].

use thiserror::Error;

/// Errors raised while managing or talking to the daemon.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum DaemonError {
    /// A filesystem or socket IO operation failed (binding the listener,
    /// connecting, reading/writing the pidfile, removing residue).
    #[error("daemon io: {0}")]
    Io(String),
    /// A frame on the wire was malformed: a bad length prefix or an unknown
    /// request/response discriminant.
    #[error("daemon protocol: {0}")]
    Protocol(String),
    /// `start` found a daemon already answering the liveness handshake.
    #[error("daemon already running (pid {pid})")]
    AlreadyRunning {
        /// PID recorded in the live daemon's pidfile, or 0 if unreadable.
        pid: u32,
    },
    /// Spawning the detached daemon process failed.
    #[error("daemon spawn: {0}")]
    Spawn(String),
    /// The daemon did not reach the expected state (up, or down) within the
    /// allotted readiness window.
    #[error("daemon timeout: {0}")]
    Timeout(String),
    /// Building or refreshing the warm graph failed reading storage.
    #[error("daemon storage: {0}")]
    Storage(String),
    /// Building the warm petgraph from the storage snapshot failed.
    #[error("daemon graph: {0}")]
    Graph(String),
}

impl From<std::io::Error> for DaemonError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err.to_string())
    }
}

impl From<ariadne_core::StorageError> for DaemonError {
    fn from(err: ariadne_core::StorageError) -> Self {
        Self::Storage(err.to_string())
    }
}

impl From<ariadne_graph::GraphError> for DaemonError {
    fn from(err: ariadne_graph::GraphError) -> Self {
        Self::Graph(err.to_string())
    }
}
