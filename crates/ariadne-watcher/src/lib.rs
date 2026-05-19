//! File-system watcher driving adapter façade. Re-exports the
//! notify-rs implementation, the gitignore-aware reconciler, and the
//! sink implementations that bridge the watcher event stream into
//! `ariadne_salsa::AriadneDb`. No logic in this file
//! [src: docs/folder-layout.md rule 3].

#![deny(missing_docs)]

pub mod adapters;
pub mod domain;
pub mod errors;

pub use adapters::ignore::{ARIADNE_IGNORE_FILENAME, Ignore};
pub use adapters::notify::NotifyWatcher;
pub use adapters::reconcile::Reconciler;
pub use adapters::sink::{AriadneDbSink, ChannelSink, NoopSink};
pub use errors::WatcherError;
