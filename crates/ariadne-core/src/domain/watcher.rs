//! Watcher-domain types shared between the file-system driving adapter
//! (`ariadne-watcher`) and any sink that consumes invalidations.
//!
//! Lives in ariadne-core so the [`crate::WatcherSink`] port can be expressed
//! without leaking notify-rs types past the adapter boundary
//! (see `docs/folder-layout.md` "adding-a-port" and
//! `.claude/plans/ariadne-core/tier-06-watcher.md` `exit_criteria`).

use std::path::PathBuf;

/// 32-byte blake3 content digest. Adapters compute this; the type is shared
/// so reconciliation drift can be expressed in domain terms.
pub type ContentHash = [u8; 32];

/// File-system change observed by the watcher. Translation from raw notify
/// events to this enum happens inside `ariadne-watcher`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Invalidation {
    /// File appeared on disk.
    Created {
        /// Absolute path of the created file.
        path: PathBuf,
    },
    /// File contents changed in place.
    Modified {
        /// Absolute path of the modified file.
        path: PathBuf,
    },
    /// File disappeared. Renames are emitted as `Removed { old }` followed
    /// by `Created { new }` so sinks need not model rename pairs.
    Removed {
        /// Absolute path of the removed file.
        path: PathBuf,
    },
    /// Reconciliation discovered a content-hash mismatch the notify stream
    /// missed (R7 mitigation against macOS `FSEvents` drop under load).
    HashDrift {
        /// Absolute path with drifting content.
        path: PathBuf,
        /// Sink's last-known hash for this path. All zeros if the sink had
        /// no prior record.
        old_hash: ContentHash,
        /// Freshly computed blake3 of the on-disk content.
        new_hash: ContentHash,
    },
}

impl Invalidation {
    /// Path the invalidation targets, regardless of variant.
    #[must_use]
    pub fn path(&self) -> &PathBuf {
        match self {
            Self::Created { path }
            | Self::Modified { path }
            | Self::Removed { path }
            | Self::HashDrift { path, .. } => path,
        }
    }
}

/// Summary of a single reconciliation pass. Returned by the periodic
/// scanner so callers (CLI, tests) can observe progress without subscribing
/// to the invalidation stream.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ReconciliationReport {
    /// Number of files visited by the gitignore-aware walk.
    pub files_checked: usize,
    /// Number of [`Invalidation::HashDrift`] events emitted.
    pub drifts_emitted: usize,
    /// Files the walker failed to read; stringified for portability across
    /// the port boundary.
    pub errors: Vec<String>,
}
