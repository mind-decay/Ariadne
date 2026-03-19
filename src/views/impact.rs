use std::collections::BTreeMap;
use std::fmt::Write;

use crate::model::{CanonicalPath, ProjectGraph, SubgraphResult};

/// Generate L2 blast radius report as markdown.
pub fn generate_blast_radius_view(
    file: &str,
    blast_result: &BTreeMap<CanonicalPath, u32>,
    graph: &ProjectGraph,
) -> String {
    let mut out = String::new();
    writeln!(out, "# Blast Radius: `{}`", file).unwrap();
    writeln!(out).unwrap();
    writeln!(out, "**Affected files:** {}", blast_result.len()).unwrap();
    writeln!(out).unwrap();

    // Group by distance
    let mut by_distance: BTreeMap<u32, Vec<&CanonicalPath>> = BTreeMap::new();
    for (path, &dist) in blast_result {
        by_distance.entry(dist).or_default().push(path);
    }

    for (distance, files) in &by_distance {
        if *distance == 0 {
            writeln!(out, "## Source (distance 0)").unwrap();
        } else {
            writeln!(out, "## Distance {}", distance).unwrap();
        }
        writeln!(out).unwrap();
        for f in files {
            let node_info = graph
                .nodes
                .get(*f)
                .map(|n| format!(" ({}, {})", n.file_type.as_str(), n.layer.as_str()))
                .unwrap_or_default();
            writeln!(out, "- `{}`{}", f.as_str(), node_info).unwrap();
        }
        writeln!(out).unwrap();
    }

    out
}

/// Generate L2 subgraph view as markdown.
pub fn generate_subgraph_view(
    subgraph: &SubgraphResult,
) -> String {
    let mut out = String::new();
    let centers: Vec<&str> = subgraph.center_files.iter().map(|p| p.as_str()).collect();
    writeln!(
        out,
        "# Subgraph: {}",
        centers.join(", ")
    )
    .unwrap();
    writeln!(out).unwrap();
    writeln!(
        out,
        "**Depth:** {} | **Nodes:** {} | **Edges:** {}",
        subgraph.depth,
        subgraph.nodes.len(),
        subgraph.edges.len()
    )
    .unwrap();
    writeln!(out).unwrap();

    // Node table
    writeln!(out, "## Files").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "| File | Type | Layer | Cluster |").unwrap();
    writeln!(out, "|------|------|------:|---------|").unwrap();
    for (path, node) in &subgraph.nodes {
        writeln!(
            out,
            "| `{}` | {} | {} | {} |",
            path.as_str(),
            node.file_type.as_str(),
            node.arch_depth,
            node.cluster.as_str()
        )
        .unwrap();
    }
    writeln!(out).unwrap();

    // Edges
    if !subgraph.edges.is_empty() {
        writeln!(out, "## Edges").unwrap();
        writeln!(out).unwrap();
        for edge in &subgraph.edges {
            writeln!(
                out,
                "- `{}` → `{}` ({})",
                edge.from.as_str(),
                edge.to.as_str(),
                edge.edge_type.as_str()
            )
            .unwrap();
        }
        writeln!(out).unwrap();
    }

    out
}
