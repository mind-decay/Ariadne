use std::collections::BTreeMap;

use crate::algo::round4;
use crate::model::{CanonicalPath, Edge, EdgeType, ProjectGraph};

/// Whether an edge should be included in PageRank computation.
/// Only imports and re_exports — excludes tests and type_imports (D-063).
fn is_pagerank_edge(edge: &Edge) -> bool {
    matches!(edge.edge_type, EdgeType::Imports | EdgeType::ReExports)
}

/// Compute PageRank on the import graph.
///
/// Ranks files by authority: files that many important files depend on score highest.
/// Standard PageRank on the original import graph (A→B where A imports B) naturally
/// gives high rank to B (the dependency), which is the desired "foundation" ranking.
/// Uses power iteration with fixed parameters for determinism (D-049).
pub fn pagerank(
    graph: &ProjectGraph,
    damping: f64,
    max_iterations: u32,
    tolerance: f64,
) -> BTreeMap<CanonicalPath, f64> {
    let nodes: Vec<&CanonicalPath> = graph.nodes.keys().collect();
    let n = nodes.len();
    if n == 0 {
        return BTreeMap::new();
    }
    let n_f64 = n as f64;

    // Build adjacency on the original import graph.
    // Edge from→to means "from imports to". PageRank gives high rank to `to`
    // (files that are imported by many important files = foundations).
    let mut incoming: BTreeMap<&CanonicalPath, Vec<&CanonicalPath>> = BTreeMap::new();
    let mut out_degree: BTreeMap<&CanonicalPath, usize> = BTreeMap::new();

    for node in &nodes {
        incoming.insert(node, Vec::new());
        out_degree.insert(node, 0);
    }

    for edge in &graph.edges {
        if !is_pagerank_edge(edge) {
            continue;
        }
        if !graph.nodes.contains_key(&edge.from) || !graph.nodes.contains_key(&edge.to) {
            continue;
        }
        // to receives incoming from from
        incoming.entry(&edge.to).or_default().push(&edge.from);
        *out_degree.entry(&edge.from).or_default() += 1;
    }

    // Power iteration
    let mut ranks: BTreeMap<&CanonicalPath, f64> =
        nodes.iter().map(|n| (*n, 1.0 / n_f64)).collect();

    for _ in 0..max_iterations {
        // Dangling nodes: zero out-degree — redistribute rank equally.
        let dangling_sum: f64 = nodes
            .iter()
            .filter(|n| out_degree.get(*n).copied().unwrap_or(0) == 0)
            .map(|n| ranks[n])
            .sum();

        let mut new_ranks: BTreeMap<&CanonicalPath, f64> = BTreeMap::new();
        let mut max_diff: f64 = 0.0;

        for node in &nodes {
            let in_sum: f64 = incoming[node]
                .iter()
                .map(|u| {
                    let od = out_degree.get(u).copied().unwrap_or(1).max(1);
                    ranks[u] / od as f64
                })
                .sum();

            let new_rank = (1.0 - damping) / n_f64 + damping * (dangling_sum / n_f64 + in_sum);
            let diff = (new_rank - ranks[node]).abs();
            if diff > max_diff {
                max_diff = diff;
            }
            new_ranks.insert(node, new_rank);
        }

        ranks = new_ranks;
        if max_diff < tolerance {
            break;
        }
    }

    ranks
        .into_iter()
        .map(|(k, v)| (k.clone(), round4(v)))
        .collect()
}

