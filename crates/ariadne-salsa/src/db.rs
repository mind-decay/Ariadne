//! `AriadneDb` — the project's salsa database (tier-04 step 6 + step 11).
//!
//! Tier-04 wires only the salsa surface; the actual seeding from storage and
//! the delta write-back through `WriteTxn` are stubbed until tier-06+
//! [src: .claude/plans/ariadne-core/tier-04-salsa.md step 6 + step 11
//! `exposed but not yet driven`].

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::sync::Mutex;

use ariadne_core::{
    Changeset, EdgeKey, FileId, FileRecord, ReadSnapshot, RevisionId, Storage, StorageError,
    SymbolId, SymbolRecord, Visibility, WriteTxn,
};
use salsa::{EventKind, Setter, Storage as SalsaStorage};

use crate::derive::{self, FileFacts, LocalSymbol, SymbolCandidate};
use crate::derived::{
    ScipFactsRaw, SyntacticFactsRaw, scip_facts_for_file, symbols_for_file, syntactic_facts,
};
use crate::inputs::{FileContentInput, ScipFactsInput, SyntacticFactsInput, durability_for};
use crate::memory::MemoryReport;

/// Type alias for the event-log channel used by tests and the memory probe.
pub type EventLog = Arc<Mutex<Vec<String>>>;

/// Chunk size for the prior-set scan in [`AriadneDb::commit_revision`]'s
/// diff-aware stale-delete pass. Matches the cold-index count chunk so the
/// streamed prior set stays memory-bounded on the 100K-file workload.
const DIFF_CHUNK: usize = 4096;

/// One file's seeded salsa inputs plus the `FileRecord` the driver writes
/// back. [`AriadneDb::commit_revision`] iterates these to derive symbols and
/// assemble the changeset; the salsa-input handles are `Copy` ids, so cloning
/// the registry with the database is cheap.
#[derive(Clone)]
struct SeededFile {
    file_id: FileId,
    record: FileRecord,
    content: FileContentInput,
    scip: ScipFactsInput,
    facts: SyntacticFactsInput,
}

/// One changed file's re-derivation inputs: its stable id, the metadata record
/// to upsert, the new bytes, and the parsed facts. Bundled so
/// [`AriadneDb::rederive_file`] takes a single value per changed file — the
/// shape the tier-08 watcher produces from one filesystem event (tier-07b).
#[derive(Debug, Clone)]
pub struct FileDerivation {
    /// Stable file id (the redb primary key for this path).
    pub file_id: FileId,
    /// File metadata record to upsert.
    pub record: FileRecord,
    /// File bytes (kept so the content hash and SFC `def_range` end track the
    /// edit; see [`AriadneDb::seed_file`]).
    pub content: Vec<u8>,
    /// Parsed syntactic facts for the file.
    pub facts: SyntacticFactsRaw,
}

/// Ariadne's salsa database. Owns the `salsa::Storage<Self>`, an optional
/// recompute-event log used by [`crate::AriadneDb::with_event_log`], and the
/// per-file input registry the driver derives over (tier-07a).
#[salsa::db]
#[derive(Clone)]
pub struct AriadneDb {
    storage: SalsaStorage<Self>,
    event_log: Option<EventLog>,
    files: Vec<SeededFile>,
}

impl Default for AriadneDb {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for AriadneDb {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AriadneDb")
            .field("event_log", &self.event_log.is_some())
            .finish_non_exhaustive()
    }
}

impl AriadneDb {
    /// Build a fresh, empty database with no event log.
    #[must_use]
    pub fn new() -> Self {
        Self {
            storage: SalsaStorage::new(None),
            event_log: None,
            files: Vec::new(),
        }
    }

    /// Build a database whose recompute events are mirrored into `log`.
    /// Used by the equivalence test to assert cache hits and by the
    /// memory probe sanity test.
    #[must_use]
    pub fn with_event_log(log: EventLog) -> Self {
        let cb_log = Arc::clone(&log);
        let callback = move |event: salsa::Event| {
            if matches!(event.kind, EventKind::WillExecute { .. }) {
                if let Ok(mut g) = cb_log.lock() {
                    g.push(format!("{event:?}"));
                }
            }
        };
        Self {
            storage: SalsaStorage::new(Some(Box::new(callback))),
            event_log: Some(log),
            files: Vec::new(),
        }
    }

