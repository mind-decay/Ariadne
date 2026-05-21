//! Crate-local orchestration for the CLI driving adapter.
//!
//! Tier-13 streams the cold index. The parallel parse fans out across a
//! `rayon` thread pool; each worker reads + parses + extracts one file and
//! hands the per-file facts down a bounded `sync_channel` to a single
//! committer thread that writes redb in bounded file/symbol/edge batches.
//! Commit overlaps parse, and the in-RAM working set is bounded by the
//! batch size, not the corpus size. Each worker caches a compiled-`Query`
//! `FactExtractor` per `Lang`, so the fact query is not recompiled per
//! file; the host and injected tree-sitter parsers are built per file by
//! `ariadne_parser::parse_file` [src: docs/adr/0010-streaming-cold-index.md].

use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, SyncSender, sync_channel};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use ariadne_core::{
    Changeset, EdgeKey, EdgeKind, EdgeRecord, FileId, FileRecord, Lang, Span, Storage, SymbolId,
    SymbolRecord, WriteTxn,
};
use ariadne_parser::{DeclKind, FactExtractor, ParserRegistry, SyntacticFacts};
use ariadne_scip::IngestPlan;
use ariadne_storage::RedbStorage;
use ignore::{DirEntry, WalkBuilder, WalkState};
use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;
use serde::Serialize;

use crate::config::Config;

/// Files per redb write transaction. Bounds the committer's in-RAM
/// `Changeset` *while the parse still runs*, so the parse-time working set
/// stays small and dirty pages flush per batch instead of holding the whole
/// corpus in one transaction [src: tier-13 step 5].
const COMMIT_BATCH: usize = 4096;

/// Edges per redb write transaction. Edge resolution runs after the channel
/// closes, so — unlike [`COMMIT_BATCH`] — the edge batch bounds no
/// parse-time working set; the whole resolved edge list is already in RAM.
/// A large bound keeps redb dirty pages per transaction bounded while
/// collapsing the post-parse commit from hundreds of `fsync`s to a handful
/// [src: tier-13 step 7 — N tuned from the measured commit breakdown:
/// 858 4096-edge transactions were ~42s of the 67s cold index].
const EDGE_COMMIT_BATCH: usize = 262_144;

/// Bound on the parse → commit channel. Large enough that parse is throttled
/// only when the committer genuinely lags [src: tier-13 step 4].
const CHANNEL_CAPACITY: usize = 4096;

/// Project-root-relative location of the redb index.
#[must_use]
pub fn index_path(root: &Path) -> PathBuf {
    root.join(".ariadne").join("index.redb")
}

/// Map a path to its [`Lang`] by file extension, via the canonical
/// [`Lang::from_extension`] table. Only the fourteen tree-sitter grammars
/// registered in [`ParserRegistry`] are recognised; everything else returns
/// `None` and is skipped by the syntactic indexer. The `.tsx`/`.jsx`/`.h`
/// rationale lives on [`Lang::from_extension`].
#[must_use]
pub fn lang_for_path(path: &Path) -> Option<Lang> {
    Lang::from_extension(path.extension()?.to_str()?)
}

/// JSON-line summary emitted on stdout when `index` completes
/// [src: tier-10 step 4 "emit JSON-line summary on stdout"].
#[derive(Debug, Serialize)]
pub struct IndexSummary {
    /// Files walked, parsed, and committed.
    pub files: usize,
    /// Symbols materialised from declarations.
    pub symbols: usize,
    /// Call edges resolved between symbols.
    pub edges: usize,
    /// Languages encountered, by tag.
    pub langs: Vec<String>,
    /// SCIP indexers whose ingest succeeded, by lang tag.
    pub scip_successes: Vec<String>,
    /// SCIP indexers reported missing on PATH, by binary name.
    pub scip_missing: Vec<String>,
    /// Files skipped because parsing aborted.
    pub parse_failures: usize,
    /// Persisted revision after the final commit.
    pub revision: u64,
    /// Wall-clock duration of the cold index, milliseconds.
    pub elapsed_ms: u128,
}

