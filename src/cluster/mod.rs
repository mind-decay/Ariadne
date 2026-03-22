use std::collections::BTreeMap;

use crate::model::{CanonicalPath, Cluster, ClusterId, ClusterMap, ProjectGraph};

/// Assign files to directory-based clusters and compute cohesion metrics.
///
/// Does not mutate the graph — returns a standalone `ClusterMap`.
pub fn assign_clusters(graph: &ProjectGraph) -> ClusterMap {
    // Step 1: Group files by cluster name
    let mut groups: BTreeMap<String, Vec<CanonicalPath>> = BTreeMap::new();

    for path in graph.nodes.keys() {
        let cluster_name = extract_cluster_name(path);
        groups.entry(cluster_name).or_default().push(path.clone());
    }

    // Sort file lists within each group
    for files in groups.values_mut() {
        files.sort();
    }

    // Step 2: Build a lookup from path → cluster name for edge counting
    let mut path_to_cluster: BTreeMap<&CanonicalPath, String> = BTreeMap::new();
    for (name, files) in &groups {
        for file in files {
            path_to_cluster.insert(file, name.clone());
        }
    }

    // Step 3: Count edges per cluster
    let mut internal_counts: BTreeMap<String, u32> = BTreeMap::new();
    let mut external_counts: BTreeMap<String, u32> = BTreeMap::new();

    for edge in &graph.edges {
        let from_cluster = path_to_cluster.get(&edge.from);
        let to_cluster = path_to_cluster.get(&edge.to);

        match (from_cluster, to_cluster) {
            (Some(fc), Some(tc)) if fc == tc => {
                *internal_counts.entry(fc.clone()).or_insert(0) += 1;
            }
            (Some(fc), Some(tc)) => {
                *external_counts.entry(fc.clone()).or_insert(0) += 1;
                *external_counts.entry(tc.clone()).or_insert(0) += 1;
            }
            (Some(fc), None) => {
                *external_counts.entry(fc.clone()).or_insert(0) += 1;
            }
            (None, Some(tc)) => {
                *external_counts.entry(tc.clone()).or_insert(0) += 1;
            }
            (None, None) => {}
        }
    }

    // Step 4: Build ClusterMap
    let mut clusters = BTreeMap::new();

    for (name, files) in groups {
        let internal = internal_counts.get(&name).copied().unwrap_or(0);
        let external = external_counts.get(&name).copied().unwrap_or(0);
        let cohesion = if internal == 0 && external == 0 {
            1.0
        } else {
            let raw = internal as f64 / (internal as f64 + external as f64);
            (raw * 10000.0).round() / 10000.0
        };

        let file_count = files.len();
        clusters.insert(
            ClusterId::new(name),
            Cluster {
                files,
                file_count,
                internal_edges: internal,
                external_edges: external,
                cohesion,
            },
        );
    }

    ClusterMap { clusters }
}

