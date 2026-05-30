//! Daemon driving-adapter façade. Re-exports the lifecycle orchestration
//! (`start`/`stop`/`status`/`serve`/`serve_live`/`ping`), the live-update
//! engine + warm-graph dump used by the tier-08 incrementality test, the pure
//! path + status policy, and the crate error type. No logic in this file
//! [src: docs/folder-layout.md rule 3].

#![deny(missing_docs)]

pub mod adapters;
pub mod domain;
pub mod errors;

pub use adapters::ipc::{
    StartReport, ping, query, running_as_daemon_child, serve, serve_live, start, status, stop,
};
pub use domain::dump::CatalogDump;
pub use domain::lifecycle::{DaemonPaths, DaemonStatus, Pid, ReclaimDecision, reclaim_decision};
pub use domain::live::LiveEngine;
pub use errors::DaemonError;
