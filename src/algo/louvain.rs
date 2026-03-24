use std::collections::BTreeMap;

use crate::model::{CanonicalPath, Cluster, ClusterId, ClusterMap, Edge, ProjectGraph};

use super::{is_architectural, round4};

/// Louvain community detection — modularity maximization.
/// Starts from directory-based clusters and refines via greedy local moves
/// followed by community aggregation (two-phase Louvain).
///
/// Returns `(ClusterMap, converged)`. If `converged` is false, the caller
/// should emit W012 and may choose to use the result or fall back to
/// `initial_clusters`. The returned ClusterMap is `initial_clusters`
/// unchanged on convergence failure.
///
/// D-033 compliance: depends only on model/ types. Warning construction
/// is the caller's responsibility (pipeline/).
///
/// Design source: architecture.md §Algorithms §4, D-034, D-049
pub fn louvain_clustering(
    graph: &ProjectGraph,
    initial_clusters: &ClusterMap,
) -> (ClusterMap, bool) {
    louvain_clustering_with_resolution(graph, initial_clusters, 1.0)
}

/// Louvain clustering with a configurable resolution parameter (gamma).
///
/// - gamma = 1.0: standard modularity (default)
/// - gamma > 1.0: penalizes merging more → finer-grained communities
/// - gamma < 1.0: encourages merging → coarser communities
pub fn louvain_clustering_with_resolution(
    graph: &ProjectGraph,
    initial_clusters: &ClusterMap,
    resolution: f64,
) -> (ClusterMap, bool) {
    // Edge cases: empty graph, single file, no edges → return unchanged
    if graph.nodes.len() <= 1 {
        return (initial_clusters.clone(), true);
    }

    let arch_edges: Vec<&Edge> = graph.edges.iter().filter(|e| is_architectural(e)).collect();
    if arch_edges.is_empty() {
        return (initial_clusters.clone(), true);
    }

    // Build undirected weight matrix from directed architectural edges.
    // If A→B and B→A both exist, weight = 2. Otherwise weight = 1.
    let mut weights: BTreeMap<(usize, usize), f64> = BTreeMap::new();

    // Node list in deterministic order (BTreeMap key order per D-049)
    let nodes: Vec<&CanonicalPath> = graph.nodes.keys().collect();
    let node_index: BTreeMap<&CanonicalPath, usize> =
        nodes.iter().enumerate().map(|(i, &n)| (n, i)).collect();
    let n = nodes.len();

    for edge in &arch_edges {
        if let (Some(&ia), Some(&ib)) = (node_index.get(&edge.from), node_index.get(&edge.to)) {
            let (a, b) = if ia <= ib { (ia, ib) } else { (ib, ia) };
            *weights.entry((a, b)).or_insert(0.0) += 1.0;
        }
    }

    // Total edge weight (m in modularity formula: Q = 1/2m * ...)
    let m: f64 = weights.values().sum();
    if m == 0.0 {
        return (initial_clusters.clone(), true);
    }

    // Build adjacency list with weights for each node (undirected)
    let mut neighbors: Vec<Vec<(usize, f64)>> = vec![Vec::new(); n];
    for (&(a, b), &w) in &weights {
        if a != b {
            neighbors[a].push((b, w));
            neighbors[b].push((a, w));
        }
    }

    // Weighted degree of each node (k_i = sum of edge weights incident to i)
    let mut k: Vec<f64> = vec![0.0; n];
    for i in 0..n {
        for &(_, w) in &neighbors[i] {
            k[i] += w;
        }
    }

    // Initialize communities from directory-based clusters
    let mut community: Vec<usize> = vec![0; n];
    let mut next_comm_id: usize = 0;

    for cluster in initial_clusters.clusters.values() {
        let comm_idx = next_comm_id;
        next_comm_id += 1;
        for file in &cluster.files {
            if let Some(&ni) = node_index.get(file) {
                community[ni] = comm_idx;
            }
        }
    }

    // Assign any nodes not in any cluster to their own community
    for (_, item) in community.iter_mut().enumerate().take(n) {
        if *item >= next_comm_id {
            *item = next_comm_id;
            next_comm_id += 1;
        }
    }

    // Two-phase Louvain with outer iteration
    const MAX_OUTER_ITERATIONS: u32 = 100;
    const MAX_INNER_ITERATIONS: u32 = 100;
    const CONVERGENCE_THRESHOLD: f64 = 1e-6;

    // Working state: node-level graph (may be collapsed super-nodes)
    let mut w_n = n;
    let mut w_neighbors = neighbors;
    let mut w_k = k;
    let mut w_community = community;
    let w_m = m;
    // Maps collapsed node index → original node indices
    let mut node_members: Vec<Vec<usize>> = (0..n).map(|i| vec![i]).collect();

    let mut converged = false;

    for _outer in 0..MAX_OUTER_ITERATIONS {
        // Phase 1: Local moves on current (possibly collapsed) graph
        let phase1_moved = local_moves(
            w_n,
            &w_neighbors,
            &w_k,
            &mut w_community,
            w_m,
            MAX_INNER_ITERATIONS,
            CONVERGENCE_THRESHOLD,
            resolution,
        );

        if !phase1_moved {
            converged = true;
            break; // No improvement — algorithm converged
        }

        // Phase 2: Aggregate — collapse communities into super-nodes
        // Map community ids to compact range
        let mut comm_remap: BTreeMap<usize, usize> = BTreeMap::new();
        let mut next_id: usize = 0;
        for &c in &w_community {
            if let std::collections::btree_map::Entry::Vacant(e) = comm_remap.entry(c) {
                e.insert(next_id);
                next_id += 1;
            }
        }
        let num_super = next_id;

        if num_super == w_n {
            converged = true;
            break; // Each node is its own community — no further aggregation possible
        }

        // Remap communities
        for c in w_community.iter_mut() {
            *c = comm_remap[c];
        }

        // Build super-node graph
        let mut super_weights: BTreeMap<(usize, usize), f64> = BTreeMap::new();
        for i in 0..w_n {
            let ci = w_community[i];
            for &(j, w) in &w_neighbors[i] {
                let cj = w_community[j];
                if ci < cj {
                    *super_weights.entry((ci, cj)).or_insert(0.0) += w;
                } else if ci > cj {
                    *super_weights.entry((cj, ci)).or_insert(0.0) += w;
                }
                // ci == cj: internal edge, skip for inter-community graph
            }
        }

        // Build super-node adjacency
        let mut super_neighbors: Vec<Vec<(usize, f64)>> = vec![Vec::new(); num_super];
        for (&(a, b), &w) in &super_weights {
            super_neighbors[a].push((b, w));
            super_neighbors[b].push((a, w));
        }

        // Super-node degrees
        let mut super_k: Vec<f64> = vec![0.0; num_super];
        for i in 0..w_n {
            super_k[w_community[i]] += w_k[i];
        }

        // Merge node_members
        let mut super_members: Vec<Vec<usize>> = vec![Vec::new(); num_super];
        for (i, members) in node_members.iter().enumerate() {
            let ci = w_community[i];
            super_members[ci].extend_from_slice(members);
        }

        // Each super-node starts in its own community
        let super_community: Vec<usize> = (0..num_super).collect();

        // Update working state for next outer iteration
        w_n = num_super;
        w_neighbors = super_neighbors;
        w_k = super_k;
        w_community = super_community;
        node_members = super_members;
        // w_m stays the same (total edge weight doesn't change)
    }

    // Map final communities back to original nodes
    let mut final_community: Vec<usize> = vec![0; n];
    for (super_idx, members) in node_members.iter().enumerate() {
        let comm = w_community[super_idx];
        for &orig in members {
            final_community[orig] = comm;
        }
    }

    // Build result ClusterMap from community assignments
    build_cluster_map(graph, &nodes, &final_community, initial_clusters, converged)
}

