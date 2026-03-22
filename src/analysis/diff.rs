use std::collections::{BTreeMap, BTreeSet};

use crate::algo::round4;
use crate::analysis::metrics::ClusterMetrics;
use crate::analysis::smells::detect_smells;
use crate::model::*;

/// Compute the structural diff between two graph snapshots.
#[allow(clippy::too_many_arguments)]
pub fn compute_structural_diff(
    old_graph: &ProjectGraph,
    old_stats: &StatsOutput,
    old_clusters: &ClusterMap,
    old_metrics: &BTreeMap<ClusterId, ClusterMetrics>,
    new_graph: &ProjectGraph,
    new_stats: &StatsOutput,
    new_clusters: &ClusterMap,
    new_metrics: &BTreeMap<ClusterId, ClusterMetrics>,
) -> StructuralDiff {
    let added_nodes = find_added_nodes(old_graph, new_graph);
    let removed_nodes = find_removed_nodes(old_graph, new_graph);
    let (added_edges, removed_edges) = diff_edges(old_graph, new_graph);
    let changed_layers = diff_layers(old_graph, new_graph);
    let changed_clusters = diff_clusters_filtered(
        old_graph,
        new_graph,
        old_clusters,
        new_clusters,
        &added_edges,
        &removed_edges,
    );
    let (new_cycles, resolved_cycles) = diff_cycles(&old_stats.sccs, &new_stats.sccs);

    let old_smells = detect_smells(old_graph, old_stats, old_clusters, old_metrics);
    let new_smells_all = detect_smells(new_graph, new_stats, new_clusters, new_metrics);
    let (new_smells, resolved_smells) = diff_smells(&old_smells, &new_smells_all);

    let magnitude = compute_magnitude(
        &added_edges,
        &removed_edges,
        &added_nodes,
        &removed_nodes,
        new_graph,
    );
    let change_type = classify_change(
        &added_nodes,
        &removed_nodes,
        &added_edges,
        &removed_edges,
        &new_cycles,
        magnitude,
    );

    StructuralDiff {
        added_nodes,
        removed_nodes,
        added_edges,
        removed_edges,
        changed_layers,
        changed_clusters,
        new_cycles,
        resolved_cycles,
        new_smells,
        resolved_smells,
        summary: DiffSummary {
            structural_change_magnitude: magnitude,
            change_type,
        },
    }
}

fn find_added_nodes(old: &ProjectGraph, new: &ProjectGraph) -> Vec<CanonicalPath> {
    let mut added: Vec<CanonicalPath> = new
        .nodes
        .keys()
        .filter(|k| !old.nodes.contains_key(*k))
        .cloned()
        .collect();
    added.sort();
    added
}

fn find_removed_nodes(old: &ProjectGraph, new: &ProjectGraph) -> Vec<CanonicalPath> {
    let mut removed: Vec<CanonicalPath> = old
        .nodes
        .keys()
        .filter(|k| !new.nodes.contains_key(*k))
        .cloned()
        .collect();
    removed.sort();
    removed
}

/// Edge comparison key: (from, to, edge_type). Symbols ignored per spec.
type EdgeKey = (String, String, String);

fn edge_key(e: &Edge) -> EdgeKey {
    (
        e.from.as_str().to_string(),
        e.to.as_str().to_string(),
        e.edge_type.as_str().to_string(),
    )
}

fn diff_edges(old: &ProjectGraph, new: &ProjectGraph) -> (Vec<Edge>, Vec<Edge>) {
    let old_keys: BTreeSet<EdgeKey> = old.edges.iter().map(edge_key).collect();
    let new_keys: BTreeSet<EdgeKey> = new.edges.iter().map(edge_key).collect();

    let added: Vec<Edge> = new
        .edges
        .iter()
        .filter(|e| !old_keys.contains(&edge_key(e)))
        .cloned()
        .collect();

    let removed: Vec<Edge> = old
        .edges
        .iter()
        .filter(|e| !new_keys.contains(&edge_key(e)))
        .cloned()
        .collect();

    (added, removed)
}

fn diff_layers(old: &ProjectGraph, new: &ProjectGraph) -> Vec<LayerChange> {
    let mut changes = Vec::new();
    for (path, new_node) in &new.nodes {
        if let Some(old_node) = old.nodes.get(path) {
            if old_node.arch_depth != new_node.arch_depth {
                changes.push(LayerChange {
                    file: path.clone(),
                    old_depth: old_node.arch_depth,
                    new_depth: new_node.arch_depth,
                });
            }
        }
    }
    changes.sort_by(|a, b| a.file.cmp(&b.file));
    changes
}

