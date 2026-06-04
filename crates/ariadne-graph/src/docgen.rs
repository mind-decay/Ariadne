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
use crate::doc_model::{DocScope, crate_of};
use crate::docgen_insights;
use crate::errors::GraphError;
use crate::heuristics::{self, SymbolTable};

/// Cap on rows in the per-module caller / callee tables.
const TOP_N: usize = 10;
/// Hard cap on rendered architecture-diagram nodes; the workspace has ~12
/// crates, so this never truncates here but guards huge external repos.
const ARCH_MAX_NODES: usize = 24;
/// Hard cap on rendered module-neighbourhood nodes (centre + top-N callers /
/// callees). `TOP_N` bounds each side, so this only guards pathological fan-in.
const NEIGHBOURHOOD_MAX_NODES: usize = 24;

/// Render a deterministic Markdown summary for one module as system-only
/// insight: its crate-aware role, a sidecar dependency-neighbourhood SVG
/// reference, scope-filtered named inbound/outbound coupling, cycle
/// participation, dead code, and a churn × complexity risk line.
///
/// Non-source neighbours (fixtures / tests / vendored) are dropped from the
/// coupling tables via [`DocScope`]; the graph itself is never filtered (D3).
/// The risk line consumes the git-history `churn` vector; empty history
/// degrades it to an explicit "history unavailable" line (D6). The
/// neighbourhood SVG is referenced by relative path — the read-only tool emits
/// no IO, the tier-06 CLI writes the sidecar file (D4).
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
    churn: &[FileChurn],
    scope: &DocScope,
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

    // Defining file of the module (its members share one file in production);
    // basis for the crate/layer role, the SVG sidecar name, and the risk line.
    let file_path = module
        .members
        .iter()
        .map(|s| table.path(*s))
        .find(|p| !p.is_empty())
        .unwrap_or("");

    // Inbound (callers) / outbound (callees) symbol histograms from the edge
    // walk, then doc-layer source scoping over each neighbour's defining path.
    let (callers, targets) = neighbour_histograms(graph, module, &member_ix);
    let scoped_callers = scope_filter(&callers, &table, scope);
    let scoped_targets = scope_filter(&targets, &table, scope);

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

    h2(&mut md, "Role");
    let _ = writeln!(
        md,
        "{}",
        docgen_insights::module_role(&module.name, file_path, metrics)
    );
    md.push('\n');
    let _ = writeln!(
        md,
        "Members: {} · cohesion {cohesion:.2} · abstractness {:.2}.",
        module.members.len(),
        metrics.abstractness
    );
    md.push('\n');

    h2(&mut md, "Neighbourhood");
    let _ = writeln!(md, "![neighbourhood]({})", svg_ref(file_path, &module.name));
    md.push('\n');

    h2(&mut md, "Coupling");
    let _ = writeln!(
        md,
        "Afferent (Ca): {} · efferent (Ce): {} · instability (I): {:.2}.",
        metrics.afferent, metrics.efferent, metrics.instability
    );
    md.push('\n');
    push_symbol_table(&mut md, &table, "Caller", &top(&scoped_callers));
    push_symbol_table(&mut md, &table, "Callee", &top(&scoped_targets));

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

    h2(&mut md, "Dead code");
    if dead_members.is_empty() {
        md.push_str("No dead symbols detected.\n\n");
    } else {
        for id in &dead_members {
            let _ = writeln!(
                md,
                "- `{}` ({}) — no callers, no exports",
                table.name(*id),
                table.kind(*id)
            );
        }
        md.push('\n');
    }

    h2(&mut md, "Risk");
    let _ = writeln!(
        md,
        "{}",
        docgen_insights::risk_line(file_path, churn, &table)
    );
    md.push('\n');
    Ok(md)
}

/// Build the inbound-caller / outbound-callee symbol histograms for `module`
/// from the graph edge walk, counting only edges that cross the module
/// boundary (`member_ix`) [src: tier-04 step 3].
fn neighbour_histograms(
    graph: &GraphIndex,
    module: &ModuleSpec,
    member_ix: &BTreeSet<NodeIndex>,
) -> (BTreeMap<SymbolId, u32>, BTreeMap<SymbolId, u32>) {
    let mut callers: BTreeMap<SymbolId, u32> = BTreeMap::new();
    let mut targets: BTreeMap<SymbolId, u32> = BTreeMap::new();
    for s in &module.members {
        let Some(&ix) = graph.index.get(s) else {
            continue;
        };
        for er in graph.graph.edges_directed(ix, Incoming) {
            if !member_ix.contains(&er.source()) {
                *callers.entry(graph.graph[er.source()]).or_default() += 1;
            }
        }
        for er in graph.graph.edges_directed(ix, Outgoing) {
            if !member_ix.contains(&er.target()) {
                *targets.entry(graph.graph[er.target()]).or_default() += 1;
            }
        }
    }
    (callers, targets)
}

