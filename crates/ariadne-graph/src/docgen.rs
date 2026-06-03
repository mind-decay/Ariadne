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

use ariadne_core::{ReadSnapshot, SymbolId};
use petgraph::Direction::{Incoming, Outgoing};
use petgraph::algo::{condensation, toposort};
use petgraph::graph::DiGraph;
use petgraph::stable_graph::NodeIndex;
use petgraph::visit::{EdgeRef, IntoEdgeReferences};

use crate::build::GraphIndex;
use crate::coupling::{CouplingMetrics, ModuleSpec};
use crate::cycles::{Cycle, CycleReport};
use crate::dead::{DeadCodeConfig, DeadCodeReport};
use crate::doc_model::DocScope;
use crate::errors::GraphError;
use crate::heuristics::{self, SymbolTable};

/// Cap on rows in the per-module caller / callee tables.
const TOP_N: usize = 10;
/// Cap on rows in the project glossary and hot-spot tables.
const LIST_N: usize = 10;

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

/// Render a project-wide Markdown architecture overview: layer diagram
/// (SCC condensation), hot-spots, coupling table, and glossary. An empty
/// project yields a `no modules` placeholder rather than an error.
///
/// # Errors
/// Propagates [`GraphError::Storage`] when the snapshot scan fails.
pub fn for_project(
    graph: &GraphIndex,
    snap: &dyn ReadSnapshot,
    modules: &[ModuleSpec],
    scope: &DocScope,
) -> Result<String, GraphError> {
    let table = SymbolTable::from_snapshot(snap)?;
    let mut md = String::from("# Project Architecture Overview\n\n");
    h2(&mut md, "Overview");
    if modules.is_empty() {
        md.push_str("_No modules indexed._\n");
        return Ok(md);
    }
    // Doc-layer source scoping: the layer diagram and the aggregate
    // Hot-Spots / Coupling tables report only in-scope (default: Source)
    // modules; the graph itself is never filtered [src: plan.md D3].
    let scoped: Vec<ModuleSpec> = modules
        .iter()
        .filter(|m| scope.include(&m.name))
        .cloned()
        .collect();
    let cycles = graph.cycle_report();
    let dead = graph.dead_code(&DeadCodeConfig::default());
    let _ = writeln!(
        md,
        "{} modules · {} symbols · {} edges · {} dependency cycle(s).",
        modules.len(),
        graph.symbol_count(),
        graph.edge_count(),
        cycles.cycles.len()
    );
    md.push('\n');

    h2(&mut md, "Layers");
    md.push_str(&render_layers(graph, &scoped));
    md.push('\n');

    let stats: Vec<ModuleStat> = scoped
        .iter()
        .map(|m| module_stat(graph, m, &cycles, &dead))
        .collect();

    h2(&mut md, "Hot-Spots");
    push_hotspots(&mut md, &stats);
    h2(&mut md, "Coupling");
    push_coupling(&mut md, &stats);
    h2(&mut md, "Glossary");
    push_glossary(&mut md, graph, &table);
    Ok(md)
}

/// Append a level-2 header followed by a blank line.
fn h2(md: &mut String, title: &str) {
    md.push_str("## ");
    md.push_str(title);
    md.push_str("\n\n");
}

/// One-line static purpose inference from the module's coupling shape.
fn purpose(m: &CouplingMetrics) -> &'static str {
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

/// Per-module aggregate row backing the hot-spot and coupling tables.
struct ModuleStat {
    name: String,
    afferent: u32,
    efferent: u32,
    instability: f32,
    abstractness: f32,
    distance: f32,
    cycles: u32,
    dead: u32,
}

/// Collapse one module into a [`ModuleStat`].
fn module_stat(
    graph: &GraphIndex,
    m: &ModuleSpec,
    cycles: &CycleReport,
    dead: &DeadCodeReport,
) -> ModuleStat {
    let metrics = graph
        .coupling_report(std::slice::from_ref(m))
        .rows
        .into_iter()
        .next()
        .expect("coupling_report emits one row per input module");
    let cyc = cycles
        .cycles
        .iter()
        .filter(|c| c.members.iter().any(|s| m.members.contains(s)))
        .count();
    let d = dead
        .symbols
        .iter()
        .filter(|d| m.members.contains(&d.id))
        .count();
    ModuleStat {
        name: metrics.name,
        afferent: metrics.afferent,
        efferent: metrics.efferent,
        instability: metrics.instability,
        abstractness: metrics.abstractness,
        distance: metrics.distance,
        cycles: u32::try_from(cyc).unwrap_or(u32::MAX),
        dead: u32::try_from(d).unwrap_or(u32::MAX),
    }
}

