//! Impact queries: `blast_radius`, `file_summary`, `plan_assist`, `diff_blast`.

use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::path::Path;

use ariadne_core::{
    AffectedTestsReport, BlastRadiusReport, ComponentRow, DaemonResponse, DependencyRow,
    DiffBlastReport, DiffSeed, EdgeKind, EdgeKindFilter, FileId, FileSummaryReport, LineHunk,
    PlanAssistReport, PlanFileRow, ReadSnapshot, StorageError, SymbolId, SymbolSummary, Verbosity,
};
use ariadne_graph::economy::{self, Budget, Verbosity as EconVerbosity};
use ariadne_graph::{EdgeKindSet, FileSpanSource, spans_from};

use crate::domain::catalog::WarmCatalog;
use crate::domain::dispatch::summarize;

const DEFAULT_DEPTH: u8 = 3;
const DEFAULT_MAX_FILES: u32 = 16;
const TOP_DEPS: usize = 5;
/// Free-form kind tag carried by component symbols (ADR-0012).
const COMPONENT_KIND: &str = "component";

/// Map a request edge-kind filter to the in-RAM graph's [`EdgeKindSet`]. Every
/// variant resolves to a PRODUCIBLE graph kind (one `from_core` emits), so no
/// advertised filter returns empty (scip-driven-edges D5, T3). `Overrides` and
/// `Inherits` both alias to `OVERRIDES`: SCIP's `is_implementation` conflates
/// interface-impl, method-override, and inheritance under one signal, so both
/// honestly select the `Implements`→`Overrides` edges rather than a never-
/// produced `INHERITS` flag [src: scip-driven-edges D5; scip.proto:489-497].
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
            EdgeKindFilter::Overrides | EdgeKindFilter::Inherits => EdgeKindSet::OVERRIDES,
            EdgeKindFilter::Reads => EdgeKindSet::READS,
            EdgeKindFilter::Writes => EdgeKindSet::WRITES,
        };
    }
    set
}

/// Reverse-reachability blast radius of `symbol` at `depth`, filtered to
/// `kinds` (all kinds when empty / missing), with `must_touch` / `may_touch`
/// each capped to one page sharing a single multi-list cursor and projected at
/// `verbosity` — the warm twin of the cold `tools::blast_radius` handler, so
/// their JSON is byte-identical (parity). A malformed / stale cursor surfaces
/// as a typed `DaemonResponse::InvalidInput`.
// Mirrors the `DaemonQuery::BlastRadius` variant fields 1:1 (the dispatcher
// destructures and forwards them); bundling would just add indirection.
#[allow(clippy::too_many_arguments)]
pub(crate) fn blast_radius(
    cat: &WarmCatalog,
    symbol: &str,
    depth: Option<u8>,
    kinds: Option<&[EdgeKindFilter]>,
    limit: Option<u32>,
    cursor: Option<&str>,
    verbosity: Verbosity,
) -> DaemonResponse {
    let Some(id) = cat.find_symbol(symbol) else {
        return DaemonResponse::Error(format!("symbol {symbol} not found"));
    };
    let depth = depth.unwrap_or(DEFAULT_DEPTH).max(1);
    let set = filter_to_set(kinds.unwrap_or(&[]));
    let Some(radius) = cat.graph.blast_radius(id, depth, set) else {
        return DaemonResponse::Error(format!("symbol {symbol} absent from graph"));
    };
    let must: Vec<SymbolSummary> = radius
        .must_touch
        .into_iter()
        .map(|s| summarize(cat, s))
        .collect();
    let may: Vec<SymbolSummary> = radius
        .may_touch
        .into_iter()
        .map(|s| summarize(cat, s))
        .collect();
    let depth_used = radius.depth_used;
    let symbol_sum = summarize(cat, id);

    let revision = u32::try_from(cat.revision).unwrap_or(u32::MAX);
    let decoded = match cursor
        .map(|c| economy::Cursor::decode(c, revision))
        .transpose()
    {
        Ok(c) => c,
        Err(err) => return DaemonResponse::InvalidInput(err.to_string()),
    };
    let econ = to_economy(verbosity);
    let budget = Budget {
        limit: limit.map_or(economy::DEFAULT_PAGE, |l| l as usize),
        cursor: decoded,
        verbosity: econ,
    };
    let total_must = must.len();
    let total_may = may.len();
    let must_page = economy::paginate_sublist(must, cmp_blast_sym, &budget, 0);
    let may_page = economy::paginate_sublist(may, cmp_blast_sym, &budget, 1);
    let next_cursor = economy::multi_cursor(
        &[
            (must_page.next_offset, must_page.remainder),
            (may_page.next_offset, may_page.remainder),
        ],
        revision,
    );
    let mut truncated = Vec::new();
    if must_page.remainder {
        truncated.push((must_page.rows.len(), total_must, "must_touch"));
    }
    if may_page.remainder {
        truncated.push((may_page.rows.len(), total_may, "may_touch"));
    }
    let note = next_cursor
        .as_ref()
        .map(|_| economy::multi_truncation_note(&truncated));
    DaemonResponse::BlastRadius(BlastRadiusReport {
        symbol: project_blast_sym(symbol_sum, econ),
        must_touch: must_page
            .rows
            .into_iter()
            .map(|s| project_blast_sym(s, econ))
            .collect(),
        may_touch: may_page
            .rows
            .into_iter()
            .map(|s| project_blast_sym(s, econ))
            .collect(),
        depth_used,
        next_cursor,
        note,
    })
}