/// Per-phase wall-clock breakdown of one cold index, in milliseconds. Since
/// tier-13 streams the commit behind the parse, `parse` overlaps the
/// file/symbol-batch portion of `commit`; the phases no longer sum to the
/// total. `resolve` and the edge portion of `commit` run after the parse
/// finishes [src: tier-13 steps 4-6].
#[derive(Debug, Clone, Copy)]
pub struct PhaseTimings {
    /// Parallel file-system walk + path sort.
    pub walk: u128,
    /// Wall time of the parallel parse fan-out.
    pub parse: u128,
    /// Committer-side edge resolution (post-parse).
    pub resolve: u128,
    /// Cumulative time inside redb `apply` calls across every batch.
    pub commit: u128,
    /// External SCIP indexers — `0` unless `--scip` was passed.
    pub scip: u128,
}

/// Wall-time breakdown of the parse phase, summed across all workers (so
/// each figure exceeds the parse wall by roughly the worker count). Lets a
/// slow sub-step — file read, tree-sitter parse, or fact extraction — be
/// attributed without a profiler [src: tier-13 step 1].
#[derive(Debug, Clone, Copy)]
pub struct ParseSubTimings {
    /// `std::fs::read` of the file bytes.
    pub read: u128,
    /// tree-sitter parse.
    pub parse: u128,
    /// Query-driven fact extraction.
    pub extract: u128,
}

/// Atomic nanosecond accumulators behind [`ParseSubTimings`]; every parse
/// worker adds its per-file sub-durations here.
#[derive(Debug, Default)]
struct ParseProbe {
    read: AtomicU64,
    parse: AtomicU64,
    extract: AtomicU64,
}

impl ParseProbe {
    /// Read the accumulators back as a millisecond breakdown.
    fn snapshot(&self) -> ParseSubTimings {
        ParseSubTimings {
            read: u128::from(self.read.load(Ordering::Relaxed)) / 1_000_000,
            parse: u128::from(self.parse.load(Ordering::Relaxed)) / 1_000_000,
            extract: u128::from(self.extract.load(Ordering::Relaxed)) / 1_000_000,
        }
    }
}

/// Accumulate one elapsed span into an atomic nanosecond counter.
fn add_ns(counter: &AtomicU64, elapsed: Duration) {
    counter.fetch_add(
        u64::try_from(elapsed.as_nanos()).unwrap_or(u64::MAX),
        Ordering::Relaxed,
    );
}

/// A declaration promoted to a symbol, kept for edge resolution.
struct LocalSymbol {
    id: SymbolId,
    def_range: (u32, u32),
}

/// A symbol-name candidate kept for deterministic edge-`dst` selection. The
/// candidate lists are sorted by `(file, def_start)` once the channel
/// closes, reproducing tier-12's `FileId`-ordered `candidates.first()`
/// selection regardless of parse-completion order [src: tier-13 step 6].
struct SymbolCandidate {
    id: SymbolId,
    file: FileId,
    def_start: u32,
}

/// Per-file facts retained between the symbol pass and the edge pass. Each
/// `(name, range)` pair is an unresolved site — a callee, a rendered child
/// component, or a hook — the edge pass resolves against the global symbol
/// table [src: tier-05 step 4].
struct FileFacts {
    file_id: FileId,
    lang: Lang,
    symbols: Vec<LocalSymbol>,
    calls: Vec<(String, (u32, u32))>,
    renders: Vec<(String, (u32, u32))>,
    hooks: Vec<(String, (u32, u32))>,
}

/// One file's parse output, streamed to the committer. Holds no raw bytes —
/// only the [`FileRecord`] metadata and the extracted facts.
struct ParsedFile {
    id: FileId,
    record: FileRecord,
    lang: Lang,
    rel_path: String,
    /// `None` when the parse aborted (timeout or extraction failure).
    facts: Option<SyntacticFacts>,
}

