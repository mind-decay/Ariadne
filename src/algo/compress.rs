use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::model::compress::*;
use crate::model::*;

/// Compress graph to L0 (project level): one node per cluster (D-041).
pub fn compress_l0(
    graph: &ProjectGraph,
    clusters: &ClusterMap,
    stats: &StatsOutput,
) -> CompressedGraph {
    let mut nodes = Vec::new();

    for (cluster_id, cluster) in &clusters.clusters {
        // Find top-3 files by centrality in this cluster
        let mut file_scores: Vec<(&CanonicalPath, f64)> = cluster
            .files
            .iter()
            .map(|f| (f, stats.centrality.get(f.as_str()).copied().unwrap_or(0.0)))
            .collect();
        file_scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        let key_files: Vec<String> = file_scores
            .iter()
            .take(3)
            .map(|(f, _)| f.as_str().to_string())
            .collect();

        nodes.push(CompressedNode {
            name: cluster_id.as_str().to_string(),
            node_type: CompressedNodeType::Cluster,
            file_count: Some(cluster.file_count as u32),
            cohesion: Some(cluster.cohesion),
            key_files,
            file_type: None,
            layer: None,
            centrality: None,
        });
    }

    // Build cluster lookup for each file
    let file_cluster: BTreeMap<&CanonicalPath, &ClusterId> = clusters
        .clusters
        .iter()
        .flat_map(|(id, c)| c.files.iter().map(move |f| (f, id)))
        .collect();

    // Aggregate inter-cluster edges
    let mut edge_weights: BTreeMap<(&str, &str), u32> = BTreeMap::new();
    for edge in &graph.edges {
        let from_cluster = file_cluster.get(&edge.from).map(|c| c.as_str());
        let to_cluster = file_cluster.get(&edge.to).map(|c| c.as_str());
        if let (Some(fc), Some(tc)) = (from_cluster, to_cluster) {
            if fc != tc {
                *edge_weights.entry((fc, tc)).or_default() += 1;
            }
        }
    }

    let edges: Vec<CompressedEdge> = edge_weights
        .into_iter()
        .map(|((from, to), weight)| CompressedEdge {
            from: from.to_string(),
            to: to.to_string(),
            weight,
            edge_type: None,
        })
        .collect();

    let mut cg = CompressedGraph {
        level: CompressionLevel::Project,
        focus: None,
        nodes,
        edges,
        token_estimate: 0,
    };
    cg.token_estimate = estimate_tokens(&cg);
    cg
}

/// Compress graph to L1 (cluster level): all files in one cluster (D-041).
///
/// Nodes are individual files (CompressedNodeType::File).
/// Internal edges have full detail (edge_type set). External edges are aggregated
/// by cluster — their `from`/`to` fields reference cluster names (not file paths),
/// `weight` is the count of inter-cluster edges, and `edge_type` is None.
pub fn compress_l1(
    graph: &ProjectGraph,
    clusters: &ClusterMap,
    stats: &StatsOutput,
    cluster_name: &ClusterId,
) -> Result<CompressedGraph, String> {
    let cluster = clusters
        .clusters
        .get(cluster_name)
        .ok_or_else(|| format!("Cluster '{}' not found", cluster_name.as_str()))?;

    let cluster_files: BTreeSet<&CanonicalPath> = cluster.files.iter().collect();

    // Build cluster lookup for external edge aggregation
    let file_cluster: BTreeMap<&CanonicalPath, &ClusterId> = clusters
        .clusters
        .iter()
        .flat_map(|(id, c)| c.files.iter().map(move |f| (f, id)))
        .collect();

    let mut nodes = Vec::new();
    for file in &cluster.files {
        if let Some(node) = graph.nodes.get(file) {
            nodes.push(CompressedNode {
                name: file.as_str().to_string(),
                node_type: CompressedNodeType::File,
                file_count: None,
                cohesion: None,
                key_files: vec![],
                file_type: Some(node.file_type.as_str().to_string()),
                layer: Some(node.layer.as_str().to_string()),
                centrality: stats.centrality.get(file.as_str()).copied(),
            });
        }
    }

    let mut edges = Vec::new();
    // External edges: aggregated by target cluster
    let mut ext_edge_weights: BTreeMap<(&str, &str), u32> = BTreeMap::new();

    for edge in &graph.edges {
        let from_in = cluster_files.contains(&edge.from);
        let to_in = cluster_files.contains(&edge.to);

        if from_in && to_in {
            // Internal edge: full detail
            edges.push(CompressedEdge {
                from: edge.from.as_str().to_string(),
                to: edge.to.as_str().to_string(),
                weight: 1,
                edge_type: Some(edge.edge_type.as_str().to_string()),
            });
        } else if from_in {
            // Outgoing external: aggregate by target cluster
            if let Some(tc) = file_cluster.get(&edge.to).map(|c| c.as_str()) {
                *ext_edge_weights
                    .entry((cluster_name.as_str(), tc))
                    .or_default() += 1;
            }
        } else if to_in {
            // Incoming external: aggregate by source cluster
            if let Some(fc) = file_cluster.get(&edge.from).map(|c| c.as_str()) {
                *ext_edge_weights
                    .entry((fc, cluster_name.as_str()))
                    .or_default() += 1;
            }
        }
    }

    for ((from, to), weight) in ext_edge_weights {
        edges.push(CompressedEdge {
            from: from.to_string(),
            to: to.to_string(),
            weight,
            edge_type: None,
        });
    }

    let mut cg = CompressedGraph {
        level: CompressionLevel::Cluster,
        focus: Some(cluster_name.as_str().to_string()),
        nodes,
        edges,
        token_estimate: 0,
    };
    cg.token_estimate = estimate_tokens(&cg);
    Ok(cg)
}