/// Map the protocol verbosity onto the economy use case's verbosity.
fn to_economy(v: Verbosity) -> EconVerbosity {
    match v {
        Verbosity::Concise => EconVerbosity::Concise,
        Verbosity::Detailed => EconVerbosity::Detailed,
    }
}

/// Stable order for a blast-radius dependent page (identical to the cold
/// handler, D4): by file, then byte offset, then name. Read before any concise
/// projection nulls `byte_start`.
fn cmp_blast_sym(a: &SymbolSummary, b: &SymbolSummary) -> Ordering {
    a.file
        .cmp(&b.file)
        .then(a.byte_start.cmp(&b.byte_start))
        .then(a.name.cmp(&b.name))
}

/// Drop a dependent row's cryptic id/offset fields in concise verbosity (D3),
/// matching the cold handler.
fn project_blast_sym(mut sym: SymbolSummary, verbosity: EconVerbosity) -> SymbolSummary {
    if matches!(verbosity, EconVerbosity::Concise) {
        sym.id = None;
        sym.byte_start = None;
        sym.byte_end = None;
    }
    sym
}

/// Canonical name of an edge destination, or `<unknown>` when absent.
fn dst_name(cat: &WarmCatalog, dst: SymbolId) -> String {
    cat.meta_of(dst)
        .map_or_else(|| String::from("<unknown>"), |m| m.name.clone())
}

