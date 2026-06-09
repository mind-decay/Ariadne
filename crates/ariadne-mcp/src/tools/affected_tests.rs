//! `affected_tests` cold path — runs the `ariadne_graph` `affected_tests` use
//! case against the cold [`Catalog`] (Block A, A1).
//!
//! The git diff (`ariadne_git::diff`) runs in the server handler before this
//! (both the daemon and cold paths need its `hunks` + `changed_paths`); this
//! module classifies the catalog's symbols into the test-root set, joins the
//! line hunks to the indexed symbol spans, and returns the test subset of the
//! changed seeds ∪ their reverse-reachable closure. The cold catalog carries
//! the symbol spans but not the per-file `blake3`, so `storage` supplies the
//! byte-offset validity guard; the shared `spans_from` builder drops a file
//! whose on-disk bytes diverged from the index (→ unresolved, never a wrong
//! seed), mirroring the cold `diff_blast` tool (tier-15c D3).

use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::path::Path;

use ariadne_core::{FileId, LineHunk, ReadSnapshot, Storage, SymbolId};
use ariadne_graph::economy::{self, Budget, Verbosity as EconVerbosity};
use ariadne_graph::{
    EdgeKindSet, FileSpanSource, TestRootInput, classify_test_symbols, spans_from,
};

use crate::catalog::Catalog;
use crate::errors::McpError;
use crate::tools::summarize;
use crate::types::{
    AffectedTestsOutput, EdgeKindFilter, SymbolSummary, Verbosity as WireVerbosity,
};

const DEFAULT_DEPTH: u8 = 3;

fn filter_to_set(filter: &[EdgeKindFilter]) -> EdgeKindSet {
    if filter.is_empty() {
        return EdgeKindSet::ALL;
    }
    let mut set = EdgeKindSet::empty();
    for f in filter {
        set |= match f {
            EdgeKindFilter::Calls => EdgeKindSet::CALLS,
            EdgeKindFilter::Imports => EdgeKindSet::IMPORTS,
            EdgeKindFilter::TypeOf => EdgeKindSet::TYPE_OF,
            EdgeKindFilter::Defines => EdgeKindSet::DEFINES,
            EdgeKindFilter::Overrides => EdgeKindSet::OVERRIDES,
            EdgeKindFilter::Reads => EdgeKindSet::READS,
            EdgeKindFilter::Writes => EdgeKindSet::WRITES,
            EdgeKindFilter::Inherits => EdgeKindSet::INHERITS,
        };
    }
    set
}

/// Compute the tests a changeset reaches over the cold catalog. `hunks` +
/// `changed_paths` come from the already-run `ariadne_git::diff`; the test-root
/// set is classified on the fly from the catalog's per-symbol metadata (the
/// same pure classifier the warm projection precomputes), so the cold and warm
/// answers match. `storage` supplies each changed file's indexed `blake3`, and
/// `root` anchors the on-disk reads. The two top-level lists (`tests`, `seeds`)
/// each cap to one page sharing a single diff-aware multi-list cursor and are
/// projected at `verbosity` (Block 1, tier-04); the warm handler mirrors this so
/// the JSON is byte-identical (parity).
///
/// # Errors
/// Propagates storage snapshot failures from the `blake3` lookup, or
/// [`McpError::InvalidInput`] when `cursor` is malformed, was minted against a
/// different index revision, or was minted against a different changeset.
// Each argument is a distinct, plan-mandated facet of the cold join — the cold
// catalog, the storage handle, the project root, the changeset's hunks +
// changed-path list, the v1 depth/kind filter, and the tier-04 economy controls.
#[allow(clippy::too_many_arguments)]
pub fn handle<S: Storage>(
    cat: &Catalog,
    storage: &S,
    root: &Path,
    hunks: &[LineHunk],
    changed_paths: &[String],
    depth: Option<u8>,
    kinds: Option<&[EdgeKindFilter]>,
    limit: Option<u32>,
    cursor: Option<&str>,
    verbosity: WireVerbosity,
) -> Result<AffectedTestsOutput, McpError> {
    let depth = depth.unwrap_or(DEFAULT_DEPTH).max(1);
    let set = filter_to_set(kinds.unwrap_or(&[]));

    let test_roots = classify_test_symbols(cat.symbols.iter().map(|(id, m)| TestRootInput {
        id: *id,
        lang: m.lang,
        path: cat.path_of(m.file).unwrap_or(""),
        kind: &m.kind,
        name: &m.name,
        attributes: &m.attributes,
    }));

    let spans = spans_from(collect_span_sources(cat, storage, root, changed_paths)?);
    let report = cat
        .graph
        .affected_tests(&spans, hunks, changed_paths, &test_roots, depth, set);

    let tests: Vec<SymbolSummary> = report
        .tests
        .into_iter()
        .map(|x| summarize(cat, x))
        .collect();
    let seeds: Vec<SymbolSummary> = report
        .seeds
        .into_iter()
        .map(|x| summarize(cat, x))
        .collect();

    let revision = u32::try_from(cat.revision).unwrap_or(u32::MAX);
    let fingerprint = economy::diff_fingerprint(changed_paths);
    let decoded = cursor
        .map(|c| economy::DiffCursor::decode(c, revision, fingerprint))
        .transpose()
        .map_err(|e| McpError::InvalidInput(e.to_string()))?;
    let budget = Budget {
        limit: limit.map_or(economy::DEFAULT_PAGE, |l| l as usize),
        cursor: decoded.map(|c| c.window()),
        verbosity: to_economy(verbosity),
    };
    Ok(page(
        tests,
        seeds,
        report.unresolved,
        &budget,
        revision,
        fingerprint,
    ))
}

