//! Port traits — hexagonal contracts ariadne-core declares for adapters to
//! implement. Tier-02 fills out the `Storage` port and the read/write
//! transaction traits; remaining ports stay as empty markers until their
//! tiers land. See `docs/folder-layout.md` `<adding-a-port>`.

use crate::domain::changeset::{Changeset, RevisionId};
use crate::domain::records::{EdgeKey, EdgeRecord, FileRecord, SymbolRecord};
use crate::domain::types::{FileId, SymbolId};
use crate::errors::StorageError;

/// Persistent storage port. Implemented by `ariadne-storage` (redb) in
/// tier-02. Implementations must be safe to share across threads — the
/// MVCC reader/writer protocol enforced by [`Storage::snapshot`] /
/// [`Storage::begin_write`] is the only synchronization primitive
/// downstream tiers rely on.
pub trait Storage: Send + Sync {
    /// Concrete write transaction type returned by [`Storage::begin_write`].
    /// The GAT lifetime keeps the adapter free to tie the txn to the
    /// backing store without that lifetime leaking into [`StorageError`].
    type Write<'a>: WriteTxn + 'a
    where
        Self: 'a;

    /// Concrete read snapshot type returned by [`Storage::snapshot`].
    type Read<'a>: ReadSnapshot + 'a
    where
        Self: 'a;

    /// Open a new write transaction. Only one is permitted at a time;
    /// adapters block (or error) on concurrent calls.
    ///
    /// # Errors
    /// Returns [`StorageError::Io`] for backend IO failures or
    /// [`StorageError::Corrupted`] when the on-disk format is unreadable.
    fn begin_write(&self) -> Result<Self::Write<'_>, StorageError>;

    /// Open an MVCC read snapshot. Concurrent with other reads and with the
    /// single in-flight write.
    ///
    /// # Errors
    /// Same shape as [`Storage::begin_write`].
    fn snapshot(&self) -> Result<Self::Read<'_>, StorageError>;

    /// Cached revision of the latest committed write.
    fn revision(&self) -> RevisionId;
}

/// Atomic write half of the [`Storage`] port.
pub trait WriteTxn {
    /// Apply the changeset and commit. Adapters must perform the write in a
    /// single backend transaction (auto-rollback on failure).
    ///
    /// # Errors
    /// Returns [`StorageError`] variants for IO or corruption; the txn is
    /// rolled back on any error.
    fn apply(self, changeset: &Changeset) -> Result<RevisionId, StorageError>;
}

/// Read half of the [`Storage`] port. All accessors take `&self` so the
/// snapshot can be shared across multiple query threads.
pub trait ReadSnapshot {
    /// Look up a file record by id.
    ///
    /// # Errors
    /// Backend-level IO or corruption.
    fn file(&self, id: FileId) -> Result<Option<FileRecord>, StorageError>;

    /// Symbols whose defining occurrence lives in `id`.
    ///
    /// # Errors
    /// Backend-level IO or corruption.
    fn symbols_in_file(&self, id: FileId) -> Result<Vec<SymbolRecord>, StorageError>;

    /// All edges with `src` as source, paired with their bodies.
    ///
    /// # Errors
    /// Backend-level IO or corruption.
    fn outgoing_edges(&self, src: SymbolId) -> Result<Vec<(EdgeKey, EdgeRecord)>, StorageError>;

    /// All edges with `dst` as destination, paired with their bodies.
    ///
    /// # Errors
    /// Backend-level IO or corruption.
    fn incoming_edges(&self, dst: SymbolId) -> Result<Vec<(EdgeKey, EdgeRecord)>, StorageError>;

    /// All edges whose `source_span.file` equals `file`. Drives watcher
    /// invalidation in tier-06.
    ///
    /// # Errors
    /// Backend-level IO or corruption.
    fn edges_in_file(&self, file: FileId) -> Result<Vec<EdgeKey>, StorageError>;
}

/// Parsing port. Implemented by `ariadne-parser` (tree-sitter) in tier-03.
pub trait Parser {}

/// Semantic indexing port. Implemented by `ariadne-scip` in tier-05.
pub trait Indexer {}

/// File-system event sink port. Implemented by `ariadne-watcher` (notify)
/// in tier-06.
pub trait WatcherSink {}
