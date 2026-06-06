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

use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use ariadne_core::{FileId, FileRecord, Lang, ReadSnapshot, RevisionId, ScipFacts, Storage};
use ariadne_parser::{CallKind, DeclKind, FactExtractor, ParserRegistry, SyntacticFacts};
use ariadne_salsa::{
    AriadneDb, CallRaw, DeclRaw, HookRaw, ImportRaw, RenderRaw, ScipFactsRaw, ScipOccurrenceRaw,
    ScipRelationshipRaw, SyntacticFactsRaw,
};
use ariadne_scip::{IngestPlan, IngestReport, extract_facts};
use ariadne_storage::RedbStorage;
use ignore::{DirEntry, WalkBuilder, WalkState};
use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;
use serde::Serialize;

use crate::config::Config;

/// Chunk size for the post-commit count scans. The summary's symbol/edge
/// counts are read from the committed snapshot (the authoritative persisted
/// record set), streamed a chunk at a time to bound the count's working set.
const COUNT_CHUNK: usize = 65_536;

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

/// One file's parse output, collected for the shared derivation. Retains the
/// raw bytes so the salsa `FileContentInput` carries the content hash and so
/// an SFC's synthesized `def_range` end matches `record.size`.
struct ParsedFile {
    id: FileId,
    record: FileRecord,
    lang: Lang,
    content: Vec<u8>,
    /// `None` when the parse aborted (timeout or extraction failure).
    facts: Option<SyntacticFacts>,
}

/// Run the full cold-index pipeline against `root` and commit to redb.
///
/// External SCIP indexers run when `scip` is set — default-on via the
/// `--no-scip` opt-out (scip-driven-edges D6). They run OUT OF BAND in Phase 4,
/// after the fast tree-sitter index has already committed, so the full language
/// builds they perform never count against the measured cold-index wall-clock
/// (R9) [src: docs/adr/0026-default-on-out-of-band-scip.md;
/// docs/adr/0009-parallel-cold-index.md].
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

    // Phase 2 — parallel parse, collected for the shared derivation. The
    // streaming committer is gone: the per-file derivation now lives in
    // `ariadne-salsa` so the cold index and the daemon warm graph share one
    // path [src: post-v1-roadmap plan.md RD11].
    let registry = ParserRegistry::new();
    let probe = ParseProbe::default();
    let bar = progress_bar(paths.len());

    let parse_started = Instant::now();
    let sink: Mutex<Vec<ParsedFile>> = Mutex::new(Vec::with_capacity(paths.len()));
    paths.par_iter().enumerate().for_each_init(
        || ThreadState {
            registry: registry.clone(),
            extractors: HashMap::new(),
        },
        |state, (idx, path)| {
            bar.inc(1);
            if let Some(parsed) = parse_one(root, idx, path, state, &probe) {
                sink.lock().expect("parse sink mutex poisoned").push(parsed);
            }
        },
    );
    let parse_ms = parse_started.elapsed().as_millis();
    bar.finish_and_clear();
    let mut parsed = sink.into_inner().expect("parse sink mutex poisoned");
    // Seed + derive in `FileId` order so the per-file derivation runs in the
    // same order the streaming committer once drained the channel.
    parsed.sort_by_key(|p| p.id);

    // Phase 3 — seed the salsa inputs from the parsed facts, then run the
    // shared derivation and commit one changeset.
    let storage = RedbStorage::open(&db_path).context("open redb index")?;
    let mut db = AriadneDb::new();
    let mut lang_first_seen: HashMap<String, FileId> = HashMap::new();
    let mut parse_failures = 0usize;
    let files = parsed.len();

    let seed_started = Instant::now();
    for pf in parsed {
        lang_first_seen
            .entry(pf.lang.tag())
            .and_modify(|seen| *seen = (*seen).min(pf.id))
            .or_insert(pf.id);
        let facts = if let Some(f) = pf.facts {
            convert_facts(&f)
        } else {
            parse_failures += 1;
            SyntacticFactsRaw::default()
        };
        db.seed_file(pf.id, pf.record, pf.content, facts);
    }
    let resolve_ms = seed_started.elapsed().as_millis();

    let commit_started = Instant::now();
    let mut revision = db
        .commit_revision(&storage)
        .context("commit derived changeset")?;
    let commit_ms = commit_started.elapsed().as_millis();

    // Phase 4 — SCIP ingest, off the measured fast path (scip-driven-edges D6).
    // The fast tree-sitter index has already committed; when `scip` is set the
    // external indexers run, `extract_facts` reduces their report to per-file
    // occurrence facts, those are set on the salsa inputs, and a re-commit
    // replaces covered files' edges with the precise SCIP edges (plan D2, D4).
    // A missing indexer or a hash that has moved off the index degrades that
    // file to the tree-sitter resolver (plan D4) — never a failure.
    let scip_started = Instant::now();
    let scip_report = if scip {
        Some(run_scip_ingest(&mut db, root, &storage, &mut revision)?)
    } else {
        None
    };
    let scip_ms = scip_started.elapsed().as_millis();

    // Counts come from the final committed snapshot — reflecting any SCIP edges
    // — the authoritative persisted record set the cold byte-parity gate
    // compares against.
    let snapshot = storage.snapshot().context("snapshot committed index")?;
    let mut symbols = 0usize;
    for chunk in snapshot.iter_symbols(COUNT_CHUNK)? {
        symbols += chunk.context("count symbol chunk")?.len();
    }
    let mut edges = 0usize;
    for chunk in snapshot.iter_edges(COUNT_CHUNK)? {
        edges += chunk.context("count edge chunk")?.len();
    }

    let summary = IndexSummary {
        files,
        symbols,
        edges,
        langs: ordered_langs(&lang_first_seen),
        scip_successes: scip_report
            .as_ref()
            .map(|s| s.successes.iter().map(Lang::tag).collect())
            .unwrap_or_default(),
        scip_missing: scip_report
            .as_ref()
            .map(|s| s.warnings.iter().map(|w| w.binary.clone()).collect())
            .unwrap_or_default(),
        parse_failures,
        revision: revision.0,
        elapsed_ms: started.elapsed().as_millis(),
    };
    let timings = PhaseTimings {
        walk: walk_ms,
        parse: parse_ms,
        resolve: resolve_ms,
        commit: commit_ms,
        scip: scip_ms,
    };
    Ok((summary, timings, probe.snapshot()))
}

