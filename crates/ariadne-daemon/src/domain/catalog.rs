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

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use ariadne_core::{
    Changeset, CoChangePair, FileChurn, FileId, Lang, ReadSnapshot, Storage, SymbolChurn, SymbolId,
    SymbolRecord, Visibility,
};
use ariadne_graph::{EdgeDelta, EdgeKind, GraphIndex, TestRootInput, classify_test_symbols};

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
    /// `McCabe` cyclomatic complexity of the defining occurrence
    /// (`decisions + 1`, `>= 1` for function-like symbols; `0` otherwise),
    /// carried so the Block-C analytics tools read it from RAM
    /// [src: post-v1-roadmap plan.md RD8].
    pub(crate) complexity: u32,
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
    /// Test-root projection: the symbols classified as tests (Block A, A1),
    /// precomputed once at build and maintained on `apply_changeset` so the
    /// `affected_tests` query stays a pure graph walk with no per-query
    /// re-classification [src: block-a plan.md D2].
    pub(crate) test_roots: BTreeSet<SymbolId>,
    /// Per-file Git-history churn, sorted by `path`. Loaded wholesale from the
    /// `Storage` port (tier-15a D1/D3) so the Block-C hotspot queries read it
    /// from RAM.
    pub(crate) churn: Vec<FileChurn>,
    /// File-pair co-change records, sorted by `(a, b)`.
    pub(crate) co_change: Vec<CoChangePair>,
    /// Per-symbol Git-history churn, sorted by `symbol`. Symbols absent from
    /// the table have zero attributed churn.
    pub(crate) symbol_churn: Vec<SymbolChurn>,
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

        // Load the Git-history analytics wholesale via the `Storage` port
        // (tier-15a D1) and sort each by key on load (D3) so 15b/15c output is
        // deterministic with no re-sort.
        let mut churn = storage.all_churn()?;
        churn.sort_by(|a, b| a.path.cmp(&b.path));
        let mut co_change = storage.all_co_change()?;
        co_change.sort_by(|a, b| a.a.cmp(&b.a).then_with(|| a.b.cmp(&b.b)));
        let mut symbol_churn = storage.all_symbol_churn()?;
        symbol_churn.sort_by_key(|c| c.symbol);

        // Test-root projection: classify every symbol once (D2) so the
        // `affected_tests` query never re-classifies at query time.
        let test_roots = classify_test_symbols(symbols.iter().map(|(id, m)| TestRootInput {
            id: *id,
            lang: m.lang,
            path: paths.get(&m.file).map_or("", String::as_str),
            kind: &m.kind,
            name: &m.name,
            attributes: &m.attributes,
        }));

        let revision = storage.revision().0;
        Ok(Self {
            snap,
            graph,
            paths,
            path_to_id,
            symbols,
            by_name,
            test_roots,
            churn,
            co_change,
            symbol_churn,
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
    ///
    /// The Git-history analytics vectors (`churn` / `co_change` /
    /// `symbol_churn`) are intentionally left untouched: a code-edit
    /// [`Changeset`] carries no git-history delta. A history refresh arrives as
    /// a full rebuild on the staleness handshake (tier-15a step 4; plan R-B2),
    /// which reloads them via [`WarmCatalog::build`].
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

        // Maintain the test-root projection in lock-step with the symbol churn:
        // a deleted symbol leaves the set; an upserted symbol is re-classified
        // from its new metadata (re-derive on `apply_changeset`, D2). Read first,
        // then mutate, so the immutable `self.symbols`/`self.paths` borrows are
        // released before the `self.test_roots` write.
        for sid in &cs.symbol_deletes {
            self.test_roots.remove(sid);
        }
        for (sid, _) in &cs.symbol_upserts {
            let is_test = self.symbols.get(sid).is_some_and(|meta| {
                let path = self.paths.get(&meta.file).map_or("", String::as_str);
                !classify_test_symbols(std::iter::once(TestRootInput {
                    id: *sid,
                    lang: meta.lang,
                    path,
                    kind: &meta.kind,
                    name: &meta.name,
                    attributes: &meta.attributes,
                }))
                .is_empty()
            });
            if is_test {
                self.test_roots.insert(*sid);
            } else {
                self.test_roots.remove(sid);
            }
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

#[cfg(test)]
mod tests {
    use super::*;
    use ariadne_core::{FileRecord, Span, WriteTxn};
    use ariadne_storage::RedbStorage;

    fn fid_t(n: u32) -> FileId {
        FileId::new(n).expect("nonzero file id")
    }

    fn sid_t(n: u64) -> SymbolId {
        SymbolId::new(n).expect("nonzero symbol id")
    }

    /// Seed `<root>/.ariadne/index.redb` with two files whose symbols carry
    /// non-zero `complexity`, plus churn / co-change / symbol-churn persisted
    /// deliberately out of key order so the load-time sort (D3) is exercised.
    /// Mirrors the cold MCP `seed_analytics_project` fixture exactly.
    fn seed(root: &std::path::Path) -> RedbStorage {
        let storage =
            RedbStorage::open(&root.join(".ariadne").join("index.redb")).expect("open redb");

        let mut cs = Changeset::new();
        for (id, path) in [(1u32, "src/alpha.rs"), (2, "src/beta.rs")] {
            cs = cs.upsert_file(
                fid_t(id),
                FileRecord {
                    path: path.into(),
                    lang: Lang::Rust,
                    size: 128,
                    blake3: [u8::try_from(id).expect("file id fits u8"); 32],
                    mtime_ns: i128::from(id),
                },
            );
        }
        for (id, name, file, complexity) in
            [(1u64, "crate::alpha", 1u32, 7u32), (2, "crate::beta", 2, 3)]
        {
            cs = cs.upsert_symbol(
                sid_t(id),
                SymbolRecord {
                    canonical_name: name.into(),
                    kind: "function".into(),
                    defining_file: fid_t(file),
                    defining_span: Span {
                        file: fid_t(file),
                        byte_start: 0,
                        byte_end: 64,
                    },
                    visibility: Visibility::Unknown,
                    attributes: Vec::new(),
                    complexity,
                },
            );
        }
        storage
            .begin_write()
            .expect("begin")
            .apply(&cs)
            .expect("apply changeset");

        storage
            .replace_history(
                &[
                    FileChurn {
                        path: "src/beta.rs".into(),
                        commits: 4,
                        author_keys: vec![[1u8; 8], [2u8; 8]],
                        last_changed_ns: 200,
                    },
                    FileChurn {
                        path: "src/alpha.rs".into(),
                        commits: 9,
                        author_keys: vec![[1u8; 8]],
                        last_changed_ns: 100,
                    },
                ],
                &[CoChangePair {
                    a: "src/alpha.rs".into(),
                    b: "src/beta.rs".into(),
                    count: 3,
                }],
            )
            .expect("replace history");
        storage
            .replace_symbol_churn(&[
                SymbolChurn {
                    symbol: sid_t(2),
                    commits: 2,
                },
                SymbolChurn {
                    symbol: sid_t(1),
                    commits: 5,
                },
            ])
            .expect("replace symbol churn");
        storage
    }

    /// The warm projection threads per-symbol `complexity` onto `SymbolMeta`
    /// and loads file churn / co-change / symbol churn from the `Storage` port,
    /// each sorted by key (tier-15a D1/D3). The fixture mirrors the cold MCP
    /// `seed_analytics_project` helper exactly, so the cold and warm
    /// projections are field-equal for the same fixture (exit criteria).
    #[test]
    fn build_loads_analytics_and_complexity() {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path().to_path_buf();
        let storage = seed(&root);
        let cat = WarmCatalog::build(&storage, root.display().to_string()).expect("build");

        // Complexity threaded onto every symbol meta.
        assert_eq!(cat.meta_of(sid_t(1)).expect("alpha meta").complexity, 7);
        assert_eq!(cat.meta_of(sid_t(2)).expect("beta meta").complexity, 3);

        // File churn loaded, sorted by path (alpha before beta).
        assert_eq!(
            cat.churn
                .iter()
                .map(|c| (c.path.as_str(), c.commits, c.authors()))
                .collect::<Vec<_>>(),
            vec![("src/alpha.rs", 9, 1), ("src/beta.rs", 4, 2)],
        );

        // Co-change loaded.
        assert_eq!(
            cat.co_change,
            vec![CoChangePair {
                a: "src/alpha.rs".into(),
                b: "src/beta.rs".into(),
                count: 3,
            }],
        );

        // Symbol churn loaded, sorted by SymbolId (sid(1) before sid(2)).
        assert_eq!(
            cat.symbol_churn,
            vec![
                SymbolChurn {
                    symbol: sid_t(1),
                    commits: 5,
                },
                SymbolChurn {
                    symbol: sid_t(2),
                    commits: 2,
                },
            ],
        );
    }

    /// The warm build projects the test-root set (D2): a `#[test]`-attributed
    /// Rust symbol lands in `test_roots`; a plain symbol does not.
    #[test]
    fn build_projects_test_roots() {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path().to_path_buf();
        let storage =
            RedbStorage::open(&root.join(".ariadne").join("index.redb")).expect("open redb");

        let mut cs = Changeset::new();
        cs = cs.upsert_file(
            fid_t(1),
            FileRecord {
                path: "src/lib.rs".into(),
                lang: Lang::Rust,
                size: 64,
                blake3: [1u8; 32],
                mtime_ns: 1,
            },
        );
        for (id, name, attributes) in [
            (1u64, "crate::checks", vec!["test".to_owned()]),
            (2u64, "crate::subject", Vec::new()),
        ] {
            cs = cs.upsert_symbol(
                sid_t(id),
                SymbolRecord {
                    canonical_name: name.into(),
                    kind: "function".into(),
                    defining_file: fid_t(1),
                    defining_span: Span {
                        file: fid_t(1),
                        byte_start: 0,
                        byte_end: 16,
                    },
                    visibility: Visibility::Unknown,
                    attributes,
                    complexity: 1,
                },
            );
        }
        storage
            .begin_write()
            .expect("begin")
            .apply(&cs)
            .expect("apply changeset");

        let cat = WarmCatalog::build(&storage, root.display().to_string()).expect("build");
        assert_eq!(
            cat.test_roots,
            BTreeSet::from([sid_t(1)]),
            "the #[test] fn is a test root; the plain fn is not",
        );
    }
}
