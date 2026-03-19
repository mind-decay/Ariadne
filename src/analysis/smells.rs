use std::collections::BTreeMap;

use crate::algo;
use crate::analysis::metrics::ClusterMetrics;
use crate::model::*;

/// Detect all architectural smells in the graph.
pub fn detect_smells(
    graph: &ProjectGraph,
    stats: &StatsOutput,
    clusters: &ClusterMap,
    metrics: &BTreeMap<ClusterId, ClusterMetrics>,
) -> Vec<ArchSmell> {
    let mut smells = Vec::new();
    detect_god_files(graph, stats, &mut smells);
    detect_circular_dependencies(stats, &mut smells);
    detect_layer_violations(graph, &mut smells);
    detect_hub_and_spoke(graph, clusters, &mut smells);
    detect_unstable_foundations(metrics, &mut smells);
    detect_dead_clusters(graph, clusters, &mut smells);
    detect_shotgun_surgery(graph, &mut smells);
    // Sort by (smell_type debug, files) for determinism
    smells.sort_by(|a, b| {
        format!("{:?}", a.smell_type)
            .cmp(&format!("{:?}", b.smell_type))
            .then_with(|| a.files.cmp(&b.files))
    });
    smells
}

/// God File: centrality > 0.8 AND out-degree > 20 AND lines > 500
fn detect_god_files(graph: &ProjectGraph, stats: &StatsOutput, smells: &mut Vec<ArchSmell>) {
    for (path, node) in &graph.nodes {
        let centrality = stats
            .centrality
            .get(path.as_str())
            .copied()
            .unwrap_or(0.0);

        if centrality <= 0.8 {
            continue;
        }

        let out_degree = graph
            .edges
            .iter()
            .filter(|e| e.from == *path && e.edge_type.is_architectural())
            .count();

        if out_degree <= 20 || node.lines <= 500 {
            continue;
        }

        smells.push(ArchSmell {
            smell_type: SmellType::GodFile,
            files: vec![path.clone()],
            severity: SmellSeverity::High,
            explanation: format!(
                "File has centrality {:.4}, {} outgoing deps, {} lines",
                centrality, out_degree, node.lines
            ),
            metrics: SmellMetrics {
                primary_value: centrality,
                threshold: 0.8,
            },
        });
    }
}

/// Circular Dependency: SCC size > 1
fn detect_circular_dependencies(stats: &StatsOutput, smells: &mut Vec<ArchSmell>) {
    for scc in &stats.sccs {
        if scc.len() <= 1 {
            continue;
        }
        let mut files: Vec<CanonicalPath> = scc.iter().map(|s| CanonicalPath::new(s)).collect();
        files.sort();
        smells.push(ArchSmell {
            smell_type: SmellType::CircularDependency,
            files,
            severity: SmellSeverity::High,
            explanation: format!("Circular dependency among {} files", scc.len()),
            metrics: SmellMetrics {
                primary_value: scc.len() as f64,
                threshold: 1.0,
            },
        });
    }
}

/// Layer Violation: edge from lower arch_depth to higher
fn detect_layer_violations(graph: &ProjectGraph, smells: &mut Vec<ArchSmell>) {
    for edge in &graph.edges {
        if !edge.edge_type.is_architectural() {
            continue;
        }
        let from_depth = graph.nodes.get(&edge.from).map(|n| n.arch_depth);
        let to_depth = graph.nodes.get(&edge.to).map(|n| n.arch_depth);

        if let (Some(fd), Some(td)) = (from_depth, to_depth) {
            if fd < td {
                let depth_diff = td - fd;
                let mut files = vec![edge.from.clone(), edge.to.clone()];
                files.sort();
                smells.push(ArchSmell {
                    smell_type: SmellType::LayerViolation,
                    files,
                    severity: SmellSeverity::Medium,
                    explanation: format!(
                        "Dependency from depth {} to depth {} (upward)",
                        fd, td
                    ),
                    metrics: SmellMetrics {
                        primary_value: depth_diff as f64,
                        threshold: 0.0,
                    },
                });
            }
        }
    }
}