    /// Snapshot the recompute-event log. Returns an empty vector when no
    /// log is attached.
    #[must_use]
    pub fn event_log_snapshot(&self) -> Vec<String> {
        self.event_log
            .as_ref()
            .and_then(|l| l.lock().ok().map(|g| g.clone()))
            .unwrap_or_default()
    }

    /// Seed one file's inputs from already-parsed facts (tier-07a). The
    /// composition root parses with `ariadne-parser`, converts to a
    /// [`SyntacticFactsRaw`], and calls this; the file joins the registry
    /// [`commit_revision`](Self::commit_revision) derives over. `content` is
    /// the file bytes (kept so [`crate::syntactic_facts`] tracks the content
    /// hash and so the SFC `def_range` end matches `record.size`).
    pub fn seed_file(
        &mut self,
        file_id: FileId,
        record: FileRecord,
        content: Vec<u8>,
        facts: SyntacticFactsRaw,
    ) {
        let path = record.path.clone();
        let hash = record.blake3;
        let durability = durability_for(&path);
        let content_input = FileContentInput::builder(path, content, hash)
            .durability(durability)
            .new(&*self);
        let scip = ScipFactsInput::builder(ScipFactsRaw::default(), [0u8; 32])
            .durability(durability)
            .new(&*self);
        let facts_input = SyntacticFactsInput::builder(facts)
            .durability(durability)
            .new(&*self);
        self.files.push(SeededFile {
            file_id,
            record,
            content: content_input,
            scip,
            facts: facts_input,
        });
    }

    /// Seed the salsa DB by enumerating a `Storage` snapshot into inputs
    /// (tier-07a). Used to re-hydrate the input layer from an existing redb
    /// index (daemon warm start, re-derive); redb persists no raw bytes, so
    /// the content input is empty and the facts input starts default — a
    /// composition root re-parses and resets facts before re-deriving (the
    /// tier-07b `rederive_file` path). Returns the created content inputs so
    /// callers can keep handles.
    ///
    /// # Errors
    /// Propagates storage read failures.
    pub fn seed_from_disk<S: Storage>(
        &mut self,
        storage: &S,
    ) -> Result<Vec<FileContentInput>, StorageError> {
        let snapshot = storage.snapshot()?;
        let mut created = Vec::new();
        for chunk in snapshot.iter_files(4096)? {
            for (file_id, record) in chunk? {
                let path = record.path.clone();
                let hash = record.blake3;
                let durability = durability_for(&path);
                let content = FileContentInput::builder(path, Vec::new(), hash)
                    .durability(durability)
                    .new(&*self);
                let scip = ScipFactsInput::builder(ScipFactsRaw::default(), [0u8; 32])
                    .durability(durability)
                    .new(&*self);
                let facts = SyntacticFactsInput::builder(SyntacticFactsRaw::default())
                    .durability(durability)
                    .new(&*self);
                self.files.push(SeededFile {
                    file_id,
                    record,
                    content,
                    scip,
                    facts,
                });
                created.push(content);
            }
        }
        Ok(created)
    }

    /// Populate a seeded file's SCIP facts (scip-driven-edges tier-01). The
    /// composition root runs `ariadne-scip` out of band, converts the extracted
    /// `ariadne_core::ScipFacts` into the salsa mirror, and calls this for each
    /// covered file; the next [`commit_revision`](Self::commit_revision) then
    /// emits that file's edges from SCIP instead of the tree-sitter resolver
    /// while `indexed_hash` still matches the file's content hash (plan D2, D4).
    /// A path with no seeded file is a no-op (the file was dropped or never
    /// indexed). The salsa-input handle is `Copy`, so the lookup borrow is
    /// released before the mutating setter chain runs.
    pub fn set_scip_facts(&mut self, path: &str, facts: ScipFactsRaw, indexed_hash: [u8; 32]) {
        let Some(scip) = self
            .files
            .iter()
            .find(|sf| sf.record.path == path)
            .map(|sf| sf.scip)
        else {
            return;
        };
        let durability = durability_for(path);
        scip.set_facts(self).with_durability(durability).to(facts);
        scip.set_indexed_hash(self)
            .with_durability(durability)
            .to(indexed_hash);
    }