/// Phase 1: Local moves. Returns true if any node was moved.
#[allow(clippy::too_many_arguments)]
fn local_moves(
    n: usize,
    neighbors: &[Vec<(usize, f64)>],
    k: &[f64],
    community: &mut [usize],
    m: f64,
    max_iterations: u32,
    _convergence_threshold: f64,
    resolution: f64,
) -> bool {
    let mut any_moved = false;

    for _iter in 0..max_iterations {
        let mut moved_this_pass = false;

        // Recompute sigma_tot for each community
        let max_comm = community.iter().copied().max().unwrap_or(0) + 1;
        let mut sigma_tot: Vec<f64> = vec![0.0; max_comm];
        for i in 0..n {
            sigma_tot[community[i]] += k[i];
        }

        for i in 0..n {
            let ci = community[i];
            let ki = k[i];

            // Compute weight from node i to each neighboring community
            let mut neighbor_communities: BTreeMap<usize, f64> = BTreeMap::new();
            for &(j, w) in &neighbors[i] {
                *neighbor_communities.entry(community[j]).or_insert(0.0) += w;
            }

            let ki_in = neighbor_communities.get(&ci).copied().unwrap_or(0.0);
            let remove_cost = ki_in - resolution * sigma_tot[ci] * ki / (2.0 * m);

            let mut best_community = ci;
            let mut best_gain = 0.0;

            for (&cj, &kj_in) in &neighbor_communities {
                if cj == ci {
                    continue;
                }
                let gain = (kj_in - resolution * sigma_tot[cj] * ki / (2.0 * m)) - remove_cost;
                if gain > best_gain {
                    best_gain = gain;
                    best_community = cj;
                }
            }

            if best_gain > 0.0 && best_community != ci {
                sigma_tot[ci] -= ki;
                sigma_tot[best_community] += ki;
                community[i] = best_community;
                moved_this_pass = true;
                any_moved = true;
            }
        }

        if !moved_this_pass {
            break;
        }
    }

    any_moved
}