/// Hub-and-Spoke: one file has >50% of cluster's external edges
fn detect_hub_and_spoke(
    graph: &ProjectGraph,
    clusters: &ClusterMap,
    smells: &mut Vec<ArchSmell>,
) {
    for (cluster_id, cluster) in &clusters.clusters {
        // Need at least 2 files and 2 external edges for hub detection to be meaningful
        if cluster.external_edges < 2 || cluster.files.len() < 2 {
            continue;
        }

        // Count external edges per file in this cluster
        let mut file_external: BTreeMap<&CanonicalPath, u32> = BTreeMap::new();
        let cluster_files: std::collections::BTreeSet<&CanonicalPath> =
            cluster.files.iter().collect();

        for edge in &graph.edges {
            if !edge.edge_type.is_architectural() {
                continue;
            }
            let from_in = cluster_files.contains(&edge.from);
            let to_in = cluster_files.contains(&edge.to);

            if from_in && !to_in {
                *file_external.entry(&edge.from).or_default() += 1;
            }
            if !from_in && to_in {
                *file_external.entry(&edge.to).or_default() += 1;
            }
        }

        let total_external: u32 = file_external.values().sum();
        if total_external == 0 {
            continue;
        }

        for (file, &count) in &file_external {
            let share = count as f64 / total_external as f64;
            if share > 0.5 {
                smells.push(ArchSmell {
                    smell_type: SmellType::HubAndSpoke,
                    files: vec![(*file).clone()],
                    severity: SmellSeverity::Medium,
                    explanation: format!(
                        "File handles {:.0}% of cluster '{}' external edges",
                        share * 100.0,
                        cluster_id.as_str()
                    ),
                    metrics: SmellMetrics {
                        primary_value: share,
                        threshold: 0.5,
                    },
                });
            }
        }
    }
}

/// Unstable Foundation: cluster with I > 0.7 AND Ca > 10
fn detect_unstable_foundations(
    metrics: &BTreeMap<ClusterId, ClusterMetrics>,
    smells: &mut Vec<ArchSmell>,
) {
    for (cluster_id, m) in metrics {
        if m.instability > 0.7 && m.afferent_coupling > 10 {
            smells.push(ArchSmell {
                smell_type: SmellType::UnstableFoundation,
                files: vec![CanonicalPath::new(cluster_id.as_str())],
                severity: SmellSeverity::High,
                explanation: format!(
                    "Cluster '{}' is unstable (I={:.4}) but heavily depended on (Ca={})",
                    cluster_id.as_str(),
                    m.instability,
                    m.afferent_coupling
                ),
                metrics: SmellMetrics {
                    primary_value: m.instability,
                    threshold: 0.7,
                },
            });
        }
    }
}

/// Dead Cluster: 0 incoming external edges AND not a top-level entry point
fn detect_dead_clusters(
    graph: &ProjectGraph,
    clusters: &ClusterMap,
    smells: &mut Vec<ArchSmell>,
) {
    // Find max arch_depth
    let max_depth = graph.nodes.values().map(|n| n.arch_depth).max().unwrap_or(0);

    for (cluster_id, cluster) in &clusters.clusters {
        // Check if any file in this cluster is at max depth (top-level entry)
        let is_top_level = cluster.files.iter().any(|f| {
            graph
                .nodes
                .get(f)
                .map(|n| n.arch_depth == max_depth)
                .unwrap_or(false)
        });

        if is_top_level {
            continue;
        }

        // Count incoming edges from other clusters
        let cluster_files: std::collections::BTreeSet<&CanonicalPath> =
            cluster.files.iter().collect();

        let incoming = graph.edges.iter().any(|e| {
            e.edge_type.is_architectural()
                && cluster_files.contains(&e.to)
                && !cluster_files.contains(&e.from)
        });

        if !incoming {
            smells.push(ArchSmell {
                smell_type: SmellType::DeadCluster,
                files: cluster.files.clone(),
                severity: SmellSeverity::Low,
                explanation: format!(
                    "Cluster '{}' has no incoming edges from other clusters",
                    cluster_id.as_str()
                ),
                metrics: SmellMetrics {
                    primary_value: 0.0,
                    threshold: 0.0,
                },
            });
        }
    }
}

