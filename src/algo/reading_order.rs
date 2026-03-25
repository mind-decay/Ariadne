use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::algo::scc::find_sccs;
use crate::algo::topo_sort::topological_layers;
use crate::algo::{is_architectural, AdjacencyIndex};
use crate::model::{CanonicalPath, ProjectGraph};

/// An entry in the computed reading order.
#[derive(Clone, Debug)]
pub struct ReadingOrderEntry {
    pub path: CanonicalPath,
    pub reason: String,
    pub layer: u32,
    pub depth: u32,
}

/// Result of computing reading order.
#[derive(Clone, Debug)]
pub struct ReadingOrderResult {
    pub entries: Vec<ReadingOrderEntry>,
    pub total_files: usize,
    pub warnings: Vec<String>,
}

/// Compute a topologically-sorted reading order for the given seed files and their
/// neighborhood within the project graph.
///
/// 1. Extract subgraph via `extract_subgraph`
/// 2. Build AdjacencyIndex with architectural filter
/// 3. Compute SCCs and topological layers
/// 4. BFS from seeds to compute distance for reason annotation
/// 5. Sort: layer asc, BFS depth asc, path lexicographic
pub fn compute_reading_order(
    paths: &[CanonicalPath],
    graph: &ProjectGraph,
    depth: u32,
) -> ReadingOrderResult {
    let mut warnings = Vec::new();

    // Check for seeds not in graph
    for path in paths {
        if !graph.nodes.contains_key(path) {
            warnings.push(format!("Seed file not in graph: {}", path));
        }
    }

    // Extract subgraph
    let subgraph = crate::algo::subgraph::extract_subgraph(graph, paths, depth);

    // Build a ProjectGraph from the subgraph for SCC/topo algorithms
    let sub_graph = ProjectGraph {
        nodes: subgraph.nodes,
        edges: subgraph.edges,
    };

    if sub_graph.nodes.is_empty() {
        return ReadingOrderResult {
            entries: Vec::new(),
            total_files: 0,
            warnings,
        };
    }

    // Build AdjacencyIndex with architectural filter
    let index = AdjacencyIndex::build(&sub_graph.edges, is_architectural);

    // Compute SCCs and topological layers
    let sccs = find_sccs(&sub_graph, &index);
    let layers = topological_layers(&sub_graph, &sccs, &index);

    // BFS from seeds to compute distance (forward + reverse, using architectural edges)
    let seeds: BTreeSet<&CanonicalPath> = paths
        .iter()
        .filter(|p| sub_graph.nodes.contains_key(*p))
        .collect();

    let bfs_depth = bfs_from_seeds(&seeds, &index);

    // Build entries
    let mut entries: Vec<ReadingOrderEntry> = Vec::new();
    for path in sub_graph.nodes.keys() {
        let layer = layers.get(path).copied().unwrap_or(0);
        let d = bfs_depth.get(path).copied().unwrap_or(u32::MAX);
        let reason = match d {
            0 => "seed file".to_string(),
            1 => "direct dependency".to_string(),
            n if n < u32::MAX => format!("transitive dependency (depth {})", n),
            _ => "cluster member".to_string(),
        };

        entries.push(ReadingOrderEntry {
            path: path.clone(),
            reason,
            layer,
            depth: d,
        });
    }

    // Sort: layer asc, then BFS depth asc, then path lexicographic
    entries.sort_by(|a, b| {
        a.layer
            .cmp(&b.layer)
            .then_with(|| a.depth.cmp(&b.depth))
            .then_with(|| a.path.cmp(&b.path))
    });

    let total_files = entries.len();

    ReadingOrderResult {
        entries,
        total_files,
        warnings,
    }
}

