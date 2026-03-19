use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::model::{CanonicalPath, Edge, ProjectGraph, SubgraphResult};

/// Extract a subgraph around the given files within the specified depth.
/// Uses ALL edge types (including tests — per D-034 exception for subgraph).
/// Includes cluster expansion: all files in same cluster as touched files
/// (with 100-file limit per cluster, D-035).
pub fn extract_subgraph(
    graph: &ProjectGraph,
    files: &[CanonicalPath],
    depth: u32,
) -> SubgraphResult {
    // Build forward and reverse adjacency using ALL edge types
    let mut forward: BTreeMap<&CanonicalPath, Vec<&CanonicalPath>> = BTreeMap::new();
    let mut reverse: BTreeMap<&CanonicalPath, Vec<&CanonicalPath>> = BTreeMap::new();
    for edge in &graph.edges {
        forward.entry(&edge.from).or_default().push(&edge.to);
        reverse.entry(&edge.to).or_default().push(&edge.from);
    }

    let mut touched: BTreeSet<CanonicalPath> = BTreeSet::new();

    // BFS from each center file in both directions
    for file in files {
        if !graph.nodes.contains_key(file) {
            continue;
        }
        // Forward BFS (dependencies)
        bfs(file, &forward, depth, &mut touched);
        // Reverse BFS (dependents)
        bfs(file, &reverse, depth, &mut touched);
    }

    // Cluster expansion: for each touched file, include all files in same cluster
    let mut cluster_files: BTreeMap<String, Vec<CanonicalPath>> = BTreeMap::new();
    for (path, node) in &graph.nodes {
        cluster_files
            .entry(node.cluster.as_str().to_string())
            .or_default()
            .push(path.clone());
    }

    let mut expanded = touched.clone();
    let touched_clusters: BTreeSet<String> = touched
        .iter()
        .filter_map(|p| graph.nodes.get(p))
        .map(|n| n.cluster.as_str().to_string())
        .collect();

    for cluster_name in &touched_clusters {
        if let Some(members) = cluster_files.get(cluster_name) {
            if members.len() <= 100 {
                // Include all files in cluster
                for m in members {
                    expanded.insert(m.clone());
                }
            }
            // >100 files: only include BFS-reachable files (already in touched)
        }
    }

    // Collect nodes and edges for the subgraph
    let mut nodes = BTreeMap::new();
    for path in &expanded {
        if let Some(node) = graph.nodes.get(path) {
            nodes.insert(path.clone(), node.clone());
        }
    }

    let edges: Vec<Edge> = graph
        .edges
        .iter()
        .filter(|e| expanded.contains(&e.from) && expanded.contains(&e.to))
        .cloned()
        .collect();

    SubgraphResult {
        nodes,
        edges,
        center_files: files.to_vec(),
        depth,
    }
}

fn bfs(
    start: &CanonicalPath,
    adjacency: &BTreeMap<&CanonicalPath, Vec<&CanonicalPath>>,
    max_depth: u32,
    result: &mut BTreeSet<CanonicalPath>,
) {
    let mut queue = VecDeque::new();
    let mut visited = BTreeSet::new();
    visited.insert(start);
    result.insert(start.clone());
    queue.push_back((start, 0u32));

    while let Some((current, depth)) = queue.pop_front() {
        if depth >= max_depth {
            continue;
        }
        if let Some(neighbors) = adjacency.get(current) {
            for neighbor in neighbors {
                if visited.insert(neighbor) {
                    result.insert((*neighbor).clone());
                    queue.push_back((neighbor, depth + 1));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::*;

    fn make_graph(node_names: &[&str], edges: &[(&str, &str)]) -> ProjectGraph {
        let mut nodes = BTreeMap::new();
        for name in node_names {
            nodes.insert(
                CanonicalPath::new(*name),
                Node {
                    file_type: FileType::Source,
                    layer: ArchLayer::Unknown,
                    arch_depth: 0,
                    lines: 10,
                    hash: ContentHash::new("0".to_string()),
                    exports: vec![],
                    cluster: ClusterId::new("default"),
                },
            );
        }
        let edges = edges
            .iter()
            .map(|(from, to)| Edge {
                from: CanonicalPath::new(*from),
                to: CanonicalPath::new(*to),
                edge_type: EdgeType::Imports,
                symbols: vec![],
            })
            .collect();
        ProjectGraph { nodes, edges }
    }

    #[test]
    fn depth_1_neighborhood() {
        // A→B→C→D
        let graph = make_graph(&["a", "b", "c", "d"], &[("a", "b"), ("b", "c"), ("c", "d")]);
        let result = extract_subgraph(&graph, &[CanonicalPath::new("b")], 1);
        // B's depth-1 neighborhood: A (reverse), B (self), C (forward)
        assert!(result.nodes.contains_key(&CanonicalPath::new("a")));
        assert!(result.nodes.contains_key(&CanonicalPath::new("b")));
        assert!(result.nodes.contains_key(&CanonicalPath::new("c")));
    }

    #[test]
    fn cluster_inclusion() {
        // A→B, where A and B are in different clusters, C is in same cluster as B
        let mut graph = make_graph(&["a", "b", "c"], &[("a", "b")]);
        graph.nodes.get_mut(&CanonicalPath::new("a")).unwrap().cluster = ClusterId::new("cluster_a");
        graph.nodes.get_mut(&CanonicalPath::new("b")).unwrap().cluster = ClusterId::new("cluster_b");
        graph.nodes.get_mut(&CanonicalPath::new("c")).unwrap().cluster = ClusterId::new("cluster_b");

        let result = extract_subgraph(&graph, &[CanonicalPath::new("a")], 1);
        // C should be included via cluster expansion of B
        assert!(result.nodes.contains_key(&CanonicalPath::new("c")));
    }
}
