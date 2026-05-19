//! notify-rs implementation of `ariadne_core::WatcherSink`. Tier-06 wires
//! the debouncer, gitignore filter, and reconciliation scanner.

use ariadne_core::WatcherSink;

/// Placeholder notify-rs watcher. Real implementation arrives in tier-06.
#[derive(Debug, Default)]
pub struct NotifyWatcher;

impl WatcherSink for NotifyWatcher {}
