//! Static documentation generator. Synthesises the tier-07 analytics
//! (coupling, cycles, dead-code) into deterministic Markdown — no LLM, no
//! external template engine, just `std::fmt::Write` into a `String` so
//! the same revision always renders the same bytes [src: tier-09
//! steps 3-4, plan.md D11].
//!
//! `write!` / `writeln!` into a `String` cannot fail, so every macro
//! `Result` is intentionally discarded with `let _ = …`.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use ariadne_core::{CoChangePair, FileChurn, ReadSnapshot, SymbolId};
use petgraph::Direction::{Incoming, Outgoing};
use petgraph::stable_graph::NodeIndex;
use petgraph::visit::{EdgeRef, IntoEdgeReferences};

use crate::build::GraphIndex;
use crate::coupling::{CouplingMetrics, ModuleSpec};
use crate::cycles::Cycle;
use crate::dead::DeadCodeConfig;
use crate::diagram::{DiagramEdge, DiagramNode, DiagramOpts, render_svg};
use crate::doc_model::DocScope;
use crate::docgen_insights;
use crate::errors::GraphError;
use crate::heuristics::{self, SymbolTable};

/// Cap on rows in the per-module caller / callee tables.
const TOP_N: usize = 10;
/// Hard cap on rendered architecture-diagram nodes; the workspace has ~12
/// crates, so this never truncates here but guards huge external repos.
const ARCH_MAX_NODES: usize = 24;

/// Render a Markdown summary for one module: purpose, public API,
/// inbound / outbound coupling, cycles, and dead code.
///
/// The "Public API" table lists *every* module member: `SymbolRecord`
/// carries no visibility flag pre-SCIP-refinement, so the export-only
/// filter from tier-09 step 3 cannot yet be applied. The rendered
/// section states this limitation inline.
///
/// `_scope` is accepted so every doc caller threads a [`DocScope`]
/// uniformly; module-level scope filtering arrives in tier-04.
///
/// # Errors
/// Propagates [`GraphError::Storage`] when the snapshot scan fails.
///
/// # Panics
/// Panics only on an internal invariant violation — `coupling_report`
/// must yield exactly one row per requested module.
#[allow(clippy::too_many_lines)]
pub fn for_module(
    graph: &GraphIndex,
    snap: &dyn ReadSnapshot,
    module: &ModuleSpec,
    _scope: &DocScope,
) -> Result<String, GraphError> {
    let table = SymbolTable::from_snapshot(snap)?;
    let member_ix: BTreeSet<NodeIndex> = module
        .members
        .iter()
        .filter_map(|s| graph.index.get(s).copied())
        .collect();

    let coupling = graph.coupling_report(std::slice::from_ref(module));
    let metrics = coupling
        .rows
        .first()
        .expect("coupling_report emits one row per input module");
    let cohesion = heuristics::cohesion(graph, module);

    let mut callers: BTreeMap<SymbolId, u32> = BTreeMap::new();
    let mut targets: BTreeMap<SymbolId, u32> = BTreeMap::new();
    let mut inbound_of: BTreeMap<SymbolId, u32> = BTreeMap::new();
    for s in &module.members {
        let Some(&ix) = graph.index.get(s) else {
            continue;
        };
        for er in graph.graph.edges_directed(ix, Incoming) {
            if !member_ix.contains(&er.source()) {
                *callers.entry(graph.graph[er.source()]).or_default() += 1;
                *inbound_of.entry(*s).or_default() += 1;
            }
        }
        for er in graph.graph.edges_directed(ix, Outgoing) {
            if !member_ix.contains(&er.target()) {
                *targets.entry(graph.graph[er.target()]).or_default() += 1;
            }
        }
    }

    let cycles = graph.cycle_report();
    let touching: Vec<&Cycle> = cycles
        .cycles
        .iter()
        .filter(|c| c.members.iter().any(|s| module.members.contains(s)))
        .collect();
    let dead = graph.dead_code(&DeadCodeConfig::default());
    let dead_members: Vec<SymbolId> = dead
        .symbols
        .iter()
        .map(|d| d.id)
        .filter(|id| module.members.contains(id))
        .collect();

    let mut md = String::new();
    let _ = writeln!(md, "# Module `{}`", module.name);
    md.push('\n');
    h2(&mut md, "Purpose");
    md.push_str(purpose(metrics));
    md.push_str("\n\n");
    let _ = writeln!(
        md,
        "Members: {} · cohesion {cohesion:.2} · abstractness {:.2}.",
        module.members.len(),
        metrics.abstractness
    );
    md.push('\n');

    h2(&mut md, "Public API");
    if module.members.is_empty() {
        md.push_str("_No symbols defined._\n\n");
    } else {
        md.push_str(
            "_Visibility metadata is unavailable pre-SCIP-refinement; the table lists \
every module member._\n\n",
        );
        md.push_str("| Symbol | Kind | Inbound refs |\n| --- | --- | --- |\n");
        for s in &module.members {
            let _ = writeln!(
                md,
                "| `{}` | {} | {} |",
                table.name(*s),
                table.kind(*s),
                inbound_of.get(s).copied().unwrap_or(0)
            );
        }
        md.push('\n');
    }

    h2(&mut md, "Inbound coupling");
    let _ = writeln!(md, "Afferent coupling (Ca): {}.", metrics.afferent);
    md.push('\n');
    push_symbol_table(&mut md, &table, "Caller", &top(&callers));

    h2(&mut md, "Outbound coupling");
    let _ = writeln!(
        md,
        "Efferent coupling (Ce): {} · instability (I): {:.2}.",
        metrics.efferent, metrics.instability
    );
    md.push('\n');
    push_symbol_table(&mut md, &table, "Callee", &top(&targets));

    h2(&mut md, "Cycles");
    if touching.is_empty() {
        md.push_str("Not part of any dependency cycle.\n\n");
    } else {
        let _ = writeln!(
            md,
            "Module participates in {} dependency cycle(s):",
            touching.len()
        );
        md.push('\n');
        for c in &touching {
            let names: Vec<String> = c
                .members
                .iter()
                .map(|s| format!("`{}`", table.name(*s)))
                .collect();
            let _ = writeln!(md, "- {}", names.join(" ⇄ "));
        }
        md.push('\n');
    }

    h2(&mut md, "Dead Code");
    if dead_members.is_empty() {
        md.push_str("No dead symbols detected.\n");
    } else {
        for id in &dead_members {
            let _ = writeln!(
                md,
                "- `{}` ({}) — no callers, no exports",
                table.name(*id),
                table.kind(*id)
            );
        }
    }
    Ok(md)
}

