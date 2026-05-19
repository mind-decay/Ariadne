//! redb-backed implementation of the `Storage` port.
//!
//! Tables: `META` (`&str -> u64`), `FILES` / `SYMBOLS` / `EDGES`
//! (`&[u8] -> &[u8]`), and the `EDGES_BY_FILE` multimap
//! (`&[u8] -> &[u8]`). Record bodies are postcard-encoded; keys are
//! fixed-width big-endian via [`ariadne_core::IdEncode`], so redb's default
//! `&[u8]` comparator already gives the lex-order-equals-numeric-order
//! property the design relies on.
//!
//! Implementation is split into submodules to stay inside the project's
//! 200-line authoring cap (see `CLAUDE.md`): `tables.rs` for definition
//! constants, `apply.rs` for the single-txn write path, and `snapshot.rs`
//! for read accessors. The folder name matches the external tech (one tech
//! per `adapters/<tech>` location).

mod apply;
mod scan;
mod snapshot;
mod tables;

pub(crate) use tables::SCHEMA_VERSION;

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use ariadne_core::{
    Changeset, ChunkStream, EdgeKey, EdgeRecord, FileId, FileRecord, ReadSnapshot, RevisionId,
    Storage, StorageError, SymbolId, SymbolRecord, WriteTxn,
};
use redb::{Database, ReadTransaction, ReadableDatabase, ReadableTable, WriteTransaction};

use crate::errors::RedbStorageError;
use tables::{EDGES, EDGES_BY_FILE, FILES, KEY_REVISION, KEY_SCHEMA_VERSION, META, SYMBOLS};

/// redb-backed [`Storage`] implementation. Owns the `Database` handle and a
/// cached revision counter shared with the latest committed write txn.
#[derive(Debug, Clone)]
pub struct RedbStorage {
    db: Arc<Database>,
    revision: Arc<AtomicU64>,
}

impl RedbStorage {
    /// Open (or create) a redb database at `path`. Bootstraps the `META`
    /// table on first use; returns [`StorageError::SchemaMismatch`] when the
    /// on-disk schema version differs from the running binary.
    ///
    /// # Errors
    /// Propagates filesystem / redb / schema-mismatch failures.
    pub fn open(path: &Path) -> Result<Self, StorageError> {
        Self::open_inner(path).map_err(Into::into)
    }

    fn open_inner(path: &Path) -> Result<Self, RedbStorageError> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        let db = Database::create(path)?;
        let revision = bootstrap(&db)?;
        Ok(Self {
            db: Arc::new(db),
            revision: Arc::new(AtomicU64::new(revision)),
        })
    }
}

fn bootstrap(db: &Database) -> Result<u64, RedbStorageError> {
    let txn = db.begin_write()?;
    let rev = {
        let mut meta = txn.open_table(META)?;
        let existing = meta.get(KEY_SCHEMA_VERSION)?.map(|g| g.value());
        match existing {
            Some(v) if v != SCHEMA_VERSION => {
                return Err(RedbStorageError::SchemaMismatch {
                    found: v,
                    expected: SCHEMA_VERSION,
                });
            }
            Some(_) => {}
            None => {
                meta.insert(KEY_SCHEMA_VERSION, &SCHEMA_VERSION)?;
            }
        }
        meta.get(KEY_REVISION)?.map_or(0, |g| g.value())
    };
    // Bootstrap remaining tables so a fresh DB is immediately readable.
    let _ = txn.open_table(FILES)?;
    let _ = txn.open_table(SYMBOLS)?;
    let _ = txn.open_table(EDGES)?;
    let _ = txn.open_multimap_table(EDGES_BY_FILE)?;
    txn.commit()?;
    Ok(rev)
}

impl Storage for RedbStorage {
    type Write<'a>
        = RedbWriteTxn
    where
        Self: 'a;
    type Read<'a>
        = RedbReadSnapshot
    where
        Self: 'a;

    fn begin_write(&self) -> Result<Self::Write<'_>, StorageError> {
        let txn = self.db.begin_write().map_err(RedbStorageError::from)?;
        Ok(RedbWriteTxn {
            txn,
            revision: Arc::clone(&self.revision),
        })
    }

    fn snapshot(&self) -> Result<Self::Read<'_>, StorageError> {
        let txn = self.db.begin_read().map_err(RedbStorageError::from)?;
        Ok(RedbReadSnapshot { txn })
    }

    fn revision(&self) -> RevisionId {
        RevisionId(self.revision.load(Ordering::Acquire))
    }
}

/// Owned write transaction. Consumed by [`WriteTxn::apply`].
pub struct RedbWriteTxn {
    txn: WriteTransaction,
    revision: Arc<AtomicU64>,
}

impl std::fmt::Debug for RedbWriteTxn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RedbWriteTxn").finish_non_exhaustive()
    }
}

impl WriteTxn for RedbWriteTxn {
    fn apply(self, changeset: &Changeset) -> Result<RevisionId, StorageError> {
        let Self { txn, revision } = self;
        let new_revision = apply::apply_writes(&txn, changeset).map_err(StorageError::from)?;
        txn.commit()
            .map_err(|e| StorageError::from(RedbStorageError::from(e)))?;
        revision.store(new_revision, Ordering::Release);
        Ok(RevisionId(new_revision))
    }
}

/// Owned MVCC read snapshot. Survives past concurrent writer commits.
pub struct RedbReadSnapshot {
    txn: ReadTransaction,
}

impl std::fmt::Debug for RedbReadSnapshot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RedbReadSnapshot").finish_non_exhaustive()
    }
}

impl ReadSnapshot for RedbReadSnapshot {
    fn file(&self, id: FileId) -> Result<Option<FileRecord>, StorageError> {
        snapshot::file(&self.txn, id).map_err(Into::into)
    }
    fn symbols_in_file(&self, id: FileId) -> Result<Vec<SymbolRecord>, StorageError> {
        snapshot::symbols_in_file(&self.txn, id).map_err(Into::into)
    }
    fn outgoing_edges(&self, src: SymbolId) -> Result<Vec<(EdgeKey, EdgeRecord)>, StorageError> {
        snapshot::outgoing(&self.txn, src).map_err(Into::into)
    }
    fn incoming_edges(&self, dst: SymbolId) -> Result<Vec<(EdgeKey, EdgeRecord)>, StorageError> {
        snapshot::incoming(&self.txn, dst).map_err(Into::into)
    }
    fn edges_in_file(&self, file: FileId) -> Result<Vec<EdgeKey>, StorageError> {
        snapshot::edges_in_file(&self.txn, file).map_err(Into::into)
    }
    fn iter_files(
        &self,
        chunk_size: usize,
    ) -> Result<ChunkStream<'_, (FileId, FileRecord)>, StorageError> {
        scan::iter_files(&self.txn, chunk_size)
    }
    fn iter_symbols(
        &self,
        chunk_size: usize,
    ) -> Result<ChunkStream<'_, (SymbolId, SymbolRecord)>, StorageError> {
        scan::iter_symbols(&self.txn, chunk_size)
    }
    fn iter_edges(
        &self,
        chunk_size: usize,
    ) -> Result<ChunkStream<'_, (EdgeKey, EdgeRecord)>, StorageError> {
        scan::iter_edges(&self.txn, chunk_size)
    }
}