/// Shotgun Surgery: file with blast radius > 30% of project file count
/// Optimization: only check files with out-degree > 10
fn detect_shotgun_surgery(graph: &ProjectGraph, smells: &mut Vec<ArchSmell>) {
    let total_files = graph.nodes.len();
    if total_files == 0 {
        return;
    }

    // Pre-compute out-degrees
    let mut out_degrees: BTreeMap<&CanonicalPath, usize> = BTreeMap::new();
    for edge in &graph.edges {
        if edge.edge_type.is_architectural() {
            *out_degrees.entry(&edge.from).or_default() += 1;
        }
    }

    let threshold = 0.3;

    for (path, &out_deg) in &out_degrees {
        // Optimization: files with few deps can't have 30% blast radius
        if out_deg <= 10 {
            continue;
        }

        let radius = algo::blast_radius::blast_radius(graph, path, None);
        let blast_pct = radius.len() as f64 / total_files as f64;

        if blast_pct > threshold {
            smells.push(ArchSmell {
                smell_type: SmellType::ShotgunSurgery,
                files: vec![(*path).clone()],
                severity: SmellSeverity::High,
                explanation: format!(
                    "Changing this file affects {:.0}% of the project ({} files)",
                    blast_pct * 100.0,
                    radius.len()
                ),
                metrics: SmellMetrics {
                    primary_value: blast_pct,
                    threshold,
                },
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::metrics::compute_martin_metrics;

    fn make_node(
        file_type: FileType,
        cluster: &ClusterId,
        arch_depth: u32,
        lines: u32,
    ) -> Node {
        Node {
            file_type,
            layer: ArchLayer::Unknown,
            arch_depth,
            lines,
            hash: ContentHash::new("0000000000000000".to_string()),
            exports: vec![],
            cluster: cluster.clone(),
        }
    }

    fn make_edge(from: &str, to: &str, edge_type: EdgeType) -> Edge {
        Edge {
            from: CanonicalPath::new(from),
            to: CanonicalPath::new(to),
            edge_type,
            symbols: vec![],
        }
    }

    fn make_stats(
        centrality: Vec<(&str, f64)>,
        sccs: Vec<Vec<&str>>,
    ) -> StatsOutput {
        StatsOutput {
            version: 1,
            centrality: centrality
                .into_iter()
                .map(|(k, v)| (k.to_string(), v))
                .collect(),
            sccs: sccs
                .into_iter()
                .map(|scc| scc.into_iter().map(|s| s.to_string()).collect())
                .collect(),
            layers: BTreeMap::new(),
            summary: StatsSummary {
                max_depth: 3,
                avg_in_degree: 1.0,
                avg_out_degree: 1.0,
                bottleneck_files: vec![],
                orphan_files: vec![],
            },
        }
    }

    fn empty_clusters() -> ClusterMap {
        ClusterMap {
            clusters: BTreeMap::new(),
        }
    }

    fn empty_metrics() -> BTreeMap<ClusterId, ClusterMetrics> {
        BTreeMap::new()
    }

    #[test]
    fn god_file_detected() {
        let c = ClusterId::new("c1");
        let mut nodes = BTreeMap::new();
        let god_path = CanonicalPath::new("src/god.ts");
        nodes.insert(god_path.clone(), make_node(FileType::Source, &c, 1, 600));

        // Add 25 target files
        for i in 0..25 {
            let p = CanonicalPath::new(format!("src/dep{}.ts", i));
            nodes.insert(p, make_node(FileType::Source, &c, 0, 50));
        }

        let edges: Vec<Edge> = (0..25)
            .map(|i| make_edge("src/god.ts", &format!("src/dep{}.ts", i), EdgeType::Imports))
            .collect();

        let graph = ProjectGraph { nodes, edges };
        let stats = make_stats(vec![("src/god.ts", 0.85)], vec![]);

        let smells = detect_smells(&graph, &stats, &empty_clusters(), &empty_metrics());
        assert!(
            smells.iter().any(|s| s.smell_type == SmellType::GodFile),
            "Should detect god file"
        );
    }

    #[test]
    fn god_file_below_threshold_not_detected() {
        let c = ClusterId::new("c1");
        let mut nodes = BTreeMap::new();
        nodes.insert(
            CanonicalPath::new("src/big.ts"),
            make_node(FileType::Source, &c, 1, 600),
        );
        for i in 0..25 {
            let p = CanonicalPath::new(format!("src/dep{}.ts", i));
            nodes.insert(p, make_node(FileType::Source, &c, 0, 50));
        }
        let edges: Vec<Edge> = (0..25)
            .map(|i| make_edge("src/big.ts", &format!("src/dep{}.ts", i), EdgeType::Imports))
            .collect();
        let graph = ProjectGraph { nodes, edges };
        // centrality 0.75 — below 0.8 threshold
        let stats = make_stats(vec![("src/big.ts", 0.75)], vec![]);

        let smells = detect_smells(&graph, &stats, &empty_clusters(), &empty_metrics());
        assert!(
            !smells.iter().any(|s| s.smell_type == SmellType::GodFile),
            "Should not detect god file below threshold"
        );
    }

    #[test]
    fn circular_dependency_from_scc() {
        let c = ClusterId::new("c1");
        let mut nodes = BTreeMap::new();
        for name in &["src/a.ts", "src/b.ts", "src/c.ts"] {
            nodes.insert(
                CanonicalPath::new(*name),
                make_node(FileType::Source, &c, 0, 50),
            );
        }
        let graph = ProjectGraph {
            nodes,
            edges: vec![],
        };
        let stats = make_stats(
            vec![],
            vec![vec!["src/a.ts", "src/b.ts", "src/c.ts"]],
        );

        let smells = detect_smells(&graph, &stats, &empty_clusters(), &empty_metrics());
        let cycle_smells: Vec<_> = smells
            .iter()
            .filter(|s| s.smell_type == SmellType::CircularDependency)
            .collect();
        assert_eq!(cycle_smells.len(), 1);
        assert_eq!(cycle_smells[0].files.len(), 3);
    }

    #[test]
    fn layer_violation() {
        let c = ClusterId::new("c1");
        let mut nodes = BTreeMap::new();
        nodes.insert(
            CanonicalPath::new("src/low.ts"),
            make_node(FileType::Source, &c, 1, 50),
        );
        nodes.insert(
            CanonicalPath::new("src/high.ts"),
            make_node(FileType::Source, &c, 3, 50),
        );
        let graph = ProjectGraph {
            nodes,
            edges: vec![make_edge("src/low.ts", "src/high.ts", EdgeType::Imports)],
        };
        let stats = make_stats(vec![], vec![]);

        let smells = detect_smells(&graph, &stats, &empty_clusters(), &empty_metrics());
        assert!(smells.iter().any(|s| s.smell_type == SmellType::LayerViolation));
    }

    #[test]
    fn hub_and_spoke() {
        let c1 = ClusterId::new("c1");
        let c2 = ClusterId::new("c2");
        let mut nodes = BTreeMap::new();
        // c1 has 3 files: hub, f1, f2
        nodes.insert(CanonicalPath::new("src/hub.ts"), make_node(FileType::Source, &c1, 0, 50));
        nodes.insert(CanonicalPath::new("src/f1.ts"), make_node(FileType::Source, &c1, 0, 50));
        nodes.insert(CanonicalPath::new("src/f2.ts"), make_node(FileType::Source, &c1, 0, 50));
        // c2 has external files
        for i in 0..10 {
            nodes.insert(
                CanonicalPath::new(format!("lib/ext{}.ts", i)),
                make_node(FileType::Source, &c2, 0, 50),
            );
        }

        // hub.ts has 6 external edges, f1 has 1, f2 has 1 → hub = 6/8 = 75%
        let mut edges = Vec::new();
        for i in 0..6 {
            edges.push(make_edge("src/hub.ts", &format!("lib/ext{}.ts", i), EdgeType::Imports));
        }
        edges.push(make_edge("src/f1.ts", "lib/ext7.ts", EdgeType::Imports));
        edges.push(make_edge("src/f2.ts", "lib/ext8.ts", EdgeType::Imports));

        let graph = ProjectGraph { nodes, edges };

        let mut cluster_map = BTreeMap::new();
        cluster_map.insert(
            c1.clone(),
            Cluster {
                files: vec![
                    CanonicalPath::new("src/hub.ts"),
                    CanonicalPath::new("src/f1.ts"),
                    CanonicalPath::new("src/f2.ts"),
                ],
                file_count: 3,
                internal_edges: 0,
                external_edges: 8,
                cohesion: 0.0,
            },
        );
        cluster_map.insert(
            c2.clone(),
            Cluster {
                files: (0..10)
                    .map(|i| CanonicalPath::new(format!("lib/ext{}.ts", i)))
                    .collect(),
                file_count: 10,
                internal_edges: 0,
                external_edges: 8,
                cohesion: 0.0,
            },
        );
        let clusters = ClusterMap {
            clusters: cluster_map,
        };

        let stats = make_stats(vec![], vec![]);
        let smells = detect_smells(&graph, &stats, &clusters, &empty_metrics());
        assert!(
            smells.iter().any(|s| s.smell_type == SmellType::HubAndSpoke),
            "Should detect hub-and-spoke"
        );
    }

    #[test]
    fn unstable_foundation() {
        let c1 = ClusterId::new("foundation");
        let mut metrics = BTreeMap::new();
        use crate::analysis::metrics::MetricZone;
        metrics.insert(
            c1.clone(),
            ClusterMetrics {
                cluster_id: c1.clone(),
                instability: 0.8,
                abstractness: 0.0,
                distance: 0.2,
                zone: MetricZone::OffMainSequence,
                afferent_coupling: 15,
                efferent_coupling: 60,
                abstract_files: 0,
                total_files: 5,
            },
        );

        let graph = ProjectGraph {
            nodes: BTreeMap::new(),
            edges: vec![],
        };
        let stats = make_stats(vec![], vec![]);

        let smells = detect_smells(&graph, &stats, &empty_clusters(), &metrics);
        assert!(smells
            .iter()
            .any(|s| s.smell_type == SmellType::UnstableFoundation));
    }

    #[test]
    fn dead_cluster_detected() {
        let c1 = ClusterId::new("dead");
        let c2 = ClusterId::new("alive");
        let mut nodes = BTreeMap::new();
        // dead cluster at depth 1 (not max depth 3)
        nodes.insert(
            CanonicalPath::new("src/dead.ts"),
            make_node(FileType::Source, &c1, 1, 50),
        );
        // alive cluster at max depth
        nodes.insert(
            CanonicalPath::new("src/alive.ts"),
            make_node(FileType::Source, &c2, 3, 50),
        );

        let graph = ProjectGraph {
            nodes,
            edges: vec![],
        };
        let stats = make_stats(vec![], vec![]);

        let mut cluster_map = BTreeMap::new();
        cluster_map.insert(
            c1.clone(),
            Cluster {
                files: vec![CanonicalPath::new("src/dead.ts")],
                file_count: 1,
                internal_edges: 0,
                external_edges: 0,
                cohesion: 0.0,
            },
        );
        cluster_map.insert(
            c2.clone(),
            Cluster {
                files: vec![CanonicalPath::new("src/alive.ts")],
                file_count: 1,
                internal_edges: 0,
                external_edges: 0,
                cohesion: 0.0,
            },
        );
        let clusters = ClusterMap {
            clusters: cluster_map,
        };

        let smells = detect_smells(&graph, &stats, &clusters, &empty_metrics());
        let dead_smells: Vec<_> = smells
            .iter()
            .filter(|s| s.smell_type == SmellType::DeadCluster)
            .collect();
        // dead cluster should be detected, alive (max depth) should not
        assert_eq!(dead_smells.len(), 1);
        assert!(dead_smells[0]
            .files
            .contains(&CanonicalPath::new("src/dead.ts")));
    }

    #[test]
    fn dead_cluster_top_level_not_detected() {
        let c1 = ClusterId::new("entry");
        let mut nodes = BTreeMap::new();
        // Entry point at max depth with 0 incoming
        nodes.insert(
            CanonicalPath::new("src/main.ts"),
            make_node(FileType::Source, &c1, 3, 50),
        );

        let graph = ProjectGraph {
            nodes,
            edges: vec![],
        };
        let stats = make_stats(vec![], vec![]);

        let mut cluster_map = BTreeMap::new();
        cluster_map.insert(
            c1.clone(),
            Cluster {
                files: vec![CanonicalPath::new("src/main.ts")],
                file_count: 1,
                internal_edges: 0,
                external_edges: 0,
                cohesion: 0.0,
            },
        );
        let clusters = ClusterMap {
            clusters: cluster_map,
        };

        let smells = detect_smells(&graph, &stats, &clusters, &empty_metrics());
        assert!(
            !smells.iter().any(|s| s.smell_type == SmellType::DeadCluster),
            "Top-level entry should not be dead"
        );
    }

    #[test]
    fn shotgun_surgery() {
        // Build a star: many files depend on core.ts
        // blast_radius of core.ts = all dependents (reverse BFS)
        let c = ClusterId::new("c1");
        let mut nodes = BTreeMap::new();
        let total = 35;

        nodes.insert(
            CanonicalPath::new("src/core.ts"),
            make_node(FileType::Source, &c, 0, 50),
        );
        for i in 0..total {
            nodes.insert(
                CanonicalPath::new(format!("src/f{}.ts", i)),
                make_node(FileType::Source, &c, 0, 50),
            );
        }

        let mut edges = Vec::new();
        // All files import core.ts → core has blast radius of all files
        // Also need core to have out-degree > 10 for the optimization filter
        for i in 0..total {
            edges.push(make_edge(
                &format!("src/f{}.ts", i),
                "src/core.ts",
                EdgeType::Imports,
            ));
        }
        // Give core.ts outgoing edges too (>10 for optimization filter)
        for i in 0..12 {
            edges.push(make_edge(
                "src/core.ts",
                &format!("src/f{}.ts", i),
                EdgeType::Imports,
            ));
        }

        let graph = ProjectGraph { nodes, edges };
        let stats = make_stats(vec![], vec![]);

        let smells = detect_smells(&graph, &stats, &empty_clusters(), &empty_metrics());
        assert!(
            smells
                .iter()
                .any(|s| s.smell_type == SmellType::ShotgunSurgery),
            "Should detect shotgun surgery for core.ts"
        );
    }

    #[test]
    fn clean_architecture_no_smells() {
        // Simple well-structured graph: 3 files, clean layering, no cycles
        let c = ClusterId::new("c1");
        let c2 = ClusterId::new("c2");
        let mut nodes = BTreeMap::new();
        nodes.insert(
            CanonicalPath::new("src/a.ts"),
            make_node(FileType::Source, &c, 2, 100),
        );
        nodes.insert(
            CanonicalPath::new("src/b.ts"),
            make_node(FileType::Source, &c, 1, 100),
        );
        nodes.insert(
            CanonicalPath::new("lib/c.ts"),
            make_node(FileType::Source, &c2, 0, 100),
        );

        // Clean dependency: higher depth → lower depth only
        let edges = vec![
            make_edge("src/a.ts", "src/b.ts", EdgeType::Imports),
            make_edge("src/b.ts", "lib/c.ts", EdgeType::Imports),
        ];

        let graph = ProjectGraph { nodes, edges };
        let stats = make_stats(vec![], vec![]);

        let mut cluster_map = BTreeMap::new();
        cluster_map.insert(
            c.clone(),
            Cluster {
                files: vec![
                    CanonicalPath::new("src/a.ts"),
                    CanonicalPath::new("src/b.ts"),
                ],
                file_count: 2,
                internal_edges: 1,
                external_edges: 1,
                cohesion: 0.5,
            },
        );
        cluster_map.insert(
            c2.clone(),
            Cluster {
                files: vec![CanonicalPath::new("lib/c.ts")],
                file_count: 1,
                internal_edges: 0,
                external_edges: 1,
                cohesion: 0.0,
            },
        );
        let clusters = ClusterMap {
            clusters: cluster_map,
        };

        let metrics = compute_martin_metrics(&graph, &clusters);
        let smells = detect_smells(&graph, &stats, &clusters, &metrics);
        assert!(smells.is_empty(), "Clean architecture should have no smells, got: {:?}", smells);
    }
}
