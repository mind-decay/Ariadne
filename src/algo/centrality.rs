use std::collections::{BTreeMap, VecDeque};

use crate::algo::{build_adjacency, is_architectural, round4};
use crate::model::{CanonicalPath, ProjectGraph};

/// Compute betweenness centrality using Brandes algorithm.
/// Returns normalized values in [0.0, 1.0], rounded to 4 decimal places.
pub fn betweenness_centrality(graph: &ProjectGraph) -> BTreeMap<CanonicalPath, f64> {
    let nodes: Vec<&CanonicalPath> = graph.nodes.keys().collect();
    let n = nodes.len();

    let mut centrality: BTreeMap<&CanonicalPath, f64> = BTreeMap::new();
    for &node in &nodes {
        centrality.insert(node, 0.0);
    }

    if n < 3 {
        return centrality
            .into_iter()
            .map(|(k, v)| (k.clone(), round4(v)))
            .collect();
    }

    let (forward, _) = build_adjacency(&graph.edges, is_architectural);

    // Brandes algorithm
    for &source in &nodes {
        // BFS from source
        let mut stack: Vec<&CanonicalPath> = Vec::new();
        let mut predecessors: BTreeMap<&CanonicalPath, Vec<&CanonicalPath>> = BTreeMap::new();
        let mut sigma: BTreeMap<&CanonicalPath, f64> = BTreeMap::new();
        let mut dist: BTreeMap<&CanonicalPath, i64> = BTreeMap::new();

        for &node in &nodes {
            sigma.insert(node, 0.0);
            dist.insert(node, -1);
        }
        sigma.insert(source, 1.0);
        dist.insert(source, 0);

        let mut queue = VecDeque::new();
        queue.push_back(source);

        while let Some(v) = queue.pop_front() {
            stack.push(v);
            let v_dist = dist[v];

            if let Some(neighbors) = forward.get(v) {
                for &w in neighbors {
                    // w found for the first time?
                    if dist[w] < 0 {
                        queue.push_back(w);
                        dist.insert(w, v_dist + 1);
                    }
                    // Shortest path to w via v?
                    if dist[w] == v_dist + 1 {
                        *sigma.get_mut(w).unwrap() += sigma[v];
                        predecessors.entry(w).or_default().push(v);
                    }
                }
            }
        }

        // Back-propagation
        let mut delta: BTreeMap<&CanonicalPath, f64> = BTreeMap::new();
        for &node in &nodes {
            delta.insert(node, 0.0);
        }

        while let Some(w) = stack.pop() {
            if let Some(preds) = predecessors.get(w) {
                for &v in preds {
                    let contribution = (sigma[v] / sigma[w]) * (1.0 + delta[w]);
                    *delta.get_mut(v).unwrap() += contribution;
                }
            }
            if w != source {
                *centrality.get_mut(w).unwrap() += delta[w];
            }
        }
    }

    // Normalize by (V-1)*(V-2) for directed graphs
    let normalization = (n as f64 - 1.0) * (n as f64 - 2.0);

    centrality
        .into_iter()
        .map(|(k, v)| (k.clone(), round4(v / normalization)))
        .collect()
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
    fn star_graph_center_highest() {
        // Center C, spokes A→C, B→C, D→C, C→E
        let graph = make_graph(
            &["a", "b", "c", "d", "e"],
            &[("a", "c"), ("b", "c"), ("d", "c"), ("c", "e")],
        );
        let bc = betweenness_centrality(&graph);
        let c_val = bc[&CanonicalPath::new("c")];
        for name in &["a", "b", "d", "e"] {
            assert!(c_val >= bc[&CanonicalPath::new(*name)]);
        }
    }

    #[test]
    fn linear_chain_middle_highest() {
        // A→B→C→D: B and C should have higher centrality
        let graph = make_graph(&["a", "b", "c", "d"], &[("a", "b"), ("b", "c"), ("c", "d")]);
        let bc = betweenness_centrality(&graph);
        let b_val = bc[&CanonicalPath::new("b")];
        let c_val = bc[&CanonicalPath::new("c")];
        let a_val = bc[&CanonicalPath::new("a")];
        let d_val = bc[&CanonicalPath::new("d")];
        assert!(b_val > a_val);
        assert!(c_val > d_val);
    }

    #[test]
    fn values_in_range() {
        let graph = make_graph(
            &["a", "b", "c", "d"],
            &[("a", "b"), ("b", "c"), ("c", "d"), ("a", "c")],
        );
        let bc = betweenness_centrality(&graph);
        for &v in bc.values() {
            assert!(v >= 0.0);
            assert!(v <= 1.0);
        }
    }

    #[test]
    fn fewer_than_3_nodes() {
        let graph = make_graph(&["a", "b"], &[("a", "b")]);
        let bc = betweenness_centrality(&graph);
        for &v in bc.values() {
            assert_eq!(v, 0.0);
        }
    }

    #[test]
    fn float_determinism() {
        let graph = make_graph(
            &["a", "b", "c", "d", "e"],
            &[("a", "b"), ("b", "c"), ("c", "d"), ("d", "e"), ("a", "c"), ("b", "d")],
        );
        let bc1 = betweenness_centrality(&graph);
        let bc2 = betweenness_centrality(&graph);
        assert_eq!(bc1, bc2);
    }
}