/// Run the external SCIP indexers against `root` and reduce their report to
/// per-file facts (scip-driven-edges D2, D6). The composition root calls this
/// OFF the synchronous path — the cold index folds the result inline in Phase 4
/// via [`run_scip_ingest`]; the daemon runs it on a background thread and ships
/// the facts to the live pump (`ariadne_daemon::ScipFactsBatch`). A degraded run
/// — every indexer binary absent — yields an empty batch, never a failure
/// (plan R1); covered files then keep the precise tree-sitter resolver (D4).
///
/// Returns `(relative_path, ScipFacts)` pairs ready for `set_scip_facts`.
#[must_use]
pub fn scip_facts(root: &Path) -> Vec<(String, ScipFacts)> {
    let report = IngestPlan::with_default_drivers().ingest(root);
    extract_facts(&report)
}

/// Run the out-of-band SCIP ingest and fold its edges into the index
/// (scip-driven-edges tier-01, plan D6). The external indexers run, then
/// `extract_facts` reduces the report to per-file occurrence facts; those are
/// set on the salsa inputs and a re-commit replaces covered files' edges with
/// the precise SCIP edges (plan D2, D4). `revision` is advanced to the SCIP
/// commit when any facts were applied. Returns the raw report for the summary's
/// success/missing language lists. A degraded run (no facts) leaves the fast
/// index untouched.
///
/// # Errors
/// Propagates storage write failures from the re-commit.
fn run_scip_ingest(
    db: &mut AriadneDb,
    root: &Path,
    storage: &RedbStorage,
    revision: &mut RevisionId,
) -> Result<IngestReport> {
    let report = IngestPlan::with_default_drivers().ingest(root);
    let facts = extract_facts(&report);
    if !facts.is_empty() {
        for (path, scip_facts) in facts {
            let occurrences = scip_facts
                .occurrences
                .into_iter()
                .map(|o| ScipOccurrenceRaw {
                    symbol: o.symbol,
                    byte_range: o.byte_range,
                    roles: o.roles,
                })
                .collect();
            let relationships = scip_facts
                .relationships
                .into_iter()
                .map(|r| ScipRelationshipRaw {
                    from: r.from,
                    to: r.to,
                    is_implementation: r.is_implementation,
                    is_type_definition: r.is_type_definition,
                })
                .collect();
            db.set_scip_facts(
                &path,
                ScipFactsRaw {
                    occurrences,
                    relationships,
                },
                scip_facts.indexed_hash,
            );
        }
        *revision = db
            .commit_revision(storage)
            .context("commit scip-derived edges")?;
    }
    Ok(report)
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
/// worker thread.
///
/// `extractors` is keyed by *every* [`Lang`] a worker meets across all
/// parse layers — a Vue SFC contributes both the `Vue` host-layer extractor
/// and the injected `TypeScript` extractor — so an SFC's `<script>` facts
/// reuse the same compiled query as a plain `.ts` file [src: tier-05 step 3].
struct ThreadState {
    registry: ParserRegistry,
    extractors: HashMap<Lang, FactExtractor>,
}

/// Read, parse, and extract syntactic facts for one walked path. Returns
/// `None` only when the file is unreadable (it then contributes to no
/// count); a parse abort yields a [`ParsedFile`] with `facts: None` so the
/// file is still recorded. The raw bytes are retained on the [`ParsedFile`]
/// so the salsa input layer can hold the content + hash [src: tier-07a].
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
        path: rel_path,
        lang,
        size,
        blake3: hash,
        mtime_ns,
    };
    let facts = parse_facts(lang, &content, state, probe);
    Some(ParsedFile {
        id,
        record,
        lang,
        content,
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

/// Language tags ordered by the lowest `FileId` they were observed at —
/// reproduces tier-12's first-seen-in-path-order regardless of parse order.
fn ordered_langs(first_seen: &HashMap<String, FileId>) -> Vec<String> {
    let mut langs: Vec<String> = first_seen.keys().cloned().collect();
    langs.sort_by_key(|tag| first_seen[tag]);
    langs
}

/// Convert one file's parser [`SyntacticFacts`] into the `Update`-safe
/// [`SyntacticFactsRaw`] the salsa input carries. This is the
/// composition-root boundary: `decl_kind_tag` and `Visibility::to_byte` map
/// the parser's enums to the byte/string mirrors here because `ariadne-salsa`
/// may not depend on `ariadne-parser` [src: tests/architecture.rs lines
/// 30-33; post-v1-roadmap plan.md RD11].
fn convert_facts(facts: &SyntacticFacts) -> SyntacticFactsRaw {
    SyntacticFactsRaw {
        decls: facts
            .decls
            .iter()
            .map(|d| DeclRaw {
                kind: decl_kind_tag(&d.kind),
                name: d.name.clone(),
                name_byte_range: d.name_byte_range,
                def_byte_range: d.def_byte_range,
                visibility_byte: d.visibility.to_byte(),
                attributes: d.attributes.clone(),
                complexity: d.complexity,
            })
            .collect(),
        imports: facts
            .imports
            .iter()
            .map(|i| ImportRaw {
                path: i.path.clone(),
                byte_range: i.byte_range,
            })
            .collect(),
        calls: facts
            .calls
            .iter()
            .map(|c| CallRaw {
                callee: c.callee.clone(),
                kind_byte: call_kind_byte(c.kind),
                byte_range: c.byte_range,
            })
            .collect(),
        renders: facts
            .renders
            .iter()
            .map(|r| RenderRaw {
                component: r.component.clone(),
                byte_range: r.byte_range,
            })
            .collect(),
        hooks: facts
            .hooks
            .iter()
            .map(|h| HookRaw {
                callee: h.callee.clone(),
                byte_range: h.byte_range,
            })
            .collect(),
    }
}

/// Byte mirror of an `ariadne_parser` call shape for the `Update`-safe salsa
/// `CallRaw.kind_byte` (`Free=0`, `Method=1`, `Path=2`); the resolver decodes
/// it to gate the cross-crate fallback to free calls [src: ADR-0024]. Mirrors
/// `decl_kind_tag` / `Visibility::to_byte` — the map lives at the composition
/// root because `ariadne-salsa` may not depend on `ariadne-parser`.
fn call_kind_byte(kind: CallKind) -> u8 {
    match kind {
        CallKind::Free => 0,
        CallKind::Method => 1,
        CallKind::Path => 2,
    }
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
