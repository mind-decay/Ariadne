//! Live-update engine (tier-08): the bridge from a filesystem
//! [`Invalidation`] to an incremental delta on the warm graph.
//!
//! The engine owns the salsa re-derivation database, a `path → FileId` map,
//! and a shared handle to the warm [`WarmCatalog`]. For each invalidation it
//! parses the changed file at this composition root, calls the shared
//! `ariadne-salsa` driver (`rederive_file` / `forget_file`), and folds the
//! returned [`Changeset`](ariadne_core::Changeset) into the warm catalog via
//! [`WarmCatalog::apply_changeset`]. redb is opened transiently per commit and
//! never held idle, so the tier-07 staleness handshake still works. Both this
//! commit and the accept loop's staleness rebuild open redb only while holding
//! the catalog write lock, so the two opens are serialized and can never
//! collide (single-open per process)
//! [src: .claude/plans/post-v1-roadmap/tier-08-daemon-watcher-live.md steps 3, 4;
//!  tier-08 build notes; tier-08 audit I1].

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::sync::{Arc, RwLock};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use ariadne_core::{FileId, FileRecord, Invalidation, Lang, ScipFacts};
use ariadne_parser::{FactExtractor, ParserRegistry};
use ariadne_salsa::{
    AriadneDb, FileDerivation, ScipFactsRaw, ScipOccurrenceRaw, ScipRelationshipRaw,
};
use ariadne_storage::RedbStorage;

use crate::domain::catalog::{WarmCatalog, index_path};
use crate::domain::dump::CatalogDump;
use crate::domain::facts::{lang_for_path, parse_facts};
use crate::errors::DaemonError;

/// How long the update pump blocks on the channel before re-checking its stop
/// flag, so `serve_live` can join it promptly on shutdown.
const PUMP_TICK: Duration = Duration::from_millis(100);

/// One out-of-band SCIP pass's per-file facts: `(relative_path, ScipFacts)`
/// pairs the composition root extracts from a completed indexer run and ships to
/// the live pump (scip-driven-edges D6). `ScipFacts` is the pure `ariadne-core`
/// boundary type — the daemon never links `ariadne-scip`; the CLI runs the
/// indexers and converts, exactly as it pre-computes Git hunks for the daemon
/// (RD7) [src: docs/adr/0026-default-on-out-of-band-scip.md].
pub type ScipFactsBatch = Vec<(String, ScipFacts)>;

/// Drives incremental warm-graph updates from filesystem invalidations.
pub struct LiveEngine {
    /// Project root the daemon serves.
    root: PathBuf,
    /// Salsa re-derivation database; seeded from disk at start.
    db: AriadneDb,
    /// Shared warm catalog the accept loop reads and this engine mutates.
    catalog: Arc<RwLock<WarmCatalog>>,
    /// `path → FileId` for assigning stable ids to changed files.
    path_to_id: BTreeMap<String, FileId>,
    /// Next file id to hand out for a never-seen path.
    next_id: u32,
    /// Tree-sitter parser registry (reused across edits).
    registry: ParserRegistry,
    /// Compiled fact-query cache keyed by layer [`Lang`], reused across edits so
    /// each edit does not recompile the tree-sitter fact queries — the same
    /// cache `start` builds across the seed loop [src: tier-08 audit I2].
    extractors: HashMap<Lang, FactExtractor>,
}

impl std::fmt::Debug for LiveEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LiveEngine")
            .field("root", &self.root)
            .field("tracked_files", &self.path_to_id.len())
            .finish_non_exhaustive()
    }
}

impl LiveEngine {
    /// Build a warm catalog from the redb at `project_root` and seed the salsa
    /// db from its stored files, re-read and re-parsed from disk, so the first
    /// incremental commit diffs against a baseline identical to the cold index
    /// (no spurious churn). A stored file that is unreadable or whose extension
    /// is not indexable is skipped — the next edit or reconcile pass corrects
    /// it.
    ///
    /// # Errors
    /// Propagates storage-open / catalog-build failures.
    pub fn start(project_root: &Path) -> Result<Self, DaemonError> {
        let root = project_root.to_path_buf();
        let storage = RedbStorage::open(&index_path(&root))?;
        let catalog = WarmCatalog::build(&storage, root.display().to_string())?;
        drop(storage);

        let mut db = AriadneDb::new();
        let mut path_to_id = BTreeMap::new();
        let mut next_id = 1u32;
        let registry = ParserRegistry::new();
        let mut extractors = HashMap::new();
        for (file_id, rel) in &catalog.paths {
            let abs = root.join(rel);
            let Ok(content) = std::fs::read(&abs) else {
                continue;
            };
            let Some(lang) = lang_for_path(Path::new(rel)) else {
                continue;
            };
            let record = build_record(rel, lang, &abs, &content);
            let facts = parse_facts(lang, &content, &registry, &mut extractors);
            db.seed_file(*file_id, record, content, facts);
            path_to_id.insert(rel.clone(), *file_id);
            next_id = next_id.max(file_id.get().saturating_add(1));
        }

        Ok(Self {
            root,
            db,
            catalog: Arc::new(RwLock::new(catalog)),
            path_to_id,
            next_id,
            registry,
            extractors,
        })
    }

