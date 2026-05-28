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

use std::collections::BTreeMap;

use ariadne_core::{
    ChunkStream, EdgeKey, EdgeRecord, FileId, FileRecord, ReadSnapshot, StorageError, SymbolId,
    SymbolRecord,
};

/// Chunk size for the streaming scans (mirrors `ariadne-graph::build`).
const SCAN_CHUNK: usize = 4096;

/// Owned, in-RAM [`ReadSnapshot`]. Edge bodies are stored once in
/// `all_edges`; the per-source / per-destination / per-file maps hold
/// indices into that vec, preserving the storage `(src, kind, dst)` scan
/// order so accessor output is deterministic.
#[derive(Debug, Default)]
pub(crate) struct WarmSnapshot {
    files: BTreeMap<FileId, FileRecord>,
    symbols: BTreeMap<SymbolId, SymbolRecord>,
    all_edges: Vec<(EdgeKey, EdgeRecord)>,
    out_idx: BTreeMap<SymbolId, Vec<usize>>,
    in_idx: BTreeMap<SymbolId, Vec<usize>>,
    file_edge_idx: BTreeMap<FileId, Vec<usize>>,
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
                let i = out.all_edges.len();
                out.out_idx.entry(key.src).or_default().push(i);
                out.in_idx.entry(key.dst).or_default().push(i);
                out.file_edge_idx
                    .entry(rec.source_span.file)
                    .or_default()
                    .push(i);
                out.all_edges.push((key, rec));
            }
        }
        Ok(out)
    }

    fn pick(&self, idxs: Option<&Vec<usize>>) -> Vec<(EdgeKey, EdgeRecord)> {
        idxs.map(|v| v.iter().map(|&i| self.all_edges[i].clone()).collect())
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
            .map(|v| v.iter().map(|&i| self.all_edges[i].0).collect())
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
        Ok(chunked(&self.all_edges, chunk_size))
    }
}