/// Filter cluster changes by edge correlation (Louvain noise filtering, D-057).
fn diff_clusters_filtered(
    old_graph: &ProjectGraph,
    new_graph: &ProjectGraph,
    old_clusters: &ClusterMap,
    new_clusters: &ClusterMap,
    added_edges: &[Edge],
    removed_edges: &[Edge],
) -> Vec<ClusterChange> {
    // Collect files with edge changes
    let mut files_with_edge_changes: BTreeSet<CanonicalPath> = BTreeSet::new();
    for e in added_edges {
        files_with_edge_changes.insert(e.from.clone());
        files_with_edge_changes.insert(e.to.clone());
    }
    for e in removed_edges {
        files_with_edge_changes.insert(e.from.clone());
        files_with_edge_changes.insert(e.to.clone());
    }

    // Build path→cluster maps from ClusterMap
    let old_file_clusters = build_file_cluster_map(old_clusters);
    let new_file_clusters = build_file_cluster_map(new_clusters);

    let mut changes = Vec::new();
    // Check files that exist in both graphs
    for path in new_graph.nodes.keys() {
        if !old_graph.nodes.contains_key(path) {
            continue;
        }
        let old_cluster = old_file_clusters.get(path);
        let new_cluster = new_file_clusters.get(path);

        if let (Some(oc), Some(nc)) = (old_cluster, new_cluster) {
            if oc != nc && files_with_edge_changes.contains(path) {
                changes.push(ClusterChange {
                    file: path.clone(),
                    old_cluster: oc.clone(),
                    new_cluster: nc.clone(),
                });
            }
        }
    }
    changes.sort_by(|a, b| a.file.cmp(&b.file));
    changes
}

fn build_file_cluster_map(clusters: &ClusterMap) -> BTreeMap<CanonicalPath, ClusterId> {
    let mut map = BTreeMap::new();
    for (id, cluster) in &clusters.clusters {
        for file in &cluster.files {
            map.insert(file.clone(), id.clone());
        }
    }
    map
}

/// Compare SCCs as sorted sets.
fn diff_cycles(
    old_sccs: &[Vec<String>],
    new_sccs: &[Vec<String>],
) -> (Vec<Vec<CanonicalPath>>, Vec<Vec<CanonicalPath>>) {
    let normalize = |sccs: &[Vec<String>]| -> BTreeSet<Vec<String>> {
        sccs.iter()
            .filter(|scc| scc.len() > 1)
            .map(|scc| {
                let mut sorted = scc.clone();
                sorted.sort();
                sorted
            })
            .collect()
    };

    let old_set = normalize(old_sccs);
    let new_set = normalize(new_sccs);

    let new_cycles: Vec<Vec<CanonicalPath>> = new_set
        .difference(&old_set)
        .map(|scc| scc.iter().map(CanonicalPath::new).collect())
        .collect();

    let resolved_cycles: Vec<Vec<CanonicalPath>> = old_set
        .difference(&new_set)
        .map(|scc| scc.iter().map(CanonicalPath::new).collect())
        .collect();

    (new_cycles, resolved_cycles)
}

/// Compare smells by (smell_type, sorted files) tuple.
fn diff_smells(
    old_smells: &[ArchSmell],
    new_smells: &[ArchSmell],
) -> (Vec<ArchSmell>, Vec<ArchSmell>) {
    type SmellKey = (String, Vec<String>);

    let key = |s: &ArchSmell| -> SmellKey {
        let mut files: Vec<String> = s.files.iter().map(|f| f.as_str().to_string()).collect();
        files.sort();
        (format!("{:?}", s.smell_type), files)
    };

    let old_keys: BTreeSet<SmellKey> = old_smells.iter().map(&key).collect();
    let new_keys: BTreeSet<SmellKey> = new_smells.iter().map(&key).collect();

    let new_only: Vec<ArchSmell> = new_smells
        .iter()
        .filter(|s| !old_keys.contains(&key(s)))
        .cloned()
        .collect();

    let resolved: Vec<ArchSmell> = old_smells
        .iter()
        .filter(|s| !new_keys.contains(&key(s)))
        .cloned()
        .collect();

    (new_only, resolved)
}