/// Render the module's dependency neighbourhood to a deterministic sidecar SVG
/// (tier-04 step 3): the module as a centre node, its top-N boundary-crossing
/// callers above and callees below, drawn by the tier-02 [`render_svg`]
/// emitter. Pure and IO-free — the CLI owns the file write.
///
/// Neighbour nodes are path-scope-filtered through [`DocScope`] exactly like
/// the [`for_module`] coupling tables (resolved via the snapshot's
/// [`SymbolTable`]), so a fixture/test neighbour the table omits never appears
/// in the diagram either (D3). Nodes are labelled by [`SymbolId`] (`#<id>`) —
/// the emitter draws identity, not names. An out-of-scope module (`scope`
/// rejects its name) renders an empty diagram.
///
/// # Errors
/// Propagates [`GraphError::Storage`] when the snapshot scan fails.
pub fn module_svg(
    graph: &GraphIndex,
    snap: &dyn ReadSnapshot,
    module: &ModuleSpec,
    scope: &DocScope,
) -> Result<String, GraphError> {
    let opts = DiagramOpts {
        max_nodes: NEIGHBOURHOOD_MAX_NODES,
    };
    if !scope.include(&module.name) {
        return Ok(render_svg(&[], &[], &opts));
    }
    let table = SymbolTable::from_snapshot(snap)?;
    let member_ix: BTreeSet<NodeIndex> = module
        .members
        .iter()
        .filter_map(|s| graph.index.get(s).copied())
        .collect();
    let (callers, targets) = neighbour_histograms(graph, module, &member_ix);
    let scoped_callers = scope_filter(&callers, &table, scope);
    let scoped_targets = scope_filter(&targets, &table, scope);

    let centre = module.name.clone();
    let mut node_ids: BTreeSet<String> = BTreeSet::new();
    node_ids.insert(centre.clone());
    let mut edge_set: BTreeSet<(String, String)> = BTreeSet::new();
    for (id, _) in top(&scoped_callers) {
        let node = sym_node(id);
        node_ids.insert(node.clone());
        edge_set.insert((node, centre.clone()));
    }
    for (id, _) in top(&scoped_targets) {
        let node = sym_node(id);
        node_ids.insert(node.clone());
        edge_set.insert((centre.clone(), node));
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
    Ok(render_svg(&nodes, &edges, &opts))
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
    md.push_str(&docgen_insights::architecture_section(&scoped));

    h2(&mut md, "Boundary violations");
    md.push_str(&docgen_insights::boundary_violations(graph, &table, scope));

    h2(&mut md, "Cycle clusters");
    md.push_str(&docgen_insights::cycle_clusters(graph, &table, scope));

    h2(&mut md, "Risk hot-spots");
    md.push_str(&docgen_insights::risk_hotspots(&table, churn, scope));

    h2(&mut md, "Refactor & change-coupling");
    md.push_str(&docgen_insights::change_coupling(
        graph, snap, &scoped, churn, co_change, &table, scope,
    )?);

    Ok(md)
}

/// Render the crate-level architecture DAG to a deterministic sidecar SVG
/// (D2/D4). Scoped file-modules aggregate into crate nodes via [`crate_of`];
/// inter-crate symbol edges become diagram edges, deduped and capped, then
/// drawn by the tier-02 [`render_svg`] emitter. Modules outside `crates/`
/// (e.g. `tools/`) are not crates, so they draw no node — keeping the diagram
/// consistent with the synopsis count and the Architecture table (audit I2).
/// Pure and IO-free — the CLI owns the file write.
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
        .filter_map(|m| crate_of(&m.name))
        .map(ToOwned::to_owned)
        .collect();
    let mut edge_set: BTreeSet<(String, String)> = BTreeSet::new();
    for er in graph.graph.edge_references() {
        if let (Some(&si), Some(&di)) = (member_of.get(&er.source()), member_of.get(&er.target())) {
            let (Some(src), Some(dst)) = (crate_of(&scoped[si].name), crate_of(&scoped[di].name))
            else {
                continue;
            };
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

/// Drop `symbol → count` entries whose defining path is out of `scope`
/// (fixtures / tests / vendored), keeping the coupling tables source-only (D3).
fn scope_filter(
    map: &BTreeMap<SymbolId, u32>,
    table: &SymbolTable,
    scope: &DocScope,
) -> BTreeMap<SymbolId, u32> {
    map.iter()
        .filter(|(id, _)| scope.include(table.path(**id)))
        .map(|(k, c)| (*k, *c))
        .collect()
}

/// Deterministic relative file name for a module's neighbourhood sidecar SVG,
/// slugged from its defining-file path (or module name when the path is
/// unknown): every non-alphanumeric byte becomes `-`, suffixed with `.svg`.
fn svg_ref(file_path: &str, name: &str) -> String {
    let basis = if file_path.is_empty() {
        name
    } else {
        file_path
    };
    let mut slug = String::with_capacity(basis.len() + 4);
    for c in basis.chars() {
        if c.is_ascii_alphanumeric() {
            slug.push(c.to_ascii_lowercase());
        } else {
            slug.push('-');
        }
    }
    slug.push_str(".svg");
    slug
}

/// Diagram node id/label for a neighbour symbol. The emitter is snapshot-free,
/// so the identity is the numeric [`SymbolId`] prefixed with `#`.
fn sym_node(id: SymbolId) -> String {
    format!("#{}", id.get())
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