/// Run the full cold-index pipeline against `root` and commit to redb.
///
/// External SCIP indexers run only when `scip` is set; they are deliberately
/// off the default measured path because they perform full language builds
/// [src: docs/adr/0009-parallel-cold-index.md].
///
/// # Errors
/// Propagates filesystem and storage failures.
pub fn run_index(
    root: &Path,
    config: &Config,
    fresh: bool,
    scip: bool,
) -> Result<(IndexSummary, PhaseTimings, ParseSubTimings)> {
    let started = Instant::now();
    let db_path = index_path(root);
    if fresh && db_path.exists() {
        std::fs::remove_file(&db_path)
            .with_context(|| format!("remove stale index {}", db_path.display()))?;
    }
    std::fs::create_dir_all(db_path.parent().expect("index path has a parent"))
        .context("create .ariadne directory")?;

    // Phase 1 — parallel walk.
    let walk_started = Instant::now();
    let paths = walk_repo(root, config);
    let walk_ms = walk_started.elapsed().as_millis();

    // Phase 2 — streaming parse → committer pipeline.
    let registry = ParserRegistry::new();
    let probe = ParseProbe::default();
    let bar = progress_bar(paths.len());
    let (tx, rx) = sync_channel::<ParsedFile>(CHANNEL_CAPACITY);
    let committer_db = db_path.clone();
    let committer = thread::spawn(move || run_committer(&rx, &committer_db));

    let parse_started = Instant::now();
    paths.par_iter().enumerate().for_each_init(
        || ThreadState {
            registry: registry.clone(),
            extractors: HashMap::new(),
            sender: tx.clone(),
        },
        |state, (idx, path)| {
            bar.inc(1);
            if let Some(parsed) = parse_one(root, idx, path, state, &probe) {
                // A failed send means the committer already exited; its
                // join below surfaces the real error.
                let _ = state.sender.send(parsed);
            }
        },
    );
    let parse_ms = parse_started.elapsed().as_millis();
    drop(tx);
    bar.finish_and_clear();
    let outcome = match committer.join() {
        Ok(result) => result.context("committer thread failed")?,
        Err(_) => bail!("committer thread panicked"),
    };

    // Phase 3 — opt-in SCIP ingest, off the measured fast path by default.
    let scip_started = Instant::now();
    let scip_report = if scip {
        Some(IngestPlan::with_default_drivers().ingest(root))
    } else {
        None
    };
    let scip_ms = scip_started.elapsed().as_millis();

    let summary = IndexSummary {
        files: outcome.files,
        symbols: outcome.symbols,
        edges: outcome.edges,
        langs: outcome.langs,
        scip_successes: scip_report
            .as_ref()
            .map(|s| s.successes.iter().map(Lang::tag).collect())
            .unwrap_or_default(),
        scip_missing: scip_report
            .as_ref()
            .map(|s| s.warnings.iter().map(|w| w.binary.clone()).collect())
            .unwrap_or_default(),
        parse_failures: outcome.parse_failures,
        revision: outcome.revision,
        elapsed_ms: started.elapsed().as_millis(),
    };
    let timings = PhaseTimings {
        walk: walk_ms,
        parse: parse_ms,
        resolve: outcome.resolve_ms,
        commit: outcome.commit_ms,
        scip: scip_ms,
    };
    Ok((summary, timings, probe.snapshot()))
}

/// Build the indexing progress bar.
fn progress_bar(len: usize) -> ProgressBar {
    let bar = ProgressBar::new(len as u64);
    bar.set_style(
        ProgressStyle::with_template("indexing [{bar:32}] {pos}/{len}")
            .unwrap_or_else(|_| ProgressStyle::default_bar())
            .progress_chars("=> "),
    );
    bar
}

/// Walk `root` in parallel, honouring `.gitignore` + config ignores, and
/// return every file path whose extension maps to an enabled language. Each
/// `ignore` worker pushes recognised paths into a shared sink; the collected
/// paths are sorted so the downstream `FileId` assignment is deterministic
/// run-to-run [src: tier-12 step 3].
fn walk_repo(root: &Path, config: &Config) -> Vec<PathBuf> {
    let enabled = config.enabled_langs();
    let mut builder = WalkBuilder::new(root);
    builder
        .git_ignore(config.respect_gitignore)
        .git_global(config.respect_gitignore)
        .git_exclude(config.respect_gitignore)
        .parents(config.respect_gitignore);

    let sink: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());
    let sink_ref = &sink;
    let enabled_ref = &enabled;
    let ignore_ref = config.ignore.as_slice();
    builder.build_parallel().run(|| {
        Box::new(
            move |result: Result<DirEntry, ignore::Error>| -> WalkState {
                let Ok(entry) = result else {
                    return WalkState::Continue;
                };
                if !entry.file_type().is_some_and(|t| t.is_file()) {
                    return WalkState::Continue;
                }
                let path = entry.path();
                let Some(lang) = lang_for_path(path) else {
                    return WalkState::Continue;
                };
                if !enabled_ref.contains(&lang) || is_config_ignored(root, path, ignore_ref) {
                    return WalkState::Continue;
                }
                sink_ref
                    .lock()
                    .expect("walk sink mutex poisoned")
                    .push(path.to_path_buf());
                WalkState::Continue
            },
        )
    });

    let mut paths = sink.into_inner().expect("walk sink mutex poisoned");
    paths.sort();
    paths
}

