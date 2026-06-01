//! Canonical, comparable dump of a [`WarmCatalog`] (tier-08).
//!
//! The divergence-0 proptest drives the [`crate::LiveEngine`] through a random
//! edit sequence and asserts the live-updated catalog equals one built fresh
//! from the committed storage. A direct `WarmCatalog` comparison is awkward
//! (`GraphIndex` is not `PartialEq`), so both sides project into this owned,
//! order-stable value: the full snapshot record sets, the derived
//! path/name/metadata indices, and the petgraph symbol/edge counts. Equality
//! of two dumps is the divergence-0 invariant for the warm layer
//! [src: .claude/plans/post-v1-roadmap/tier-08-daemon-watcher-live.md step 6].

use std::collections::BTreeMap;
use std::path::Path;

use ariadne_core::{
    CoChangePair, EdgeKey, EdgeRecord, FileChurn, FileId, FileRecord, Lang, ReadSnapshot,
    SymbolChurn, SymbolId, SymbolRecord, Visibility,
};
use ariadne_storage::RedbStorage;

use crate::domain::catalog::{SymbolMeta, WarmCatalog, index_path};
use crate::errors::DaemonError;

const SCAN_CHUNK: usize = 4096;

/// One symbol's cached metadata, projected for comparison (the live
/// [`SymbolMeta`] is not `PartialEq`).
#[derive(Debug, PartialEq, Eq)]
struct MetaRow {
    name: String,
    kind: String,
    file: FileId,
    byte_start: u32,
    byte_end: u32,
    lang: Lang,
    visibility: Visibility,
    attributes: Vec<String>,
    complexity: u32,
}

impl MetaRow {
    fn of(m: &SymbolMeta) -> Self {
        Self {
            name: m.name.clone(),
            kind: m.kind.clone(),
            file: m.file,
            byte_start: m.byte_start,
            byte_end: m.byte_end,
            lang: m.lang,
            visibility: m.visibility,
            attributes: m.attributes.clone(),
            complexity: m.complexity,
        }
    }
}

/// Order-stable projection of a warm catalog used to assert divergence 0
/// between the live-update path and a fresh rebuild.
#[derive(Debug, PartialEq, Eq)]
pub struct CatalogDump {
    files: Vec<(FileId, FileRecord)>,
    symbols: Vec<(SymbolId, SymbolRecord)>,
    edges: Vec<(EdgeKey, EdgeRecord)>,
    paths: BTreeMap<FileId, String>,
    path_to_id: BTreeMap<String, FileId>,
    metas: BTreeMap<SymbolId, MetaRow>,
    by_name: BTreeMap<String, Vec<SymbolId>>,
    churn: Vec<FileChurn>,
    co_change: Vec<CoChangePair>,
    symbol_churn: Vec<SymbolChurn>,
    graph_symbols: usize,
    graph_edges: usize,
}

impl CatalogDump {
    /// Project a live `WarmCatalog` into a comparable dump.
    pub(crate) fn of(cat: &WarmCatalog) -> Self {
        Self {
            files: collect(cat.snap.iter_files(SCAN_CHUNK)),
            symbols: collect(cat.snap.iter_symbols(SCAN_CHUNK)),
            edges: collect(cat.snap.iter_edges(SCAN_CHUNK)),
            paths: cat.paths.clone(),
            path_to_id: cat.path_to_id.clone(),
            metas: cat
                .symbols
                .iter()
                .map(|(id, m)| (*id, MetaRow::of(m)))
                .collect(),
            by_name: cat.by_name.clone(),
            churn: cat.churn.clone(),
            co_change: cat.co_change.clone(),
            symbol_churn: cat.symbol_churn.clone(),
            graph_symbols: cat.graph.symbol_count(),
            graph_edges: cat.graph.edge_count(),
        }
    }

    /// Build a fresh `WarmCatalog` from the committed redb at `project_root`
    /// and project it — the reference side of the divergence-0 comparison.
    ///
    /// # Errors
    /// Propagates storage-open / catalog-build failures.
    pub fn from_storage(project_root: &Path) -> Result<Self, DaemonError> {
        let storage = RedbStorage::open(&index_path(project_root))?;
        let catalog = WarmCatalog::build(&storage, project_root.display().to_string())?;
        Ok(Self::of(&catalog))
    }
}

/// Drain a chunked snapshot scan into a flat, owned vec (scan order preserved).
fn collect<T>(
    stream: Result<ariadne_core::ChunkStream<'_, T>, ariadne_core::StorageError>,
) -> Vec<T> {
    let mut out = Vec::new();
    if let Ok(chunks) = stream {
        for items in chunks.flatten() {
            out.extend(items);
        }
    }
    out
}
