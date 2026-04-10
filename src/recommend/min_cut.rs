use super::types::{MinCutResult, SymbolGraph};
use std::collections::BTreeSet;

/// Stoer-Wagner global minimum cut algorithm.
///
/// Returns the minimum weight cut of a weighted undirected graph,
/// or None if the graph has fewer than 2 nodes.
///
/// Preconditions:
/// - `graph.weights` is symmetric: `weights[i][j] == weights[j][i]`
/// - All weights are non-negative: `weights[i][j] >= 0.0`
/// - Diagonal is zero: `weights[i][i] == 0.0`
/// - `weights.len() == graph.nodes.len()` and all rows have length `graph.nodes.len()`
pub fn stoer_wagner(graph: &SymbolGraph) -> Option<MinCutResult> {
    let n = graph.nodes.len();
    if n < 2 {
        return None;
    }

    // Debug assertions for preconditions
    debug_assert_eq!(graph.weights.len(), n, "weights rows must match node count");
    debug_assert!(
        graph.weights.iter().all(|row| row.len() == n),
        "all weight rows must have length n"
    );
    debug_assert!(
        (0..n).all(|i| (0..n).all(|j| graph.weights[i][j] >= 0.0)),
        "all weights must be non-negative"
    );
    debug_assert!(
        (0..n).all(|i| graph.weights[i][i] == 0.0),
        "diagonal must be zero"
    );
    debug_assert!(
        (0..n).all(|i| (0..n).all(|j| (graph.weights[i][j] - graph.weights[j][i]).abs() < 1e-12)),
        "weights must be symmetric"
    );

    // Working copy of adjacency matrix
    let mut w: Vec<Vec<f64>> = graph.weights.clone();
    // Track which original nodes are merged into each super-node
    let mut merged: Vec<Vec<usize>> = (0..n).map(|i| vec![i]).collect();
    // Track active super-nodes
    let mut active: Vec<bool> = vec![true; n];

    let mut best_cut = f64::MAX;
    let mut best_partition: BTreeSet<usize> = BTreeSet::new();

    for _ in 0..(n - 1) {
        // Maximum adjacency ordering
        // Find an arbitrary active node to start
        let start = active.iter().position(|&a| a).expect("Stoer-Wagner: at least one active node per iteration");

        let mut in_order: Vec<bool> = vec![false; n];
        let mut key: Vec<f64> = vec![0.0; n]; // weight of connection to nodes already in order

        let mut s = start;
        let mut t = start;

        // Count active nodes
        let active_count = active.iter().filter(|&&a| a).count();

        for step in 0..active_count {
            // Pick the most tightly connected active node not yet in order
            if step == 0 {
                // First node: just pick start
                in_order[start] = true;
                t = start;
                // Update keys for neighbors of start
                for j in 0..n {
                    if active[j] && !in_order[j] {
                        key[j] += w[start][j];
                    }
                }
            } else {
                // Find active, not-in-order node with maximum key
                let mut best_key = -1.0;
                let mut best_node = 0;
                for j in 0..n {
                    if active[j] && !in_order[j] && key[j] > best_key {
                        best_key = key[j];
                        best_node = j;
                    }
                }
                s = t;
                t = best_node;
                in_order[best_node] = true;

                // Update keys for neighbors
                for j in 0..n {
                    if active[j] && !in_order[j] {
                        key[j] += w[best_node][j];
                    }
                }
            }
        }

        // Cut-of-the-phase: the key value of t when it was added
        let cut_of_phase = key[t];

        if cut_of_phase < best_cut {
            best_cut = cut_of_phase;
            best_partition = merged[t].iter().copied().collect();
        }

        // Merge t into s
        for j in 0..n {
            if active[j] && j != s && j != t {
                w[s][j] += w[t][j];
                w[j][s] += w[j][t];
            }
        }
        let t_merged: Vec<usize> = merged[t].clone();
        merged[s].extend(t_merged);
        active[t] = false;
    }

    let all_nodes: BTreeSet<usize> = (0..n).collect();
    let partition_b: BTreeSet<usize> = all_nodes.difference(&best_partition).copied().collect();

    Some(MinCutResult {
        cut_weight: best_cut,
        partition_a: best_partition,
        partition_b,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_complete_graph(n: usize, weight: f64) -> SymbolGraph {
        let nodes: Vec<String> = (0..n).map(|i| format!("node_{i}")).collect();
        let mut weights = vec![vec![0.0; n]; n];
        for i in 0..n {
            for j in 0..n {
                if i != j {
                    weights[i][j] = weight;
                }
            }
        }
        SymbolGraph { nodes, weights }
    }

    fn assert_valid_partition(result: &MinCutResult, n: usize) {
        assert!(!result.partition_a.is_empty(), "partition_a must be non-empty");
        assert!(!result.partition_b.is_empty(), "partition_b must be non-empty");
        let union: BTreeSet<usize> = result
            .partition_a
            .union(&result.partition_b)
            .copied()
            .collect();
        let expected: BTreeSet<usize> = (0..n).collect();
        assert_eq!(union, expected, "union must cover all nodes");
        let intersection: BTreeSet<usize> = result
            .partition_a
            .intersection(&result.partition_b)
            .copied()
            .collect();
        assert!(intersection.is_empty(), "partitions must be disjoint");
    }

    #[test]
    fn empty_graph() {
        let g = SymbolGraph {
            nodes: vec![],
            weights: vec![],
        };
        assert!(stoer_wagner(&g).is_none());
    }

    #[test]
    fn single_node() {
        let g = SymbolGraph {
            nodes: vec!["a".to_string()],
            weights: vec![vec![0.0]],
        };
        assert!(stoer_wagner(&g).is_none());
    }

    #[test]
    fn two_nodes_no_edge() {
        let g = SymbolGraph {
            nodes: vec!["a".to_string(), "b".to_string()],
            weights: vec![vec![0.0, 0.0], vec![0.0, 0.0]],
        };
        let result = stoer_wagner(&g).unwrap();
        assert_eq!(result.cut_weight, 0.0);
        assert_valid_partition(&result, 2);
    }

    #[test]
    fn two_nodes_weighted() {
        let g = SymbolGraph {
            nodes: vec!["a".to_string(), "b".to_string()],
            weights: vec![vec![0.0, 5.0], vec![5.0, 0.0]],
        };
        let result = stoer_wagner(&g).unwrap();
        assert_eq!(result.cut_weight, 5.0);
        assert_valid_partition(&result, 2);
    }

    #[test]
    fn triangle_graph() {
        let g = make_complete_graph(3, 1.0);
        let result = stoer_wagner(&g).unwrap();
        assert_eq!(result.cut_weight, 2.0);
        assert_valid_partition(&result, 3);
    }

    #[test]
    fn barbell_graph() {
        // Two 3-cliques connected by a single edge of weight 0.5
        // Nodes 0,1,2 form one clique; 3,4,5 form the other
        // Edge between 2 and 3 has weight 0.5
        let n = 6;
        let mut weights = vec![vec![0.0; n]; n];
        // First clique: 0-1, 0-2, 1-2
        for &(i, j) in &[(0, 1), (0, 2), (1, 2)] {
            weights[i][j] = 1.0;
            weights[j][i] = 1.0;
        }
        // Second clique: 3-4, 3-5, 4-5
        for &(i, j) in &[(3, 4), (3, 5), (4, 5)] {
            weights[i][j] = 1.0;
            weights[j][i] = 1.0;
        }
        // Bridge: 2-3
        weights[2][3] = 0.5;
        weights[3][2] = 0.5;

        let nodes: Vec<String> = (0..n).map(|i| format!("node_{i}")).collect();
        let g = SymbolGraph { nodes, weights };
        let result = stoer_wagner(&g).unwrap();
        assert_eq!(result.cut_weight, 0.5);
        assert_valid_partition(&result, n);
    }

    #[test]
    fn complete_k4() {
        let g = make_complete_graph(4, 1.0);
        let result = stoer_wagner(&g).unwrap();
        assert_eq!(result.cut_weight, 3.0);
        assert_valid_partition(&result, 4);
    }

    #[test]
    fn disconnected_graph() {
        // Two disconnected components: {0,1} and {2,3}
        let n = 4;
        let mut weights = vec![vec![0.0; n]; n];
        weights[0][1] = 1.0;
        weights[1][0] = 1.0;
        weights[2][3] = 1.0;
        weights[3][2] = 1.0;

        let nodes: Vec<String> = (0..n).map(|i| format!("node_{i}")).collect();
        let g = SymbolGraph { nodes, weights };
        let result = stoer_wagner(&g).unwrap();
        assert_eq!(result.cut_weight, 0.0);
        assert_valid_partition(&result, n);
    }

    #[test]
    fn partition_validity() {
        // Already tested in every test above via assert_valid_partition,
        // but here's an explicit standalone test with a non-trivial graph
        let g = make_complete_graph(5, 2.0);
        let result = stoer_wagner(&g).unwrap();
        assert_valid_partition(&result, 5);
    }

    #[test]
    fn large_uniform_k10() {
        let g = make_complete_graph(10, 1.0);
        let result = stoer_wagner(&g).unwrap();
        assert_eq!(result.cut_weight, 9.0);
        assert_valid_partition(&result, 10);
    }
}