/// Append the hot-spot table: top [`LIST_N`] modules by combined
/// `Ce + cycle membership + dead-code count`.
fn push_hotspots(md: &mut String, stats: &[ModuleStat]) {
    let mut ranked: Vec<(&ModuleStat, u32)> = stats
        .iter()
        .map(|s| {
            (
                s,
                s.efferent.saturating_add(s.cycles).saturating_add(s.dead),
            )
        })
        .collect();
    ranked.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.name.cmp(&b.0.name)));
    ranked.truncate(LIST_N);
    md.push_str("| Module | Ce | Cycles | Dead | Score |\n| --- | --- | --- | --- | --- |\n");
    for (s, score) in ranked {
        let _ = writeln!(
            md,
            "| `{}` | {} | {} | {} | {score} |",
            s.name, s.efferent, s.cycles, s.dead
        );
    }
    md.push('\n');
}

/// Append the per-module Martin coupling table, sorted by module name.
fn push_coupling(md: &mut String, stats: &[ModuleStat]) {
    let mut rows: Vec<&ModuleStat> = stats.iter().collect();
    rows.sort_by(|a, b| a.name.cmp(&b.name));
    md.push_str("| Module | Ca | Ce | I | A | Distance |\n| --- | --- | --- | --- | --- | --- |\n");
    for s in rows {
        let _ = writeln!(
            md,
            "| `{}` | {} | {} | {:.2} | {:.2} | {:.2} |",
            s.name, s.afferent, s.efferent, s.instability, s.abstractness, s.distance
        );
    }
    md.push('\n');
}

/// Append the glossary: top [`LIST_N`] symbols by fan-in.
fn push_glossary(md: &mut String, graph: &GraphIndex, table: &SymbolTable) {
    let mut ranked: Vec<(SymbolId, usize)> = graph
        .index
        .keys()
        .map(|&id| (id, graph.fan_in(id)))
        .collect();
    ranked.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    ranked.truncate(LIST_N);
    if ranked.is_empty() {
        md.push_str("_No symbols indexed._\n");
        return;
    }
    for (id, _) in ranked {
        let _ = writeln!(
            md,
            "- `{}` ({}) — `{}`",
            table.name(id),
            table.kind(id),
            table.path(id)
        );
    }
}

/// Render the module dependency DAG as a Mermaid `flowchart TD` block.
/// SCC condensation collapses mutually-dependent module groups; topo-sort
/// of the condensation fixes a deterministic layer ordering.
fn render_layers(graph: &GraphIndex, modules: &[ModuleSpec]) -> String {
    let member_of = heuristics::member_index(graph, modules);
    let mut mg: DiGraph<usize, ()> = DiGraph::new();
    let nodes: Vec<NodeIndex> = (0..modules.len()).map(|i| mg.add_node(i)).collect();
    let mut edges: BTreeSet<(usize, usize)> = BTreeSet::new();
    for er in graph.graph.edge_references() {
        if let (Some(&s), Some(&d)) = (member_of.get(&er.source()), member_of.get(&er.target())) {
            if s != d {
                edges.insert((s, d));
            }
        }
    }
    for (s, d) in &edges {
        mg.add_edge(nodes[*s], nodes[*d], ());
    }
    let condensed = condensation(mg, true);
    let order = toposort(&condensed, None).expect("condensation make_acyclic=true is acyclic");
    let pos: BTreeMap<NodeIndex, usize> = order.iter().enumerate().map(|(i, n)| (*n, i)).collect();

    let mut out = String::from("```mermaid\nflowchart TD\n");
    for (i, n) in order.iter().enumerate() {
        let mut names: Vec<&str> = condensed[*n]
            .iter()
            .map(|mi| modules[*mi].name.as_str())
            .collect();
        names.sort_unstable();
        let _ = writeln!(out, "    g{i}[\"{}\"]", names.join(" ⇄ "));
    }
    let mut layer_edges: Vec<(usize, usize)> = condensed
        .edge_references()
        .map(|er| (pos[&er.source()], pos[&er.target()]))
        .collect();
    layer_edges.sort_unstable();
    layer_edges.dedup();
    for (a, b) in layer_edges {
        let _ = writeln!(out, "    g{a} --> g{b}");
    }
    out.push_str("```\n");
    out
}
