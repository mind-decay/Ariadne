//! Port traits — hexagonal contracts ariadne-core declares for adapters to
//! implement. Tier-02 fills out the `Storage` port and the read/write
//! transaction traits; remaining ports stay as empty markers until their
//! tiers land. See `docs/folder-layout.md` `<adding-a-port>`.

use crate::domain::changeset::{Changeset, RevisionId};
use crate::domain::records::{
    CoChangePair, EdgeKey, EdgeRecord, FileChurn, FileRecord, SymbolRecord,
};
use crate::domain::types::{FileId, SymbolId};
use crate::domain::watcher::Invalidation;
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

    /// Wholesale-replace the persisted Git-history derivation: clear the
    /// churn + co-change tables and write `churn` / `pairs` in one backend
    /// transaction. The cold history walk (`ariadne-git`, tier-11) recomputes
    /// the full set on each index, so replacement — not merge — is the
    /// contract; tier-11a layers incremental merge on top.
    ///
    /// # Errors
    /// Returns [`StorageError`] variants for IO or corruption; the write is
    /// rolled back on any error.
    fn replace_history(
        &self,
        churn: &[FileChurn],
        pairs: &[CoChangePair],
    ) -> Result<(), StorageError>;

    /// Read every persisted [`FileChurn`] record.
    ///
    /// # Errors
    /// Backend-level IO or corruption.
    fn all_churn(&self) -> Result<Vec<FileChurn>, StorageError>;

    /// Read every persisted [`CoChangePair`] record.
    ///
    /// # Errors
    /// Backend-level IO or corruption.
    fn all_co_change(&self) -> Result<Vec<CoChangePair>, StorageError>;
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

/// Lazy stream of decoded chunks. Each item is one chunk; chunk size is
/// the caller-supplied `chunk_size` argument on the originating
/// `iter_*` call (the final chunk may be short). Boxed so the trait
/// stays dyn-compatible.
pub type ChunkStream<'a, T> = Box<dyn Iterator<Item = Result<Vec<T>, StorageError>> + 'a>;

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

    /// Stream every file record in lazy chunks. Each yielded item is a
    /// `Vec` of up to `chunk_size` decoded `(FileId, FileRecord)` pairs;
    /// the final chunk may be short. Callers materialise at most one
    /// chunk in RAM at a time so the cold-start 100K-file / 10M-LOC
    /// scan respects the workspace memory ceiling.
    ///
    /// # Errors
    /// Backend-level IO or corruption.
    fn iter_files(
        &self,
        chunk_size: usize,
    ) -> Result<ChunkStream<'_, (FileId, FileRecord)>, StorageError>;

    /// Stream every symbol record in lazy chunks (same contract as
    /// [`ReadSnapshot::iter_files`]).
    ///
    /// # Errors
    /// Backend-level IO or corruption.
    fn iter_symbols(
        &self,
        chunk_size: usize,
    ) -> Result<ChunkStream<'_, (SymbolId, SymbolRecord)>, StorageError>;

    /// Stream every edge record in lazy chunks (same contract as
    /// [`ReadSnapshot::iter_files`]).
    ///
    /// # Errors
    /// Backend-level IO or corruption.
    fn iter_edges(
        &self,
        chunk_size: usize,
    ) -> Result<ChunkStream<'_, (EdgeKey, EdgeRecord)>, StorageError>;
}

/// Parsing port. Implemented by `ariadne-parser` (tree-sitter) in tier-03.
pub trait Parser {}

/// Semantic indexing port. Implemented by `ariadne-scip` in tier-05.
pub trait Indexer {}

/// File-system event sink port. The driving watcher in `ariadne-watcher`
/// pushes [`Invalidation`]s here; downstream sinks translate them into
/// salsa input updates (tier-06). The `Send` bound is required so the
/// watcher can hand the sink across to its event thread.
pub trait WatcherSink: Send {
    /// Apply a single invalidation. Implementations log internal failures
    /// (IO during file read, lock contention) rather than propagating —
    /// the watcher must keep running.
    fn apply_invalidation(&mut self, invalidation: &Invalidation);
}