/// Render a project-wide Markdown architecture overview as deterministic,
/// system-only insight: a synopsis, a crate-level architecture table (with a
/// sidecar SVG reference), symbol-edge boundary violations, cycle clusters,
/// churn × complexity risk hot-spots, and refactor / hidden change-coupling.
/// Risk and change-coupling consume the git-history vectors `churn` /
/// `co_change`; empty history degrades to explicit lines (D6). An empty
/// project yields a `no modules` placeholder rather than an error.
///
/// # Errors
/// Propagates [`GraphError::Storage`] when the snapshot scan fails.
#[allow(clippy::too_many_arguments)]
pub fn for_project(
    graph: &GraphIndex,
    snap: &dyn ReadSnapshot,
    modules: &[ModuleSpec],
    churn: &[FileChurn],
    co_change: &[CoChangePair],
    scope: &DocScope,
) -> Result<String, GraphError> {
    let table = SymbolTable::from_snapshot(snap)?;
    let mut md = String::from("# Project Architecture Overview\n\n");
    if modules.is_empty() {
        h2(&mut md, "Synopsis");
        md.push_str("_No modules indexed._\n");
        return Ok(md);
    }
    // Doc-layer source scoping: every reported section covers only in-scope
    // (default: Source) modules; the graph itself is never filtered [D3].
    let scoped: Vec<ModuleSpec> = modules
        .iter()
        .filter(|m| scope.include(&m.name))
        .cloned()
        .collect();

    h2(&mut md, "Synopsis");
    md.push_str(&docgen_insights::synopsis(graph, &scoped, &table, scope));

    h2(&mut md, "Architecture");
    md.push_str(&docgen_insights::architecture_section(graph, &scoped));

    h2(&mut md, "Boundary violations");
    md.push_str(&docgen_insights::boundary_violations(graph, &table, scope));

    h2(&mut md, "Cycle clusters");
    md.push_str(&docgen_insights::cycle_clusters(graph, &table));

    h2(&mut md, "Risk hot-spots");
    md.push_str(&docgen_insights::risk_hotspots(&table, churn, scope));

    h2(&mut md, "Refactor & change-coupling");
    md.push_str(&docgen_insights::change_coupling(
        graph, snap, &scoped, churn, co_change, &table,
    )?);

    Ok(md)
}