fn compute_magnitude(
    added_edges: &[Edge],
    removed_edges: &[Edge],
    added_nodes: &[CanonicalPath],
    removed_nodes: &[CanonicalPath],
    new_graph: &ProjectGraph,
) -> f64 {
    let total = new_graph.edges.len() + new_graph.nodes.len();
    if total == 0 {
        return 0.0;
    }
    let changes = added_edges.len() + removed_edges.len() + added_nodes.len() + removed_nodes.len();
    round4(changes as f64 / (2.0 * total as f64))
}

fn classify_change(
    added_nodes: &[CanonicalPath],
    removed_nodes: &[CanonicalPath],
    _added_edges: &[Edge],
    removed_edges: &[Edge],
    new_cycles: &[Vec<CanonicalPath>],
    magnitude: f64,
) -> ChangeClassification {
    if !removed_edges.is_empty() && !new_cycles.is_empty() {
        return ChangeClassification::Breaking;
    }

    if !added_nodes.is_empty() && removed_nodes.is_empty() && removed_edges.is_empty() {
        return ChangeClassification::Additive;
    }

    let added_n = added_nodes.len() as f64;
    let removed_n = removed_nodes.len() as f64;
    let max_n = added_n.max(removed_n);
    let diff_n = (added_n - removed_n).abs();
    let threshold = (0.2 * max_n).max(1.0);

    if diff_n <= threshold && magnitude < 0.3 {
        return ChangeClassification::Refactor;
    }

    if removed_nodes.len() > added_nodes.len() && magnitude > 0.1 {
        return ChangeClassification::Migration;
    }

    ChangeClassification::Refactor
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::metrics::compute_martin_metrics;

    fn make_node(cluster: &ClusterId, depth: u32) -> Node {
        Node {
            file_type: FileType::Source,
            layer: ArchLayer::Unknown,
            fsd_layer: None,
            arch_depth: depth,
            lines: 50,
            hash: ContentHash::new("0000000000000000".to_string()),
            exports: vec![],
            cluster: cluster.clone(),
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

    fn make_stats(sccs: Vec<Vec<&str>>) -> StatsOutput {
        StatsOutput {
            version: 1,
            centrality: BTreeMap::new(),
            sccs: sccs
                .into_iter()
                .map(|scc| scc.into_iter().map(|s| s.to_string()).collect())
                .collect(),
            layers: BTreeMap::new(),
            summary: StatsSummary {
                max_depth: 3,
                avg_in_degree: 0.0,
                avg_out_degree: 0.0,
                bottleneck_files: vec![],
                orphan_files: vec![],
            },
        }
    }

    fn make_clusters(files: &[(&str, &str)]) -> ClusterMap {
        let mut clusters: BTreeMap<ClusterId, Vec<CanonicalPath>> = BTreeMap::new();
        for (file, cluster) in files {
            clusters
                .entry(ClusterId::new(*cluster))
                .or_default()
                .push(CanonicalPath::new(*file));
        }
        ClusterMap {
            clusters: clusters
                .into_iter()
                .map(|(id, files)| {
                    let file_count = files.len();
                    (
                        id,
                        Cluster {
                            files,
                            file_count,
                            internal_edges: 0,
                            external_edges: 0,
                            cohesion: 0.0,
                        },
                    )
                })
                .collect(),
        }
    }

    fn make_graph(
        nodes: &[(&str, &str, u32)], // (path, cluster, depth)
        edges: &[(&str, &str)],
    ) -> ProjectGraph {
        let mut graph_nodes = BTreeMap::new();
        for &(path, cluster, depth) in nodes {
            graph_nodes.insert(
                CanonicalPath::new(path),
                make_node(&ClusterId::new(cluster), depth),
            );
        }
        ProjectGraph {
            nodes: graph_nodes,
            edges: edges.iter().map(|(f, t)| make_edge(f, t)).collect(),
        }
    }

    #[test]
    fn additive_change() {
        let old = make_graph(&[("a.ts", "c1", 0)], &[]);
        let new = make_graph(
            &[("a.ts", "c1", 0), ("b.ts", "c1", 0), ("c.ts", "c1", 0)],
            &[("a.ts", "b.ts")],
        );
        let old_stats = make_stats(vec![]);
        let new_stats = make_stats(vec![]);
        let old_cl = make_clusters(&[("a.ts", "c1")]);
        let new_cl = make_clusters(&[("a.ts", "c1"), ("b.ts", "c1"), ("c.ts", "c1")]);
        let old_m = compute_martin_metrics(&old, &old_cl);
        let new_m = compute_martin_metrics(&new, &new_cl);

        let diff = compute_structural_diff(
            &old, &old_stats, &old_cl, &old_m, &new, &new_stats, &new_cl, &new_m,
        );
        assert_eq!(diff.added_nodes.len(), 2);
        assert_eq!(diff.removed_nodes.len(), 0);
        assert_eq!(diff.summary.change_type, ChangeClassification::Additive);
    }

    #[test]
    fn breaking_change_with_removed_edge() {
        let old = make_graph(
            &[("a.ts", "c1", 0), ("b.ts", "c1", 0), ("c.ts", "c1", 0)],
            &[("a.ts", "b.ts"), ("b.ts", "c.ts"), ("a.ts", "c.ts")],
        );
        let new = make_graph(
            &[("a.ts", "c1", 0), ("b.ts", "c1", 0), ("c.ts", "c1", 0)],
            &[("a.ts", "b.ts"), ("b.ts", "c.ts"), ("c.ts", "a.ts")],
        );
        let old_stats = make_stats(vec![]);
        let new_stats = make_stats(vec![vec!["a.ts", "b.ts", "c.ts"]]);
        let old_cl = make_clusters(&[("a.ts", "c1"), ("b.ts", "c1"), ("c.ts", "c1")]);
        let new_cl = old_cl.clone();
        let old_m = compute_martin_metrics(&old, &old_cl);
        let new_m = compute_martin_metrics(&new, &new_cl);

        let diff = compute_structural_diff(
            &old, &old_stats, &old_cl, &old_m, &new, &new_stats, &new_cl, &new_m,
        );
        assert!(!diff.new_cycles.is_empty());
        assert!(!diff.removed_edges.is_empty()); // a→c removed
        assert_eq!(diff.summary.change_type, ChangeClassification::Breaking);
    }

    #[test]
    fn refactor() {
        // Roughly equal add/remove, small magnitude
        let old = make_graph(&[("a.ts", "c1", 0), ("b.ts", "c1", 0)], &[("a.ts", "b.ts")]);
        let new = make_graph(
            &[("a2.ts", "c1", 0), ("b.ts", "c1", 0)],
            &[("a2.ts", "b.ts")],
        );
        let old_stats = make_stats(vec![]);
        let new_stats = make_stats(vec![]);
        let old_cl = make_clusters(&[("a.ts", "c1"), ("b.ts", "c1")]);
        let new_cl = make_clusters(&[("a2.ts", "c1"), ("b.ts", "c1")]);
        let old_m = compute_martin_metrics(&old, &old_cl);
        let new_m = compute_martin_metrics(&new, &new_cl);

        let diff = compute_structural_diff(
            &old, &old_stats, &old_cl, &old_m, &new, &new_stats, &new_cl, &new_m,
        );
        assert_eq!(diff.added_nodes.len(), 1); // a2.ts
        assert_eq!(diff.removed_nodes.len(), 1); // a.ts
        assert_eq!(diff.summary.change_type, ChangeClassification::Refactor);
    }

    #[test]
    fn migration() {
        // More removed than added, magnitude > 0.1
        let old = make_graph(
            &[
                ("a.ts", "c1", 0),
                ("b.ts", "c1", 0),
                ("c.ts", "c1", 0),
                ("d.ts", "c1", 0),
                ("e.ts", "c1", 0),
            ],
            &[
                ("a.ts", "b.ts"),
                ("b.ts", "c.ts"),
                ("c.ts", "d.ts"),
                ("d.ts", "e.ts"),
            ],
        );
        let new = make_graph(&[("a.ts", "c1", 0), ("b.ts", "c1", 0)], &[("a.ts", "b.ts")]);
        let old_stats = make_stats(vec![]);
        let new_stats = make_stats(vec![]);
        let old_cl = make_clusters(&[
            ("a.ts", "c1"),
            ("b.ts", "c1"),
            ("c.ts", "c1"),
            ("d.ts", "c1"),
            ("e.ts", "c1"),
        ]);
        let new_cl = make_clusters(&[("a.ts", "c1"), ("b.ts", "c1")]);
        let old_m = compute_martin_metrics(&old, &old_cl);
        let new_m = compute_martin_metrics(&new, &new_cl);

        let diff = compute_structural_diff(
            &old, &old_stats, &old_cl, &old_m, &new, &new_stats, &new_cl, &new_m,
        );
        assert!(diff.removed_nodes.len() > diff.added_nodes.len());
        assert_eq!(diff.summary.change_type, ChangeClassification::Migration);
    }

    #[test]
    fn louvain_noise_filtered() {
        // Same edges, but Louvain changed cluster assignment
        let old = make_graph(&[("a.ts", "c1", 0), ("b.ts", "c1", 0)], &[("a.ts", "b.ts")]);
        // Same graph, same edges — only cluster assignment changed
        let new = make_graph(&[("a.ts", "c2", 0), ("b.ts", "c1", 0)], &[("a.ts", "b.ts")]);
        let old_stats = make_stats(vec![]);
        let new_stats = make_stats(vec![]);
        let old_cl = make_clusters(&[("a.ts", "c1"), ("b.ts", "c1")]);
        let new_cl = make_clusters(&[("a.ts", "c2"), ("b.ts", "c1")]);
        let old_m = compute_martin_metrics(&old, &old_cl);
        let new_m = compute_martin_metrics(&new, &new_cl);

        let diff = compute_structural_diff(
            &old, &old_stats, &old_cl, &old_m, &new, &new_stats, &new_cl, &new_m,
        );
        // No edge changes → cluster change filtered out
        assert!(
            diff.changed_clusters.is_empty(),
            "Louvain noise should be filtered: {:?}",
            diff.changed_clusters
        );
    }

    #[test]
    fn louvain_real_change() {
        // Edge changes AND cluster reassignment → included
        let old = make_graph(
            &[("a.ts", "c1", 0), ("b.ts", "c1", 0), ("c.ts", "c2", 0)],
            &[("a.ts", "b.ts")],
        );
        let new = make_graph(
            &[("a.ts", "c2", 0), ("b.ts", "c1", 0), ("c.ts", "c2", 0)],
            &[("a.ts", "c.ts")], // edge changed
        );
        let old_stats = make_stats(vec![]);
        let new_stats = make_stats(vec![]);
        let old_cl = make_clusters(&[("a.ts", "c1"), ("b.ts", "c1"), ("c.ts", "c2")]);
        let new_cl = make_clusters(&[("a.ts", "c2"), ("b.ts", "c1"), ("c.ts", "c2")]);
        let old_m = compute_martin_metrics(&old, &old_cl);
        let new_m = compute_martin_metrics(&new, &new_cl);

        let diff = compute_structural_diff(
            &old, &old_stats, &old_cl, &old_m, &new, &new_stats, &new_cl, &new_m,
        );
        assert_eq!(diff.changed_clusters.len(), 1);
        assert_eq!(diff.changed_clusters[0].file, CanonicalPath::new("a.ts"));
    }

    #[test]
    fn cycle_diff() {
        let old_stats = make_stats(vec![vec!["a.ts", "b.ts"]]);
        let new_stats = make_stats(vec![vec!["c.ts", "d.ts"]]);

        let (new_cycles, resolved_cycles) = diff_cycles(&old_stats.sccs, &new_stats.sccs);
        assert_eq!(new_cycles.len(), 1);
        assert_eq!(resolved_cycles.len(), 1);
    }

    #[test]
    fn magnitude_calculation() {
        // 3 nodes, 2 edges in new. 1 added node, 1 added edge.
        // magnitude = (1+1) / (2 * (3+2)) = 2/10 = 0.2
        let old = make_graph(&[("a.ts", "c1", 0), ("b.ts", "c1", 0)], &[("a.ts", "b.ts")]);
        let new = make_graph(
            &[("a.ts", "c1", 0), ("b.ts", "c1", 0), ("c.ts", "c1", 0)],
            &[("a.ts", "b.ts"), ("b.ts", "c.ts")],
        );
        let (added_e, removed_e) = diff_edges(&old, &new);
        let added_n = find_added_nodes(&old, &new);
        let removed_n = find_removed_nodes(&old, &new);
        let mag = compute_magnitude(&added_e, &removed_e, &added_n, &removed_n, &new);
        assert_eq!(mag, 0.2);
    }

    #[test]
    fn empty_diff() {
        let graph = make_graph(&[("a.ts", "c1", 0)], &[]);
        let stats = make_stats(vec![]);
        let cl = make_clusters(&[("a.ts", "c1")]);
        let m = compute_martin_metrics(&graph, &cl);

        let diff = compute_structural_diff(&graph, &stats, &cl, &m, &graph, &stats, &cl, &m);
        assert_eq!(diff.summary.structural_change_magnitude, 0.0);
        assert!(diff.added_nodes.is_empty());
        assert!(diff.removed_nodes.is_empty());
        assert!(diff.added_edges.is_empty());
        assert!(diff.removed_edges.is_empty());
    }
}