/// Extract the cluster name from a canonical path.
///
/// - No directory → `"root"`
/// - Starts with `src/` → next segment after `src/` (or `"root"` if none)
/// - Otherwise → first segment
fn extract_cluster_name(path: &CanonicalPath) -> String {
    let path_str = path.as_str();
    let segments: Vec<&str> = path_str.split('/').collect();

    if segments.len() <= 1 {
        // No directory component (e.g., `main.rs`)
        return "root".to_string();
    }

    if segments[0] == "src" {
        if segments.len() > 2 {
            // src/auth/login.ts → "auth"
            segments[1].to_string()
        } else {
            // src/main.rs → "root"
            "root".to_string()
        }
    } else {
        // lib/utils.ts → "lib"
        segments[0].to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{
        ArchLayer, CanonicalPath, ClusterId, ContentHash, Edge, EdgeType, FileType, Node,
    };

    fn make_node() -> Node {
        Node {
            file_type: FileType::Source,
            layer: ArchLayer::Unknown,
            fsd_layer: None,
            arch_depth: 0,
            lines: 10,
            hash: ContentHash::new("0000000000000000".to_string()),
            exports: vec![],
            cluster: ClusterId::new(""),
        }
    }

    fn make_graph(paths: &[&str], edges: Vec<(&str, &str)>) -> ProjectGraph {
        let mut nodes = BTreeMap::new();
        for p in paths {
            nodes.insert(CanonicalPath::new(*p), make_node());
        }
        let edges = edges
            .into_iter()
            .map(|(from, to)| Edge {
                from: CanonicalPath::new(from),
                to: CanonicalPath::new(to),
                edge_type: EdgeType::Imports,
                symbols: vec![],
            })
            .collect();
        ProjectGraph { nodes, edges }
    }

    #[test]
    fn cluster_name_extraction() {
        assert_eq!(extract_cluster_name(&CanonicalPath::new("main.rs")), "root");
        assert_eq!(
            extract_cluster_name(&CanonicalPath::new("src/main.rs")),
            "root"
        );
        assert_eq!(
            extract_cluster_name(&CanonicalPath::new("src/auth/login.ts")),
            "auth"
        );
        assert_eq!(
            extract_cluster_name(&CanonicalPath::new("lib/utils.ts")),
            "lib"
        );
        assert_eq!(
            extract_cluster_name(&CanonicalPath::new("src/model/types.rs")),
            "model"
        );
    }

    #[test]
    fn basic_clustering() {
        let graph = make_graph(
            &[
                "src/auth/login.ts",
                "src/auth/register.ts",
                "src/db/conn.ts",
            ],
            vec![
                ("src/auth/login.ts", "src/auth/register.ts"),
                ("src/auth/login.ts", "src/db/conn.ts"),
            ],
        );

        let cluster_map = assign_clusters(&graph);
        assert_eq!(cluster_map.clusters.len(), 2);

        let auth = &cluster_map.clusters[&ClusterId::new("auth")];
        assert_eq!(auth.file_count, 2);
        assert_eq!(auth.internal_edges, 1);
        assert_eq!(auth.external_edges, 1);

        let db = &cluster_map.clusters[&ClusterId::new("db")];
        assert_eq!(db.file_count, 1);
        assert_eq!(db.internal_edges, 0);
        assert_eq!(db.external_edges, 1);
    }

    #[test]
    fn empty_graph() {
        let graph = make_graph(&[], vec![]);
        let cluster_map = assign_clusters(&graph);
        assert!(cluster_map.clusters.is_empty());
    }

    #[test]
    fn cohesion_all_internal() {
        let graph = make_graph(
            &["src/auth/a.ts", "src/auth/b.ts"],
            vec![("src/auth/a.ts", "src/auth/b.ts")],
        );
        let cluster_map = assign_clusters(&graph);
        let auth = &cluster_map.clusters[&ClusterId::new("auth")];
        assert_eq!(auth.cohesion, 1.0);
    }

    #[test]
    fn cohesion_no_edges() {
        let graph = make_graph(&["src/auth/a.ts"], vec![]);
        let cluster_map = assign_clusters(&graph);
        let auth = &cluster_map.clusters[&ClusterId::new("auth")];
        assert_eq!(auth.cohesion, 1.0);
    }

    #[test]
    fn files_are_sorted() {
        let graph = make_graph(&["src/auth/z.ts", "src/auth/a.ts", "src/auth/m.ts"], vec![]);
        let cluster_map = assign_clusters(&graph);
        let auth = &cluster_map.clusters[&ClusterId::new("auth")];
        let names: Vec<&str> = auth.files.iter().map(|p| p.file_name()).collect();
        assert_eq!(names, vec!["a.ts", "m.ts", "z.ts"]);
    }

    #[test]
    fn root_cluster_for_top_level_files() {
        let graph = make_graph(&["main.rs", "lib.rs"], vec![]);
        let cluster_map = assign_clusters(&graph);
        assert!(cluster_map.clusters.contains_key(&ClusterId::new("root")));
        assert_eq!(cluster_map.clusters[&ClusterId::new("root")].file_count, 2);
    }

    #[test]
    fn deterministic_output() {
        let graph = make_graph(&["src/b/x.ts", "src/a/y.ts", "src/c/z.ts"], vec![]);
        let cluster_map = assign_clusters(&graph);
        let keys: Vec<&str> = cluster_map.clusters.keys().map(|k| k.as_str()).collect();
        assert_eq!(keys, vec!["a", "b", "c"]);
    }
}