/// Render the crate-level architecture DAG to a deterministic sidecar SVG
/// (D2/D4). Scoped file-modules aggregate into crate nodes via [`crate_key`];
/// inter-crate symbol edges become diagram edges, deduped and capped, then
/// drawn by the tier-02 [`render_svg`] emitter. Pure and IO-free — the CLI
/// owns the file write.
#[must_use]
pub fn architecture_svg(graph: &GraphIndex, modules: &[ModuleSpec], scope: &DocScope) -> String {
    let scoped: Vec<ModuleSpec> = modules
        .iter()
        .filter(|m| scope.include(&m.name))
        .cloned()
        .collect();
    let member_of = heuristics::member_index(graph, &scoped);

    let node_ids: BTreeSet<String> = scoped
        .iter()
        .map(|m| docgen_insights::crate_key(&m.name).to_owned())
        .collect();
    let mut edge_set: BTreeSet<(String, String)> = BTreeSet::new();
    for er in graph.graph.edge_references() {
        if let (Some(&si), Some(&di)) = (member_of.get(&er.source()), member_of.get(&er.target())) {
            let src = docgen_insights::crate_key(&scoped[si].name);
            let dst = docgen_insights::crate_key(&scoped[di].name);
            if src != dst {
                edge_set.insert((src.to_owned(), dst.to_owned()));
            }
        }
    }

    let nodes: Vec<DiagramNode> = node_ids
        .into_iter()
        .map(|id| DiagramNode {
            label: id.clone(),
            id,
        })
        .collect();
    let edges: Vec<DiagramEdge> = edge_set
        .into_iter()
        .map(|(from, to)| DiagramEdge { from, to })
        .collect();
    render_svg(
        &nodes,
        &edges,
        &DiagramOpts {
            max_nodes: ARCH_MAX_NODES,
        },
    )
}

/// Append a level-2 header followed by a blank line.
fn h2(md: &mut String, title: &str) {
    md.push_str("## ");
    md.push_str(title);
    md.push_str("\n\n");
}

/// One-line static purpose inference from the module's coupling shape.
pub(crate) fn purpose(m: &CouplingMetrics) -> &'static str {
    if m.afferent == 0 && m.efferent == 0 {
        "Isolated module — no coupling to the rest of the graph."
    } else if m.instability < 0.3 {
        "Stable foundational module — many dependents, few dependencies."
    } else if m.instability > 0.7 {
        "Volatile leaf module — depends outward, little depended upon."
    } else {
        "Intermediate module — balanced inbound and outbound coupling."
    }
}

/// Rank a `symbol → count` histogram by descending count then ascending
/// id, capped at [`TOP_N`].
fn top(map: &BTreeMap<SymbolId, u32>) -> Vec<(SymbolId, u32)> {
    let mut v: Vec<(SymbolId, u32)> = map.iter().map(|(k, c)| (*k, *c)).collect();
    v.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    v.truncate(TOP_N);
    v
}

/// Append a `role | kind | edges` table for a ranked symbol list.
fn push_symbol_table(md: &mut String, table: &SymbolTable, role: &str, rows: &[(SymbolId, u32)]) {
    if rows.is_empty() {
        md.push_str("_None._\n\n");
        return;
    }
    let _ = writeln!(md, "| {role} | Kind | Edges |");
    md.push_str("| --- | --- | --- |\n");
    for (id, n) in rows {
        let _ = writeln!(md, "| `{}` | {} | {n} |", table.name(*id), table.kind(*id));
    }
    md.push('\n');
}
