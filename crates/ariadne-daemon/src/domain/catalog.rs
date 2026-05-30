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
use std::path::{Path, PathBuf};

use ariadne_core::{
    Changeset, FileId, Lang, ReadSnapshot, Storage, SymbolId, SymbolRecord, Visibility,
};
use ariadne_graph::{EdgeDelta, EdgeKind, GraphIndex};

use crate::domain::snapshot::WarmSnapshot;
use crate::errors::DaemonError;

const SCAN_CHUNK: usize = 4096;

/// Project-root-relative location of the redb index. Shared by the transport
/// adapter, the live-update engine, and the dump comparator so they all open
/// the same file [src: crates/ariadne-cli/src/domain/mod.rs:42-44].
#[must_use]
pub(crate) fn index_path(project_root: &Path) -> PathBuf {
    project_root.join(".ariadne").join("index.redb")
}

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

    /// Fold a committed [`Changeset`] into the warm catalog (tier-08). The
    /// snapshot mirror, the path/name/metadata indices, and the petgraph are
    /// all advanced in place — the graph via the incremental
    /// [`GraphIndex::apply_delta`] rather than a rebuild (RD6) — and the
    /// catalog revision is bumped to the just-committed `revision`. The
    /// committed changeset carries the full derived upsert set plus exhaustive
    /// stale deletes, so re-adding existing symbols/edges is idempotent and the
    /// result is byte-equal to a fresh build from the committed storage (the
    /// tier-08 divergence-0 proptest is the guard) [src: tier-08 step 4;
    ///  crates/ariadne-salsa/src/db.rs:236-335].
    pub(crate) fn apply_changeset(&mut self, cs: &Changeset, revision: u64) {
        // Mirror the snapshot first so the symbol-lang join below sees the new
        // file records.
        self.snap.apply(cs);

        for fid in &cs.file_deletes {
            if let Some(path) = self.paths.remove(fid) {
                self.path_to_id.remove(&path);
            }
        }
        for (fid, rec) in &cs.file_upserts {
            self.paths.insert(*fid, rec.path.clone());
            self.path_to_id.insert(rec.path.clone(), *fid);
        }

        for sid in &cs.symbol_deletes {
            if let Some(meta) = self.symbols.remove(sid) {
                if let Some(ids) = self.by_name.get_mut(&meta.name) {
                    if let Ok(pos) = ids.binary_search(sid) {
                        ids.remove(pos);
                    }
                    if ids.is_empty() {
                        self.by_name.remove(&meta.name);
                    }
                }
            }
        }
        for (sid, rec) in &cs.symbol_upserts {
            let lang = self
                .snap
                .file(rec.defining_file)
                .ok()
                .flatten()
                .map_or(Lang::Other("unknown"), |f| f.lang);
            // A symbol id encodes its (path, kind, name) (RD12), so an existing
            // id keeps its name — only new ids extend `by_name`, inserted in
            // ascending-id order to match the fresh build's scan order.
            if !self.symbols.contains_key(sid) {
                let ids = self.by_name.entry(rec.canonical_name.clone()).or_default();
                if let Err(pos) = ids.binary_search(sid) {
                    ids.insert(pos, *sid);
                }
            }
            self.symbols
                .insert(*sid, SymbolMeta::from_record(rec, lang));
        }

        let added: Vec<SymbolId> = cs.symbol_upserts.iter().map(|(id, _)| *id).collect();
        let removed: Vec<SymbolId> = cs.symbol_deletes.clone();
        let edge_diff = EdgeDelta {
            added: cs
                .edges_added
                .iter()
                .map(|(k, r)| (k.src, k.dst, EdgeKind::from_core(k.kind), r.weight))
                .collect(),
            removed: cs
                .edges_removed
                .iter()
                .map(|k| (k.src, k.dst, EdgeKind::from_core(k.kind)))
                .collect(),
        };
        self.graph.apply_delta(added, removed, edge_diff);

        self.revision = revision;
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
