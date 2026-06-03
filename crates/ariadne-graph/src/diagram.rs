//! Deterministic layered-DAG → SVG emitter (tier-02). Pure and IO-free:
//! the same `(nodes, edges, opts)` renders byte-identical SVG. Cycles are
//! collapsed with petgraph `condensation`, layers assigned by longest-path
//! rank over the resulting DAG (standard Sugiyama steps), nodes positioned
//! on a fixed integer grid, and the diagram serialised via `std::fmt::Write`
//! [src: tier-02; <https://en.wikipedia.org/wiki/Layered_graph_drawing>;
//! <https://developer.mozilla.org/en-US/docs/Web/SVG/Element>].
//!
//! `writeln!` into a `String` cannot fail, so each macro `Result` is
//! intentionally discarded with `let _ = …` (mirrors `docgen`).

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use petgraph::Direction::Outgoing;
use petgraph::algo::{condensation, toposort};
use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::visit::EdgeRef;

/// A node in the input diagram graph.
#[derive(Debug, Clone)]
pub struct DiagramNode {
    /// Stable identity; referenced by [`DiagramEdge`] and the deterministic
    /// tie-break for node ordering.
    pub id: String,
    /// Human-readable box label (XML-escaped on emit).
    pub label: String,
}

/// A directed edge between two [`DiagramNode`] ids.
#[derive(Debug, Clone)]
pub struct DiagramEdge {
    /// Source node id.
    pub from: String,
    /// Target node id.
    pub to: String,
}

/// Rendering options for [`render_svg`].
#[derive(Debug, Clone)]
pub struct DiagramOpts {
    /// Hard cap on rendered nodes. Over-cap input is truncated to the
    /// highest-degree `max_nodes` with a visible dropped-count annotation.
    pub max_nodes: usize,
}

// Layout constants — an integer grid, so coordinates never need float
// formatting (a non-determinism source) [src: plan.md risks/tier-02].
const MARGIN: usize = 24;
const COL_W: usize = 168;
const ROW_H: usize = 96;
const BOX_W: usize = 140;
const BOX_H: usize = 44;
const ANN_H: usize = 40;

/// Render a small directed graph to a deterministic SVG string.
///
/// The output is byte-identical across calls with the same input: nodes are
/// capped by degree, cycles collapsed, layers assigned by longest-path rank,
/// and every coordinate is an integer emitted in a fixed traversal order.
///
/// # Edge semantics (caller contract)
///
/// Edges among kept nodes are deduped, so the rendered `<line>` count is one
/// per *distinct* `(from, to)` pair and may be below `edges.len()` when the
/// input repeats an edge. A self-loop (`from == to`) counts toward its node's
/// degree for the cap ranking but is never drawn. Callers building crate- or
/// layer-level node sets should pre-aggregate edges (collapse parallels, drop
/// or fold self-edges) if exact `<line>` counts matter [src: tier-02 audit
/// F1/F2; .claude/plans/useful-docgen/audit/tier-02-report.md:66-67].
#[must_use]
pub fn render_svg(nodes: &[DiagramNode], edges: &[DiagramEdge], opts: &DiagramOpts) -> String {
    // 1. Deterministic cap: keep the top `max_nodes` by degree desc, id asc.
    let kept = select_kept(nodes, edges, opts.max_nodes);
    let dropped = nodes.len() - kept.len();

    // Index kept nodes 0..k; `id_to_ix` resolves edge endpoints.
    let id_to_ix: BTreeMap<&str, usize> = kept
        .iter()
        .enumerate()
        .map(|(ix, n)| (n.id.as_str(), ix))
        .collect();

    // Edges among kept nodes only, deduped and ordered. The `BTreeSet`
    // collapses parallel `(from, to)` edges to one `<line>` (audit F1), and the
    // `s != t` guard drops self-loops from rendering (audit F2) — see the
    // `render_svg` caller contract.
    let mut kept_edges: BTreeSet<(usize, usize)> = BTreeSet::new();
    for e in edges {
        if let (Some(&s), Some(&t)) = (id_to_ix.get(e.from.as_str()), id_to_ix.get(e.to.as_str())) {
            if s != t {
                kept_edges.insert((s, t));
            }
        }
    }

    // 2. Layering: condense cycles, toposort, longest-path rank per node.
    let layer_of = layer_assignment(kept.len(), &kept_edges);

    // 3. Positioning: group by layer, sort within a layer by id, assign column.
    let max_layer = layer_of.iter().copied().max().unwrap_or(0);
    let mut by_layer: Vec<Vec<usize>> = vec![Vec::new(); max_layer + 1];
    for (ix, &layer) in layer_of.iter().enumerate() {
        by_layer[layer].push(ix);
    }
    let mut col_of = vec![0usize; kept.len()];
    let mut max_col = 0usize;
    for row in &mut by_layer {
        row.sort_by(|&a, &b| kept[a].id.cmp(&kept[b].id));
        for (col, &ix) in row.iter().enumerate() {
            col_of[ix] = col;
            max_col = max_col.max(col);
        }
    }

    let box_x = |ix: usize| MARGIN + col_of[ix] * COL_W;
    let box_y = |ix: usize| MARGIN + layer_of[ix] * ROW_H;

    let width = MARGIN * 2 + max_col * COL_W + BOX_W;
    let mut height = MARGIN * 2 + max_layer * ROW_H + BOX_H;
    if dropped > 0 {
        height += ANN_H;
    }

    // 4. Emit. Order is fixed (marker, edges, then layer-major nodes) so the
    //    byte stream is stable for a given input.
    let mut svg = String::new();
    let _ = writeln!(
        svg,
        "<svg viewBox=\"0 0 {width} {height}\" width=\"{width}\" height=\"{height}\" \
         xmlns=\"http://www.w3.org/2000/svg\">"
    );
    svg.push_str(
        "<defs><marker id=\"arrow\" markerWidth=\"10\" markerHeight=\"10\" refX=\"9\" refY=\"3\" \
         orient=\"auto\" markerUnits=\"strokeWidth\">\
         <polygon points=\"0,0 9,3 0,6\" fill=\"#555\"/></marker></defs>\n",
    );

    for &(s, t) in &kept_edges {
        let x1 = box_x(s) + BOX_W / 2;
        let y1 = box_y(s) + BOX_H;
        let x2 = box_x(t) + BOX_W / 2;
        let y2 = box_y(t);
        let _ = writeln!(
            svg,
            "<line x1=\"{x1}\" y1=\"{y1}\" x2=\"{x2}\" y2=\"{y2}\" stroke=\"#555\" \
             marker-end=\"url(#arrow)\"/>"
        );
    }

    for row in &by_layer {
        for &ix in row {
            let x = box_x(ix);
            let y = box_y(ix);
            let cx = x + BOX_W / 2;
            let cy = y + BOX_H / 2 + 5;
            let _ = writeln!(
                svg,
                "<rect x=\"{x}\" y=\"{y}\" width=\"{BOX_W}\" height=\"{BOX_H}\" rx=\"6\" \
                 fill=\"#eef\" stroke=\"#557\"/>"
            );
            let _ = writeln!(
                svg,
                "<text x=\"{cx}\" y=\"{cy}\" text-anchor=\"middle\" font-family=\"sans-serif\" \
                 font-size=\"13\">{}</text>",
                xml_escape(&kept[ix].label)
            );
        }
    }

    if dropped > 0 {
        let tx = width / 2;
        let ty = height - MARGIN;
        let _ = writeln!(
            svg,
            "<text x=\"{tx}\" y=\"{ty}\" text-anchor=\"middle\" font-family=\"sans-serif\" \
             font-size=\"13\" fill=\"#900\">dropped {dropped} nodes</text>"
        );
    }

    svg.push_str("</svg>\n");
    svg
}

