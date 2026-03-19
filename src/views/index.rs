use std::collections::BTreeMap;
use std::fmt::Write;

use crate::model::{ClusterMap, ProjectGraph, StatsOutput};

/// Generate L0 index.md content.
pub fn generate_index(graph: &ProjectGraph, clusters: &ClusterMap, stats: &StatsOutput) -> String {
    let mut out = String::new();
    writeln!(out, "# Project Index").unwrap();
    writeln!(out).unwrap();

    // Build reverse adjacency for dependent counts
    let mut dependents_count: BTreeMap<&str, u32> = BTreeMap::new();
    for edge in &graph.edges {
        if edge.edge_type.is_architectural() {
            *dependents_count.entry(edge.to.as_str()).or_default() += 1;
        }
    }

    // Architecture summary
    writeln!(out, "## Architecture Summary").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "- **Files:** {}", graph.nodes.len()).unwrap();
    writeln!(out, "- **Edges:** {}", graph.edges.len()).unwrap();
    writeln!(out, "- **Clusters:** {}", clusters.clusters.len()).unwrap();
    writeln!(out, "- **Max depth:** {}", stats.summary.max_depth).unwrap();
    writeln!(
        out,
        "- **Avg in-degree:** {:.4}",
        stats.summary.avg_in_degree
    )
    .unwrap();
    writeln!(
        out,
        "- **Avg out-degree:** {:.4}",
        stats.summary.avg_out_degree
    )
    .unwrap();
    writeln!(out).unwrap();

    // Cluster table
    writeln!(out, "## Clusters").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "| Cluster | Files | Key File | Cohesion |").unwrap();
    writeln!(out, "|---------|------:|----------|--------:|").unwrap();

    for (cluster_id, cluster) in &clusters.clusters {
        // Key file: highest centrality in cluster
        let key_file = cluster
            .files
            .iter()
            .filter_map(|f| stats.centrality.get(f.as_str()).map(|&c| (f.as_str(), c)))
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(f, _)| f)
            .unwrap_or("-");

        writeln!(
            out,
            "| {} | {} | `{}` | {:.4} |",
            cluster_id, cluster.file_count, key_file, cluster.cohesion
        )
        .unwrap();
    }
    writeln!(out).unwrap();

    // Critical files (BC > 0.7)
    if !stats.summary.bottleneck_files.is_empty() {
        writeln!(out, "## Critical Files").unwrap();
        writeln!(out).unwrap();
        writeln!(out, "| File | Centrality | Dependents |").unwrap();
        writeln!(out, "|------|----------:|----------:|").unwrap();
        for file in &stats.summary.bottleneck_files {
            let bc = stats.centrality.get(file).copied().unwrap_or(0.0);
            let deps = dependents_count.get(file.as_str()).copied().unwrap_or(0);
            writeln!(out, "| `{}` | {:.4} | {} |", file, bc, deps).unwrap();
        }
        writeln!(out).unwrap();
    }

    // Circular dependencies
    if !stats.sccs.is_empty() {
        writeln!(out, "## Circular Dependencies").unwrap();
        writeln!(out).unwrap();
        for (i, scc) in stats.sccs.iter().enumerate() {
            writeln!(out, "{}. {} files: {}", i + 1, scc.len(), scc.join(" → ")).unwrap();
        }
        writeln!(out).unwrap();
    }

    // Orphan files
    if !stats.summary.orphan_files.is_empty() {
        writeln!(out, "## Orphan Files").unwrap();
        writeln!(out).unwrap();
        for file in &stats.summary.orphan_files {
            writeln!(out, "- `{}`", file).unwrap();
        }
        writeln!(out).unwrap();
    }

    out
}
