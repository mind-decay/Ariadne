use std::collections::BTreeMap;

use crate::algo::AdjacencyIndex;
use crate::model::{CanonicalPath, ProjectGraph};

/// Find all strongly connected components of size > 1 using iterative Tarjan's algorithm.
/// Returns SCCs sorted deterministically: inner Vecs sorted lexicographically,
/// outer Vec sorted by first element.
pub fn find_sccs(graph: &ProjectGraph, index: &AdjacencyIndex) -> Vec<Vec<CanonicalPath>> {
    let forward = &index.forward;

    let nodes: Vec<&CanonicalPath> = graph.nodes.keys().collect();
    let mut index_counter: u32 = 0;
    let mut indices: BTreeMap<&CanonicalPath, u32> = BTreeMap::new();
    let mut lowlinks: BTreeMap<&CanonicalPath, u32> = BTreeMap::new();
    let mut on_stack: BTreeMap<&CanonicalPath, bool> = BTreeMap::new();
    let mut stack: Vec<&CanonicalPath> = Vec::new();
    let mut result: Vec<Vec<CanonicalPath>> = Vec::new();

    // Iterative Tarjan's to avoid stack overflow on deep graphs
    for start in &nodes {
        if indices.contains_key(start) {
            continue;
        }

        // DFS stack: (node, neighbor_index, is_root_call)
        let mut dfs_stack: Vec<(&CanonicalPath, usize)> = Vec::new();

        // Initialize start node
        indices.insert(start, index_counter);
        lowlinks.insert(start, index_counter);
        index_counter += 1;
        on_stack.insert(start, true);
        stack.push(start);
        dfs_stack.push((start, 0));

        while let Some((node, neighbor_idx)) = dfs_stack.last_mut() {
            let neighbors = forward.get(*node).map(|v| v.as_slice()).unwrap_or(&[]);

            if *neighbor_idx < neighbors.len() {
                let neighbor = neighbors[*neighbor_idx];
                *neighbor_idx += 1;

                if !indices.contains_key(neighbor) {
                    // Tree edge: push neighbor
                    indices.insert(neighbor, index_counter);
                    lowlinks.insert(neighbor, index_counter);
                    index_counter += 1;
                    on_stack.insert(neighbor, true);
                    stack.push(neighbor);
                    dfs_stack.push((neighbor, 0));
                } else if *on_stack.get(neighbor).unwrap_or(&false) {
                    // Back edge: update lowlink
                    let node_lowlink = lowlinks[*node];
                    let neighbor_index = indices[neighbor];
                    if neighbor_index < node_lowlink {
                        lowlinks.insert(*node, neighbor_index);
                    }
                }
            } else {
                // All neighbors processed — check if this is an SCC root
                let node = *node;
                if lowlinks[node] == indices[node] {
                    let mut scc = Vec::new();
                    loop {
                        let w = stack.pop().unwrap();
                        on_stack.insert(w, false);
                        scc.push(w.clone());
                        if w == node {
                            break;
                        }
                    }
                    if scc.len() > 1 {
                        scc.sort();
                        result.push(scc);
                    }
                }

                dfs_stack.pop();

                // Propagate lowlink to parent
                if let Some((parent, _)) = dfs_stack.last() {
                    let node_lowlink = lowlinks[node];
                    let parent_lowlink = lowlinks[*parent];
                    if node_lowlink < parent_lowlink {
                        lowlinks.insert(*parent, node_lowlink);
                    }
                }
            }
        }
    }

    // Sort outer vec by first element for determinism
    result.sort_by(|a, b| a[0].cmp(&b[0]));
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::algo::is_architectural;
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
    fn linear_chain_no_sccs() {
        let graph = make_graph(&["a", "b", "c"], &[("a", "b"), ("b", "c")]);
        let index = AdjacencyIndex::build(&graph.edges, is_architectural);
        let sccs = find_sccs(&graph, &index);
        assert!(sccs.is_empty());
    }

    #[test]
    fn simple_cycle() {
        let graph = make_graph(&["a", "b"], &[("a", "b"), ("b", "a")]);
        let index = AdjacencyIndex::build(&graph.edges, is_architectural);
        let sccs = find_sccs(&graph, &index);
        assert_eq!(sccs.len(), 1);
        assert_eq!(
            sccs[0],
            vec![CanonicalPath::new("a"), CanonicalPath::new("b")]
        );
    }

    #[test]
    fn two_separate_cycles() {
        let graph = make_graph(
            &["a", "b", "c", "d"],
            &[("a", "b"), ("b", "a"), ("c", "d"), ("d", "c")],
        );
        let index = AdjacencyIndex::build(&graph.edges, is_architectural);
        let sccs = find_sccs(&graph, &index);
        assert_eq!(sccs.len(), 2);
        assert_eq!(
            sccs[0],
            vec![CanonicalPath::new("a"), CanonicalPath::new("b")]
        );
        assert_eq!(
            sccs[1],
            vec![CanonicalPath::new("c"), CanonicalPath::new("d")]
        );
    }

    #[test]
    fn dag_no_sccs() {
        let graph = make_graph(
            &["a", "b", "c", "d"],
            &[("a", "b"), ("a", "c"), ("b", "d"), ("c", "d")],
        );
        let index = AdjacencyIndex::build(&graph.edges, is_architectural);
        let sccs = find_sccs(&graph, &index);
        assert!(sccs.is_empty());
    }

    #[test]
    fn fully_connected() {
        let graph = make_graph(
            &["a", "b", "c"],
            &[
                ("a", "b"),
                ("b", "c"),
                ("c", "a"),
                ("a", "c"),
                ("b", "a"),
                ("c", "b"),
            ],
        );
        let index = AdjacencyIndex::build(&graph.edges, is_architectural);
        let sccs = find_sccs(&graph, &index);
        assert_eq!(sccs.len(), 1);
        assert_eq!(sccs[0].len(), 3);
    }

    #[test]
    fn empty_graph() {
        let graph = ProjectGraph {
            nodes: BTreeMap::new(),
            edges: vec![],
        };
        let index = AdjacencyIndex::build(&graph.edges, is_architectural);
        let sccs = find_sccs(&graph, &index);
        assert!(sccs.is_empty());
    }

    #[test]
    fn tests_edges_excluded() {
        let mut graph = make_graph(&["a", "b"], &[]);
        graph.edges.push(Edge {
            from: CanonicalPath::new("a"),
            to: CanonicalPath::new("b"),
            edge_type: EdgeType::Tests,
            symbols: vec![],
        });
        graph.edges.push(Edge {
            from: CanonicalPath::new("b"),
            to: CanonicalPath::new("a"),
            edge_type: EdgeType::Tests,
            symbols: vec![],
        });
        let index = AdjacencyIndex::build(&graph.edges, is_architectural);
        let sccs = find_sccs(&graph, &index);
        assert!(sccs.is_empty());
    }
}
