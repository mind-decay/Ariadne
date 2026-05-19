//! Pure changeset value type. `Storage::WriteTxn::apply` consumes one of
//! these per atomic commit.

use serde::{Deserialize, Serialize};

use super::records::{EdgeKey, EdgeRecord, FileRecord, SymbolRecord};
use super::types::{FileId, SymbolId};

/// Monotonic revision tag emitted by [`crate::WriteTxn::apply`] per commit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct RevisionId(pub u64);

/// All upserts and deletes that compose a single atomic commit. Field order
/// is the apply order — see [`crate::WriteTxn::apply`] for the contract.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Changeset {
    /// Files to insert or replace.
    pub file_upserts: Vec<(FileId, FileRecord)>,
    /// Files to remove (transitively drops symbols + edges keyed by them).
    pub file_deletes: Vec<FileId>,
    /// Symbols to insert or replace.
    pub symbol_upserts: Vec<(SymbolId, SymbolRecord)>,
    /// Symbols to remove.
    pub symbol_deletes: Vec<SymbolId>,
    /// Edges to insert.
    pub edges_added: Vec<(EdgeKey, EdgeRecord)>,
    /// Edges to remove.
    pub edges_removed: Vec<EdgeKey>,
}

impl Changeset {
    /// Builder constructor.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Push a file upsert.
    #[must_use]
    pub fn upsert_file(mut self, id: FileId, rec: FileRecord) -> Self {
        self.file_upserts.push((id, rec));
        self
    }

    /// Push a file delete.
    #[must_use]
    pub fn delete_file(mut self, id: FileId) -> Self {
        self.file_deletes.push(id);
        self
    }

    /// Push a symbol upsert.
    #[must_use]
    pub fn upsert_symbol(mut self, id: SymbolId, rec: SymbolRecord) -> Self {
        self.symbol_upserts.push((id, rec));
        self
    }

    /// Push a symbol delete.
    #[must_use]
    pub fn delete_symbol(mut self, id: SymbolId) -> Self {
        self.symbol_deletes.push(id);
        self
    }

    /// Push an edge add.
    #[must_use]
    pub fn add_edge(mut self, key: EdgeKey, rec: EdgeRecord) -> Self {
        self.edges_added.push((key, rec));
        self
    }

    /// Push an edge remove.
    #[must_use]
    pub fn remove_edge(mut self, key: EdgeKey) -> Self {
        self.edges_removed.push(key);
        self
    }
}