/// True when any path segment equals a config ignore entry.
fn is_config_ignored(root: &Path, path: &Path, patterns: &[String]) -> bool {
    let rel = path.strip_prefix(root).unwrap_or(path).to_string_lossy();
    patterns.iter().any(|p| {
        let trimmed = p.trim_end_matches('/');
        !trimmed.is_empty() && rel.split('/').any(|seg| seg == trimmed)
    })
}

/// Per-worker parse state. `for_each_init` builds one per `rayon` worker, so
/// no `FactExtractor` is ever shared — it is `!Send`, built lazily on the
/// worker thread. The `SyncSender` clone is the worker's handle onto the
/// bounded parse → commit channel [src: tier-13 step 3].
///
/// `extractors` is keyed by *every* [`Lang`] a worker meets across all
/// parse layers — a Vue SFC contributes both the `Vue` host-layer extractor
/// and the injected `TypeScript` extractor — so an SFC's `<script>` facts
/// reuse the same compiled query as a plain `.ts` file [src: tier-05 step 3].
struct ThreadState {
    registry: ParserRegistry,
    extractors: HashMap<Lang, FactExtractor>,
    sender: SyncSender<ParsedFile>,
}

/// Read, parse, and extract syntactic facts for one walked path. Returns
/// `None` only when the file is unreadable (it then contributes to no
/// count); a parse abort yields a [`ParsedFile`] with `facts: None` so the
/// file is still recorded. The byte buffer and parse tree drop before
/// return, so the raw-byte peak scales with the worker count, not the file
/// count [src: tier-12 step 4].
fn parse_one(
    root: &Path,
    idx: usize,
    path: &Path,
    state: &mut ThreadState,
    probe: &ParseProbe,
) -> Option<ParsedFile> {
    let lang = lang_for_path(path)?;

    let read_started = Instant::now();
    let content = std::fs::read(path).ok()?;
    add_ns(&probe.read, read_started.elapsed());

    let rel_path = path
        .strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/");
    let hash = *blake3::hash(&content).as_bytes();
    let meta = std::fs::metadata(path).ok();
    let size = meta
        .as_ref()
        .map_or(content.len() as u64, std::fs::Metadata::len);
    let mtime_ns = meta
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .and_then(|d| i128::try_from(d.as_nanos()).ok())
        .unwrap_or(0);
    let id = FileId::new(u32::try_from(idx + 1).expect("file count fits u32"))
        .expect("file id starts at 1 and increments");
    let record = FileRecord {
        path: rel_path.clone(),
        lang,
        size,
        blake3: hash,
        mtime_ns,
    };
    let facts = parse_facts(lang, &content, state, probe);
    // `content` drops here — raw bytes never outlive this single parse.
    Some(ParsedFile {
        id,
        record,
        lang,
        rel_path,
        facts,
    })
}

/// Parse `content` into a tier-03 multi-layer `ParsedFile` and extract its
/// merged facts. `None` on a parser/extractor build failure or a parse abort
/// — the file is still recorded, just factless.
///
/// A single-grammar file (`.rs`/`.ts`/`.tsx`/…) is the host-only degenerate
/// case; an SFC (`.vue`/`.svelte`/`.astro`) adds an injected `<script>` /
/// frontmatter layer. Every layer's facts are extracted with the worker's
/// cached `FactExtractor` for *that layer's* `Lang`, then folded through the
/// shared `SyntacticFacts::absorb_layer` + `finalize` merge that
/// `ariadne_parser::extract_syntactic_facts` also uses; this path differs
/// only in sourcing extractors from the per-worker cache instead of a
/// one-shot compile [src: tier-05 step 3].
fn parse_facts(
    lang: Lang,
    content: &[u8],
    state: &mut ThreadState,
    probe: &ParseProbe,
) -> Option<SyntacticFacts> {
    let parse_started = Instant::now();
    let parsed = ariadne_parser::parse_file(lang, &state.registry, content, None, &[]);
    add_ns(&probe.parse, parse_started.elapsed());
    let parsed = parsed.ok()?;

    let extract_started = Instant::now();
    let mut merged = SyntacticFacts::default();
    for (layer_lang, tree) in std::iter::once(&parsed.host).chain(parsed.injected.iter()) {
        let extractor = match state.extractors.entry(*layer_lang) {
            Entry::Occupied(e) => e.into_mut(),
            Entry::Vacant(e) => {
                e.insert(FactExtractor::for_lang(*layer_lang, &state.registry).ok()?)
            }
        };
        merged.absorb_layer(extractor.extract(tree, content));
    }
    merged.finalize();
    add_ns(&probe.extract, extract_started.elapsed());
    Some(merged)
}

