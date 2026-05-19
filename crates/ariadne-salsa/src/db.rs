//! `AriadneDb` — the project's salsa database (tier-04 step 6 + step 11).
//!
//! Tier-04 wires only the salsa surface; the actual seeding from storage and
//! the delta write-back through `WriteTxn` are stubbed until tier-06+
//! [src: .claude/plans/ariadne-core/tier-04-salsa.md step 6 + step 11
//! `exposed but not yet driven`].

use std::sync::Arc;
use std::sync::Mutex;

use ariadne_core::{Changeset, RevisionId, Storage, StorageError};
use salsa::{EventKind, Storage as SalsaStorage};

use crate::inputs::FileContentInput;

/// Type alias for the event-log channel used by tests and the memory probe.
pub type EventLog = Arc<Mutex<Vec<String>>>;

/// Ariadne's salsa database. Owns the `salsa::Storage<Self>` and an optional
/// recompute-event log used by [`crate::AriadneDb::with_event_log`].
#[salsa::db]
#[derive(Clone)]
pub struct AriadneDb {
    storage: SalsaStorage<Self>,
    event_log: Option<EventLog>,
}

impl Default for AriadneDb {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for AriadneDb {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AriadneDb")
            .field("event_log", &self.event_log.is_some())
            .finish_non_exhaustive()
    }
}

impl AriadneDb {
    /// Build a fresh, empty database with no event log.
    #[must_use]
    pub fn new() -> Self {
        Self {
            storage: SalsaStorage::new(None),
            event_log: None,
        }
    }

    /// Build a database whose recompute events are mirrored into `log`.
    /// Used by the equivalence test to assert cache hits and by the
    /// memory probe sanity test.
    #[must_use]
    pub fn with_event_log(log: EventLog) -> Self {
        let cb_log = Arc::clone(&log);
        let callback = move |event: salsa::Event| {
            if matches!(event.kind, EventKind::WillExecute { .. }) {
                if let Ok(mut g) = cb_log.lock() {
                    g.push(format!("{event:?}"));
                }
            }
        };
        Self {
            storage: SalsaStorage::new(Some(Box::new(callback))),
            event_log: Some(log),
        }
    }

    /// Snapshot the recompute-event log. Returns an empty vector when no
    /// log is attached.
    #[must_use]
    pub fn event_log_snapshot(&self) -> Vec<String> {
        self.event_log
            .as_ref()
            .and_then(|l| l.lock().ok().map(|g| g.clone()))
            .unwrap_or_default()
    }

    /// Seed the salsa DB by reading file records from a `Storage` adapter.
    /// Tier-04 ships the surface; tier-06+ adds file enumeration to the
    /// `Storage` port and populates real inputs here. Returns the inputs
    /// created so callers can keep handles.
    ///
    /// # Errors
    /// Propagates storage read failures.
    pub fn seed_from_disk<S: Storage>(
        &mut self,
        storage: &S,
    ) -> Result<Vec<FileContentInput>, StorageError> {
        // Open a snapshot so the call exercises the storage port; tier-04
        // does not yet enumerate files (the port lacks a list_files method
        // — added in a later tier).
        let _snapshot = storage.snapshot()?;
        Ok(Vec::new())
    }

    /// Commit the current salsa revision's deltas back to a `Storage`
    /// adapter via a write transaction. Tier-04 exposes the method shape;
    /// real changeset derivation lands when the watcher (tier-06) drives
    /// re-derivation [src: tier-04 plan step 11].
    ///
    /// # Errors
    /// Propagates storage write failures.
    pub fn commit_revision<S: Storage>(&self, storage: &S) -> Result<RevisionId, StorageError> {
        let txn = storage.begin_write()?;
        let empty = Changeset::new();
        ariadne_core::WriteTxn::apply(txn, &empty)
    }
}

#[salsa::db]
impl salsa::Database for AriadneDb {}