/// Per-file roll-up: defined symbols, fan-in/out totals, the top-5
/// dependent files, and the component-graph neighbourhood (ADR-0012).
pub(crate) fn file_summary(cat: &WarmCatalog, path: &str) -> DaemonResponse {
    let Some(&file_id) = cat.path_to_id.get(path) else {
        return DaemonResponse::Error(format!("file {path} not found"));
    };
    let mut symbols = Vec::new();
    let mut components = Vec::new();
    let mut fan_in: u32 = 0;
    let mut fan_out: u32 = 0;
    let mut dep_counts: BTreeMap<FileId, u32> = BTreeMap::new();

    for (sid, meta) in &cat.symbols {
        if meta.file != file_id {
            continue;
        }
        symbols.push(summarize(cat, *sid));
        let fi = u32::try_from(cat.graph.fan_in(*sid)).unwrap_or(u32::MAX);
        let fo = u32::try_from(cat.graph.fan_out(*sid)).unwrap_or(u32::MAX);
        fan_in = fan_in.saturating_add(fi);
        fan_out = fan_out.saturating_add(fo);

        let is_component = meta.kind == COMPONENT_KIND;
        let mut renders = Vec::new();
        let mut hooks = Vec::new();
        let outgoing = match cat.snap.outgoing_edges(*sid) {
            Ok(edges) => edges,
            Err(err) => return DaemonResponse::Error(err.to_string()),
        };
        for (key, rec) in &outgoing {
            let target_file = rec.source_span.file;
            if target_file != file_id {
                *dep_counts.entry(target_file).or_insert(0) += 1;
            }
            if is_component {
                match key.kind {
                    EdgeKind::Renders => renders.push(dst_name(cat, key.dst)),
                    EdgeKind::UsesHook => hooks.push(dst_name(cat, key.dst)),
                    _ => {}
                }
            }
        }
        if is_component {
            renders.sort();
            hooks.sort();
            components.push(ComponentRow {
                component: meta.name.clone(),
                renders,
                hooks,
            });
        }
    }
    symbols.sort_by_key(|a| a.byte_start);
    components.sort_by(|a, b| a.component.cmp(&b.component));

    let mut deps: Vec<DependencyRow> = dep_counts
        .into_iter()
        .filter_map(|(fid, edges)| {
            cat.path_of(fid).map(|p| DependencyRow {
                file: p.to_owned(),
                edges,
            })
        })
        .collect();
    deps.sort_by(|a, b| b.edges.cmp(&a.edges).then(a.file.cmp(&b.file)));
    deps.truncate(TOP_DEPS);

    DaemonResponse::FileSummary(FileSummaryReport {
        path: path.to_owned(),
        symbols,
        fan_in,
        fan_out,
        top_dependencies: deps,
        components,
    })
}

/// Ranked file list implicated by changing `symbol`.
pub(crate) fn plan_assist(
    cat: &WarmCatalog,
    symbol: &str,
    max_files: Option<u32>,
) -> DaemonResponse {
    let Some(id) = cat.find_symbol(symbol) else {
        return DaemonResponse::Error(format!("symbol {symbol} not found"));
    };
    let max = usize::try_from(max_files.unwrap_or(DEFAULT_MAX_FILES).max(1)).unwrap_or(usize::MAX);
    let file_of = |sid: SymbolId| cat.file_of(sid);
    let plan = cat.graph.plan_assist(id, max, &file_of);
    let mut rows = Vec::with_capacity(plan.files.len());
    for row in plan.files {
        let Some(path) = cat.path_of(row.file) else {
            continue;
        };
        let mut why: Vec<String> = row
            .why
            .into_iter()
            .filter_map(|s| cat.meta_of(s).map(|m| m.name.clone()))
            .collect();
        why.sort();
        rows.push(PlanFileRow {
            file: path.to_owned(),
            why,
            certainty: row.certainty,
        });
    }
    DaemonResponse::PlanAssist(PlanAssistReport { files: rows })
}