    /// Derive every seeded file and commit one `Changeset` to a `Storage`
    /// adapter (tier-07a). Per-file symbol derivation runs through the salsa-
    /// memoized [`crate::symbols_for_file`]; the global edge resolution is the
    /// pure driver pass `crate::derive::resolve_edges` over the union of all
    /// symbols (it needs every symbol, so it cannot be per-file memoized —
    /// it mirrors the CLI committer's two-phase structure). File and symbol
    /// upserts plus resolved edges are applied in one transaction; the
    /// `RevisionId` of that commit is returned [src: post-v1-roadmap plan.md
    /// RD11; crates/ariadne-core/src/domain/changeset.rs:16-28].
    ///
    /// # Errors
    /// Propagates storage write failures.
    pub fn commit_revision<S: Storage>(&self, storage: &S) -> Result<RevisionId, StorageError> {
        Ok(self.commit_changeset(storage)?.0)
    }

    /// Build the changeset, commit it in one transaction, and return both the
    /// new [`RevisionId`] and the committed [`Changeset`]. The tier-08 daemon
    /// uses the returned changeset to mirror the delta into the warm petgraph
    /// via `GraphIndex::apply_delta` — the warm-graph apply needs the symbol /
    /// edge add+delete sets, not just the revision tag [src: post-v1-roadmap
    /// plan.md tier-08 step 4].
    fn commit_changeset<S: Storage>(
        &self,
        storage: &S,
    ) -> Result<(RevisionId, Changeset), StorageError> {
        let changeset = self.build_changeset(storage)?;
        let txn = storage.begin_write()?;
        let revision = WriteTxn::apply(txn, &changeset)?;
        Ok((revision, changeset))
    }

    /// Assemble the diff-aware [`Changeset`] for the current derivation without
    /// committing. Shared by [`commit_revision`](Self::commit_revision) and
    /// [`commit_changeset`](Self::commit_changeset).
    fn build_changeset<S: Storage>(&self, storage: &S) -> Result<Changeset, StorageError> {
        let mut changeset = Changeset::new();
        let mut name_to_symbols: HashMap<String, Vec<SymbolCandidate>> = HashMap::new();
        let mut facts_by_file: Vec<FileFacts> = Vec::with_capacity(self.files.len());
        // Covered files (current SCIP facts) take the SCIP edge pass; the rest
        // take the precise tree-sitter resolver (plan D4).
        let mut scip_facts_by_file: Vec<derive::ScipFileFacts> = Vec::new();

        for sf in &self.files {
            let file_id = sf.file_id;
            let rel_path = &sf.record.path;
            // Scoping key for edge resolution: the crate this file belongs to
            // (ADR-0024). Computed once per file and shared by its symbol
            // candidates and its caller facts.
            let package = derive::package_of(rel_path).to_owned();
            changeset.file_upserts.push((file_id, sf.record.clone()));

            // Both queries are salsa-memoized; the second call is a cache hit.
            let syms = symbols_for_file(self, sf.content, sf.facts);
            let raw = syntactic_facts(self, sf.content, sf.facts);

            // `nth` disambiguates same-`(name, kind)` decls within this file by
            // their source-order occurrence index, replacing the byte offset in
            // the symbol id so it survives offset-shifting edits (RD12). The
            // per-file symbol order is deterministic (`build_symbols` preserves
            // the parsed decl order), so `nth` matches between an incremental
            // commit and a full rebuild.
            let mut nth_of: HashMap<(&str, &str), u32> = HashMap::new();
            let mut locals = Vec::with_capacity(syms.len());
            for s in syms.iter() {
                let nth = {
                    let slot = nth_of
                        .entry((s.canonical_name.as_str(), s.kind.as_str()))
                        .or_insert(0);
                    let n = *slot;
                    *slot += 1;
                    n
                };
                let id = derive::symbol_id(rel_path, &s.kind, &s.canonical_name, nth);
                let def_range = s.defining_byte_range;
                changeset.symbol_upserts.push((
                    id,
                    SymbolRecord {
                        canonical_name: s.canonical_name.clone(),
                        kind: s.kind.clone(),
                        defining_file: file_id,
                        defining_span: derive::span(file_id, def_range),
                        visibility: Visibility::from_byte(s.visibility_byte).unwrap_or_default(),
                        attributes: s.attributes.clone(),
                        complexity: s.complexity,
                    },
                ));
                name_to_symbols
                    .entry(s.canonical_name.clone())
                    .or_default()
                    .push(SymbolCandidate {
                        id,
                        file: file_id,
                        def_start: def_range.0,
                        package: package.clone(),
                    });
                locals.push(LocalSymbol { id, def_range });
            }

            // Coverage gate (plan D4): a file's edges come from SCIP only while
            // its SCIP facts were indexed at the file's current content hash.
            // The all-zero default `indexed_hash` means "no SCIP facts", so an
            // unindexed file — or one edited since SCIP ran (hash drift) — falls
            // through to the precise tree-sitter resolver below. A covered file
            // is excluded from `facts_by_file`, so `resolve_edges` is skipped for
            // it and no stale resolved edge survives a live edit.
            let facts_hash = sf.scip.indexed_hash(self);
            let covered = facts_hash != [0u8; 32] && facts_hash == sf.record.blake3;
            if covered {
                let scip = scip_facts_for_file(self, sf.scip);
                let mut occurrences: Vec<(String, (u32, u32), u32)> = scip
                    .occurrences
                    .iter()
                    .map(|o| (o.symbol.clone(), o.byte_range, o.roles))
                    .collect();
                // Sort by range so the SCIP edge set is independent of
                // occurrence order (plan determinism constraint).
                occurrences.sort_by_key(|(_, range, _)| *range);
                scip_facts_by_file.push(derive::ScipFileFacts {
                    file_id,
                    lang: sf.record.lang,
                    symbols: locals,
                    occurrences,
                });
            } else {
                facts_by_file.push(derive::file_facts(
                    file_id,
                    package,
                    sf.record.lang,
                    locals,
                    &raw,
                ));
            }
        }

        // Global resolution needs every symbol — sort the per-file facts and
        // the per-name candidates so the output is independent of seeding
        // order, then resolve over the union. Uncovered files resolve through
        // the precise tree-sitter resolver; covered files through the SCIP edge
        // pass. A file is in exactly one set, so the two edge sets are disjoint
        // by `src` (plan D4).
        facts_by_file.sort_by_key(|f| f.file_id);
        let resolved = derive::sort_candidates(name_to_symbols);
        for (key, rec) in derive::resolve_edges(&facts_by_file, &resolved) {
            changeset.edges_added.push((key, rec));
        }
        scip_facts_by_file.sort_by_key(|f| f.file_id);
        for (key, rec) in derive::resolve_scip_edges(&scip_facts_by_file) {
            changeset.edges_added.push((key, rec));
        }

        // Diff against the prior committed set so the changeset removes records
        // no longer derived — a file/symbol/edge present in the last commit but
        // absent from this derivation is deleted. This makes an incremental
        // commit equal to a full rebuild (divergence 0) and lets a forgotten or
        // edited file shed its stale symbols and the edges incident to them
        // [src: post-v1-roadmap plan.md RD12; crates/ariadne-core/src/domain/changeset.rs:20,24,28].
        // First commit (empty prior) yields no deletes — the tier-07a
        // upsert-only behaviour. Reading the whole prior set is O(total); R-B4
        // accepts that per-edit cost alongside the global edge-resolution pass.
        Self::fill_stale_deletes(storage, &mut changeset)?;
        Ok(changeset)
    }