/// Compress graph to L2 (file level): file + N-hop neighborhood (D-041).
pub fn compress_l2(
    graph: &ProjectGraph,
    _clusters: &ClusterMap,
    stats: &StatsOutput,
    file_path: &CanonicalPath,
    depth: u32,
) -> Result<CompressedGraph, String> {
    if !graph.nodes.contains_key(file_path) {
        return Err(format!("File '{}' not found in graph", file_path.as_str()));
    }

    // BFS forward and reverse to collect N-hop neighborhood
    let mut visited: BTreeSet<&CanonicalPath> = BTreeSet::new();
    let mut queue: VecDeque<(&CanonicalPath, u32)> = VecDeque::new();

    visited.insert(file_path);
    queue.push_back((file_path, 0));

    // Build adjacency for BFS
    let mut forward: BTreeMap<&CanonicalPath, Vec<&CanonicalPath>> = BTreeMap::new();
    let mut reverse: BTreeMap<&CanonicalPath, Vec<&CanonicalPath>> = BTreeMap::new();
    for edge in &graph.edges {
        forward.entry(&edge.from).or_default().push(&edge.to);
        reverse.entry(&edge.to).or_default().push(&edge.from);
    }

    while let Some((node, dist)) = queue.pop_front() {
        if dist >= depth {
            continue;
        }
        // Forward neighbors
        if let Some(neighbors) = forward.get(node) {
            for n in neighbors {
                if visited.insert(n) {
                    queue.push_back((n, dist + 1));
                }
            }
        }
        // Reverse neighbors
        if let Some(neighbors) = reverse.get(node) {
            for n in neighbors {
                if visited.insert(n) {
                    queue.push_back((n, dist + 1));
                }
            }
        }
    }

    let mut nodes = Vec::new();
    for path in &visited {
        if let Some(node) = graph.nodes.get(*path) {
            nodes.push(CompressedNode {
                name: path.as_str().to_string(),
                node_type: CompressedNodeType::File,
                file_count: None,
                cohesion: None,
                key_files: vec![],
                file_type: Some(node.file_type.as_str().to_string()),
                layer: Some(node.layer.as_str().to_string()),
                centrality: stats.centrality.get(path.as_str()).copied(),
            });
        }
    }

    let edges: Vec<CompressedEdge> = graph
        .edges
        .iter()
        .filter(|e| visited.contains(&e.from) && visited.contains(&e.to))
        .map(|e| CompressedEdge {
            from: e.from.as_str().to_string(),
            to: e.to.as_str().to_string(),
            weight: 1,
            edge_type: Some(e.edge_type.as_str().to_string()),
        })
        .collect();

    let mut cg = CompressedGraph {
        level: CompressionLevel::File,
        focus: Some(file_path.as_str().to_string()),
        nodes,
        edges,
        token_estimate: 0,
    };
    cg.token_estimate = estimate_tokens(&cg);
    Ok(cg)
}