/// Keep the highest-degree `max_nodes` nodes, ties broken by id ascending.
/// Degree counts edge incidences over nodes present in the input; a self-loop
/// (`from == to`) therefore counts twice for its node, nudging cap priority
/// even though it is never drawn (audit F2).
fn select_kept<'a>(
    nodes: &'a [DiagramNode],
    edges: &[DiagramEdge],
    max_nodes: usize,
) -> Vec<&'a DiagramNode> {
    let present: BTreeSet<&str> = nodes.iter().map(|n| n.id.as_str()).collect();
    let mut degree: BTreeMap<&str, usize> = nodes.iter().map(|n| (n.id.as_str(), 0)).collect();
    for e in edges {
        if present.contains(e.from.as_str()) {
            *degree.get_mut(e.from.as_str()).expect("present") += 1;
        }
        if present.contains(e.to.as_str()) {
            *degree.get_mut(e.to.as_str()).expect("present") += 1;
        }
    }
    let mut ranked: Vec<&DiagramNode> = nodes.iter().collect();
    ranked.sort_by(|a, b| {
        degree[b.id.as_str()]
            .cmp(&degree[a.id.as_str()])
            .then_with(|| a.id.cmp(&b.id))
    });
    ranked.truncate(max_nodes);
    ranked
}

/// Assign a layer (longest-path rank) to each of the `k` kept nodes.
/// Cycles are collapsed via `condensation`, then a single topological pass
/// relaxes layers forward [src: `render_layers` docgen.rs:405-422].
fn layer_assignment(k: usize, kept_edges: &BTreeSet<(usize, usize)>) -> Vec<usize> {
    if k == 0 {
        return Vec::new();
    }
    let mut g: DiGraph<usize, ()> = DiGraph::new();
    let nodes: Vec<NodeIndex> = (0..k).map(|i| g.add_node(i)).collect();
    for &(s, t) in kept_edges {
        g.add_edge(nodes[s], nodes[t], ());
    }
    let condensed = condensation(g, true);
    let order = toposort(&condensed, None).expect("condensation make_acyclic=true is acyclic");

    // Longest-path rank: topo order guarantees predecessors are final first.
    let mut comp_layer: BTreeMap<NodeIndex, usize> = order.iter().map(|&n| (n, 0usize)).collect();
    for &n in &order {
        let cur = comp_layer[&n];
        for er in condensed.edges_directed(n, Outgoing) {
            let slot = comp_layer
                .get_mut(&er.target())
                .expect("toposort covers all nodes");
            *slot = (*slot).max(cur + 1);
        }
    }

    // Map each original node to its component's layer.
    let mut layer = vec![0usize; k];
    for comp in condensed.node_indices() {
        let l = comp_layer[&comp];
        for &orig in &condensed[comp] {
            layer[orig] = l;
        }
    }
    layer
}

/// Escape the five XML predefined entities for safe inclusion in text/markup.
fn xml_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(c),
        }
    }
    out
}