    /// Fill `changeset`'s delete vectors with every persisted file/symbol/edge
    /// id that this revision's upserts do not re-derive. Reads a fresh read
    /// snapshot of the committed state (MVCC-concurrent with the pending
    /// write) and diffs it against the changeset's upsert/add sets.
    fn fill_stale_deletes<S: Storage>(
        storage: &S,
        changeset: &mut Changeset,
    ) -> Result<(), StorageError> {
        let prior = storage.snapshot()?;

        let derived_files: HashSet<FileId> =
            changeset.file_upserts.iter().map(|(id, _)| *id).collect();
        for chunk in prior.iter_files(DIFF_CHUNK)? {
            for (id, _) in chunk? {
                if !derived_files.contains(&id) {
                    changeset.file_deletes.push(id);
                }
            }
        }

        let derived_symbols: HashSet<SymbolId> =
            changeset.symbol_upserts.iter().map(|(id, _)| *id).collect();
        for chunk in prior.iter_symbols(DIFF_CHUNK)? {
            for (id, _) in chunk? {
                if !derived_symbols.contains(&id) {
                    changeset.symbol_deletes.push(id);
                }
            }
        }

        let derived_edges: HashSet<EdgeKey> =
            changeset.edges_added.iter().map(|(k, _)| *k).collect();
        for chunk in prior.iter_edges(DIFF_CHUNK)? {
            for (key, _) in chunk? {
                if !derived_edges.contains(&key) {
                    changeset.edges_removed.push(key);
                }
            }
        }

        Ok(())
    }