/// Map the MCP-facing verbosity onto the economy use case's verbosity.
fn to_economy(v: WireVerbosity) -> EconVerbosity {
    match v {
        WireVerbosity::Concise => EconVerbosity::Concise,
        WireVerbosity::Detailed => EconVerbosity::Detailed,
    }
}

/// Stable order for a row page: by file, then byte offset, then name — a
/// deterministic top-N independent of graph order (tier-04). Read before any
/// concise projection nulls `byte_start`.
fn cmp_sym(a: &SymbolSummary, b: &SymbolSummary) -> Ordering {
    a.file
        .cmp(&b.file)
        .then(a.byte_start.cmp(&b.byte_start))
        .then(a.name.cmp(&b.name))
}

/// Drop the cryptic id/offset fields in concise verbosity (tier-04 D3).
fn project(mut sym: SymbolSummary, verbosity: EconVerbosity) -> SymbolSummary {
    if matches!(verbosity, EconVerbosity::Concise) {
        sym.id = None;
        sym.byte_start = None;
        sym.byte_end = None;
    }
    sym
}

/// Sort, cap, project, and steer the two top-level lists behind one diff-aware
/// multi-list cursor. Shared shape with the warm daemon handler (parity).
// Each parameter is a distinct piece of the already-built report; bundling them
// would only add indirection, like the cold `diff_blast::page`.
#[allow(clippy::too_many_arguments)]
fn page(
    tests: Vec<SymbolSummary>,
    seeds: Vec<SymbolSummary>,
    unresolved: Vec<String>,
    budget: &Budget,
    revision: u32,
    fingerprint: u64,
) -> AffectedTestsOutput {
    let total_tests = tests.len();
    let total_seeds = seeds.len();
    let tests_page = economy::paginate_sublist(tests, cmp_sym, budget, 0);
    let seeds_page = economy::paginate_sublist(seeds, cmp_sym, budget, 1);
    let next_cursor = economy::diff_multi_cursor(
        &[
            (tests_page.next_offset, tests_page.remainder),
            (seeds_page.next_offset, seeds_page.remainder),
        ],
        revision,
        fingerprint,
    );
    let mut truncated = Vec::new();
    if tests_page.remainder {
        truncated.push((tests_page.rows.len(), total_tests, "tests"));
    }
    if seeds_page.remainder {
        truncated.push((seeds_page.rows.len(), total_seeds, "seeds"));
    }
    let note = next_cursor
        .as_ref()
        .map(|_| economy::multi_truncation_note(&truncated));
    AffectedTestsOutput {
        tests: tests_page
            .rows
            .into_iter()
            .map(|s| project(s, budget.verbosity))
            .collect(),
        seeds: seeds_page
            .rows
            .into_iter()
            .map(|s| project(s, budget.verbosity))
            .collect(),
        unresolved,
        next_cursor,
        note,
    }
}

/// Build the per-file span sources for the changed paths: each changed file's
/// indexed `blake3` (read transiently from `storage`), its symbols' defining
/// byte spans (from the cold catalog), and its current on-disk bytes under
/// `root`. A changed path with no indexed symbols is skipped — it owns no seed,
/// so it surfaces as `unresolved`. Mirrors the cold `diff_blast` tool's join.
fn collect_span_sources<S: Storage>(
    cat: &Catalog,
    storage: &S,
    root: &Path,
    changed_paths: &[String],
) -> Result<Vec<FileSpanSource>, McpError> {
    let snap = storage.snapshot().map_err(McpError::Storage)?;

    let mut hash_of_file: BTreeMap<FileId, [u8; 32]> = BTreeMap::new();
    let mut file_of_path: BTreeMap<String, FileId> = BTreeMap::new();
    for path in changed_paths {
        if let Some(&fid) = cat.path_to_id.get(path) {
            if let Some(rec) = snap.file(fid).map_err(McpError::Storage)? {
                hash_of_file.insert(fid, rec.blake3);
                file_of_path.insert(path.clone(), fid);
            }
        }
    }

    let mut symbols_of_file: BTreeMap<FileId, Vec<(SymbolId, u32, u32)>> = BTreeMap::new();
    for (sid, meta) in &cat.symbols {
        if hash_of_file.contains_key(&meta.file) {
            symbols_of_file.entry(meta.file).or_default().push((
                *sid,
                meta.byte_start,
                meta.byte_end,
            ));
        }
    }

    let mut sources = Vec::new();
    for (path, fid) in file_of_path {
        let Some(symbols) = symbols_of_file.remove(&fid) else {
            continue;
        };
        let Ok(content) = std::fs::read(root.join(&path)) else {
            continue;
        };
        sources.push(FileSpanSource {
            blake3: hash_of_file[&fid],
            path,
            symbols,
            content,
        });
    }
    Ok(sources)
}
