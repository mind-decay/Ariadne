use std::collections::{BTreeMap, BTreeSet};

use crate::algo::blast_radius::blast_radius;
use crate::algo::context::estimate_tokens;
use crate::algo::test_map::{find_tests_for, TestHit};
use crate::algo::AdjacencyIndex;
use crate::model::{CanonicalPath, ClusterMap, ProjectGraph, StatsOutput};

/// Classification of a change's structural impact.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChangeClass {
    /// Change only adds new files/edges, no existing code affected beyond the changed files.
    Additive,
    /// Change modifies existing code with bounded impact.
    Modification,
    /// Change crosses architectural layers or has wide blast radius.
    Structural,
}

impl ChangeClass {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Additive => "additive",
            Self::Modification => "modification",
            Self::Structural => "structural",
        }
    }
}

/// Result of impact analysis for a set of changed files.
#[derive(Clone, Debug)]
pub struct ImpactResult {
    pub total_affected: usize,
    pub affected_files: BTreeMap<CanonicalPath, u32>,
    pub affected_tests: Vec<TestHit>,
    pub layers_crossed: u32,
    pub layer_direction: String,
    pub clusters_affected: usize,
    pub risks: Vec<String>,
    pub change_class: ChangeClass,
    pub token_estimate: u32,
    pub warnings: Vec<String>,
}

