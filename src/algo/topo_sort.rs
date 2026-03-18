use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::algo::{build_adjacency, is_architectural};
use crate::model::{CanonicalPath, ProjectGraph};

/// Compute topological layers after SCC contraction.
/// Layer 0 = no outgoing architectural deps (leaves).
/// Layer N = depends on layers 0..N-1 (longest-path assignment).
/// All files within an SCC get the same layer.
pub fn topological_layers(
    graph: &ProjectGraph,
    sccs: &[Vec<CanonicalPath>],
) -> BTreeMap<CanonicalPath, u32> {
    let (forward, _) = build_adjacency(&graph.edges, is_architectural);

    // Build SCC membership: file → scc_index
    let mut scc_membership: BTreeMap<&CanonicalPath, usize> = BTreeMap::new();
    for (idx, scc) in sccs.iter().enumerate() {
        for file in scc {
            scc_membership.insert(file, idx);
        }
    }

    // Assign supernode IDs: each SCC gets one ID, standalone nodes get their own
    let mut node_to_super: BTreeMap<&CanonicalPath, usize> = BTreeMap::new();
    let mut next_super = sccs.len();
    for node in graph.nodes.keys() {
        if let Some(&scc_idx) = scc_membership.get(node) {
            node_to_super.insert(node, scc_idx);
        } else {
            node_to_super.insert(node, next_super);
            next_super += 1;
        }
    }

    // Build contracted DAG edges (supernode → supernode)
    let mut dag_forward: BTreeMap<usize, BTreeSet<usize>> = BTreeMap::new();
    let mut dag_reverse: BTreeMap<usize, BTreeSet<usize>> = BTreeMap::new();
    let mut all_supernodes: BTreeSet<usize> = BTreeSet::new();

    for node in graph.nodes.keys() {
        let from_super = node_to_super[node];
        all_supernodes.insert(from_super);
        if let Some(neighbors) = forward.get(node) {
            for neighbor in neighbors {
                if let Some(&to_super) = node_to_super.get(neighbor) {
                    if from_super != to_super {
                        dag_forward.entry(from_super).or_default().insert(to_super);
                        dag_reverse.entry(to_super).or_default().insert(from_super);
                    }
                }
            }
        }
    }

    // Longest-path via modified Kahn's algorithm
    // Start from sinks (no outgoing edges) at layer 0
    let mut in_degree: BTreeMap<usize, usize> = BTreeMap::new();
    for &sn in &all_supernodes {
        // "in_degree" here is actually out-degree for reversed perspective
        // We want to process from sinks, so we use reverse edges
        in_degree.insert(sn, 0);
    }
    // Count how many nodes point TO each node in the reversed DAG
    // Actually: we want longest path from sinks. Sinks = nodes with no outgoing edges.
    // Reverse the DAG: process from sinks using reversed edges.
    let mut out_degree: BTreeMap<usize, usize> = BTreeMap::new();
    for &sn in &all_supernodes {
        out_degree.insert(sn, dag_forward.get(&sn).map_or(0, |s| s.len()));
    }

    let mut queue: VecDeque<usize> = VecDeque::new();
    let mut layer: BTreeMap<usize, u32> = BTreeMap::new();

    // Sinks: no outgoing edges → layer 0
    for &sn in &all_supernodes {
        if out_degree[&sn] == 0 {
            layer.insert(sn, 0);
            queue.push_back(sn);
        }
    }

    // BFS in reverse (from sinks toward sources)
    while let Some(current) = queue.pop_front() {
        let current_layer = layer[&current];
        if let Some(predecessors) = dag_reverse.get(&current) {
            for &pred in predecessors {
                // Longest path: layer = max(current_layer + 1, existing)
                let new_layer = current_layer + 1;
                let existing = layer.entry(pred).or_insert(0);
                if new_layer > *existing {
                    *existing = new_layer;
                }
                // Decrement outgoing count for predecessor
                let count = out_degree.get_mut(&pred).unwrap();
                *count -= 1;
                if *count == 0 {
                    queue.push_back(pred);
                }
            }
        }
    }

    // Map supernode layers back to file layers
    let mut result = BTreeMap::new();
    for (node, &super_id) in &node_to_super {
        let l = layer.get(&super_id).copied().unwrap_or(0);
        result.insert((*node).clone(), l);
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::algo::scc::find_sccs;
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
    fn linear_chain() {
        // A→B→C: A=2, B=1, C=0
        let graph = make_graph(&["a", "b", "c"], &[("a", "b"), ("b", "c")]);
        let sccs = find_sccs(&graph);
        let layers = topological_layers(&graph, &sccs);
        assert_eq!(layers[&CanonicalPath::new("a")], 2);
        assert_eq!(layers[&CanonicalPath::new("b")], 1);
        assert_eq!(layers[&CanonicalPath::new("c")], 0);
    }

    #[test]
    fn dag_with_multiple_paths() {
        // A→B, A→C, B→D, C→D
        // A=2, B=1, C=1, D=0
        let graph = make_graph(
            &["a", "b", "c", "d"],
            &[("a", "b"), ("a", "c"), ("b", "d"), ("c", "d")],
        );
        let sccs = find_sccs(&graph);
        let layers = topological_layers(&graph, &sccs);
        assert_eq!(layers[&CanonicalPath::new("a")], 2);
        assert_eq!(layers[&CanonicalPath::new("b")], 1);
        assert_eq!(layers[&CanonicalPath::new("c")], 1);
        assert_eq!(layers[&CanonicalPath::new("d")], 0);
    }

    #[test]
    fn graph_with_cycle() {
        // A→B→A (cycle), B→C
        // SCC={A,B} shares a layer. C=0, {A,B}=1
        let graph = make_graph(&["a", "b", "c"], &[("a", "b"), ("b", "a"), ("b", "c")]);
        let sccs = find_sccs(&graph);
        let layers = topological_layers(&graph, &sccs);
        assert_eq!(
            layers[&CanonicalPath::new("a")],
            layers[&CanonicalPath::new("b")]
        );
        assert_eq!(layers[&CanonicalPath::new("c")], 0);
        assert!(layers[&CanonicalPath::new("a")] > 0);
    }

    #[test]
    fn single_node() {
        let graph = make_graph(&["a"], &[]);
        let sccs = find_sccs(&graph);
        let layers = topological_layers(&graph, &sccs);
        assert_eq!(layers[&CanonicalPath::new("a")], 0);
    }

    #[test]
    fn empty_graph() {
        let graph = ProjectGraph {
            nodes: BTreeMap::new(),
            edges: vec![],
        };
        let sccs = find_sccs(&graph);
        let layers = topological_layers(&graph, &sccs);
        assert!(layers.is_empty());
    }
}
