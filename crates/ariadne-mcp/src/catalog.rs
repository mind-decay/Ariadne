//! In-RAM derived indices the MCP tools query against. Built once at
//! `serve_stdio` startup (cold start <100ms on 10K files — tier-08
//! `exit_criteria`) by streaming the redb snapshot and the petgraph
//! `GraphIndex`.
//!
//! Tier-08 keeps these indices immutable for a server lifetime; tier-10
//! orchestration will trigger a rebuild whenever the watcher commits a
//! revision (`Storage::revision()` bump). The MCP-side data path stays
//! read-only.

use std::collections::BTreeMap;

use ariadne_core::{
    CoChangePair, FileChurn, FileId, Lang, ReadSnapshot, Storage, SymbolChurn, SymbolId,
    SymbolRecord, Visibility,
};
use ariadne_graph::GraphIndex;

use crate::errors::McpError;

const SCAN_CHUNK: usize = 4096;

/// Per-symbol cached metadata. The records returned by `Storage::iter_symbols`
/// own their strings — we keep just the fields the tools need.
#[derive(Debug, Clone)]
pub struct SymbolMeta {
    /// Canonical name.
    pub name: String,
    /// Free-form kind tag.
    pub kind: String,
    /// File the defining occurrence lives in.
    pub file: FileId,
    /// Defining span `byte_start`.
    pub byte_start: u32,
    /// Defining span `byte_end`.
    pub byte_end: u32,
    /// Language of the defining file. Joined from
    /// [`ariadne_core::FileRecord::lang`] at catalog build time so the
    /// tier-05 dead-code classifier can run on the production path.
    pub lang: Lang,
    /// Visibility lattice on the defining occurrence.
    pub visibility: Visibility,
    /// Attribute / annotation / decorator identifiers attached to the
    /// declaration (e.g. `"test"`, `"Override"`, `"pytest.fixture"`).
    pub attributes: Vec<String>,
    /// `McCabe` cyclomatic complexity of the defining occurrence
    /// (`decisions + 1`, `>= 1` for function-like symbols; `0` otherwise),
    /// carried so the Block-C analytics tools read it straight from RAM
    /// [src: post-v1-roadmap plan.md RD8].
    pub complexity: u32,
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
            complexity: rec.complexity,
        }
    }
}

/// Read-only catalog driving all MCP tools.
#[derive(Debug)]
pub struct Catalog {
    /// [`FileId`] → project-root-relative path.
    pub paths: BTreeMap<FileId, String>,
    /// Path → [`FileId`] reverse map.
    pub path_to_id: BTreeMap<String, FileId>,
    /// [`SymbolId`] → per-symbol metadata.
    pub symbols: BTreeMap<SymbolId, SymbolMeta>,
    /// Canonical name → all [`SymbolId`]s with that name. Multiple entries
    /// are possible across files; the tools surface the full list and let
    /// the caller disambiguate.
    pub by_name: BTreeMap<String, Vec<SymbolId>>,
    /// In-RAM graph: `(src, kind, dst)` adjacency for analytics.
    pub graph: GraphIndex,
    /// Per-file Git-history churn, sorted by `path`. Loaded wholesale from the
    /// `Storage` port (tier-15a D1/D3) so the Block-C hotspot tools read it
    /// from RAM.
    pub churn: Vec<FileChurn>,
    /// File-pair co-change records, sorted by `(a, b)`.
    pub co_change: Vec<CoChangePair>,
    /// Per-symbol Git-history churn, sorted by `symbol`. Symbols absent from
    /// the table have zero attributed churn.
    pub symbol_churn: Vec<SymbolChurn>,
    /// Latest persisted revision the catalog snapshotted from.
    pub revision: u64,
    /// Project root the server was launched against.
    pub root: String,
}

impl Catalog {
    /// Build a fresh catalog from a `Storage` snapshot. Streams files +
    /// symbols in 4096-record chunks (constant working-set memory).
    ///
    /// # Errors
    /// Propagates [`ariadne_core::StorageError`] from snapshot scans and
    /// [`ariadne_graph::GraphError`] from the petgraph build step.
    pub fn build<S: Storage>(storage: &S, root: String) -> Result<Self, McpError> {
        let snap = storage.snapshot().map_err(McpError::Storage)?;
        let mut paths = BTreeMap::new();
        let mut path_to_id = BTreeMap::new();
        let mut lang_of: BTreeMap<FileId, Lang> = BTreeMap::new();
        for chunk in snap.iter_files(SCAN_CHUNK).map_err(McpError::Storage)? {
            for (id, rec) in chunk.map_err(McpError::Storage)? {
                path_to_id.insert(rec.path.clone(), id);
                lang_of.insert(id, rec.lang);
                paths.insert(id, rec.path);
            }
        }

        let mut symbols: BTreeMap<SymbolId, SymbolMeta> = BTreeMap::new();
        let mut by_name: BTreeMap<String, Vec<SymbolId>> = BTreeMap::new();
        for chunk in snap.iter_symbols(SCAN_CHUNK).map_err(McpError::Storage)? {
            for (id, rec) in chunk.map_err(McpError::Storage)? {
                let lang = lang_of
                    .get(&rec.defining_file)
                    .copied()
                    .unwrap_or(Lang::Other("unknown"));
                let meta = SymbolMeta::from_record(&rec, lang);
                by_name.entry(meta.name.clone()).or_default().push(id);
                symbols.insert(id, meta);
            }
        }

        let graph = GraphIndex::build_from_snapshot(&snap).map_err(McpError::Graph)?;

        // Load the Git-history analytics wholesale via the `Storage` port
        // (tier-15a D1) and sort each by key on load (D3) so 15b/15c output is
        // deterministic with no re-sort.
        let mut churn = storage.all_churn().map_err(McpError::Storage)?;
        churn.sort_by(|a, b| a.path.cmp(&b.path));
        let mut co_change = storage.all_co_change().map_err(McpError::Storage)?;
        co_change.sort_by(|a, b| a.a.cmp(&b.a).then_with(|| a.b.cmp(&b.b)));
        let mut symbol_churn = storage.all_symbol_churn().map_err(McpError::Storage)?;
        symbol_churn.sort_by_key(|c| c.symbol);

        let revision = storage.revision().0;

        Ok(Self {
            paths,
            path_to_id,
            symbols,
            by_name,
            graph,
            churn,
            co_change,
            symbol_churn,
            revision,
            root,
        })
    }

    /// Look up the first symbol with `name`, if any. Tools that need
    /// disambiguation iterate `by_name` directly.
    #[must_use]
    pub fn find_symbol(&self, name: &str) -> Option<SymbolId> {
        self.by_name.get(name).and_then(|v| v.first().copied())
    }

    /// Fetch the file path for a [`FileId`].
    #[must_use]
    pub fn path_of(&self, file: FileId) -> Option<&str> {
        self.paths.get(&file).map(String::as_str)
    }

    /// Fetch metadata for a symbol.
    #[must_use]
    pub fn meta_of(&self, id: SymbolId) -> Option<&SymbolMeta> {
        self.symbols.get(&id)
    }

    /// Resolve `SymbolId → FileId` (used by `plan_assist`).
    #[must_use]
    pub fn file_of(&self, id: SymbolId) -> Option<FileId> {
        self.symbols.get(&id).map(|m| m.file)
    }
}
