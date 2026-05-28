//! Impact queries: `blast_radius`, `file_summary`, `plan_assist`.

use std::collections::BTreeMap;

use ariadne_core::{
    BlastRadiusReport, ComponentRow, DaemonResponse, DependencyRow, EdgeKind, EdgeKindFilter,
    FileId, FileSummaryReport, PlanAssistReport, PlanFileRow, ReadSnapshot, SymbolId,
};
use ariadne_graph::EdgeKindSet;

use crate::domain::catalog::WarmCatalog;
use crate::domain::dispatch::summarize;

const DEFAULT_DEPTH: u8 = 3;
const DEFAULT_MAX_FILES: u32 = 16;
const TOP_DEPS: usize = 5;
/// Free-form kind tag carried by component symbols (ADR-0012).
const COMPONENT_KIND: &str = "component";

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

/// Reverse-reachability blast radius of `symbol` at `depth`, filtered to
/// `kinds` (all kinds when empty / missing).
pub(crate) fn blast_radius(
    cat: &WarmCatalog,
    symbol: &str,
    depth: Option<u8>,
    kinds: Option<&[EdgeKindFilter]>,
) -> DaemonResponse {
    let Some(id) = cat.find_symbol(symbol) else {
        return DaemonResponse::Error(format!("symbol {symbol} not found"));
    };
    let depth = depth.unwrap_or(DEFAULT_DEPTH).max(1);
    let set = filter_to_set(kinds.unwrap_or(&[]));
    let Some(radius) = cat.graph.blast_radius(id, depth, set) else {
        return DaemonResponse::Error(format!("symbol {symbol} absent from graph"));
    };
    DaemonResponse::BlastRadius(BlastRadiusReport {
        symbol: summarize(cat, id),
        must_touch: radius
            .must_touch
            .into_iter()
            .map(|s| summarize(cat, s))
            .collect(),
        may_touch: radius
            .may_touch
            .into_iter()
            .map(|s| summarize(cat, s))
            .collect(),
        depth_used: radius.depth_used,
    })
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
