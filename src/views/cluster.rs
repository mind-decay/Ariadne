use std::collections::BTreeMap;
use std::fmt::Write;

use crate::model::{EdgeType, ProjectGraph};
use crate::serial::StatsOutput;

/// Generate L1 cluster detail view.
pub fn generate_cluster_view(
    cluster_name: &str,
    graph: &ProjectGraph,
    stats: &StatsOutput,
) -> String {
    let mut out = String::new();
    writeln!(out, "# Cluster: {}", cluster_name).unwrap();
    writeln!(out).unwrap();

    // Collect files in this cluster
    let cluster_files: Vec<&str> = graph
        .nodes
        .iter()
        .filter(|(_, node)| node.cluster.as_str() == cluster_name)
        .map(|(path, _)| path.as_str())
        .collect();

    if cluster_files.is_empty() {
        writeln!(out, "*No files in this cluster.*").unwrap();
        return out;
    }

    // Compute in/out degree per file
    let mut in_degree: BTreeMap<&str, u32> = BTreeMap::new();
    let mut out_degree: BTreeMap<&str, u32> = BTreeMap::new();
    for edge in &graph.edges {
        if edge.edge_type.is_architectural() {
            *out_degree.entry(edge.from.as_str()).or_default() += 1;
            *in_degree.entry(edge.to.as_str()).or_default() += 1;
        }
    }

    // File table
    writeln!(out, "## Files").unwrap();
    writeln!(out).unwrap();
    writeln!(
        out,
        "| File | Type | Layer | In | Out | Centrality |"
    )
    .unwrap();
    writeln!(
        out,
        "|------|------|------:|---:|----:|-----------:|"
    )
    .unwrap();

    for &file in &cluster_files {
        if let Some(node) = graph.nodes.get(&crate::model::CanonicalPath::new(file)) {
            let bc = stats.centrality.get(file).copied().unwrap_or(0.0);
            let ind = in_degree.get(file).copied().unwrap_or(0);
            let outd = out_degree.get(file).copied().unwrap_or(0);
            writeln!(
                out,
                "| `{}` | {} | {} | {} | {} | {:.4} |",
                file,
                node.file_type.as_str(),
                node.arch_depth,
                ind,
                outd,
                bc
            )
            .unwrap();
        }
    }
    writeln!(out).unwrap();

    // Internal dependencies
    let internal_edges: Vec<_> = graph
        .edges
        .iter()
        .filter(|e| {
            e.edge_type.is_architectural()
                && cluster_files.contains(&e.from.as_str())
                && cluster_files.contains(&e.to.as_str())
        })
        .collect();

    if !internal_edges.is_empty() {
        writeln!(out, "## Internal Dependencies").unwrap();
        writeln!(out).unwrap();
        for edge in &internal_edges {
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

    // External deps (outgoing from this cluster)
    let external_out: Vec<_> = graph
        .edges
        .iter()
        .filter(|e| {
            e.edge_type.is_architectural()
                && cluster_files.contains(&e.from.as_str())
                && !cluster_files.contains(&e.to.as_str())
        })
        .collect();

    if !external_out.is_empty() {
        writeln!(out, "## External Dependencies").unwrap();
        writeln!(out).unwrap();
        for edge in &external_out {
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

    // External dependents (incoming to this cluster)
    let external_in: Vec<_> = graph
        .edges
        .iter()
        .filter(|e| {
            e.edge_type.is_architectural()
                && !cluster_files.contains(&e.from.as_str())
                && cluster_files.contains(&e.to.as_str())
        })
        .collect();

    if !external_in.is_empty() {
        writeln!(out, "## External Dependents").unwrap();
        writeln!(out).unwrap();
        for edge in &external_in {
            writeln!(
                out,
                "- `{}` ← `{}` ({})",
                edge.to.as_str(),
                edge.from.as_str(),
                edge.edge_type.as_str()
            )
            .unwrap();
        }
        writeln!(out).unwrap();
    }

    // Tests section
    let test_edges: Vec<_> = graph
        .edges
        .iter()
        .filter(|e| {
            e.edge_type == EdgeType::Tests
                && (cluster_files.contains(&e.from.as_str())
                    || cluster_files.contains(&e.to.as_str()))
        })
        .collect();

    if !test_edges.is_empty() {
        writeln!(out, "## Tests").unwrap();
        writeln!(out).unwrap();
        for edge in &test_edges {
            writeln!(
                out,
                "- `{}` tests `{}`",
                edge.from.as_str(),
                edge.to.as_str()
            )
            .unwrap();
        }
        writeln!(out).unwrap();
    }

    out
}
