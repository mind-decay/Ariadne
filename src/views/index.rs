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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::*;

    fn make_empty_graph() -> (ProjectGraph, ClusterMap, StatsOutput) {
        let graph = ProjectGraph {
            nodes: BTreeMap::new(),
            edges: vec![],
        };
        let clusters = ClusterMap {
            clusters: BTreeMap::new(),
        };
        let stats = StatsOutput {
            version: 1,
            centrality: BTreeMap::new(),
            sccs: vec![],
            layers: BTreeMap::new(),
            summary: StatsSummary {
                max_depth: 0,
                avg_in_degree: 0.0,
                avg_out_degree: 0.0,
                bottleneck_files: vec![],
                orphan_files: vec![],
            },
        };
        (graph, clusters, stats)
    }

    fn make_small_graph() -> (ProjectGraph, ClusterMap, StatsOutput) {
        let mut nodes = BTreeMap::new();
        nodes.insert(
            CanonicalPath::new("src/a.ts"),
            Node {
                file_type: FileType::Source,
                layer: ArchLayer::Service,
                arch_depth: 1,
                lines: 100,
                hash: ContentHash::new("aaa".to_string()),
                exports: vec![Symbol::new("foo")],
                cluster: ClusterId::new("src"),
            },
        );
        nodes.insert(
            CanonicalPath::new("src/b.ts"),
            Node {
                file_type: FileType::Source,
                layer: ArchLayer::Util,
                arch_depth: 0,
                lines: 50,
                hash: ContentHash::new("bbb".to_string()),
                exports: vec![],
                cluster: ClusterId::new("src"),
            },
        );
        nodes.insert(
            CanonicalPath::new("lib/c.ts"),
            Node {
                file_type: FileType::Source,
                layer: ArchLayer::Data,
                arch_depth: 0,
                lines: 30,
                hash: ContentHash::new("ccc".to_string()),
                exports: vec![],
                cluster: ClusterId::new("lib"),
            },
        );
        let edges = vec![Edge {
            from: CanonicalPath::new("src/a.ts"),
            to: CanonicalPath::new("src/b.ts"),
            edge_type: EdgeType::Imports,
            symbols: vec![],
        }];
        let graph = ProjectGraph { nodes, edges };

        let mut cluster_map = BTreeMap::new();
        cluster_map.insert(
            ClusterId::new("src"),
            Cluster {
                files: vec![
                    CanonicalPath::new("src/a.ts"),
                    CanonicalPath::new("src/b.ts"),
                ],
                file_count: 2,
                internal_edges: 1,
                external_edges: 0,
                cohesion: 1.0,
            },
        );
        cluster_map.insert(
            ClusterId::new("lib"),
            Cluster {
                files: vec![CanonicalPath::new("lib/c.ts")],
                file_count: 1,
                internal_edges: 0,
                external_edges: 0,
                cohesion: 0.0,
            },
        );
        let clusters = ClusterMap {
            clusters: cluster_map,
        };

        let mut centrality = BTreeMap::new();
        centrality.insert("src/a.ts".to_string(), 0.8);
        centrality.insert("src/b.ts".to_string(), 0.2);
        let stats = StatsOutput {
            version: 1,
            centrality,
            sccs: vec![],
            layers: BTreeMap::new(),
            summary: StatsSummary {
                max_depth: 1,
                avg_in_degree: 0.33,
                avg_out_degree: 0.33,
                bottleneck_files: vec!["src/a.ts".to_string()],
                orphan_files: vec!["lib/c.ts".to_string()],
            },
        };
        (graph, clusters, stats)
    }

    #[test]
    fn empty_graph_generates_valid_markdown() {
        let (graph, clusters, stats) = make_empty_graph();
        let md = generate_index(&graph, &clusters, &stats);
        assert!(md.contains("# Project Index"));
        assert!(md.contains("**Files:** 0"));
        assert!(md.contains("**Edges:** 0"));
        assert!(md.contains("**Clusters:** 0"));
    }

    #[test]
    fn small_graph_shows_clusters() {
        let (graph, clusters, stats) = make_small_graph();
        let md = generate_index(&graph, &clusters, &stats);
        assert!(md.contains("**Files:** 3"));
        assert!(md.contains("**Clusters:** 2"));
        assert!(md.contains("| lib |"));
        assert!(md.contains("| src |"));
    }

    #[test]
    fn small_graph_shows_critical_files() {
        let (graph, clusters, stats) = make_small_graph();
        let md = generate_index(&graph, &clusters, &stats);
        assert!(md.contains("## Critical Files"));
        assert!(md.contains("src/a.ts"));
    }

    #[test]
    fn small_graph_shows_orphan_files() {
        let (graph, clusters, stats) = make_small_graph();
        let md = generate_index(&graph, &clusters, &stats);
        assert!(md.contains("## Orphan Files"));
        assert!(md.contains("lib/c.ts"));
    }

    #[test]
    fn special_chars_in_paths() {
        let mut nodes = BTreeMap::new();
        nodes.insert(
            CanonicalPath::new("src/file with spaces.ts"),
            Node {
                file_type: FileType::Source,
                layer: ArchLayer::Unknown,
                arch_depth: 0,
                lines: 10,
                hash: ContentHash::new("xxx".to_string()),
                exports: vec![],
                cluster: ClusterId::new("src"),
            },
        );
        let graph = ProjectGraph {
            nodes,
            edges: vec![],
        };
        let clusters = ClusterMap {
            clusters: BTreeMap::new(),
        };
        let stats = StatsOutput {
            version: 1,
            centrality: BTreeMap::new(),
            sccs: vec![],
            layers: BTreeMap::new(),
            summary: StatsSummary {
                max_depth: 0,
                avg_in_degree: 0.0,
                avg_out_degree: 0.0,
                bottleneck_files: vec![],
                orphan_files: vec![],
            },
        };
        let md = generate_index(&graph, &clusters, &stats);
        assert!(md.contains("**Files:** 1"));
    }
}