/// Estimate token count from serialized JSON size (D-062).
/// Simple heuristic: 1 token ≈ 4 bytes of JSON.
fn estimate_tokens(graph: &CompressedGraph) -> u32 {
    let json = serde_json::to_string(graph).unwrap_or_default();
    (json.len() / 4).max(1) as u32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ArchLayer, ClusterId, ContentHash, FileType, Node};

    fn make_node(cluster: &str) -> Node {
        Node {
            file_type: FileType::Source,
            layer: ArchLayer::Unknown,
            arch_depth: 0,
            lines: 10,
            hash: ContentHash::new("0000000000000000".to_string()),
            exports: vec![],
            cluster: ClusterId::new(cluster),
        }
    }

    fn make_edge(from: &str, to: &str) -> Edge {
        Edge {
            from: CanonicalPath::new(from),
            to: CanonicalPath::new(to),
            edge_type: EdgeType::Imports,
            symbols: vec![],
        }
    }

    fn make_test_graph() -> (ProjectGraph, ClusterMap, StatsOutput) {
        let mut nodes = BTreeMap::new();
        nodes.insert(CanonicalPath::new("src/auth/login.ts"), make_node("auth"));
        nodes.insert(CanonicalPath::new("src/auth/session.ts"), make_node("auth"));
        nodes.insert(CanonicalPath::new("src/api/routes.ts"), make_node("api"));
        nodes.insert(CanonicalPath::new("src/api/handler.ts"), make_node("api"));
        nodes.insert(CanonicalPath::new("src/util/hash.ts"), make_node("util"));

        let edges = vec![
            make_edge("src/auth/login.ts", "src/auth/session.ts"),
            make_edge("src/auth/login.ts", "src/util/hash.ts"),
            make_edge("src/api/routes.ts", "src/api/handler.ts"),
            make_edge("src/api/handler.ts", "src/auth/session.ts"),
            make_edge("src/api/handler.ts", "src/util/hash.ts"),
        ];

        let graph = ProjectGraph { nodes, edges };

        let mut cluster_map = BTreeMap::new();
        cluster_map.insert(
            ClusterId::new("auth"),
            Cluster {
                files: vec![
                    CanonicalPath::new("src/auth/login.ts"),
                    CanonicalPath::new("src/auth/session.ts"),
                ],
                file_count: 2,
                internal_edges: 1,
                external_edges: 2,
                cohesion: 0.5,
            },
        );
        cluster_map.insert(
            ClusterId::new("api"),
            Cluster {
                files: vec![
                    CanonicalPath::new("src/api/routes.ts"),
                    CanonicalPath::new("src/api/handler.ts"),
                ],
                file_count: 2,
                internal_edges: 1,
                external_edges: 2,
                cohesion: 0.5,
            },
        );
        cluster_map.insert(
            ClusterId::new("util"),
            Cluster {
                files: vec![CanonicalPath::new("src/util/hash.ts")],
                file_count: 1,
                internal_edges: 0,
                external_edges: 2,
                cohesion: 0.0,
            },
        );
        let clusters = ClusterMap {
            clusters: cluster_map,
        };

        let mut centrality = BTreeMap::new();
        centrality.insert("src/util/hash.ts".to_string(), 0.8);
        centrality.insert("src/auth/session.ts".to_string(), 0.6);
        centrality.insert("src/api/handler.ts".to_string(), 0.4);
        centrality.insert("src/auth/login.ts".to_string(), 0.2);
        centrality.insert("src/api/routes.ts".to_string(), 0.1);

        let stats = StatsOutput {
            version: 1,
            centrality,
            sccs: vec![],
            layers: BTreeMap::new(),
            summary: StatsSummary {
                max_depth: 2,
                avg_in_degree: 1.0,
                avg_out_degree: 1.0,
                bottleneck_files: vec![],
                orphan_files: vec![],
            },
        };

        (graph, clusters, stats)
    }

    #[test]
    fn l0_node_count_equals_cluster_count() {
        let (graph, clusters, stats) = make_test_graph();
        let result = compress_l0(&graph, &clusters, &stats);
        assert_eq!(result.nodes.len(), clusters.clusters.len());
        assert_eq!(result.level, CompressionLevel::Project);
    }

    #[test]
    fn l0_edge_weights_are_inter_cluster_counts() {
        let (graph, clusters, stats) = make_test_graph();
        let result = compress_l0(&graph, &clusters, &stats);

        // auth→util: 1 edge (login→hash)
        // api→auth: 1 edge (handler→session)
        // api→util: 1 edge (handler→hash)
        assert!(!result.edges.is_empty());
        for edge in &result.edges {
            assert!(edge.weight >= 1);
            assert_ne!(edge.from, edge.to, "No self-edges in L0");
        }
    }

    #[test]
    fn l0_key_files_top3_by_centrality() {
        let (graph, clusters, stats) = make_test_graph();
        let result = compress_l0(&graph, &clusters, &stats);

        for node in &result.nodes {
            assert!(node.key_files.len() <= 3);
            if node.name == "auth" {
                // session.ts has higher centrality than login.ts
                assert_eq!(node.key_files[0], "src/auth/session.ts");
            }
        }
    }

    #[test]
    fn l0_token_estimate_positive() {
        let (graph, clusters, stats) = make_test_graph();
        let result = compress_l0(&graph, &clusters, &stats);
        assert!(result.token_estimate > 0);
    }

    #[test]
    fn l1_contains_all_cluster_files() {
        let (graph, clusters, stats) = make_test_graph();
        let result = compress_l1(&graph, &clusters, &stats, &ClusterId::new("auth")).unwrap();

        assert_eq!(result.level, CompressionLevel::Cluster);
        assert_eq!(result.focus.as_deref(), Some("auth"));

        let file_names: BTreeSet<&str> = result.nodes.iter().map(|n| n.name.as_str()).collect();
        assert!(file_names.contains("src/auth/login.ts"));
        assert!(file_names.contains("src/auth/session.ts"));
        assert_eq!(result.nodes.len(), 2);
    }

    #[test]
    fn l1_external_edges_aggregated() {
        let (graph, clusters, stats) = make_test_graph();
        let result = compress_l1(&graph, &clusters, &stats, &ClusterId::new("auth")).unwrap();

        // Should have: 1 internal edge (login→session)
        // External edges aggregated by cluster
        let internal: Vec<_> = result
            .edges
            .iter()
            .filter(|e| e.edge_type.is_some())
            .collect();
        let external: Vec<_> = result
            .edges
            .iter()
            .filter(|e| e.edge_type.is_none())
            .collect();

        assert_eq!(internal.len(), 1, "Should have 1 internal edge");
        assert!(!external.is_empty(), "Should have external edges");
    }

    #[test]
    fn l1_unknown_cluster_error() {
        let (graph, clusters, stats) = make_test_graph();
        let result = compress_l1(&graph, &clusters, &stats, &ClusterId::new("nonexistent"));
        assert!(result.is_err());
    }

    #[test]
    fn l2_correct_neighborhood() {
        let (graph, clusters, stats) = make_test_graph();
        let result = compress_l2(
            &graph,
            &clusters,
            &stats,
            &CanonicalPath::new("src/auth/login.ts"),
            2,
        )
        .unwrap();

        assert_eq!(result.level, CompressionLevel::File);
        assert_eq!(result.focus.as_deref(), Some("src/auth/login.ts"));

        let file_names: BTreeSet<&str> = result.nodes.iter().map(|n| n.name.as_str()).collect();
        // login.ts is center
        assert!(file_names.contains("src/auth/login.ts"));
        // Direct neighbors (depth 1): session.ts, hash.ts
        assert!(file_names.contains("src/auth/session.ts"));
        assert!(file_names.contains("src/util/hash.ts"));
    }

    #[test]
    fn l2_depth_1_direct_only() {
        let (graph, clusters, stats) = make_test_graph();
        let result = compress_l2(
            &graph,
            &clusters,
            &stats,
            &CanonicalPath::new("src/api/routes.ts"),
            1,
        )
        .unwrap();

        let file_names: BTreeSet<&str> = result.nodes.iter().map(|n| n.name.as_str()).collect();
        // routes.ts + handler.ts (direct neighbor)
        assert!(file_names.contains("src/api/routes.ts"));
        assert!(file_names.contains("src/api/handler.ts"));
        // session.ts and hash.ts are at depth 2 from routes.ts → should NOT be included
        assert!(
            !file_names.contains("src/auth/session.ts"),
            "depth=1 should not include 2-hop neighbors"
        );
    }

    #[test]
    fn l2_unknown_file_error() {
        let (graph, clusters, stats) = make_test_graph();
        let result = compress_l2(
            &graph,
            &clusters,
            &stats,
            &CanonicalPath::new("nonexistent.ts"),
            2,
        );
        assert!(result.is_err());
    }

    #[test]
    fn all_edges_reference_valid_nodes() {
        let (graph, clusters, stats) = make_test_graph();

        // L0
        let l0 = compress_l0(&graph, &clusters, &stats);
        let l0_names: BTreeSet<&str> = l0.nodes.iter().map(|n| n.name.as_str()).collect();
        for edge in &l0.edges {
            assert!(
                l0_names.contains(edge.from.as_str()),
                "L0 edge.from '{}' not in nodes",
                edge.from
            );
            assert!(
                l0_names.contains(edge.to.as_str()),
                "L0 edge.to '{}' not in nodes",
                edge.to
            );
        }

        // L2
        let l2 = compress_l2(
            &graph,
            &clusters,
            &stats,
            &CanonicalPath::new("src/auth/login.ts"),
            2,
        )
        .unwrap();
        let l2_names: BTreeSet<&str> = l2.nodes.iter().map(|n| n.name.as_str()).collect();
        for edge in &l2.edges {
            assert!(
                l2_names.contains(edge.from.as_str()),
                "L2 edge.from '{}' not in nodes",
                edge.from
            );
            assert!(
                l2_names.contains(edge.to.as_str()),
                "L2 edge.to '{}' not in nodes",
                edge.to
            );
        }
    }
}