/// Counts + timings the committer hands back on join.
struct CommitOutcome {
    files: usize,
    symbols: usize,
    edges: usize,
    parse_failures: usize,
    langs: Vec<String>,
    revision: u64,
    /// Cumulative time inside redb `apply` calls.
    commit_ms: u128,
    /// Post-drain edge-resolution time (sort + `resolve_edges`).
    resolve_ms: u128,
}

/// Mutable accumulators the committer threads through the drain loop.
#[derive(Default)]
struct CommitState {
    batch: Changeset,
    name_to_symbols: HashMap<String, Vec<SymbolCandidate>>,
    facts_by_file: Vec<FileFacts>,
    lang_first_seen: HashMap<String, FileId>,
    files: usize,
    symbols: usize,
    parse_failures: usize,
}

impl CommitState {
    /// Fold one parsed file into the accumulators and the pending batch.
    fn absorb(&mut self, file: ParsedFile) {
        let ParsedFile {
            id,
            record,
            lang,
            rel_path,
            facts,
        } = file;
        self.files += 1;
        self.lang_first_seen
            .entry(lang.tag())
            .and_modify(|seen| *seen = (*seen).min(id))
            .or_insert(id);
        // Captured before `record` moves into the batch — bounds the
        // synthesized SFC component's whole-file definition span below.
        let file_len = u32::try_from(record.size).unwrap_or(u32::MAX);
        self.batch.file_upserts.push((id, record));

        let Some(facts) = facts else {
            self.parse_failures += 1;
            return;
        };
        let mut locals = Vec::with_capacity(facts.decls.len() + 1);
        // An SFC (`.vue`/`.svelte`/`.astro`) carries exactly one component —
        // the file itself — but emits no enclosing `Component` decl: its
        // template render sites sit in the host layer, its decls in the
        // injected `<script>` layer. Synthesize a file-spanning `Component`
        // symbol named for the file stem so those renders have a graph
        // source, and so a cross-file `<Child/>` resolves to `Child`'s
        // SFC [src: tier-05 step 4; user scope decision].
        if is_sfc_lang(lang) {
            let name = sfc_component_name(&rel_path);
            let sid = symbol_id(&rel_path, &name, 0);
            let def_range = (0, file_len);
            self.batch.symbol_upserts.push((
                sid,
                SymbolRecord {
                    canonical_name: name.clone(),
                    kind: "component".to_owned(),
                    defining_file: id,
                    defining_span: span(id, def_range),
                },
            ));
            self.name_to_symbols
                .entry(name)
                .or_default()
                .push(SymbolCandidate {
                    id: sid,
                    file: id,
                    def_start: 0,
                });
            locals.push(LocalSymbol { id: sid, def_range });
            self.symbols += 1;
        }
        for decl in &facts.decls {
            let sid = symbol_id(&rel_path, &decl.name, decl.def_byte_range.0);
            self.batch.symbol_upserts.push((
                sid,
                SymbolRecord {
                    canonical_name: decl.name.clone(),
                    kind: decl_kind_tag(&decl.kind),
                    defining_file: id,
                    defining_span: span(id, decl.def_byte_range),
                },
            ));
            self.name_to_symbols
                .entry(decl.name.clone())
                .or_default()
                .push(SymbolCandidate {
                    id: sid,
                    file: id,
                    def_start: decl.def_byte_range.0,
                });
            locals.push(LocalSymbol {
                id: sid,
                def_range: decl.def_byte_range,
            });
            self.symbols += 1;
        }
        self.facts_by_file.push(FileFacts {
            file_id: id,
            lang,
            symbols: locals,
            calls: facts
                .calls
                .iter()
                .map(|c| (c.callee.clone(), c.byte_range))
                .collect(),
            renders: facts
                .renders
                .iter()
                .map(|r| (r.component.clone(), r.byte_range))
                .collect(),
            hooks: facts
                .hooks
                .iter()
                .map(|h| (h.callee.clone(), h.byte_range))
                .collect(),
        });
    }
}

