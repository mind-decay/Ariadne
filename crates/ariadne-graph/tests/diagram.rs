//! Tier-02 layered-DAG → SVG emitter: determinism, structure, truncation.
//! Byte assertions are the regression guard for the same-bytes guarantee
//! [src: tier-02 `exit_criteria`; plan.md constraints].

use ariadne_graph::{DiagramEdge, DiagramNode, DiagramOpts, render_svg};

fn node(id: &str) -> DiagramNode {
    DiagramNode {
        id: id.to_string(),
        label: id.to_uppercase(),
    }
}

fn edge(from: &str, to: &str) -> DiagramEdge {
    DiagramEdge {
        from: from.to_string(),
        to: to.to_string(),
    }
}

/// A 4-node DAG (a→b, a→c, b→d, c→d) renders deterministic, well-formed SVG.
#[test]
fn render_svg_is_deterministic_and_well_formed() {
    let nodes = vec![node("a"), node("b"), node("c"), node("d")];
    let edges = vec![
        edge("a", "b"),
        edge("a", "c"),
        edge("b", "d"),
        edge("c", "d"),
    ];
    let opts = DiagramOpts { max_nodes: 40 };

    let first = render_svg(&nodes, &edges, &opts);
    let second = render_svg(&nodes, &edges, &opts);
    assert_eq!(first, second, "same input must yield byte-identical SVG");

    assert!(
        first.starts_with("<svg "),
        "must start with <svg root: {first}"
    );
    assert!(first.contains("viewBox="), "must declare a viewBox");
    assert!(
        first.contains("width=") && first.contains("height="),
        "must declare width/height"
    );
    assert!(
        first.contains("xmlns=\"http://www.w3.org/2000/svg\""),
        "must declare the SVG namespace"
    );
    assert_eq!(first.matches("<rect").count(), 4, "one rect per node");
    assert_eq!(first.matches("<text").count(), 4, "one text per node");
    assert_eq!(
        first.matches("<line").count(),
        4,
        "one edge element per edge"
    );
    assert_eq!(
        first.matches("<marker id=\"arrow\"").count(),
        1,
        "exactly one arrowhead marker"
    );
}

/// Over-cap input is truncated deterministically with a visible dropped count.
#[test]
fn over_cap_input_truncates_with_visible_dropped_count() {
    // 60-node fan: hub `a000` points at 59 leaves; hub has the highest degree.
    let mut nodes = vec![node("a000")];
    let mut edges = Vec::new();
    for i in 0..59 {
        let leaf = format!("n{i:03}");
        nodes.push(node(&leaf));
        edges.push(edge("a000", &leaf));
    }
    assert_eq!(nodes.len(), 60);

    let opts = DiagramOpts { max_nodes: 40 };
    let svg = render_svg(&nodes, &edges, &opts);

    assert!(
        svg.contains("dropped 20 nodes"),
        "must annotate the dropped count, not cap silently: {svg}"
    );
    assert!(
        svg.matches("<rect").count() <= 40,
        "must respect the node cap"
    );
    assert_eq!(
        svg,
        render_svg(&nodes, &edges, &opts),
        "truncation must be deterministic"
    );
}