/// BFS from seed files using both forward and reverse architectural edges.
/// Returns distance from nearest seed for each reachable node.
fn bfs_from_seeds(
    seeds: &BTreeSet<&CanonicalPath>,
    index: &AdjacencyIndex,
) -> BTreeMap<CanonicalPath, u32> {
    let mut distances: BTreeMap<CanonicalPath, u32> = BTreeMap::new();
    let mut queue: VecDeque<(&CanonicalPath, u32)> = VecDeque::new();
    let mut visited: BTreeSet<&CanonicalPath> = BTreeSet::new();

    for &seed in seeds {
        distances.insert(seed.clone(), 0);
        visited.insert(seed);
        queue.push_back((seed, 0));
    }

    while let Some((current, depth)) = queue.pop_front() {
        // Forward neighbors
        if let Some(fwd) = index.forward.get(current) {
            for neighbor in fwd {
                if visited.insert(neighbor) {
                    distances.insert((*neighbor).clone(), depth + 1);
                    queue.push_back((neighbor, depth + 1));
                }
            }
        }
        // Reverse neighbors
        if let Some(rev) = index.reverse.get(current) {
            for neighbor in rev {
                if visited.insert(neighbor) {
                    distances.insert((*neighbor).clone(), depth + 1);
                    queue.push_back((neighbor, depth + 1));
                }
            }
        }
    }

    distances
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
                    fsd_layer: None,
                    arch_depth: 0,
                    lines: 10,
                    hash: ContentHash::new("0".to_string()),
                    exports: vec![],
                    cluster: ClusterId::new("default"),
                    symbols: Vec::new(),
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
    fn linear_chain_ordering() {
        // a -> b -> c: reading from c (leaf) first
        let graph = make_graph(&["a", "b", "c"], &[("a", "b"), ("b", "c")]);
        let result = compute_reading_order(&[CanonicalPath::new("a")], &graph, 3);

        assert_eq!(result.total_files, 3);
        assert!(result.warnings.is_empty());

        // Layer 0 = c (leaf), layer 1 = b, layer 2 = a
        let paths: Vec<&str> = result.entries.iter().map(|e| e.path.as_str()).collect();
        assert_eq!(paths, vec!["c", "b", "a"]);

        // Check reasons
        let seed = result.entries.iter().find(|e| e.path.as_str() == "a").unwrap();
        assert_eq!(seed.reason, "seed file");
        let direct = result.entries.iter().find(|e| e.path.as_str() == "b").unwrap();
        assert_eq!(direct.reason, "direct dependency");
        let transitive = result.entries.iter().find(|e| e.path.as_str() == "c").unwrap();
        assert_eq!(transitive.reason, "transitive dependency (depth 2)");
    }

    #[test]
    fn diamond_dag() {
        // a -> b, a -> c, b -> d, c -> d
        let graph = make_graph(
            &["a", "b", "c", "d"],
            &[("a", "b"), ("a", "c"), ("b", "d"), ("c", "d")],
        );
        let result = compute_reading_order(&[CanonicalPath::new("a")], &graph, 3);

        assert_eq!(result.total_files, 4);

        // d is layer 0, b and c are layer 1, a is layer 2
        // Within same layer and same depth, sorted by path
        let paths: Vec<&str> = result.entries.iter().map(|e| e.path.as_str()).collect();
        assert_eq!(paths[0], "d"); // layer 0
        // b and c at layer 1, both depth 1, sorted alphabetically
        assert_eq!(paths[1], "b");
        assert_eq!(paths[2], "c");
        assert_eq!(paths[3], "a"); // layer 2
    }

    #[test]
    fn single_file() {
        let graph = make_graph(&["a"], &[]);
        let result = compute_reading_order(&[CanonicalPath::new("a")], &graph, 3);

        assert_eq!(result.total_files, 1);
        assert_eq!(result.entries[0].path.as_str(), "a");
        assert_eq!(result.entries[0].reason, "seed file");
        assert_eq!(result.entries[0].layer, 0);
    }

    #[test]
    fn multiple_seeds() {
        // a -> c, b -> c
        let graph = make_graph(&["a", "b", "c"], &[("a", "c"), ("b", "c")]);
        let result = compute_reading_order(
            &[CanonicalPath::new("a"), CanonicalPath::new("b")],
            &graph,
            3,
        );

        assert_eq!(result.total_files, 3);

        // Both a and b are seeds (depth 0)
        let a_entry = result.entries.iter().find(|e| e.path.as_str() == "a").unwrap();
        let b_entry = result.entries.iter().find(|e| e.path.as_str() == "b").unwrap();
        assert_eq!(a_entry.reason, "seed file");
        assert_eq!(b_entry.reason, "seed file");

        // c is direct dependency of both seeds
        let c_entry = result.entries.iter().find(|e| e.path.as_str() == "c").unwrap();
        assert_eq!(c_entry.reason, "direct dependency");
    }

    #[test]
    fn nonexistent_seed_warning() {
        let graph = make_graph(&["a"], &[]);
        let result = compute_reading_order(&[CanonicalPath::new("nonexistent")], &graph, 3);

        assert_eq!(result.warnings.len(), 1);
        assert!(result.warnings[0].contains("nonexistent"));
        assert_eq!(result.total_files, 0);
    }
}