    /// Re-derive a single file: mutate (or create) that file's salsa inputs
    /// through the setter chain so salsa recomputes only its `symbols_for_file`,
    /// then commit the diff-aware changeset and return both its [`RevisionId`]
    /// and the committed [`Changeset`]. The tier-08 watcher calls this on a
    /// content edit, then feeds the changeset to the warm-graph `apply_delta`;
    /// matching the file by path reuses the existing input handles so the
    /// recompute stays incremental [src: post-v1-roadmap plan.md RD12; tier-08
    /// step 4].
    ///
    /// # Errors
    /// Propagates storage read/write failures.
    pub fn rederive_file<S: Storage>(
        &mut self,
        file: FileDerivation,
        storage: &S,
    ) -> Result<(RevisionId, Changeset), StorageError> {
        let FileDerivation {
            file_id,
            record,
            content,
            facts,
        } = file;
        let path = record.path.clone();
        let durability = durability_for(&path);
        if let Some(idx) = self.files.iter().position(|sf| sf.record.path == path) {
            let content_input = self.files[idx].content;
            let facts_input = self.files[idx].facts;
            let hash = record.blake3;
            content_input
                .set_content(self)
                .with_durability(durability)
                .to(content);
            content_input
                .set_hash(self)
                .with_durability(durability)
                .to(hash);
            facts_input
                .set_facts(self)
                .with_durability(durability)
                .to(facts);
            self.files[idx].file_id = file_id;
            self.files[idx].record = record;
        } else {
            self.seed_file(file_id, record, content, facts);
        }
        self.commit_changeset(storage)
    }

    /// Forget a single file: drop it from the derivation registry so the next
    /// diff-aware commit removes its symbols and the edges incident to them,
    /// and so edges from other files that referenced one of its symbols drop
    /// when re-resolution leaves them unresolved. Returns the new
    /// [`RevisionId`] and the committed [`Changeset`] for the warm-graph
    /// `apply_delta`. The tier-08 watcher calls this on a file deletion
    /// [src: post-v1-roadmap plan.md RD12; tier-08 step 4].
    ///
    /// Bound (audit I3): this drops the file from `self.files` but leaves its
    /// `FileContentInput`/`SyntacticFactsInput` allocated in salsa storage, so a
    /// delete-then-recreate of the same path orphans the prior inputs. There is
    /// no divergence/correctness impact — the diff-aware commit keys on persisted
    /// records, not on live inputs — and the leak is bounded by the churn count;
    /// eviction is wired when the tier-08 watcher lands input tracking.
    ///
    /// # Errors
    /// Propagates storage read/write failures.
    pub fn forget_file<S: Storage>(
        &mut self,
        path: &str,
        storage: &S,
    ) -> Result<(RevisionId, Changeset), StorageError> {
        self.files.retain(|sf| sf.record.path != path);
        self.commit_changeset(storage)
    }

    /// Per-table memory report: for every seeded file, deep-size the memoized
    /// output of each tracked per-file query and accumulate it under that
    /// query's table. Each call is a salsa cache hit — the derivation already
    /// computed these — so the report reflects resident bytes without
    /// recomputation. `syntactic_facts` and `symbols_for_file` carry the real
    /// derived data; `scip_facts` is empty until a composition root populates
    /// the SCIP input (scip-driven-edges tier-01). `edges_for_file` is resolved
    /// by the pure driver pass (`crate::derive::resolve_edges` /
    /// `resolve_scip_edges`), not memoized in its salsa table, and
    /// `blast_radius` is an on-demand per-symbol query outside the seeding flow;
    /// neither is enumerable in salsa 0.26.2, so both report 0. This is the
    /// mechanism the cold `ariadne mem` command and the daemon warm-graph probe
    /// share to enforce the 256 MiB-per-table R1 ceiling
    /// [src: crates/ariadne-salsa/src/memory.rs; CLAUDE.md R1].
    #[must_use]
    pub fn memory_report(&self) -> MemoryReport {
        let mut syntactic = 0u64;
        let mut symbols = 0u64;
        let mut scip = 0u64;
        for sf in &self.files {
            syntactic += crate::memory::syntactic_facts_bytes(
                syntactic_facts(self, sf.content, sf.facts).as_ref(),
            );
            symbols += crate::memory::symbols_vec_bytes(
                symbols_for_file(self, sf.content, sf.facts).as_ref(),
            );
            scip += crate::memory::scip_facts_bytes(scip_facts_for_file(self, sf.scip).as_ref());
        }
        MemoryReport::from_table_bytes(syntactic, scip, symbols)
    }
}

#[salsa::db]
impl salsa::Database for AriadneDb {}
