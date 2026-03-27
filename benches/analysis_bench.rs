mod helpers;

use std::collections::BTreeMap;

use criterion::{criterion_group, criterion_main, Criterion};

use ariadne_graph::algo;
use ariadne_graph::analysis::diff::compute_structural_diff;
use ariadne_graph::analysis::metrics::compute_martin_metrics;
use ariadne_graph::analysis::smells::detect_smells;
use ariadne_graph::model::*;

/// Build a synthetic graph with clusters, layers, and realistic edge distribution.
fn build_synthetic_graph(node_count: usize, edge_count: usize) -> ProjectGraph {
    let mut nodes = BTreeMap::new();
    for i in 0..node_count {
        let path = CanonicalPath::new(format!("src/file_{}.ts", i));
        nodes.insert(
            path,
            Node {
                file_type: if i % 50 == 0 {
                    FileType::TypeDef
                } else {
                    FileType::Source
                },
                layer: ArchLayer::Unknown,
                fsd_layer: None,
                arch_depth: (i % 5) as u32,
                lines: 50 + (i % 500) as u32,
                hash: ContentHash::new(format!("{:016x}", i)),
                exports: vec![Symbol::new(format!("export_{}", i))],
                cluster: ClusterId::new(format!("cluster_{}", i % 30)),
                    symbols: Vec::new(),
            },
        );
    }

    let node_keys: Vec<CanonicalPath> = nodes.keys().cloned().collect();
    let mut edges = Vec::with_capacity(edge_count);
    for i in 0..edge_count {
        let from_idx = i % node_count;
        let to_idx = (i * 7 + 13) % node_count;
        if from_idx != to_idx {
            edges.push(Edge {
                from: node_keys[from_idx].clone(),
                to: node_keys[to_idx].clone(),
                edge_type: if i % 20 == 0 {
                    EdgeType::ReExports
                } else {
                    EdgeType::Imports
                },
                symbols: vec![],
            });
        }
    }

    ProjectGraph { nodes, edges }
}

fn build_clusters(graph: &ProjectGraph) -> ClusterMap {
    let mut cluster_files: BTreeMap<ClusterId, Vec<CanonicalPath>> = BTreeMap::new();
    for (path, node) in &graph.nodes {
        cluster_files
            .entry(node.cluster.clone())
            .or_default()
            .push(path.clone());
    }

    let mut clusters = BTreeMap::new();
    for (id, files) in cluster_files {
        let file_count = files.len();
        // Count internal/external edges
        let cluster_set: std::collections::BTreeSet<&CanonicalPath> = files.iter().collect();
        let mut internal = 0u32;
        let mut external = 0u32;
        for edge in &graph.edges {
            let from_in = cluster_set.contains(&edge.from);
            let to_in = cluster_set.contains(&edge.to);
            if from_in && to_in {
                internal += 1;
            } else if from_in || to_in {
                external += 1;
            }
        }
        clusters.insert(
            id,
            Cluster {
                files,
                file_count,
                internal_edges: internal,
                external_edges: external,
                cohesion: if internal + external > 0 {
                    internal as f64 / (internal + external) as f64
                } else {
                    0.0
                },
            },
        );
    }

    ClusterMap { clusters }
}

fn build_stats(graph: &ProjectGraph) -> StatsOutput {
    let index = algo::AdjacencyIndex::build(&graph.edges, algo::is_architectural);
    let sccs = algo::scc::find_sccs(graph, &index);
    let layers = algo::topo_sort::topological_layers(graph, &sccs, &index);
    let centrality = algo::centrality::betweenness_centrality(graph, &index);
    algo::stats::compute_stats(graph, &centrality, &sccs, &layers)
}

// --- Benchmarks ---

fn bench_martin_metrics(c: &mut Criterion) {
    let graph = build_synthetic_graph(3000, 8000);
    let clusters = build_clusters(&graph);

    c.bench_function("martin_metrics_3000", |b| {
        b.iter(|| compute_martin_metrics(&graph, &clusters))
    });
}

fn bench_smell_detection(c: &mut Criterion) {
    let graph = build_synthetic_graph(3000, 8000);
    let clusters = build_clusters(&graph);
    let stats = build_stats(&graph);
    let metrics = compute_martin_metrics(&graph, &clusters);

    let mut group = c.benchmark_group("smell_detection");
    group.sample_size(10);
    group.bench_function("detect_smells_3000", |b| {
        b.iter(|| detect_smells(&graph, &stats, &clusters, &metrics, None, None))
    });
    group.finish();
}

fn bench_structural_diff(c: &mut Criterion) {
    // Build "old" graph
    let old_graph = build_synthetic_graph(3000, 8000);
    let old_clusters = build_clusters(&old_graph);
    let old_stats = build_stats(&old_graph);
    let old_metrics = compute_martin_metrics(&old_graph, &old_clusters);

    // Build "new" graph with ~50 changes: add 25 nodes, remove 25 edges
    let mut new_nodes = old_graph.nodes.clone();
    for i in 3000..3025 {
        let path = CanonicalPath::new(format!("src/new_file_{}.ts", i));
        new_nodes.insert(
            path,
            Node {
                file_type: FileType::Source,
                layer: ArchLayer::Unknown,
                fsd_layer: None,
                arch_depth: (i % 5) as u32,
                lines: 100,
                hash: ContentHash::new(format!("{:016x}", i)),
                exports: vec![Symbol::new(format!("export_{}", i))],
                cluster: ClusterId::new(format!("cluster_{}", i % 30)),
                    symbols: Vec::new(),
            },
        );
    }
    let new_edges: Vec<Edge> = old_graph.edges.iter().skip(25).cloned().collect();
    let new_graph = ProjectGraph {
        nodes: new_nodes,
        edges: new_edges,
    };
    let new_clusters = build_clusters(&new_graph);
    let new_stats = build_stats(&new_graph);
    let new_metrics = compute_martin_metrics(&new_graph, &new_clusters);

    c.bench_function("structural_diff_3000_50changes", |b| {
        b.iter(|| {
            compute_structural_diff(
                &old_graph,
                &old_stats,
                &old_clusters,
                &old_metrics,
                &new_graph,
                &new_stats,
                &new_clusters,
                &new_metrics,
            )
        })
    });
}

criterion_group!(
    benches,
    bench_martin_metrics,
    bench_smell_detection,
    bench_structural_diff,
);
criterion_main!(benches);