/// True for the framework single-file-component langs. An SFC's template
/// render sites have no enclosing function declaration, so the committer
/// synthesizes a per-file `Component` symbol for them [src: tier-05 step 4].
fn is_sfc_lang(lang: Lang) -> bool {
    matches!(lang, Lang::Vue | Lang::Svelte | Lang::Astro)
}

/// Component name for a synthesized SFC symbol: the file stem (`Card` for
/// `ui/Card.vue`). Falls back to the whole relative path if it has no stem.
fn sfc_component_name(rel_path: &str) -> String {
    Path::new(rel_path)
        .file_stem()
        .map_or_else(|| rel_path.to_owned(), |s| s.to_string_lossy().into_owned())
}

/// Drain the parse → commit channel, writing redb in bounded batches.
///
/// File and symbol upserts are committed every [`COMMIT_BATCH`] files while
/// the parse still runs; edges are resolved once the channel closes (global
/// name resolution needs every symbol) and committed in the same batch size
/// [src: tier-13 steps 5-6].
fn run_committer(rx: &Receiver<ParsedFile>, db_path: &Path) -> Result<CommitOutcome> {
    let storage = RedbStorage::open(db_path).context("open redb index")?;
    let mut state = CommitState::default();
    let mut files_in_batch = 0usize;
    let mut revision = 0u64;
    let mut commit_ms = 0u128;

    while let Ok(file) = rx.recv() {
        state.absorb(file);
        files_in_batch += 1;
        if files_in_batch >= COMMIT_BATCH {
            revision = commit_batch(&storage, &mut state.batch, &mut commit_ms)?;
            files_in_batch = 0;
        }
    }
    if files_in_batch > 0 {
        revision = commit_batch(&storage, &mut state.batch, &mut commit_ms)?;
    }

    let resolve_started = Instant::now();
    state.facts_by_file.sort_by_key(|f| f.file_id);
    let resolved = sort_candidates(state.name_to_symbols);
    let edge_list = resolve_edges(&state.facts_by_file, &resolved);
    let resolve_ms = resolve_started.elapsed().as_millis();

    let mut edges = 0usize;
    let mut edge_batch = Changeset::new();
    for (key, rec) in edge_list {
        edge_batch.edges_added.push((key, rec));
        edges += 1;
        if edges % EDGE_COMMIT_BATCH == 0 {
            revision = commit_batch(&storage, &mut edge_batch, &mut commit_ms)?;
        }
    }
    if !edge_batch.edges_added.is_empty() {
        revision = commit_batch(&storage, &mut edge_batch, &mut commit_ms)?;
    }

    Ok(CommitOutcome {
        files: state.files,
        symbols: state.symbols,
        edges,
        parse_failures: state.parse_failures,
        langs: ordered_langs(&state.lang_first_seen),
        revision,
        commit_ms,
        resolve_ms,
    })
}

/// Apply one batch as a single redb transaction, accumulating the `apply`
/// wall time. Each call is its own transaction, so dirty pages flush per
/// batch and never hold the whole corpus [src: tier-13 step 5].
fn commit_batch(storage: &RedbStorage, batch: &mut Changeset, commit_ms: &mut u128) -> Result<u64> {
    let changeset = std::mem::take(batch);
    let started = Instant::now();
    let txn = storage.begin_write().context("begin redb write txn")?;
    let revision = txn.apply(&changeset).context("commit changeset batch")?;
    *commit_ms += started.elapsed().as_millis();
    Ok(revision.0)
}

/// Reduce each name's candidate list to `SymbolId`s, sorted by
/// `(defining FileId, def byte start)` so edge-`dst` selection is
/// independent of parse-completion order [src: tier-13 step 6].
fn sort_candidates(
    name_to_symbols: HashMap<String, Vec<SymbolCandidate>>,
) -> HashMap<String, Vec<SymbolId>> {
    name_to_symbols
        .into_iter()
        .map(|(name, mut cands)| {
            cands.sort_by_key(|c| (c.file, c.def_start));
            (name, cands.into_iter().map(|c| c.id).collect())
        })
        .collect()
}

