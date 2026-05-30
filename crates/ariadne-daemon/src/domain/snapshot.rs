//! In-RAM mirror of a storage [`ReadSnapshot`].
//!
//! The warm daemon materialises the redb snapshot once at (re)build time
//! into this struct, then drops the storage handle. Every query then reads
//! through this RAM mirror — `find_references`/`file_summary` need edge
//! source spans and `docgen` re-scans symbols/files, none of which the
//! petgraph `GraphIndex` carries — so the daemon never cold-reads redb
//! between refreshes (RD6) and never holds the single-open redb lock while
//! idle, which is what lets an external indexer advance the file and the
//! staleness handshake observe it
//! [src: .claude/plans/post-v1-roadmap/tier-07-daemon-warm-graph.md step 4].
//!
//! tier-08 makes the mirror live: [`WarmSnapshot::apply`] folds a committed
//! [`Changeset`] into the maps in place so the daemon's update pipeline keeps
//! it current without a full rebuild [src: tier-08 step 4]. Edge bodies live
//! in a `BTreeMap<EdgeKey, EdgeRecord>` and the per-source / per-destination /
//! per-file indices hold `BTreeSet<EdgeKey>`; `EdgeKey`'s derived `Ord` equals
//! the storage `to_bytes` key order (big-endian ids + kind byte), so iterating
//! a `BTreeSet` reproduces the storage scan order the accessors must preserve
//! [src: crates/ariadne-core/src/domain/records.rs:90-113].

use std::collections::{BTreeMap, BTreeSet};

use ariadne_core::{
    Changeset, ChunkStream, EdgeKey, EdgeRecord, FileId, FileRecord, ReadSnapshot, StorageError,
    SymbolId, SymbolRecord,
};

/// Chunk size for the streaming scans (mirrors `ariadne-graph::build`).
const SCAN_CHUNK: usize = 4096;

/// Owned, in-RAM [`ReadSnapshot`]. Edge bodies are stored once in `edges`,
/// keyed by [`EdgeKey`]; the per-source / per-destination / per-file maps hold
/// the keys, so a `BTreeSet` iteration yields them in storage scan order.
#[derive(Debug, Default)]
pub(crate) struct WarmSnapshot {
    files: BTreeMap<FileId, FileRecord>,
    symbols: BTreeMap<SymbolId, SymbolRecord>,
    edges: BTreeMap<EdgeKey, EdgeRecord>,
    out_idx: BTreeMap<SymbolId, BTreeSet<EdgeKey>>,
    in_idx: BTreeMap<SymbolId, BTreeSet<EdgeKey>>,
    file_edge_idx: BTreeMap<FileId, BTreeSet<EdgeKey>>,
}

impl WarmSnapshot {
    /// Drain a storage snapshot's files, symbols, and edges into RAM.
    ///
    /// # Errors
    /// Propagates [`StorageError`] from the underlying snapshot scans.
    pub(crate) fn from_snapshot(snap: &dyn ReadSnapshot) -> Result<Self, StorageError> {
        let mut out = Self::default();
        for chunk in snap.iter_files(SCAN_CHUNK)? {
            for (id, rec) in chunk? {
                out.files.insert(id, rec);
            }
        }
        for chunk in snap.iter_symbols(SCAN_CHUNK)? {
            for (id, rec) in chunk? {
                out.symbols.insert(id, rec);
            }
        }
        for chunk in snap.iter_edges(SCAN_CHUNK)? {
            for (key, rec) in chunk? {
                out.insert_edge(key, rec);
            }
        }
        Ok(out)
    }

    /// Fold a committed [`Changeset`] into the mirror in place (tier-08). The
    /// changeset's delete vectors are exhaustive — the diff-aware committer
    /// emits a delete for every persisted symbol/edge no longer derived, so
    /// processing each vector independently (no file-delete cascade) reproduces
    /// the storage `WriteTxn::apply` end-state and keeps the mirror byte-equal
    /// to a fresh rebuild (the tier-08 divergence-0 proptest is the guard)
    /// [src: crates/ariadne-salsa/src/db.rs:341-378;
    ///  crates/ariadne-storage/src/adapters/redb/apply.rs].
    pub(crate) fn apply(&mut self, cs: &Changeset) {
        for &fid in &cs.file_deletes {
            self.files.remove(&fid);
        }
        for (fid, rec) in &cs.file_upserts {
            self.files.insert(*fid, rec.clone());
        }
        for (sid, rec) in &cs.symbol_upserts {
            self.symbols.insert(*sid, rec.clone());
        }
        for sid in &cs.symbol_deletes {
            self.symbols.remove(sid);
        }
        for key in &cs.edges_removed {
            self.remove_edge(*key);
        }
        for (key, rec) in &cs.edges_added {
            self.insert_edge(*key, rec.clone());
        }
    }

