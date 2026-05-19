//! File-system watcher driving adapter façade. Re-exports the notify-rs
//! implementation of `ariadne_core::WatcherSink`. No logic in this file.

#![deny(missing_docs)]

pub mod adapters;
pub mod domain;
pub mod errors;

pub use adapters::notify::NotifyWatcher;
pub use errors::WatcherError;
