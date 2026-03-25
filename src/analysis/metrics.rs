use std::collections::BTreeMap;

use serde::Serialize;

use crate::algo::round4;
use crate::model::*;

/// Martin metrics for a single cluster.
#[derive(Debug, Clone, Serialize)]
pub struct ClusterMetrics {
    pub cluster_id: ClusterId,
    pub instability: f64,
    pub abstractness: f64,
    pub distance: f64,
    pub zone: MetricZone,
    pub afferent_coupling: u32,
    pub efferent_coupling: u32,
    pub abstract_files: u32,
    pub total_files: u32,
}

/// Zone classification on the A/I diagram.
#[derive(Debug, Clone, Copy, Serialize, PartialEq)]
pub enum MetricZone {
    MainSequence,
    ZoneOfPain,
    ZoneOfUselessness,
    OffMainSequence,
}

/// Compute Martin metrics (Instability, Abstractness, Distance) per cluster.
pub fn compute_martin_metrics(
    graph: &ProjectGraph,
    clusters: &ClusterMap,
) -> BTreeMap<ClusterId, ClusterMetrics> {
    let mut result = BTreeMap::new();

    // Precompute Ca/Ce for all clusters in a single pass over edges
    let mut ca_map: BTreeMap<&ClusterId, u32> = BTreeMap::new();
    let mut ce_map: BTreeMap<&ClusterId, u32> = BTreeMap::new();

    for edge in &graph.edges {
        if !edge.edge_type.is_architectural() {
            continue;
        }
        let from_cluster = graph.nodes.get(&edge.from).map(|n| &n.cluster);
        let to_cluster = graph.nodes.get(&edge.to).map(|n| &n.cluster);

        if let (Some(fc), Some(tc)) = (from_cluster, to_cluster) {
            if fc != tc {
                *ca_map.entry(tc).or_default() += 1;
                *ce_map.entry(fc).or_default() += 1;
            }
        }
    }

    // Precompute re-export counts per file for barrel detection
    let mut re_export_counts: BTreeMap<&CanonicalPath, usize> = BTreeMap::new();
    for edge in &graph.edges {
        if edge.edge_type == EdgeType::ReExports {
            *re_export_counts.entry(&edge.from).or_default() += 1;
        }
    }

    for (cluster_id, cluster) in &clusters.clusters {
        let total_files = cluster.files.len() as u32;
        if total_files == 0 {
            continue;
        }

        let ca = ca_map.get(cluster_id).copied().unwrap_or(0);
        let ce = ce_map.get(cluster_id).copied().unwrap_or(0);

        // I = Ce / (Ca + Ce), edge case: 0/0 → 0.0
        let instability = if ca + ce == 0 {
            0.0
        } else {
            round4(ce as f64 / (ca + ce) as f64)
        };

        // Count abstract files
        let abstract_files = cluster
            .files
            .iter()
            .filter(|path| {
                graph
                    .nodes
                    .get(*path)
                    .map(|node| {
                        is_abstract_file_fast(
                            node,
                            re_export_counts.get(path).copied().unwrap_or(0),
                        )
                    })
                    .unwrap_or(false)
            })
            .count() as u32;

        // A = Na / Nc
        let abstractness = round4(abstract_files as f64 / total_files as f64);

        // D = |A + I - 1|
        let distance = round4((abstractness + instability - 1.0).abs());

        let zone = classify_zone(distance, abstractness, instability);

        result.insert(
            cluster_id.clone(),
            ClusterMetrics {
                cluster_id: cluster_id.clone(),
                instability,
                abstractness,
                distance,
                zone,
                afferent_coupling: ca,
                efferent_coupling: ce,
                abstract_files,
                total_files,
            },
        );
    }

    result
}

/// Classify a file as abstract using precomputed re-export count.
fn is_abstract_file_fast(node: &Node, re_export_count: usize) -> bool {
    if node.file_type == FileType::TypeDef {
        return true;
    }

    // Check symbol-level abstractness: traits/interfaces with public visibility
    let public_symbols: Vec<_> = node
        .symbols
        .iter()
        .filter(|s| s.visibility == Visibility::Public)
        .collect();
    let trait_interface_count = public_symbols
        .iter()
        .filter(|s| s.kind == SymbolKind::Trait || s.kind == SymbolKind::Interface)
        .count();

    if !public_symbols.is_empty() {
        // File with <3 public symbols but at least 1 trait/interface → abstract
        if public_symbols.len() < 3 && trait_interface_count >= 1 {
            return true;
        }
        // >50% of public symbols are traits/interfaces → abstract
        if trait_interface_count as f64 / public_symbols.len() as f64 > 0.5 {
            return true;
        }
    }

    if node.exports.is_empty() {
        return false;
    }
    let ratio = re_export_count as f64 / node.exports.len() as f64;
    ratio > 0.8
}

