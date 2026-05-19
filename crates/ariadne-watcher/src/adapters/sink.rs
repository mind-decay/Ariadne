//! `WatcherSink` implementations (tier-06 step 7).
//!
//! * [`ChannelSink`] — testing aid that forwards every invalidation to a
//!   `std::sync::mpsc::Receiver` so tests can assert event order.
//! * [`AriadneDbSink`] — production sink. Holds an `Arc<Mutex<AriadneDb>>`
//!   plus a `path → FileContentInput` map so repeated invalidations for
//!   the same file update the existing salsa input (which is how salsa
//!   bumps revisions and triggers incremental re-derivation
//!   [src: tier-04 plan step 9]).
//!
//! IO failures in the sink (reading the on-disk bytes for an updated
//! file) are logged via tracing and dropped — the watcher must keep
//! running even when a single file is unreadable. The
//! [`ariadne_core::WatcherSink`] trait reflects this with an infallible
//! `apply_invalidation` signature.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};

use ariadne_core::{Invalidation, WatcherSink};
use ariadne_salsa::{AriadneDb, FileContentInput, Setter, durability_for};
use tracing::warn;

/// Testing sink. Drops every invalidation onto a `mpsc::Sender`.
#[derive(Debug)]
pub struct ChannelSink {
    tx: Sender<Invalidation>,
}

impl ChannelSink {
    /// Build a sink/receiver pair. The receiver outlives the sink so the
    /// channel stays alive for assertions after the watcher shuts down.
    #[must_use]
    pub fn pair() -> (Self, Receiver<Invalidation>) {
        let (tx, rx) = mpsc::channel();
        (Self { tx }, rx)
    }
}

impl WatcherSink for ChannelSink {
    fn apply_invalidation(&mut self, inv: &Invalidation) {
        let _ = self.tx.send(inv.clone());
    }
}

/// Discard-everything sink — used by reconcile bootstrap in tests where
/// only the second pass is asserted on.
#[derive(Debug, Default)]
pub struct NoopSink;

impl WatcherSink for NoopSink {
    fn apply_invalidation(&mut self, _inv: &Invalidation) {}
}

/// Production sink that drives the salsa input layer.
pub struct AriadneDbSink {
    db: Arc<Mutex<AriadneDb>>,
    inputs: HashMap<PathBuf, FileContentInput>,
}

impl std::fmt::Debug for AriadneDbSink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AriadneDbSink")
            .field("tracked_paths", &self.inputs.len())
            .finish_non_exhaustive()
    }
}

impl AriadneDbSink {
    /// Wrap a shared database.
    #[must_use]
    pub fn new(db: Arc<Mutex<AriadneDb>>) -> Self {
        Self {
            db,
            inputs: HashMap::new(),
        }
    }

    /// Read the file bytes + hash, then either create the
    /// [`FileContentInput`] or update the existing one with the proper
    /// durability tier per [`ariadne_salsa::durability_for`].
    fn upsert_from_disk(&mut self, path: &PathBuf) {
        let bytes = match std::fs::read(path) {
            Ok(b) => b,
            Err(e) => {
                warn!(
                    target: "ariadne_watcher",
                    "sink read failed for {}: {e}", path.display()
                );
                return;
            }
        };
        let hash: [u8; 32] = *blake3::hash(&bytes).as_bytes();
        let path_str = path.to_string_lossy().into_owned();
        let durability = durability_for(&path_str);
        let mut db = match self.db.lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };
        if let Some(input) = self.inputs.get(path).copied() {
            input
                .set_content(&mut *db)
                .with_durability(durability)
                .to(bytes);
            input
                .set_hash(&mut *db)
                .with_durability(durability)
                .to(hash);
        } else {
            let input = FileContentInput::builder(path_str, bytes, hash)
                .durability(durability)
                .new(&*db);
            self.inputs.insert(path.clone(), input);
        }
    }

    fn forget(&mut self, path: &PathBuf) {
        if let Some(input) = self.inputs.remove(path) {
            let path_str = path.to_string_lossy().into_owned();
            let durability = durability_for(&path_str);
            let mut db = match self.db.lock() {
                Ok(g) => g,
                Err(p) => p.into_inner(),
            };
            input
                .set_content(&mut *db)
                .with_durability(durability)
                .to(Vec::new());
            input
                .set_hash(&mut *db)
                .with_durability(durability)
                .to([0u8; 32]);
        }
    }

    /// Number of files the sink currently tracks. Test hook.
    #[must_use]
    pub fn tracked(&self) -> usize {
        self.inputs.len()
    }

    /// Return the salsa `FileContentInput` handle the sink built for `path`,
    /// or `None` if no invalidation has been applied for it yet.
    ///
    /// Exposed so the tier-06 end-to-end test can hand the input back to
    /// `ariadne_salsa::symbols_for_file` and observe re-derivation after an
    /// edit. The returned handle is a salsa-internal `Copy` id; mutating
    /// the input still has to flow through `apply_invalidation`.
    #[must_use]
    pub fn input_for(&self, path: &Path) -> Option<FileContentInput> {
        self.inputs.get(path).copied()
    }
}

impl WatcherSink for AriadneDbSink {
    fn apply_invalidation(&mut self, inv: &Invalidation) {
        match inv {
            Invalidation::Created { path }
            | Invalidation::Modified { path }
            | Invalidation::HashDrift { path, .. } => self.upsert_from_disk(path),
            Invalidation::Removed { path } => self.forget(path),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channel_sink_forwards_invalidations() {
        let (mut sink, rx) = ChannelSink::pair();
        let path = PathBuf::from("/tmp/x");
        sink.apply_invalidation(&Invalidation::Created { path: path.clone() });
        let got = rx.recv().unwrap();
        assert!(matches!(got, Invalidation::Created { path: p } if p == path));
    }

    #[test]
    fn ariadne_db_sink_creates_input_on_first_event() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("a.rs");
        std::fs::write(&file, b"hi").unwrap();
        let db = Arc::new(Mutex::new(AriadneDb::new()));
        let mut sink = AriadneDbSink::new(Arc::clone(&db));
        sink.apply_invalidation(&Invalidation::Created { path: file.clone() });
        assert_eq!(sink.tracked(), 1);
        sink.apply_invalidation(&Invalidation::Modified { path: file.clone() });
        assert_eq!(sink.tracked(), 1, "second event must reuse input");
    }

    #[test]
    fn ariadne_db_sink_drops_input_on_remove() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("b.rs");
        std::fs::write(&file, b"hi").unwrap();
        let db = Arc::new(Mutex::new(AriadneDb::new()));
        let mut sink = AriadneDbSink::new(Arc::clone(&db));
        sink.apply_invalidation(&Invalidation::Created { path: file.clone() });
        sink.apply_invalidation(&Invalidation::Removed { path: file });
        assert_eq!(sink.tracked(), 0);
    }
}
