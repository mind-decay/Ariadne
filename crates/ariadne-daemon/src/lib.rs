//! Daemon driving-adapter façade. Re-exports the lifecycle orchestration
//! (`start`/`stop`/`status`/`serve`/`ping`), the pure path + status policy,
//! and the crate error type. No logic in this file [src: docs/folder-layout.md
//! rule 3].

#![deny(missing_docs)]

pub mod adapters;
pub mod domain;
pub mod errors;

pub use adapters::ipc::{StartReport, ping, query, serve, start, status, stop};
pub use domain::lifecycle::{DaemonPaths, DaemonStatus, Pid, ReclaimDecision, reclaim_decision};
pub use errors::DaemonError;