/// Language tags ordered by the lowest `FileId` they were observed at —
/// reproduces tier-12's first-seen-in-path-order regardless of parse order.
fn ordered_langs(first_seen: &HashMap<String, FileId>) -> Vec<String> {
    let mut langs: Vec<String> = first_seen.keys().cloned().collect();
    langs.sort_by_key(|tag| first_seen[tag]);
    langs
}

/// Resolve every call / render / hook site to a typed `src -> dst` edge.
///
/// A call site becomes a [`EdgeKind::References`] edge, a render site a
/// [`EdgeKind::Renders`] edge, a hook site a [`EdgeKind::UsesHook`] edge.
/// For each, `src` is the innermost declaration whose span contains the site
/// (the enclosing component, for a render or hook) and `dst` is the named
/// symbol — same-file match preferred, else the first global match. An
/// unresolved `src` or `dst`, or a self-loop, drops the edge: the same
/// best-effort policy for all three kinds [src: tier-05 step 4].
fn resolve_edges(
    facts_by_file: &[FileFacts],
    name_to_symbols: &HashMap<String, Vec<SymbolId>>,
) -> Vec<(EdgeKey, EdgeRecord)> {
    let mut seen: HashSet<EdgeKey> = HashSet::new();
    let mut out = Vec::new();
    for facts in facts_by_file {
        let local_ids: HashSet<SymbolId> = facts.symbols.iter().map(|l| l.id).collect();
        let mut resolve = |kind: EdgeKind, name: &str, range: (u32, u32)| {
            let Some(src) = enclosing_symbol(&facts.symbols, range) else {
                return;
            };
            let Some(candidates) = name_to_symbols.get(name) else {
                return;
            };
            let Some(dst) = candidates
                .iter()
                .find(|c| local_ids.contains(c))
                .or_else(|| candidates.first())
                .copied()
            else {
                return;
            };
            if dst == src {
                return;
            }
            let key = EdgeKey { src, kind, dst };
            if !seen.insert(key) {
                return;
            }
            out.push((
                key,
                EdgeRecord {
                    source_span: span(facts.file_id, range),
                    evidence_lang: facts.lang,
                    weight: 1,
                },
            ));
        };
        for (callee, range) in &facts.calls {
            resolve(EdgeKind::References, callee, *range);
        }
        for (component, range) in &facts.renders {
            resolve(EdgeKind::Renders, component, *range);
        }
        for (callee, range) in &facts.hooks {
            resolve(EdgeKind::UsesHook, callee, *range);
        }
    }
    out
}

/// Innermost declaration whose definition span contains `range`.
fn enclosing_symbol(locals: &[LocalSymbol], range: (u32, u32)) -> Option<SymbolId> {
    locals
        .iter()
        .filter(|l| l.def_range.0 <= range.0 && range.1 <= l.def_range.1)
        .min_by_key(|l| l.def_range.1 - l.def_range.0)
        .map(|l| l.id)
}

fn span(file: FileId, range: (u32, u32)) -> Span {
    Span {
        file,
        byte_start: range.0,
        byte_end: range.1,
    }
}

/// Stable 64-bit symbol id: blake3 of `path#name@offset`, forced non-zero.
fn symbol_id(path: &str, name: &str, offset: u32) -> SymbolId {
    let key = format!("{path}#{name}@{offset}");
    let digest = blake3::hash(key.as_bytes());
    let raw = u64::from_le_bytes(digest.as_bytes()[..8].try_into().expect("8 bytes"));
    SymbolId::new(raw).unwrap_or_else(|| SymbolId::new(1).expect("1 is non-zero"))
}

/// Short stable tag for an `ariadne_parser` declaration kind.
fn decl_kind_tag(kind: &DeclKind) -> String {
    match kind {
        DeclKind::Function => "function",
        DeclKind::Method => "method",
        DeclKind::Class => "class",
        DeclKind::Struct => "struct",
        DeclKind::Enum => "enum",
        DeclKind::Interface => "interface",
        DeclKind::Trait => "trait",
        DeclKind::TypeAlias => "type",
        DeclKind::Record => "record",
        DeclKind::Object => "object",
        DeclKind::Module => "module",
        DeclKind::Variable => "variable",
        DeclKind::Component => "component",
        DeclKind::Other(s) => s.as_str(),
    }
    .to_owned()
}
