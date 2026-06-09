//! `diff_blast_radius` cold path — runs the `ariadne_graph` `diff_blast` use
//! case against the cold [`Catalog`].
//!
//! The git diff (`ariadne_git::diff`) runs in the server handler before this
//! (both the daemon and cold paths need its `hunks` + `changed_paths`); this
//! module joins those line hunks to the indexed symbol spans and folds the
//! per-seed blast radius into the deduped report. The cold catalog carries the
//! symbol spans but not the per-file `blake3`, so `storage` supplies the
//! byte-offset validity guard; the shared `spans_from` builder drops a file
//! whose on-disk bytes diverged from the index (→ unresolved, never a wrong
//! seed) [src: .claude/plans/post-v1-roadmap/tier-15c-diff-blast-radius-tool.md
//!  step 5; D3].

use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::path::Path;

use ariadne_core::{FileId, LineHunk, ReadSnapshot, Storage, SymbolId};
use ariadne_graph::economy::{self, Budget, Verbosity as EconVerbosity};
use ariadne_graph::{EdgeKindSet, FileSpanSource, spans_from};

use crate::catalog::Catalog;
use crate::errors::McpError;
use crate::tools::summarize;
use crate::types::{
    DiffBlastOutput, DiffSeedRow, EdgeKindFilter, SymbolSummary, Verbosity as WireVerbosity,
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

/// Compute the diff-aware blast radius over the cold catalog. `hunks` +
/// `changed_paths` come from the already-run `ariadne_git::diff`; `storage`
/// supplies each changed file's indexed `blake3`, and `root` anchors the
/// on-disk reads. The three top-level lists (`seeds`, `must_touch`, `may_touch`)
/// each cap to one page sharing a single diff-aware multi-list cursor; each
/// seed's inner must/may are bounded by the fixed cap (= `limit`) with a
/// reported count (Block 1, tier-04). The warm `impact::diff_blast` mirrors this
/// shape so the JSON is byte-identical (parity).
///
/// # Errors
/// Propagates storage snapshot failures from the `blake3` lookup, or
/// [`McpError::InvalidInput`] when `cursor` is malformed, was minted against a
/// different index revision, or was minted against a different changeset (its
/// changed-paths fingerprint no longer matches).
// Each argument is a distinct, plan-mandated facet of the cold join: the cold
// catalog (graph + spans), the storage handle (per-file `blake3`), the project
// root (on-disk reads), the changeset's hunks + changed-path list, the v1
// depth/kind filter, and the tier-04 economy controls. Bundling them would add
// an input type the tier does not call for.
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
) -> Result<DiffBlastOutput, McpError> {
    let depth = depth.unwrap_or(DEFAULT_DEPTH).max(1);
    let set = filter_to_set(kinds.unwrap_or(&[]));
    let spans = spans_from(collect_span_sources(cat, storage, root, changed_paths)?);
    let report = cat
        .graph
        .diff_blast(&spans, hunks, changed_paths, depth, set);

    let revision = u32::try_from(cat.revision).unwrap_or(u32::MAX);
    let fingerprint = economy::diff_fingerprint(changed_paths);
    let decoded = cursor
        .map(|c| economy::DiffCursor::decode(c, revision, fingerprint))
        .transpose()
        .map_err(|e| McpError::InvalidInput(e.to_string()))?;
    let verbosity = to_economy(verbosity);
    let budget = Budget {
        limit: limit.map_or(economy::DEFAULT_PAGE, |l| l as usize),
        cursor: decoded.map(|c| c.window()),
        verbosity,
    };

    // Build each seed row (inner must/may capped at `limit` with a count); the
    // seed symbol stays detailed until after the seeds page sort.
    let seeds: Vec<DiffSeedRow> = report
        .seeds
        .into_iter()
        .map(|s| {
            let (must_touch, must_touch_total) =
                inner_page(s.must_touch.into_iter().map(|x| summarize(cat, x)), &budget);
            let (may_touch, may_touch_total) =
                inner_page(s.may_touch.into_iter().map(|x| summarize(cat, x)), &budget);
            DiffSeedRow {
                symbol: summarize(cat, s.symbol),
                must_touch,
                may_touch,
                depth_used: s.depth_used,
                must_touch_total,
                may_touch_total,
            }
        })
        .collect();
    let must: Vec<SymbolSummary> = report
        .must_touch
        .into_iter()
        .map(|x| summarize(cat, x))
        .collect();
    let may: Vec<SymbolSummary> = report
        .may_touch
        .into_iter()
        .map(|x| summarize(cat, x))
        .collect();

    Ok(page(
        seeds,
        must,
        may,
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

/// Stable order for a dependent / seed-symbol page: by file, then byte offset,
/// then name — a deterministic top-N independent of graph order (tier-04).
/// Read before any concise projection nulls `byte_start`.
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

/// Sort + bound one seed's inner list by the fixed cap (= `budget.limit`),
/// returning the capped+projected page and the full count before capping
/// (reported, never silently dropped — and never a nested cursor, tier-04).
fn inner_page(
    rows: impl Iterator<Item = SymbolSummary>,
    budget: &Budget,
) -> (Vec<SymbolSummary>, u32) {
    let mut rows: Vec<SymbolSummary> = rows.collect();
    let total = u32::try_from(rows.len()).unwrap_or(u32::MAX);
    rows.sort_by(cmp_sym);
    rows.truncate(budget.limit);
    let page = rows
        .into_iter()
        .map(|s| project(s, budget.verbosity))
        .collect();
    (page, total)
}

/// Sort, cap, project, and steer the three top-level lists behind one diff-aware
/// multi-list cursor. Shared shape with the warm daemon handler so the JSON is
/// byte-identical (parity).
// Each parameter is a distinct piece of the already-built report the page
// assembles; bundling them would only add indirection.
#[allow(clippy::too_many_arguments)]
fn page(
    seeds: Vec<DiffSeedRow>,
    must: Vec<SymbolSummary>,
    may: Vec<SymbolSummary>,
    unresolved: Vec<String>,
    budget: &Budget,
    revision: u32,
    fingerprint: u64,
) -> DiffBlastOutput {
    let total_seeds = seeds.len();
    let total_must = must.len();
    let total_may = may.len();
    let seeds_page =
        economy::paginate_sublist(seeds, |a, b| cmp_sym(&a.symbol, &b.symbol), budget, 0);
    let must_page = economy::paginate_sublist(must, cmp_sym, budget, 1);
    let may_page = economy::paginate_sublist(may, cmp_sym, budget, 2);
    let next_cursor = economy::diff_multi_cursor(
        &[
            (seeds_page.next_offset, seeds_page.remainder),
            (must_page.next_offset, must_page.remainder),
            (may_page.next_offset, may_page.remainder),
        ],
        revision,
        fingerprint,
    );
    let mut truncated = Vec::new();
    if seeds_page.remainder {
        truncated.push((seeds_page.rows.len(), total_seeds, "seeds"));
    }
    if must_page.remainder {
        truncated.push((must_page.rows.len(), total_must, "must_touch"));
    }
    if may_page.remainder {
        truncated.push((may_page.rows.len(), total_may, "may_touch"));
    }
    let note = next_cursor
        .as_ref()
        .map(|_| economy::multi_truncation_note(&truncated));
    DiffBlastOutput {
        seeds: seeds_page
            .rows
            .into_iter()
            .map(|mut s| {
                s.symbol = project(s.symbol, budget.verbosity);
                s
            })
            .collect(),
        must_touch: must_page
            .rows
            .into_iter()
            .map(|s| project(s, budget.verbosity))
            .collect(),
        may_touch: may_page
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
/// byte spans (from the cold catalog, which carries them), and its current
/// on-disk bytes under `root`. A changed path with no indexed symbols is
/// skipped — it owns no seed, so it surfaces as `unresolved`. Mirrors the CLI
/// `build_symbol_lines` + daemon `collect_span_sources` shape (tier-15c D3).
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