fn classify_zone(distance: f64, abstractness: f64, instability: f64) -> MetricZone {
    if distance < 0.3 {
        MetricZone::MainSequence
    } else if abstractness < 0.5 && instability < 0.5 {
        MetricZone::ZoneOfPain
    } else if abstractness > 0.5 && instability > 0.5 {
        MetricZone::ZoneOfUselessness
    } else {
        MetricZone::OffMainSequence
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal graph for testing metrics.
    fn build_test_graph(
        nodes: Vec<(&str, FileType, ClusterId, Vec<Symbol>)>,
        edges: Vec<(&str, &str, EdgeType)>,
    ) -> (ProjectGraph, ClusterMap) {
        let mut graph_nodes = BTreeMap::new();
        let mut cluster_files: BTreeMap<ClusterId, Vec<CanonicalPath>> = BTreeMap::new();

        for (path, file_type, cluster_id, exports) in &nodes {
            let cp = CanonicalPath::new(*path);
            graph_nodes.insert(
                cp.clone(),
                Node {
                    file_type: *file_type,
                    layer: ArchLayer::Unknown,
                    fsd_layer: None,
                    arch_depth: 0,
                    lines: 100,
                    hash: ContentHash::new("0000000000000000".to_string()),
                    exports: exports.clone(),
                    cluster: cluster_id.clone(),
                    symbols: Vec::new(),
                },
            );
            cluster_files
                .entry(cluster_id.clone())
                .or_default()
                .push(cp);
        }

        let graph_edges: Vec<Edge> = edges
            .iter()
            .map(|(from, to, edge_type)| Edge {
                from: CanonicalPath::new(*from),
                to: CanonicalPath::new(*to),
                edge_type: *edge_type,
                symbols: vec![],
            })
            .collect();

        let graph = ProjectGraph {
            nodes: graph_nodes,
            edges: graph_edges,
        };

        let mut clusters = BTreeMap::new();
        for (id, files) in cluster_files {
            let file_count = files.len();
            clusters.insert(
                id,
                Cluster {
                    files,
                    file_count,
                    internal_edges: 0,
                    external_edges: 0,
                    cohesion: 0.0,
                },
            );
        }

        (graph, ClusterMap { clusters })
    }

    #[test]
    fn isolated_cluster_instability_zero() {
        let cid = ClusterId::new("isolated");
        let (graph, clusters) = build_test_graph(
            vec![("src/a.ts", FileType::Source, cid.clone(), vec![])],
            vec![],
        );
        let metrics = compute_martin_metrics(&graph, &clusters);
        let m = &metrics[&cid];
        assert_eq!(m.instability, 0.0);
    }

    #[test]
    fn fully_outgoing_cluster_instability_one() {
        let c1 = ClusterId::new("c1");
        let c2 = ClusterId::new("c2");
        let (graph, clusters) = build_test_graph(
            vec![
                ("src/a.ts", FileType::Source, c1.clone(), vec![]),
                ("lib/b.ts", FileType::Source, c2.clone(), vec![]),
            ],
            vec![("src/a.ts", "lib/b.ts", EdgeType::Imports)],
        );
        let metrics = compute_martin_metrics(&graph, &clusters);
        assert_eq!(metrics[&c1].instability, 1.0); // Ce=1, Ca=0 → 1.0
        assert_eq!(metrics[&c2].instability, 0.0); // Ce=0, Ca=1 → 0.0
    }

    #[test]
    fn all_typedef_cluster_abstractness_one() {
        let cid = ClusterId::new("types");
        let (graph, clusters) = build_test_graph(
            vec![
                (
                    "types/a.d.ts",
                    FileType::TypeDef,
                    cid.clone(),
                    vec![Symbol::new("A")],
                ),
                (
                    "types/b.d.ts",
                    FileType::TypeDef,
                    cid.clone(),
                    vec![Symbol::new("B")],
                ),
            ],
            vec![],
        );
        let metrics = compute_martin_metrics(&graph, &clusters);
        assert_eq!(metrics[&cid].abstractness, 1.0);
    }

    #[test]
    fn no_abstract_cluster_abstractness_zero() {
        let cid = ClusterId::new("src");
        let (graph, clusters) = build_test_graph(
            vec![
                (
                    "src/a.ts",
                    FileType::Source,
                    cid.clone(),
                    vec![Symbol::new("foo")],
                ),
                (
                    "src/b.ts",
                    FileType::Source,
                    cid.clone(),
                    vec![Symbol::new("bar")],
                ),
            ],
            vec![],
        );
        let metrics = compute_martin_metrics(&graph, &clusters);
        assert_eq!(metrics[&cid].abstractness, 0.0);
    }

    #[test]
    fn barrel_file_detection() {
        let cid = ClusterId::new("barrel");
        // File with 5 exports and 4 re-export edges → 80% → abstract (>80% is strict)
        // Actually 4/5 = 0.8, need >0.8, so NOT abstract
        // File with 5 exports and 5 re-export edges → 100% → abstract
        let (graph, clusters) = build_test_graph(
            vec![
                (
                    "src/index.ts",
                    FileType::Source,
                    cid.clone(),
                    vec![
                        Symbol::new("a"),
                        Symbol::new("b"),
                        Symbol::new("c"),
                        Symbol::new("d"),
                        Symbol::new("e"),
                    ],
                ),
                (
                    "src/barrel.ts",
                    FileType::Source,
                    cid.clone(),
                    vec![
                        Symbol::new("x"),
                        Symbol::new("y"),
                        Symbol::new("z"),
                        Symbol::new("w"),
                        Symbol::new("v"),
                    ],
                ),
            ],
            vec![
                // index.ts: 4/5 re-exports = 0.8, NOT >0.8 → concrete
                ("src/index.ts", "lib/a.ts", EdgeType::ReExports),
                ("src/index.ts", "lib/b.ts", EdgeType::ReExports),
                ("src/index.ts", "lib/c.ts", EdgeType::ReExports),
                ("src/index.ts", "lib/d.ts", EdgeType::ReExports),
                // barrel.ts: 5/5 re-exports = 1.0, >0.8 → abstract
                ("src/barrel.ts", "lib/x.ts", EdgeType::ReExports),
                ("src/barrel.ts", "lib/y.ts", EdgeType::ReExports),
                ("src/barrel.ts", "lib/z.ts", EdgeType::ReExports),
                ("src/barrel.ts", "lib/w.ts", EdgeType::ReExports),
                ("src/barrel.ts", "lib/v.ts", EdgeType::ReExports),
            ],
        );
        let metrics = compute_martin_metrics(&graph, &clusters);
        // 1 abstract out of 2 = 0.5
        assert_eq!(metrics[&cid].abstract_files, 1);
        assert_eq!(metrics[&cid].abstractness, 0.5);
    }

    #[test]
    fn zone_of_pain() {
        // Zone of Pain: D >= 0.3, A < 0.5, I < 0.5
        // A=0.0, I=0.0 → D = |0+0-1| = 1.0 → Zone of Pain
        let cid = ClusterId::new("pain");
        let (graph, clusters) = build_test_graph(
            vec![("src/a.ts", FileType::Source, cid.clone(), vec![])],
            vec![],
        );
        let metrics = compute_martin_metrics(&graph, &clusters);
        assert_eq!(metrics[&cid].zone, MetricZone::ZoneOfPain);
        assert!(metrics[&cid].distance >= 0.3);
    }

    #[test]
    fn zone_of_uselessness() {
        // Zone of Uselessness: D >= 0.3, A > 0.5, I > 0.5
        // Need cluster with A > 0.5 (mostly abstract) and I > 0.5 (mostly outgoing)
        let c1 = ClusterId::new("useless");
        let c2 = ClusterId::new("other");
        let (graph, clusters) = build_test_graph(
            vec![
                (
                    "src/a.d.ts",
                    FileType::TypeDef,
                    c1.clone(),
                    vec![Symbol::new("A")],
                ),
                (
                    "src/b.d.ts",
                    FileType::TypeDef,
                    c1.clone(),
                    vec![Symbol::new("B")],
                ),
                ("src/c.ts", FileType::Source, c1.clone(), vec![]),
                ("lib/x.ts", FileType::Source, c2.clone(), vec![]),
            ],
            // c1 has 3 outgoing edges, 1 incoming → Ce=3, Ca=1 → I = 3/4 = 0.75
            vec![
                ("src/a.d.ts", "lib/x.ts", EdgeType::Imports),
                ("src/b.d.ts", "lib/x.ts", EdgeType::Imports),
                ("src/c.ts", "lib/x.ts", EdgeType::Imports),
                ("lib/x.ts", "src/a.d.ts", EdgeType::Imports),
            ],
        );
        let metrics = compute_martin_metrics(&graph, &clusters);
        let m = &metrics[&c1];
        // A = 2/3 ≈ 0.6667, I = 3/4 = 0.75
        assert!(m.abstractness > 0.5);
        assert!(m.instability > 0.5);
        assert_eq!(m.zone, MetricZone::ZoneOfUselessness);
    }

    #[test]
    fn main_sequence() {
        // Main Sequence: D < 0.3 → A + I ≈ 1.0
        // A = 0.5, I = 0.5 → D = |0.5 + 0.5 - 1| = 0.0
        let c1 = ClusterId::new("balanced");
        let c2 = ClusterId::new("other");
        let (graph, clusters) = build_test_graph(
            vec![
                (
                    "src/a.d.ts",
                    FileType::TypeDef,
                    c1.clone(),
                    vec![Symbol::new("A")],
                ),
                ("src/b.ts", FileType::Source, c1.clone(), vec![]),
                ("lib/x.ts", FileType::Source, c2.clone(), vec![]),
            ],
            // c1: Ca=1, Ce=1 → I = 0.5
            vec![
                ("src/b.ts", "lib/x.ts", EdgeType::Imports),
                ("lib/x.ts", "src/a.d.ts", EdgeType::Imports),
            ],
        );
        let metrics = compute_martin_metrics(&graph, &clusters);
        let m = &metrics[&c1];
        assert_eq!(m.abstractness, 0.5);
        assert_eq!(m.instability, 0.5);
        assert_eq!(m.distance, 0.0);
        assert_eq!(m.zone, MetricZone::MainSequence);
    }

    #[test]
    fn determinism() {
        let c1 = ClusterId::new("c1");
        let c2 = ClusterId::new("c2");
        let build = || {
            build_test_graph(
                vec![
                    ("src/a.ts", FileType::Source, c1.clone(), vec![]),
                    ("lib/b.ts", FileType::Source, c2.clone(), vec![]),
                ],
                vec![("src/a.ts", "lib/b.ts", EdgeType::Imports)],
            )
        };

        let (g1, cl1) = build();
        let (g2, cl2) = build();
        let m1 = compute_martin_metrics(&g1, &cl1);
        let m2 = compute_martin_metrics(&g2, &cl2);

        for (id, cm1) in &m1 {
            let cm2 = &m2[id];
            assert_eq!(cm1.instability, cm2.instability);
            assert_eq!(cm1.abstractness, cm2.abstractness);
            assert_eq!(cm1.distance, cm2.distance);
            assert_eq!(cm1.zone, cm2.zone);
        }
    }

    #[test]
    fn metrics_in_valid_range() {
        let c1 = ClusterId::new("c1");
        let c2 = ClusterId::new("c2");
        let (graph, clusters) = build_test_graph(
            vec![
                (
                    "src/a.ts",
                    FileType::Source,
                    c1.clone(),
                    vec![Symbol::new("x")],
                ),
                (
                    "src/b.d.ts",
                    FileType::TypeDef,
                    c1.clone(),
                    vec![Symbol::new("y")],
                ),
                ("lib/c.ts", FileType::Source, c2.clone(), vec![]),
            ],
            vec![
                ("src/a.ts", "lib/c.ts", EdgeType::Imports),
                ("lib/c.ts", "src/b.d.ts", EdgeType::TypeImports),
            ],
        );
        let metrics = compute_martin_metrics(&graph, &clusters);
        for m in metrics.values() {
            assert!(
                m.instability >= 0.0 && m.instability <= 1.0,
                "I out of range: {}",
                m.instability
            );
            assert!(
                m.abstractness >= 0.0 && m.abstractness <= 1.0,
                "A out of range: {}",
                m.abstractness
            );
            assert!(
                m.distance >= 0.0 && m.distance <= 1.0,
                "D out of range: {}",
                m.distance
            );
        }
    }
}