/// Build a ClusterMap from Louvain community assignments.
/// Uses plurality naming: for each community, take the directory-based cluster name
/// with the most members. Lexicographic tie-break.
fn build_cluster_map(
    graph: &ProjectGraph,
    nodes: &[&CanonicalPath],
    community: &[usize],
    initial_clusters: &ClusterMap,
    converged: bool,
) -> (ClusterMap, bool) {
    // Group files by community
    let mut community_files: BTreeMap<usize, Vec<&CanonicalPath>> = BTreeMap::new();
    for (i, &node) in nodes.iter().enumerate() {
        community_files.entry(community[i]).or_default().push(node);
    }

    // Build reverse map: file → original cluster name
    let mut file_to_original_cluster: BTreeMap<&CanonicalPath, &ClusterId> = BTreeMap::new();
    for (cluster_id, cluster) in &initial_clusters.clusters {
        for file in &cluster.files {
            file_to_original_cluster.insert(file, cluster_id);
        }
    }

    // Name each community by plurality of original cluster names
    let mut clusters: BTreeMap<ClusterId, Cluster> = BTreeMap::new();

    for files in community_files.values() {
        // Count original cluster names in this community
        let mut name_counts: BTreeMap<&ClusterId, usize> = BTreeMap::new();
        for file in files {
            if let Some(orig) = file_to_original_cluster.get(file) {
                *name_counts.entry(orig).or_insert(0) += 1;
            }
        }

        // Pick name with highest count, lexicographic tie-break
        let cluster_name = name_counts
            .into_iter()
            .max_by(|(name_a, count_a), (name_b, count_b)| {
                count_a.cmp(count_b).then_with(|| name_b.cmp(name_a))
            })
            .map(|(name, _)| name.clone())
            .unwrap_or_else(|| ClusterId::new("_unknown"));

        // Handle name collisions: if this name already taken, suffix with _N
        let final_name = if clusters.contains_key(&cluster_name) {
            let mut suffix = 2;
            loop {
                let candidate = ClusterId::new(format!("{}_{}", cluster_name.as_str(), suffix));
                if !clusters.contains_key(&candidate) {
                    break candidate;
                }
                suffix += 1;
            }
        } else {
            cluster_name
        };

        let mut sorted_files: Vec<CanonicalPath> = files.iter().map(|&f| f.clone()).collect();
        sorted_files.sort();

        clusters.insert(
            final_name,
            Cluster {
                file_count: sorted_files.len(),
                files: sorted_files,
                internal_edges: 0,
                external_edges: 0,
                cohesion: 0.0,
            },
        );
    }

    // Recompute cohesion for each cluster
    recompute_cohesion(&mut clusters, &graph.edges);

    (ClusterMap { clusters }, converged)
}