    /// Insert one edge body and register it in the three indices.
    fn insert_edge(&mut self, key: EdgeKey, rec: EdgeRecord) {
        self.out_idx.entry(key.src).or_default().insert(key);
        self.in_idx.entry(key.dst).or_default().insert(key);
        self.file_edge_idx
            .entry(rec.source_span.file)
            .or_default()
            .insert(key);
        self.edges.insert(key, rec);
    }

    /// Remove one edge body and drop it from the three indices. The file index
    /// is keyed by the stored body's `source_span.file`, so the body is read
    /// before removal. Empty index buckets are pruned to keep iteration order
    /// identical to a fresh build.
    fn remove_edge(&mut self, key: EdgeKey) {
        let Some(rec) = self.edges.remove(&key) else {
            return;
        };
        Self::drop_from(&mut self.out_idx, &key.src, key);
        Self::drop_from(&mut self.in_idx, &key.dst, key);
        Self::drop_from(&mut self.file_edge_idx, &rec.source_span.file, key);
    }

    fn drop_from<K: Ord>(map: &mut BTreeMap<K, BTreeSet<EdgeKey>>, bucket: &K, key: EdgeKey) {
        if let Some(set) = map.get_mut(bucket) {
            set.remove(&key);
            if set.is_empty() {
                map.remove(bucket);
            }
        }
    }

    /// Clone the edges named by `keys` (in `BTreeSet` order = scan order).
    fn pick(&self, keys: Option<&BTreeSet<EdgeKey>>) -> Vec<(EdgeKey, EdgeRecord)> {
        keys.map(|set| set.iter().map(|k| (*k, self.edges[k].clone())).collect())
            .unwrap_or_default()
    }
}

/// Clone a slice of items into a chunked, owned stream. Each yielded chunk
/// is at most `chunk` items; the final chunk may be short.
fn chunked<T: Clone + 'static>(data: &[T], chunk: usize) -> ChunkStream<'static, T> {
    let chunk = chunk.max(1);
    let parts: Vec<Result<Vec<T>, StorageError>> =
        data.chunks(chunk).map(|c| Ok(c.to_vec())).collect();
    Box::new(parts.into_iter())
}

impl ReadSnapshot for WarmSnapshot {
    fn file(&self, id: FileId) -> Result<Option<FileRecord>, StorageError> {
        Ok(self.files.get(&id).cloned())
    }

    fn symbols_in_file(&self, id: FileId) -> Result<Vec<SymbolRecord>, StorageError> {
        Ok(self
            .symbols
            .values()
            .filter(|r| r.defining_file == id)
            .cloned()
            .collect())
    }

    fn outgoing_edges(&self, src: SymbolId) -> Result<Vec<(EdgeKey, EdgeRecord)>, StorageError> {
        Ok(self.pick(self.out_idx.get(&src)))
    }

    fn incoming_edges(&self, dst: SymbolId) -> Result<Vec<(EdgeKey, EdgeRecord)>, StorageError> {
        Ok(self.pick(self.in_idx.get(&dst)))
    }

    fn edges_in_file(&self, file: FileId) -> Result<Vec<EdgeKey>, StorageError> {
        Ok(self
            .file_edge_idx
            .get(&file)
            .map(|set| set.iter().copied().collect())
            .unwrap_or_default())
    }

    fn iter_files(
        &self,
        chunk_size: usize,
    ) -> Result<ChunkStream<'_, (FileId, FileRecord)>, StorageError> {
        let data: Vec<(FileId, FileRecord)> =
            self.files.iter().map(|(id, r)| (*id, r.clone())).collect();
        Ok(chunked(&data, chunk_size))
    }

    fn iter_symbols(
        &self,
        chunk_size: usize,
    ) -> Result<ChunkStream<'_, (SymbolId, SymbolRecord)>, StorageError> {
        let data: Vec<(SymbolId, SymbolRecord)> = self
            .symbols
            .iter()
            .map(|(id, r)| (*id, r.clone()))
            .collect();
        Ok(chunked(&data, chunk_size))
    }

    fn iter_edges(
        &self,
        chunk_size: usize,
    ) -> Result<ChunkStream<'_, (EdgeKey, EdgeRecord)>, StorageError> {
        let data: Vec<(EdgeKey, EdgeRecord)> =
            self.edges.iter().map(|(k, r)| (*k, r.clone())).collect();
        Ok(chunked(&data, chunk_size))
    }
}