/// Combined importance: 0.5 * normalized_centrality + 0.5 * normalized_pagerank (D-042).
///
/// Centrality keys are `String` (from `StatsOutput`), PageRank keys are `CanonicalPath`.
pub fn combined_importance(
    centrality: &BTreeMap<String, f64>,
    pr: &BTreeMap<CanonicalPath, f64>,
) -> BTreeMap<CanonicalPath, f64> {
    let max_c = centrality.values().copied().fold(0.0f64, f64::max);
    let max_p = pr.values().copied().fold(0.0f64, f64::max);

    pr.keys()
        .map(|path| {
            let c = centrality.get(path.as_str()).copied().unwrap_or(0.0);
            let p = pr.get(path).copied().unwrap_or(0.0);
            let norm_c = if max_c > 0.0 { c / max_c } else { 0.0 };
            let norm_p = if max_p > 0.0 { p / max_p } else { 0.0 };
            (path.clone(), round4(0.5 * norm_c + 0.5 * norm_p))
        })
        .collect()
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

    fn make_test_edge(from: &str, to: &str) -> Edge {
        Edge {
            from: CanonicalPath::new(from),
            to: CanonicalPath::new(to),
            edge_type: EdgeType::Tests,
            symbols: vec![],
        }
    }

    #[test]
    fn empty_graph_returns_empty() {
        let graph = ProjectGraph {
            nodes: BTreeMap::new(),
            edges: vec![],
        };
        let result = pagerank(&graph, 0.85, 100, 1e-6);
        assert!(result.is_empty());
    }

    #[test]
    fn chain_graph_foundation_has_highest_rank() {
        // A imports B, B imports C → C is most depended-on
        let mut nodes = BTreeMap::new();
        nodes.insert(CanonicalPath::new("a.ts"), make_node("root"));
        nodes.insert(CanonicalPath::new("b.ts"), make_node("root"));
        nodes.insert(CanonicalPath::new("c.ts"), make_node("root"));

        let edges = vec![make_edge("a.ts", "b.ts"), make_edge("b.ts", "c.ts")];

        let graph = ProjectGraph { nodes, edges };
        let result = pagerank(&graph, 0.85, 100, 1e-6);

        let rank_a = result[&CanonicalPath::new("a.ts")];
        let rank_b = result[&CanonicalPath::new("b.ts")];
        let rank_c = result[&CanonicalPath::new("c.ts")];

        assert!(
            rank_c > rank_b,
            "c ({rank_c}) should rank higher than b ({rank_b})"
        );
        assert!(
            rank_b > rank_a,
            "b ({rank_b}) should rank higher than a ({rank_a})"
        );
    }

    #[test]
    fn star_graph_center_has_highest_rank() {
        // A, B, C, D all import E → E has highest PageRank
        let mut nodes = BTreeMap::new();
        for name in ["a.ts", "b.ts", "c.ts", "d.ts", "e.ts"] {
            nodes.insert(CanonicalPath::new(name), make_node("root"));
        }

        let edges = vec![
            make_edge("a.ts", "e.ts"),
            make_edge("b.ts", "e.ts"),
            make_edge("c.ts", "e.ts"),
            make_edge("d.ts", "e.ts"),
        ];

        let graph = ProjectGraph { nodes, edges };
        let result = pagerank(&graph, 0.85, 100, 1e-6);

        let rank_e = result[&CanonicalPath::new("e.ts")];
        for name in ["a.ts", "b.ts", "c.ts", "d.ts"] {
            let rank = result[&CanonicalPath::new(name)];
            assert!(
                rank_e > rank,
                "e ({rank_e}) should rank higher than {name} ({rank})"
            );
        }
    }

    #[test]
    fn ranks_sum_to_approximately_one() {
        let mut nodes = BTreeMap::new();
        nodes.insert(CanonicalPath::new("a.ts"), make_node("root"));
        nodes.insert(CanonicalPath::new("b.ts"), make_node("root"));
        nodes.insert(CanonicalPath::new("c.ts"), make_node("root"));

        let edges = vec![make_edge("a.ts", "b.ts")];
        let graph = ProjectGraph { nodes, edges };
        let result = pagerank(&graph, 0.85, 100, 1e-6);

        let sum: f64 = result.values().sum();
        assert!(
            (sum - 1.0).abs() < 0.01,
            "PageRank sum should be ≈1.0, got {sum}"
        );
    }

    #[test]
    fn disconnected_graph_sums_to_one() {
        let mut nodes = BTreeMap::new();
        nodes.insert(CanonicalPath::new("a.ts"), make_node("c1"));
        nodes.insert(CanonicalPath::new("b.ts"), make_node("c1"));
        nodes.insert(CanonicalPath::new("x.ts"), make_node("c2"));
        nodes.insert(CanonicalPath::new("y.ts"), make_node("c2"));

        let edges = vec![make_edge("a.ts", "b.ts"), make_edge("x.ts", "y.ts")];

        let graph = ProjectGraph { nodes, edges };
        let result = pagerank(&graph, 0.85, 100, 1e-6);

        let sum: f64 = result.values().sum();
        assert!(
            (sum - 1.0).abs() < 0.01,
            "PageRank sum should be ≈1.0, got {sum}"
        );
    }

    #[test]
    fn self_loop_converges() {
        let mut nodes = BTreeMap::new();
        nodes.insert(CanonicalPath::new("a.ts"), make_node("root"));

        let edges = vec![make_edge("a.ts", "a.ts")];
        let graph = ProjectGraph { nodes, edges };
        let result = pagerank(&graph, 0.85, 100, 1e-6);

        assert_eq!(result.len(), 1);
        let rank = result[&CanonicalPath::new("a.ts")];
        assert!(
            (rank - 1.0).abs() < 0.01,
            "Single node should have rank ≈1.0, got {rank}"
        );
    }

    #[test]
    fn test_edges_excluded() {
        let mut nodes = BTreeMap::new();
        nodes.insert(CanonicalPath::new("a.ts"), make_node("root"));
        nodes.insert(CanonicalPath::new("b.ts"), make_node("root"));
        nodes.insert(CanonicalPath::new("c.ts"), make_node("root"));

        let edges = vec![make_test_edge("a.ts", "b.ts"), make_edge("a.ts", "c.ts")];

        let graph = ProjectGraph { nodes, edges };
        let result = pagerank(&graph, 0.85, 100, 1e-6);

        let rank_b = result[&CanonicalPath::new("b.ts")];
        let rank_c = result[&CanonicalPath::new("c.ts")];
        assert!(
            rank_c > rank_b,
            "c ({rank_c}) should rank higher than b ({rank_b}) since test edge excluded"
        );
    }

    #[test]
    fn determinism_across_runs() {
        let mut nodes = BTreeMap::new();
        for name in ["a.ts", "b.ts", "c.ts", "d.ts", "e.ts"] {
            nodes.insert(CanonicalPath::new(name), make_node("root"));
        }
        let edges = vec![
            make_edge("a.ts", "b.ts"),
            make_edge("b.ts", "c.ts"),
            make_edge("c.ts", "d.ts"),
            make_edge("d.ts", "e.ts"),
            make_edge("a.ts", "e.ts"),
        ];

        let graph = ProjectGraph { nodes, edges };

        let first = pagerank(&graph, 0.85, 100, 1e-6);
        for _ in 0..10 {
            let result = pagerank(&graph, 0.85, 100, 1e-6);
            assert_eq!(first, result, "PageRank should be deterministic");
        }
    }

    #[test]
    fn combined_importance_balances_scores() {
        let mut centrality = BTreeMap::new();
        centrality.insert("a.ts".to_string(), 1.0);
        centrality.insert("b.ts".to_string(), 0.0);
        centrality.insert("c.ts".to_string(), 0.5);

        let mut pr = BTreeMap::new();
        pr.insert(CanonicalPath::new("a.ts"), 0.0);
        pr.insert(CanonicalPath::new("b.ts"), 1.0);
        pr.insert(CanonicalPath::new("c.ts"), 0.5);

        let result = combined_importance(&centrality, &pr);

        let score_a = result[&CanonicalPath::new("a.ts")];
        let score_b = result[&CanonicalPath::new("b.ts")];
        let score_c = result[&CanonicalPath::new("c.ts")];

        // a: high centrality, zero pagerank → 0.5
        assert!(
            (score_a - 0.5).abs() < 0.01,
            "a should be ≈0.5, got {score_a}"
        );
        // b: zero centrality, high pagerank → 0.5
        assert!(
            (score_b - 0.5).abs() < 0.01,
            "b should be ≈0.5, got {score_b}"
        );
        // c: both moderate → 0.5
        assert!(
            (score_c - 0.5).abs() < 0.01,
            "c should be ≈0.5, got {score_c}"
        );
    }

    #[test]
    fn combined_importance_max_is_one() {
        let mut centrality = BTreeMap::new();
        centrality.insert("a.ts".to_string(), 1.0);

        let mut pr = BTreeMap::new();
        pr.insert(CanonicalPath::new("a.ts"), 1.0);

        let result = combined_importance(&centrality, &pr);
        let score = result[&CanonicalPath::new("a.ts")];
        assert!(
            (score - 1.0).abs() < 0.001,
            "Max combined score should be 1.0, got {score}"
        );
    }
}