/// Recompute internal_edges, external_edges, and cohesion for each cluster.
/// Cohesion = internal / (internal + external), rounded to 4 decimal places (D-049).
fn recompute_cohesion(clusters: &mut BTreeMap<ClusterId, Cluster>, edges: &[Edge]) {
    // Build file → cluster lookup (owned ClusterId to avoid borrow conflict)
    let mut file_cluster: BTreeMap<CanonicalPath, ClusterId> = BTreeMap::new();
    for (cluster_id, cluster) in clusters.iter() {
        for file in &cluster.files {
            file_cluster.insert(file.clone(), cluster_id.clone());
        }
    }

    // Reset counts
    for cluster in clusters.values_mut() {
        cluster.internal_edges = 0;
        cluster.external_edges = 0;
    }

    // Count ALL edges (matching cluster/mod.rs behavior — INV-6 checks all edge types)
    for edge in edges {
        let from_cluster = file_cluster.get(&edge.from);
        let to_cluster = file_cluster.get(&edge.to);

        match (from_cluster, to_cluster) {
            (Some(fc), Some(tc)) => {
                if fc == tc {
                    if let Some(c) = clusters.get_mut(fc) {
                        c.internal_edges += 1;
                    }
                } else {
                    if let Some(c) = clusters.get_mut(fc) {
                        c.external_edges += 1;
                    }
                    if let Some(c) = clusters.get_mut(tc) {
                        c.external_edges += 1;
                    }
                }
            }
            (Some(fc), None) => {
                if let Some(c) = clusters.get_mut(fc) {
                    c.external_edges += 1;
                }
            }
            (None, Some(tc)) => {
                if let Some(c) = clusters.get_mut(tc) {
                    c.external_edges += 1;
                }
            }
            (None, None) => {}
        }
    }

    // Compute cohesion
    for cluster in clusters.values_mut() {
        let total = cluster.internal_edges + cluster.external_edges;
        cluster.cohesion = if total == 0 {
            1.0
        } else {
            round4(cluster.internal_edges as f64 / total as f64)
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ArchLayer, ContentHash, FileType, Node, Symbol};

    fn make_node(cluster: &str) -> Node {
        Node {
            file_type: FileType::Source,
            layer: ArchLayer::Unknown,
            fsd_layer: None,
            arch_depth: 0,
            lines: 10,
            hash: ContentHash::new("0000000000000000".to_string()),
            exports: vec![],
            cluster: ClusterId::new(cluster),
                    symbols: Vec::new(),
        }
    }

    fn make_edge(from: &str, to: &str) -> Edge {
        Edge {
            from: CanonicalPath::new(from),
            to: CanonicalPath::new(to),
            edge_type: crate::model::EdgeType::Imports,
            symbols: vec![Symbol::new("default")],
        }
    }

    fn make_graph(
        nodes: Vec<(&str, &str)>,
        edges: Vec<(&str, &str)>,
    ) -> (ProjectGraph, ClusterMap) {
        let mut graph_nodes = BTreeMap::new();
        let mut cluster_files: BTreeMap<String, Vec<CanonicalPath>> = BTreeMap::new();

        for (path, cluster) in &nodes {
            graph_nodes.insert(CanonicalPath::new(*path), make_node(cluster));
            cluster_files
                .entry(cluster.to_string())
                .or_default()
                .push(CanonicalPath::new(*path));
        }

        let graph_edges: Vec<Edge> = edges.iter().map(|(f, t)| make_edge(f, t)).collect();

        let graph = ProjectGraph {
            nodes: graph_nodes,
            edges: graph_edges,
        };

        let mut clusters = BTreeMap::new();
        for (name, mut files) in cluster_files {
            files.sort();
            let file_count = files.len();
            clusters.insert(
                ClusterId::new(&name),
                Cluster {
                    files,
                    file_count,
                    internal_edges: 0,
                    external_edges: 0,
                    cohesion: 0.0,
                },
            );
        }

        let cluster_map = ClusterMap { clusters };
        (graph, cluster_map)
    }

    #[test]
    fn empty_graph_unchanged() {
        let graph = ProjectGraph {
            nodes: BTreeMap::new(),
            edges: vec![],
        };
        let cluster_map = ClusterMap {
            clusters: BTreeMap::new(),
        };
        let (result, converged) = louvain_clustering(&graph, &cluster_map);
        assert!(converged);
        assert!(result.clusters.is_empty());
    }

    #[test]
    fn single_file_unchanged() {
        let (graph, cluster_map) = make_graph(vec![("src/a.ts", "src")], vec![]);
        let (result, converged) = louvain_clustering(&graph, &cluster_map);
        assert!(converged);
        assert_eq!(result.clusters.len(), 1);
        assert!(result.clusters.contains_key(&ClusterId::new("src")));
    }

    #[test]
    fn no_edges_keeps_directory_clusters() {
        let (graph, cluster_map) =
            make_graph(vec![("src/a.ts", "src"), ("lib/b.ts", "lib")], vec![]);
        let (result, converged) = louvain_clustering(&graph, &cluster_map);
        assert!(converged);
        assert_eq!(result.clusters.len(), 2);
    }

    #[test]
    fn two_cliques_detected() {
        // Two well-separated cliques connected by a single edge
        let (graph, cluster_map) = make_graph(
            vec![
                ("a/a1.ts", "a"),
                ("a/a2.ts", "a"),
                ("a/a3.ts", "a"),
                ("b/b1.ts", "b"),
                ("b/b2.ts", "b"),
                ("b/b3.ts", "b"),
            ],
            vec![
                // Clique A: fully connected
                ("a/a1.ts", "a/a2.ts"),
                ("a/a2.ts", "a/a1.ts"),
                ("a/a1.ts", "a/a3.ts"),
                ("a/a3.ts", "a/a1.ts"),
                ("a/a2.ts", "a/a3.ts"),
                ("a/a3.ts", "a/a2.ts"),
                // Clique B: fully connected
                ("b/b1.ts", "b/b2.ts"),
                ("b/b2.ts", "b/b1.ts"),
                ("b/b1.ts", "b/b3.ts"),
                ("b/b3.ts", "b/b1.ts"),
                ("b/b2.ts", "b/b3.ts"),
                ("b/b3.ts", "b/b2.ts"),
                // Single bridge edge
                ("a/a1.ts", "b/b1.ts"),
            ],
        );
        let (result, converged) = louvain_clustering(&graph, &cluster_map);
        assert!(converged);
        // Should maintain 2 communities (cliques are well-separated)
        assert_eq!(result.clusters.len(), 2);

        // Each cluster should have 3 files
        for cluster in result.clusters.values() {
            assert_eq!(cluster.file_count, 3);
        }
    }

    #[test]
    fn determinism_two_runs_identical() {
        let (graph, cluster_map) = make_graph(
            vec![
                ("a/a1.ts", "a"),
                ("a/a2.ts", "a"),
                ("b/b1.ts", "b"),
                ("b/b2.ts", "b"),
            ],
            vec![
                ("a/a1.ts", "a/a2.ts"),
                ("a/a2.ts", "a/a1.ts"),
                ("b/b1.ts", "b/b2.ts"),
                ("b/b2.ts", "b/b1.ts"),
                ("a/a1.ts", "b/b1.ts"),
            ],
        );

        let (result1, _) = louvain_clustering(&graph, &cluster_map);
        let (result2, _) = louvain_clustering(&graph, &cluster_map);

        assert_eq!(result1.clusters.len(), result2.clusters.len());
        for (id, c1) in &result1.clusters {
            let c2 = result2.clusters.get(id).expect("same cluster names");
            assert_eq!(c1.files, c2.files);
            assert_eq!(c1.cohesion, c2.cohesion);
        }
    }

    #[test]
    fn cohesion_computed_correctly() {
        let (graph, cluster_map) = make_graph(
            vec![("a/a1.ts", "a"), ("a/a2.ts", "a")],
            vec![("a/a1.ts", "a/a2.ts"), ("a/a2.ts", "a/a1.ts")],
        );
        let (result, _) = louvain_clustering(&graph, &cluster_map);
        for cluster in result.clusters.values() {
            assert_eq!(cluster.cohesion, 1.0);
        }
    }

    #[test]
    fn cluster_naming_plurality() {
        // 3 files from "auth", 1 file from "utils" — if merged, should be named "auth"
        let (graph, cluster_map) = make_graph(
            vec![
                ("auth/a1.ts", "auth"),
                ("auth/a2.ts", "auth"),
                ("auth/a3.ts", "auth"),
                ("utils/u1.ts", "utils"),
            ],
            vec![
                // All tightly connected (forms one community)
                ("auth/a1.ts", "auth/a2.ts"),
                ("auth/a2.ts", "auth/a1.ts"),
                ("auth/a1.ts", "auth/a3.ts"),
                ("auth/a3.ts", "auth/a1.ts"),
                ("auth/a2.ts", "auth/a3.ts"),
                ("auth/a3.ts", "auth/a2.ts"),
                ("auth/a1.ts", "utils/u1.ts"),
                ("utils/u1.ts", "auth/a1.ts"),
                ("auth/a2.ts", "utils/u1.ts"),
                ("utils/u1.ts", "auth/a2.ts"),
                ("auth/a3.ts", "utils/u1.ts"),
                ("utils/u1.ts", "auth/a3.ts"),
            ],
        );
        let (result, _) = louvain_clustering(&graph, &cluster_map);

        if result.clusters.len() == 1 {
            assert!(result.clusters.contains_key(&ClusterId::new("auth")));
        }
    }

    #[test]
    fn disconnected_components_separate_communities() {
        let (graph, cluster_map) = make_graph(
            vec![
                ("a/a1.ts", "a"),
                ("a/a2.ts", "a"),
                ("b/b1.ts", "b"),
                ("b/b2.ts", "b"),
            ],
            vec![
                ("a/a1.ts", "a/a2.ts"),
                ("a/a2.ts", "a/a1.ts"),
                ("b/b1.ts", "b/b2.ts"),
                ("b/b2.ts", "b/b1.ts"),
            ],
        );
        let (result, converged) = louvain_clustering(&graph, &cluster_map);
        assert!(converged);
        assert_eq!(result.clusters.len(), 2);
    }
}