/// Diff-aware blast radius of a changeset (tier-15c). The client computed the
/// `hunks` + `changed_paths` (the git diff lives at the MCP composition root —
/// the daemon never links `ariadne-git`, RD7); this builds the per-file symbol
/// spans from the warm symbols + the changed files' bytes (hash-guarded so a
/// file stale against its index degrades to `unresolved`, never a wrong seed),
/// then runs the graph `diff_blast` use case and projects each `SymbolId` via
/// the shared `summarize`.
// Mirrors the `DaemonQuery::DiffBlast` variant fields 1:1; bundling the tier-04
// economy controls would only add indirection, like the cold twin.
#[allow(clippy::too_many_arguments)]
pub(crate) fn diff_blast(
    cat: &WarmCatalog,
    hunks: &[LineHunk],
    changed_paths: &[String],
    depth: Option<u8>,
    kinds: Option<&[EdgeKindFilter]>,
    limit: Option<u32>,
    cursor: Option<&str>,
    verbosity: Verbosity,
) -> DaemonResponse {
    let depth = depth.unwrap_or(DEFAULT_DEPTH).max(1);
    let set = filter_to_set(kinds.unwrap_or(&[]));
    let sources = match collect_span_sources(cat, changed_paths) {
        Ok(sources) => sources,
        Err(err) => return DaemonResponse::Error(err.to_string()),
    };
    let spans = spans_from(sources);
    let report = cat
        .graph
        .diff_blast(&spans, hunks, changed_paths, depth, set);

    let revision = u32::try_from(cat.revision).unwrap_or(u32::MAX);
    let fingerprint = economy::diff_fingerprint(changed_paths);
    let decoded = match cursor
        .map(|c| economy::DiffCursor::decode(c, revision, fingerprint))
        .transpose()
    {
        Ok(c) => c,
        Err(err) => return DaemonResponse::InvalidInput(err.to_string()),
    };
    let econ = to_economy(verbosity);
    let budget = Budget {
        limit: limit.map_or(economy::DEFAULT_PAGE, |l| l as usize),
        cursor: decoded.map(|c| c.window()),
        verbosity: econ,
    };

    // Build each seed row, inner must/may capped at `limit` with a reported
    // count; the seed symbol stays detailed until after the seeds page sort.
    let seeds: Vec<DiffSeed> = report
        .seeds
        .into_iter()
        .map(|s| {
            let (must_touch, must_touch_total) =
                inner_page(s.must_touch.into_iter().map(|x| summarize(cat, x)), &budget);
            let (may_touch, may_touch_total) =
                inner_page(s.may_touch.into_iter().map(|x| summarize(cat, x)), &budget);
            DiffSeed {
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

    diff_blast_page(
        seeds,
        must,
        may,
        report.unresolved,
        &budget,
        revision,
        fingerprint,
    )
}

/// Sort, cap, project, and steer the three top-level lists behind one diff-aware
/// multi-list cursor — the warm twin of the cold `tools::diff_blast::page`, so
/// the JSON is byte-identical (parity).
// Each parameter is a distinct piece of the already-built report; bundling them
// would only add indirection.
#[allow(clippy::too_many_arguments)]
fn diff_blast_page(
    seeds: Vec<DiffSeed>,
    must: Vec<SymbolSummary>,
    may: Vec<SymbolSummary>,
    unresolved: Vec<String>,
    budget: &Budget,
    revision: u32,
    fingerprint: u64,
) -> DaemonResponse {
    let total_seeds = seeds.len();
    let total_must = must.len();
    let total_may = may.len();
    let seeds_page =
        economy::paginate_sublist(seeds, |a, b| cmp_blast_sym(&a.symbol, &b.symbol), budget, 0);
    let must_page = economy::paginate_sublist(must, cmp_blast_sym, budget, 1);
    let may_page = economy::paginate_sublist(may, cmp_blast_sym, budget, 2);
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
    DaemonResponse::DiffBlast(DiffBlastReport {
        seeds: seeds_page
            .rows
            .into_iter()
            .map(|mut s| {
                s.symbol = project_blast_sym(s.symbol, budget.verbosity);
                s
            })
            .collect(),
        must_touch: must_page
            .rows
            .into_iter()
            .map(|s| project_blast_sym(s, budget.verbosity))
            .collect(),
        may_touch: may_page
            .rows
            .into_iter()
            .map(|s| project_blast_sym(s, budget.verbosity))
            .collect(),
        unresolved,
        next_cursor,
        note,
    })
}

/// Sort + bound one seed's inner list by the fixed cap (= `budget.limit`),
/// returning the capped+projected page and the full count before capping
/// (reported, never silently dropped — and never a nested cursor, tier-04). The
/// warm twin of the cold `tools::diff_blast::inner_page`.
fn inner_page(
    rows: impl Iterator<Item = SymbolSummary>,
    budget: &Budget,
) -> (Vec<SymbolSummary>, u32) {
    let mut rows: Vec<SymbolSummary> = rows.collect();
    let total = u32::try_from(rows.len()).unwrap_or(u32::MAX);
    rows.sort_by(cmp_blast_sym);
    rows.truncate(budget.limit);
    let page = rows
        .into_iter()
        .map(|s| project_blast_sym(s, budget.verbosity))
        .collect();
    (page, total)
}

/// Static test-impact reachability of a changeset (Block A, A1). Same warm
/// shape as [`diff_blast`]: the client computed the `hunks` + `changed_paths`
/// (the daemon never links `ariadne-git`, RD7); this builds the per-file symbol
/// spans from the warm symbols + the changed files' bytes, then intersects the
/// reverse-reachable closure with the precomputed `test_roots` projection and
/// projects each `SymbolId` via the shared `summarize`.
// Mirrors the `DaemonQuery::AffectedTests` variant fields 1:1; bundling the
// tier-04 economy controls would only add indirection, like the cold twin.
#[allow(clippy::too_many_arguments)]
pub(crate) fn affected_tests(
    cat: &WarmCatalog,
    hunks: &[LineHunk],
    changed_paths: &[String],
    depth: Option<u8>,
    kinds: Option<&[EdgeKindFilter]>,
    limit: Option<u32>,
    cursor: Option<&str>,
    verbosity: Verbosity,
) -> DaemonResponse {
    let depth = depth.unwrap_or(DEFAULT_DEPTH).max(1);
    let set = filter_to_set(kinds.unwrap_or(&[]));
    let sources = match collect_span_sources(cat, changed_paths) {
        Ok(sources) => sources,
        Err(err) => return DaemonResponse::Error(err.to_string()),
    };
    let spans = spans_from(sources);
    let report =
        cat.graph
            .affected_tests(&spans, hunks, changed_paths, &cat.test_roots, depth, set);

    let tests: Vec<SymbolSummary> = report
        .tests
        .into_iter()
        .map(|s| summarize(cat, s))
        .collect();
    let seeds: Vec<SymbolSummary> = report
        .seeds
        .into_iter()
        .map(|s| summarize(cat, s))
        .collect();

    let revision = u32::try_from(cat.revision).unwrap_or(u32::MAX);
    let fingerprint = economy::diff_fingerprint(changed_paths);
    let decoded = match cursor
        .map(|c| economy::DiffCursor::decode(c, revision, fingerprint))
        .transpose()
    {
        Ok(c) => c,
        Err(err) => return DaemonResponse::InvalidInput(err.to_string()),
    };
    let econ = to_economy(verbosity);
    let budget = Budget {
        limit: limit.map_or(economy::DEFAULT_PAGE, |l| l as usize),
        cursor: decoded.map(|c| c.window()),
        verbosity: econ,
    };
    let total_tests = tests.len();
    let total_seeds = seeds.len();
    let tests_page = economy::paginate_sublist(tests, cmp_blast_sym, &budget, 0);
    let seeds_page = economy::paginate_sublist(seeds, cmp_blast_sym, &budget, 1);
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
    DaemonResponse::AffectedTests(AffectedTestsReport {
        tests: tests_page
            .rows
            .into_iter()
            .map(|s| project_blast_sym(s, econ))
            .collect(),
        seeds: seeds_page
            .rows
            .into_iter()
            .map(|s| project_blast_sym(s, econ))
            .collect(),
        unresolved: report.unresolved,
        next_cursor,
        note,
    })
}

/// Build the per-file span sources for the changed paths from the warm catalog:
/// each changed file's indexed `blake3` (the byte-offset validity guard), its
/// symbols' defining byte spans, and its current on-disk bytes read under the
/// project root. A changed path with no indexed symbols is skipped (it owns no
/// seed, so it surfaces as `unresolved`); the `blake3` guard inside `spans_from`
/// drops a file whose on-disk bytes diverged from the index. Mirrors the CLI
/// `build_symbol_lines` shape (tier-15c D3).
///
/// # Errors
/// Propagates snapshot read failures from the per-file `blake3` lookup, matching
/// the cold path (`tools::diff_blast`) and the daemon's `file_summary` handler —
/// a backend read error surfaces as a query error, not a silently-dropped seed.
fn collect_span_sources(
    cat: &WarmCatalog,
    changed_paths: &[String],
) -> Result<Vec<FileSpanSource>, StorageError> {
    let mut hash_of_file: BTreeMap<FileId, [u8; 32]> = BTreeMap::new();
    let mut file_of_path: BTreeMap<String, FileId> = BTreeMap::new();
    for path in changed_paths {
        if let Some(&fid) = cat.path_to_id.get(path) {
            if let Some(rec) = cat.snap.file(fid)? {
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

    let root = Path::new(&cat.root);
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

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use ariadne_graph::EdgeKind as GraphEdgeKind;

    use super::*;

    /// Every derivation `EdgeKind` a tier can emit — the domain over which
    /// `from_core`'s image (the set of PRODUCIBLE graph kinds) is computed.
    const DERIVATION_KINDS: [EdgeKind; 9] = [
        EdgeKind::Defines,
        EdgeKind::References,
        EdgeKind::Imports,
        EdgeKind::Renders,
        EdgeKind::UsesHook,
        EdgeKind::Reads,
        EdgeKind::Writes,
        EdgeKind::Implements,
        EdgeKind::TypeOf,
    ];

    /// All eight in-RAM graph kinds, for inverting a single-flag `EdgeKindSet`.
    const GRAPH_KINDS: [GraphEdgeKind; 8] = [
        GraphEdgeKind::Calls,
        GraphEdgeKind::Imports,
        GraphEdgeKind::TypeOf,
        GraphEdgeKind::Defines,
        GraphEdgeKind::Overrides,
        GraphEdgeKind::Reads,
        GraphEdgeKind::Writes,
        GraphEdgeKind::Inherits,
    ];

    /// All eight request filter variants.
    const FILTERS: [EdgeKindFilter; 8] = [
        EdgeKindFilter::Calls,
        EdgeKindFilter::Imports,
        EdgeKindFilter::TypeOf,
        EdgeKindFilter::Defines,
        EdgeKindFilter::Overrides,
        EdgeKindFilter::Reads,
        EdgeKindFilter::Writes,
        EdgeKindFilter::Inherits,
    ];

    /// Graph kinds reachable from the derivation alphabet through `from_core` —
    /// the only path stored edges take into the warm graph.
    fn producible() -> HashSet<GraphEdgeKind> {
        DERIVATION_KINDS
            .into_iter()
            .map(GraphEdgeKind::from_core)
            .collect()
    }

    /// The single graph kind a one-element filter's `EdgeKindSet` selects.
    fn resolved_kind(filter: EdgeKindFilter) -> GraphEdgeKind {
        let set = filter_to_set(&[filter]);
        GRAPH_KINDS
            .into_iter()
            .find(|k| k.to_flag() == set)
            .unwrap_or_else(|| panic!("filter {filter:?} maps to no single graph EdgeKind"))
    }

    /// The total-mapping honesty check (scip-driven-edges D5, T3): every
    /// `EdgeKindFilter` variant resolves, through `filter_to_set` →
    /// `EdgeKindSet` → graph kind, to a kind `from_core` actually produces. A
    /// filter that mapped to a never-produced kind would silently return empty.
    #[test]
    fn every_filter_maps_to_a_producible_edge_kind() {
        let producible = producible();
        for filter in FILTERS {
            let kind = resolved_kind(filter);
            assert!(
                producible.contains(&kind),
                "filter {filter:?} resolves to graph {kind:?}, which `from_core` never produces \
                 — the daemon must advertise no edge-kind it cannot produce",
            );
        }
    }

    /// The 5 filters that returned empty before tiers 02–03 (`TypeOf`,
    /// `Overrides`, `Reads`, `Writes`, `Inherits`) now each resolve to a real,
    /// producible edge kind — the headline honesty fix (plan D5).
    #[test]
    fn previously_empty_filters_now_resolve_to_real_edges() {
        let producible = producible();
        for filter in [
            EdgeKindFilter::TypeOf,
            EdgeKindFilter::Overrides,
            EdgeKindFilter::Reads,
            EdgeKindFilter::Writes,
            EdgeKindFilter::Inherits,
        ] {
            assert!(
                producible.contains(&resolved_kind(filter)),
                "{filter:?} must now resolve to a producible edge kind",
            );
        }
        // `is_implementation` conflation: both `Overrides` and `Inherits` select
        // the `Implements`→`Overrides` edges (plan D5), and `TypeOf` its own.
        assert_eq!(
            resolved_kind(EdgeKindFilter::Overrides),
            GraphEdgeKind::Overrides
        );
        assert_eq!(
            resolved_kind(EdgeKindFilter::Inherits),
            GraphEdgeKind::Overrides
        );
        assert_eq!(resolved_kind(EdgeKindFilter::TypeOf), GraphEdgeKind::TypeOf);
    }
}