/// Analyze the impact of a set of changed files on the project graph.
///
/// 1. Compute blast_radius per changed file (max_depth Some(3)), merge keeping min distance
/// 2. Find tests via find_tests_for on changed files
/// 3. Layer crossing: min/max arch_depth of all affected
/// 4. Risks: high centrality (>0.5), crosses >1 layer, >2 clusters, no tests
/// 5. ChangeClass: Additive if affected == changes.len(), Structural if layers>1 or affected>20, else Modification
/// 6. Token estimate: sum estimate_tokens for affected files
/// 7. Warning if change path not in graph, return partial results
pub fn analyze_impact(
    changes: &[CanonicalPath],
    graph: &ProjectGraph,
    index: &AdjacencyIndex,
    stats: &StatsOutput,
    _clusters: &ClusterMap,
) -> ImpactResult {
    let mut warnings = Vec::new();
    let mut affected_files: BTreeMap<CanonicalPath, u32> = BTreeMap::new();

    // Check for changes not in graph
    for change in changes {
        if !graph.nodes.contains_key(change) {
            warnings.push(format!("Changed file not in graph: {}", change));
        }
    }

    // 1. Compute blast radius per changed file, merge keeping min distance
    for change in changes {
        let radius = blast_radius(graph, change, Some(3), index);
        for (path, dist) in radius {
            let entry = affected_files.entry(path).or_insert(u32::MAX);
            if dist < *entry {
                *entry = dist;
            }
        }
    }

    let total_affected = affected_files.len();

    // 2. Find tests via find_tests_for
    let test_result = find_tests_for(changes, graph, index);
    let affected_tests = test_result.tests;

    // 3. Layer crossing: min/max arch_depth of all affected files
    let mut min_depth = u32::MAX;
    let mut max_depth = 0u32;
    for path in affected_files.keys() {
        if let Some(node) = graph.nodes.get(path) {
            min_depth = min_depth.min(node.arch_depth);
            max_depth = max_depth.max(node.arch_depth);
        }
    }
    if min_depth == u32::MAX {
        min_depth = 0;
    }
    let layers_crossed = max_depth.saturating_sub(min_depth);
    let layer_direction = if layers_crossed == 0 {
        "none".to_string()
    } else if max_depth > min_depth {
        "upward".to_string()
    } else {
        "none".to_string()
    };

    // 4. Clusters affected
    let affected_clusters: BTreeSet<String> = affected_files
        .keys()
        .filter_map(|p| graph.nodes.get(p))
        .map(|n| n.cluster.as_str().to_string())
        .collect();
    let clusters_affected = affected_clusters.len();

    // 5. Risks
    let mut risks = Vec::new();

    // High centrality files
    for path in affected_files.keys() {
        let centrality = stats
            .centrality
            .get(path.as_str())
            .copied()
            .unwrap_or(0.0);
        if centrality > 0.5 {
            risks.push(format!("High centrality file affected: {} ({:.2})", path, centrality));
        }
    }

    if layers_crossed > 1 {
        risks.push(format!("Crosses {} architectural layers", layers_crossed));
    }

    if clusters_affected > 2 {
        risks.push(format!("Affects {} clusters", clusters_affected));
    }

    if affected_tests.is_empty() && !changes.is_empty() {
        risks.push("No test coverage detected for changed files".to_string());
    }

    // 6. Change classification
    let change_class = if total_affected <= changes.len() {
        ChangeClass::Additive
    } else if layers_crossed > 1 || total_affected > 20 {
        ChangeClass::Structural
    } else {
        ChangeClass::Modification
    };

    // 7. Token estimate: sum estimate_tokens for affected files
    let token_estimate: u32 = affected_files
        .keys()
        .filter_map(|p| graph.nodes.get(p))
        .map(estimate_tokens)
        .sum();

    ImpactResult {
        total_affected,
        affected_files,
        affected_tests,
        layers_crossed,
        layer_direction,
        clusters_affected,
        risks,
        change_class,
        token_estimate,
        warnings,
    }
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

    fn make_stats(centrality: &[(&str, f64)]) -> StatsOutput {
        StatsOutput {
            version: 1,
            centrality: centrality
                .iter()
                .map(|(k, v)| (k.to_string(), *v))
                .collect(),
            sccs: vec![],
            layers: BTreeMap::new(),
            summary: StatsSummary {
                max_depth: 0,
                avg_in_degree: 0.0,
                avg_out_degree: 0.0,
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

    #[test]
    fn single_change_blast_radius() {
        // a -> b -> c: changing c affects b (depth 1) and a (depth 2) via reverse edges
        let graph = make_graph(&["a", "b", "c"], &[("a", "b"), ("b", "c")]);
        let index = AdjacencyIndex::build(&graph.edges, is_architectural);
        let stats = make_stats(&[]);

        let result = analyze_impact(
            &[CanonicalPath::new("c")],
            &graph,
            &index,
            &stats,
            &empty_clusters(),
        );

        assert_eq!(result.total_affected, 3); // c(0), b(1), a(2)
        assert_eq!(result.affected_files[&CanonicalPath::new("c")], 0);
        assert_eq!(result.affected_files[&CanonicalPath::new("b")], 1);
        assert_eq!(result.affected_files[&CanonicalPath::new("a")], 2);
    }

    #[test]
    fn layer_crossing_detection() {
        let mut graph = make_graph(&["a", "b", "c"], &[("a", "b"), ("b", "c")]);
        // Set different arch_depths
        graph.nodes.get_mut(&CanonicalPath::new("a")).unwrap().arch_depth = 0;
        graph.nodes.get_mut(&CanonicalPath::new("b")).unwrap().arch_depth = 1;
        graph.nodes.get_mut(&CanonicalPath::new("c")).unwrap().arch_depth = 3;

        let index = AdjacencyIndex::build(&graph.edges, is_architectural);
        let stats = make_stats(&[]);

        let result = analyze_impact(
            &[CanonicalPath::new("c")],
            &graph,
            &index,
            &stats,
            &empty_clusters(),
        );

        assert_eq!(result.layers_crossed, 3); // max(3) - min(0) = 3
        assert!(result.risks.iter().any(|r| r.contains("layer")));
    }

    #[test]
    fn risk_high_centrality() {
        let graph = make_graph(&["a", "b"], &[("a", "b")]);
        let index = AdjacencyIndex::build(&graph.edges, is_architectural);
        let stats = make_stats(&[("b", 0.8)]);

        let result = analyze_impact(
            &[CanonicalPath::new("b")],
            &graph,
            &index,
            &stats,
            &empty_clusters(),
        );

        assert!(result.risks.iter().any(|r| r.contains("centrality")));
    }

    #[test]
    fn classification_additive() {
        // Single file, no dependents -> affected == changes.len()
        let graph = make_graph(&["a"], &[]);
        let index = AdjacencyIndex::build(&graph.edges, is_architectural);
        let stats = make_stats(&[]);

        let result = analyze_impact(
            &[CanonicalPath::new("a")],
            &graph,
            &index,
            &stats,
            &empty_clusters(),
        );

        assert_eq!(result.change_class, ChangeClass::Additive);
    }

    #[test]
    fn classification_structural() {
        // Create a graph with many dependents to trigger structural classification
        let mut node_names: Vec<String> = vec!["core".to_string()];
        let mut edge_pairs: Vec<(String, String)> = Vec::new();
        for i in 0..25 {
            let name = format!("dep_{}", i);
            edge_pairs.push((name.clone(), "core".to_string()));
            node_names.push(name);
        }

        let node_refs: Vec<&str> = node_names.iter().map(|s| s.as_str()).collect();
        let edge_refs: Vec<(&str, &str)> = edge_pairs
            .iter()
            .map(|(a, b)| (a.as_str(), b.as_str()))
            .collect();

        let graph = make_graph(&node_refs, &edge_refs);
        let index = AdjacencyIndex::build(&graph.edges, is_architectural);
        let stats = make_stats(&[]);

        let result = analyze_impact(
            &[CanonicalPath::new("core")],
            &graph,
            &index,
            &stats,
            &empty_clusters(),
        );

        assert_eq!(result.change_class, ChangeClass::Structural);
        assert!(result.total_affected > 20);
    }

    #[test]
    fn multiple_changes_union() {
        // a -> c, b -> c: changing both a and b
        let graph = make_graph(&["a", "b", "c"], &[("a", "c"), ("b", "c")]);
        let index = AdjacencyIndex::build(&graph.edges, is_architectural);
        let stats = make_stats(&[]);

        let result = analyze_impact(
            &[CanonicalPath::new("a"), CanonicalPath::new("b")],
            &graph,
            &index,
            &stats,
            &empty_clusters(),
        );

        // Both a and b are affected at distance 0, no reverse deps
        assert!(result.affected_files.contains_key(&CanonicalPath::new("a")));
        assert!(result.affected_files.contains_key(&CanonicalPath::new("b")));
        assert_eq!(result.affected_files[&CanonicalPath::new("a")], 0);
        assert_eq!(result.affected_files[&CanonicalPath::new("b")], 0);
    }

    #[test]
    fn empty_changes() {
        let graph = make_graph(&["a"], &[]);
        let index = AdjacencyIndex::build(&graph.edges, is_architectural);
        let stats = make_stats(&[]);

        let result = analyze_impact(
            &[],
            &graph,
            &index,
            &stats,
            &empty_clusters(),
        );

        assert_eq!(result.total_affected, 0);
        assert!(result.affected_files.is_empty());
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn test_detection() {
        let mut graph = make_graph(
            &["src/lib.rs", "tests/lib_test.rs"],
            &[("tests/lib_test.rs", "src/lib.rs")],
        );
        graph
            .nodes
            .get_mut(&CanonicalPath::new("tests/lib_test.rs"))
            .unwrap()
            .file_type = FileType::Test;

        // Use a filter that includes all edges for test detection
        let index = AdjacencyIndex::build(&graph.edges, |_| true);
        let stats = make_stats(&[]);

        let result = analyze_impact(
            &[CanonicalPath::new("src/lib.rs")],
            &graph,
            &index,
            &stats,
            &empty_clusters(),
        );

        assert!(!result.affected_tests.is_empty());
        assert!(result
            .affected_tests
            .iter()
            .any(|t| t.path.as_str() == "tests/lib_test.rs"));
    }
}