    /// Apply one filesystem invalidation: re-derive (create/modify) or forget
    /// (remove) the file and fold the resulting delta into the warm catalog.
    ///
    /// # Errors
    /// Propagates storage read/write failures from the commit.
    pub fn apply(&mut self, inv: &Invalidation) -> Result<(), DaemonError> {
        match inv {
            Invalidation::Created { path }
            | Invalidation::Modified { path }
            | Invalidation::HashDrift { path, .. } => self.upsert(path),
            Invalidation::Removed { path } => self.forget(path),
        }
    }

    /// Re-derive a created/modified file from its on-disk bytes.
    fn upsert(&mut self, abs_path: &Path) -> Result<(), DaemonError> {
        let rel = rel_path(&self.root, abs_path);
        let Some(lang) = lang_for_path(Path::new(&rel)) else {
            return Ok(());
        };
        let Ok(content) = std::fs::read(abs_path) else {
            return Ok(());
        };
        let record = build_record(&rel, lang, abs_path, &content);
        let file_id = self.assign_id(&rel);
        let facts = parse_facts(lang, &content, &self.registry, &mut self.extractors);
        let deriv = FileDerivation {
            file_id,
            record,
            content,
            facts,
        };

        let mut cat = self.catalog.write().expect("warm-catalog write lock");
        let storage = RedbStorage::open(&index_path(&self.root))?;
        let (revision, changeset) = self.db.rederive_file(deriv, &storage)?;
        drop(storage);
        cat.apply_changeset(&changeset, revision.0);
        Ok(())
    }

    /// Forget a removed file. No-op when the path was never tracked, so a
    /// stray removal does not trigger a wasteful full-derivation commit.
    fn forget(&mut self, abs_path: &Path) -> Result<(), DaemonError> {
        let rel = rel_path(&self.root, abs_path);
        if self.path_to_id.remove(&rel).is_none() {
            return Ok(());
        }
        let mut cat = self.catalog.write().expect("warm-catalog write lock");
        let storage = RedbStorage::open(&index_path(&self.root))?;
        let (revision, changeset) = self.db.forget_file(&rel, &storage)?;
        drop(storage);
        cat.apply_changeset(&changeset, revision.0);
        Ok(())
    }

    /// Apply one out-of-band SCIP pass: set the salsa SCIP inputs for the
    /// covered files, re-commit so the derivation replaces their tree-sitter
    /// resolver edges with the precise SCIP edges (plan D4), then rebuild the
    /// warm catalog from the committed redb. Runs on the pump thread, off the
    /// synchronous query path; the only lock held is the brief warm-catalog
    /// write around the re-commit + rebuild, so a query already in flight while
    /// the external indexers were building never blocked — it read the current
    /// resolver / last-covered edges [src: docs/adr/0026-default-on-out-of-band-scip.md].
    ///
    /// A file whose content hash has drifted off its indexed hash, or that is no
    /// longer tracked, degrades to the precise resolver inside `commit_revision`
    /// — never a stale edge (plan D4). An empty batch is a no-op. The redb open
    /// is serialized under the warm-catalog write lock, matching the pump's edit
    /// commit and the accept loop's staleness rebuild (single-open per process,
    /// tier-08 I1).
    ///
    /// # Errors
    /// Propagates storage read/write failures from the re-commit or rebuild.
    pub(crate) fn apply_scip_facts(&mut self, batch: ScipFactsBatch) -> Result<(), DaemonError> {
        if batch.is_empty() {
            return Ok(());
        }
        let mut cat = self.catalog.write().expect("warm-catalog write lock");
        let storage = RedbStorage::open(&index_path(&self.root))?;
        for (path, facts) in batch {
            let indexed_hash = facts.indexed_hash;
            self.db
                .set_scip_facts(&path, scip_facts_raw(facts), indexed_hash);
        }
        self.db.commit_revision(&storage)?;
        let fresh = WarmCatalog::build(&storage, self.root.display().to_string())?;
        drop(storage);
        *cat = fresh;
        Ok(())
    }

    /// Stable file id for `rel`: the existing id when tracked, else a fresh one.
    fn assign_id(&mut self, rel: &str) -> FileId {
        if let Some(id) = self.path_to_id.get(rel) {
            return *id;
        }
        let id = FileId::new(self.next_id).expect("file id starts at 1 and increments");
        self.next_id = self.next_id.saturating_add(1);
        self.path_to_id.insert(rel.to_owned(), id);
        id
    }

    /// Clone the shared catalog handle for the accept loop.
    pub(crate) fn catalog_arc(&self) -> Arc<RwLock<WarmCatalog>> {
        Arc::clone(&self.catalog)
    }

