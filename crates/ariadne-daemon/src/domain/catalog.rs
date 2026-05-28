//! The daemon's warm in-RAM catalog: the petgraph `GraphIndex` plus the
//! derived name / path / metadata indices every query resolves against,
//! and the `WarmSnapshot` mirror that backs edge-span and `docgen` reads.
//!
//! Mirrors the projection the v1 MCP `Catalog` builds, so daemon-served
//! results match the cold path. Built by streaming a storage snapshot in
//! 4096-record chunks; the caller (the transport adapter) opens redb,
//! builds this, then drops the handle
//! [src: .claude/plans/post-v1-roadmap/tier-07-daemon-warm-graph.md step 4;
//!  crates/ariadne-mcp/src/catalog.rs].

use std::collections::BTreeMap;

use ariadne_core::{FileId, Lang, ReadSnapshot, Storage, SymbolId, SymbolRecord, Visibility};
use ariadne_graph::GraphIndex;

use crate::domain::snapshot::WarmSnapshot;
use crate::errors::DaemonError;

const SCAN_CHUNK: usize = 4096;

/// Per-symbol cached metadata — the fields the queries need, owned so the
/// catalog outlives the storage handle it was built from.
#[derive(Debug, Clone)]
pub(crate) struct SymbolMeta {
    /// Canonical name.
    pub(crate) name: String,
    /// Free-form kind tag.
    pub(crate) kind: String,
    /// File the defining occurrence lives in.
    pub(crate) file: FileId,
    /// Defining-span byte start.
    pub(crate) byte_start: u32,
    /// Defining-span byte end.
    pub(crate) byte_end: u32,
    /// Language of the defining file (joined for the dead-code classifier).
    pub(crate) lang: Lang,
    /// Visibility lattice on the defining occurrence.
    pub(crate) visibility: Visibility,
    /// Attribute / annotation / decorator identifiers on the declaration.
    pub(crate) attributes: Vec<String>,
}

impl SymbolMeta {
    fn from_record(rec: &SymbolRecord, lang: Lang) -> Self {
        Self {
            name: rec.canonical_name.clone(),
            kind: rec.kind.clone(),
            file: rec.defining_file,
            byte_start: rec.defining_span.byte_start,
            byte_end: rec.defining_span.byte_end,
            lang,
            visibility: rec.visibility,
            attributes: rec.attributes.clone(),
        }
    }
}

/// Read-only warm catalog driving all daemon queries.
#[derive(Debug)]
pub(crate) struct WarmCatalog {
    /// In-RAM snapshot mirror backing edge-span and `docgen` reads.
    pub(crate) snap: WarmSnapshot,
    /// In-RAM analytics graph.
    pub(crate) graph: GraphIndex,
    /// [`FileId`] → project-root-relative path.
    pub(crate) paths: BTreeMap<FileId, String>,
    /// Path → [`FileId`] reverse map.
    pub(crate) path_to_id: BTreeMap<String, FileId>,
    /// [`SymbolId`] → per-symbol metadata.
    pub(crate) symbols: BTreeMap<SymbolId, SymbolMeta>,
    /// Canonical name → all [`SymbolId`]s carrying it.
    pub(crate) by_name: BTreeMap<String, Vec<SymbolId>>,
    /// Persisted redb revision this catalog was built from.
    pub(crate) revision: u64,
    /// Project root the daemon was launched against (for `project_status`).
    pub(crate) root: String,
}

impl WarmCatalog {
    /// Build a warm catalog from a `Storage` snapshot. Streams files +
    /// symbols in chunks (constant working-set memory).
    ///
    /// # Errors
    /// Propagates storage scan and graph-build failures.
    pub(crate) fn build<S: Storage>(storage: &S, root: String) -> Result<Self, DaemonError> {
        let redb_snap = storage.snapshot()?;
        let snap = WarmSnapshot::from_snapshot(&redb_snap)?;
        drop(redb_snap);
        let graph = GraphIndex::build_from_snapshot(&snap)?;

        let mut paths = BTreeMap::new();
        let mut path_to_id = BTreeMap::new();
        let mut lang_of: BTreeMap<FileId, Lang> = BTreeMap::new();
        for chunk in snap.iter_files(SCAN_CHUNK)? {
            for (id, rec) in chunk? {
                path_to_id.insert(rec.path.clone(), id);
                lang_of.insert(id, rec.lang);
                paths.insert(id, rec.path);
            }
        }

        let mut symbols: BTreeMap<SymbolId, SymbolMeta> = BTreeMap::new();
        let mut by_name: BTreeMap<String, Vec<SymbolId>> = BTreeMap::new();
        for chunk in snap.iter_symbols(SCAN_CHUNK)? {
            for (id, rec) in chunk? {
                let lang = lang_of
                    .get(&rec.defining_file)
                    .copied()
                    .unwrap_or(Lang::Other("unknown"));
                let meta = SymbolMeta::from_record(&rec, lang);
                by_name.entry(meta.name.clone()).or_default().push(id);
                symbols.insert(id, meta);
            }
        }

        let revision = storage.revision().0;
        Ok(Self {
            snap,
            graph,
            paths,
            path_to_id,
            symbols,
            by_name,
            revision,
            root,
        })
    }

    /// First symbol carrying `name`, if any.
    pub(crate) fn find_symbol(&self, name: &str) -> Option<SymbolId> {
        self.by_name.get(name).and_then(|v| v.first().copied())
    }

    /// File path for a [`FileId`].
    pub(crate) fn path_of(&self, file: FileId) -> Option<&str> {
        self.paths.get(&file).map(String::as_str)
    }

    /// Metadata for a symbol.
    pub(crate) fn meta_of(&self, id: SymbolId) -> Option<&SymbolMeta> {
        self.symbols.get(&id)
    }

    /// Resolve `SymbolId → FileId`.
    pub(crate) fn file_of(&self, id: SymbolId) -> Option<FileId> {
        self.symbols.get(&id).map(|m| m.file)
    }

    /// Whether a request expecting redb `revision` outruns the revision this
    /// catalog was built from — i.e. the on-disk index advanced and the warm
    /// graph is stale, so the daemon must rebuild before answering (R-B2).
    /// `revision == 0` (liveness probes, revision-unaware clients) is never
    /// stale.
    pub(crate) fn is_stale(&self, revision: u64) -> bool {
        revision > self.revision
    }
}
