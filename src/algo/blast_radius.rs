use std::collections::{BTreeMap, VecDeque};

use crate::algo::AdjacencyIndex;
use crate::model::{CanonicalPath, ProjectGraph};

/// Compute blast radius via reverse BFS from the given file.
/// Returns a BTreeMap of file → distance (source file included at distance 0).
/// `max_depth` of `None` means unbounded.
pub fn blast_radius(
    graph: &ProjectGraph,
    file: &CanonicalPath,
    max_depth: Option<u32>,
    index: &AdjacencyIndex,
) -> BTreeMap<CanonicalPath, u32> {
    let mut result = BTreeMap::new();

    if !graph.nodes.contains_key(file) {
        return result;
    }

    let reverse = &index.reverse;

    let mut queue = VecDeque::new();
    result.insert(file.clone(), 0);
    queue.push_back((file, 0u32));

    while let Some((current, depth)) = queue.pop_front() {
        if let Some(max) = max_depth {
            if depth >= max {
                continue;
            }
        }

        if let Some(dependents) = reverse.get(current) {
            for dependent in dependents {
                if !result.contains_key(*dependent) {
                    result.insert((*dependent).clone(), depth + 1);
                    queue.push_back((*dependent, depth + 1));
                }
            }
        }
    }

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
                    fsd_layer: None,
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
        // A→B→C: blast_radius(C) = {C:0, B:1, A:2}
        let graph = make_graph(&["a", "b", "c"], &[("a", "b"), ("b", "c")]);
        let index = AdjacencyIndex::build(&graph.edges, is_architectural);
        let result = blast_radius(&graph, &CanonicalPath::new("c"), None, &index);
        assert_eq!(result[&CanonicalPath::new("c")], 0);
        assert_eq!(result[&CanonicalPath::new("b")], 1);
        assert_eq!(result[&CanonicalPath::new("a")], 2);
    }

    #[test]
    fn with_depth_limit() {
        let graph = make_graph(&["a", "b", "c"], &[("a", "b"), ("b", "c")]);
        let index = AdjacencyIndex::build(&graph.edges, is_architectural);
        let result = blast_radius(&graph, &CanonicalPath::new("c"), Some(1), &index);
        assert_eq!(result.len(), 2);
        assert_eq!(result[&CanonicalPath::new("c")], 0);
        assert_eq!(result[&CanonicalPath::new("b")], 1);
        assert!(!result.contains_key(&CanonicalPath::new("a")));
    }

    #[test]
    fn disconnected_node() {
        let graph = make_graph(&["a", "b"], &[]);
        let index = AdjacencyIndex::build(&graph.edges, is_architectural);
        let result = blast_radius(&graph, &CanonicalPath::new("a"), None, &index);
        assert_eq!(result.len(), 1);
        assert_eq!(result[&CanonicalPath::new("a")], 0);
    }

    #[test]
    fn nonexistent_file() {
        let graph = make_graph(&["a"], &[]);
        let index = AdjacencyIndex::build(&graph.edges, is_architectural);
        let result = blast_radius(&graph, &CanonicalPath::new("z"), None, &index);
        assert!(result.is_empty());
    }

    #[test]
    fn re_export_propagation() {
        // A imports B, B re_exports C: blast_radius(C) should reach A
        let mut graph = make_graph(&["a", "b", "c"], &[]);
        graph.edges.push(Edge {
            from: CanonicalPath::new("a"),
            to: CanonicalPath::new("b"),
            edge_type: EdgeType::Imports,
            symbols: vec![],
        });
        graph.edges.push(Edge {
            from: CanonicalPath::new("b"),
            to: CanonicalPath::new("c"),
            edge_type: EdgeType::ReExports,
            symbols: vec![],
        });
        let index = AdjacencyIndex::build(&graph.edges, is_architectural);
        let result = blast_radius(&graph, &CanonicalPath::new("c"), None, &index);
        assert_eq!(result.len(), 3);
    }
}