    /// Project the current warm catalog into a comparable dump (proptest hook).
    ///
    /// # Panics
    /// Panics if the warm-catalog lock is poisoned (a prior holder panicked).
    #[must_use]
    pub fn dump(&self) -> CatalogDump {
        CatalogDump::of(&self.catalog.read().expect("warm-catalog read lock"))
    }

    /// Per-table memory report of the warm re-derivation database — the
    /// mandatory per-tier Salsa/in-RAM-graph probe (CLAUDE.md R1: any table
    /// over 256 MiB is a hard fail). Delegates to the tier-04
    /// [`AriadneDb::memory_report`] mechanism the cold `ariadne mem` command
    /// uses, so the daemon's warm graph is held to the same per-table budget
    /// [src: crates/ariadne-salsa/src/memory.rs;
    ///  .claude/plans/post-v1-roadmap/tier-10-cli-daemon-client-slo.md step 5].
    #[must_use]
    pub fn memory_report(&self) -> ariadne_salsa::MemoryReport {
        self.db.memory_report()
    }

    /// Move the engine onto a background thread that drains `events` until the
    /// channel disconnects or `stop` is set, applying each invalidation to the
    /// warm graph. A failed apply is dropped so a single unreadable file cannot
    /// take the daemon down (the reconcile pass re-emits a `HashDrift` later) —
    /// the same keep-running policy as the watcher sink
    /// [src: crates/ariadne-watcher/src/adapters/sink.rs:11-15].
    pub(crate) fn spawn_pump(
        mut self,
        events: Receiver<Invalidation>,
        scip_events: Receiver<ScipFactsBatch>,
        stop: Arc<AtomicBool>,
    ) -> JoinHandle<()> {
        thread::Builder::new()
            .name("ariadne-daemon-update".into())
            .spawn(move || {
                loop {
                    if stop.load(Ordering::Relaxed) {
                        break;
                    }
                    match events.recv_timeout(PUMP_TICK) {
                        Ok(inv) => {
                            let _ = self.apply(&inv);
                        }
                        Err(RecvTimeoutError::Timeout) => {}
                        Err(RecvTimeoutError::Disconnected) => break,
                    }
                    // Drain any out-of-band SCIP batches the composition root
                    // pushed. Folding runs on this one thread so the salsa db
                    // stays single-owner; a failed fold is dropped (keep-running
                    // policy), the next pass re-establishes coverage.
                    while let Ok(batch) = scip_events.try_recv() {
                        let _ = self.apply_scip_facts(batch);
                    }
                }
            })
            .expect("spawn update thread")
    }
}

/// Project-root-relative path for an absolute filesystem path, with `\`
/// normalised to `/` — the same form the cold index records
/// [src: crates/ariadne-cli/src/domain/mod.rs:387-391].
fn rel_path(root: &Path, abs: &Path) -> String {
    abs.strip_prefix(root)
        .unwrap_or(abs)
        .to_string_lossy()
        .replace('\\', "/")
}

/// Build a [`FileRecord`] from disk metadata + content hash, matching the cold
/// index's `parse_one` exactly [src: crates/ariadne-cli/src/domain/mod.rs:392-410].
fn build_record(rel: &str, lang: Lang, abs: &Path, content: &[u8]) -> FileRecord {
    let hash = *blake3::hash(content).as_bytes();
    let meta = std::fs::metadata(abs).ok();
    let size = meta
        .as_ref()
        .map_or(content.len() as u64, std::fs::Metadata::len);
    let mtime_ns = meta
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .and_then(|d| i128::try_from(d.as_nanos()).ok())
        .unwrap_or(0);
    FileRecord {
        path: rel.to_owned(),
        lang,
        size,
        blake3: hash,
        mtime_ns,
    }
}

/// Convert a pure-core [`ScipFacts`] into the `Update`-safe salsa mirror the
/// SCIP input carries. The composition-root boundary conversion, mirroring
/// `crate::domain::facts`'s parser-fact conversion — `ariadne-salsa` may not
/// depend on `ariadne-scip`, and the daemon never links it, so the core type is
/// mapped to `ScipFactsRaw` here [src: crates/ariadne-salsa/src/derived.rs
/// `ScipFactsRaw`; docs/adr/0026-default-on-out-of-band-scip.md].
fn scip_facts_raw(facts: ScipFacts) -> ScipFactsRaw {
    ScipFactsRaw {
        occurrences: facts
            .occurrences
            .into_iter()
            .map(|o| ScipOccurrenceRaw {
                symbol: o.symbol,
                byte_range: o.byte_range,
                roles: o.roles,
            })
            .collect(),
        relationships: facts
            .relationships
            .into_iter()
            .map(|r| ScipRelationshipRaw {
                from: r.from,
                to: r.to,
                is_implementation: r.is_implementation,
                is_type_definition: r.is_type_definition,
            })
            .collect(),
    }
}
