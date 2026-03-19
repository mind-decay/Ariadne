mod helpers;

use std::collections::BTreeMap;

use criterion::{criterion_group, criterion_main, Criterion};

use ariadne_graph::algo;
use ariadne_graph::model::*;

/// Build a synthetic in-memory ProjectGraph for algorithm benchmarks.
fn build_synthetic_graph(node_count: usize, edge_count: usize) -> ProjectGraph {
    let mut nodes = BTreeMap::new();
    for i in 0..node_count {
        let path = CanonicalPath::new(format!("src/file_{}.ts", i));
        nodes.insert(
            path,
            Node {
                file_type: FileType::Source,
                layer: ArchLayer::Unknown,
                arch_depth: 0,
                lines: 100,
                hash: ContentHash::new(format!("{:016x}", i)),
                exports: vec![Symbol::new(format!("export_{}", i))],
                cluster: ClusterId::new(format!("cluster_{}", i % 30)),
            },
        );
    }

    let node_keys: Vec<CanonicalPath> = nodes.keys().cloned().collect();
    let mut edges = Vec::with_capacity(edge_count);
    for i in 0..edge_count {
        let from_idx = i % node_count;
        let to_idx = (i * 7 + 13) % node_count; // pseudo-random but deterministic
        if from_idx != to_idx {
            edges.push(Edge {
                from: node_keys[from_idx].clone(),
                to: node_keys[to_idx].clone(),
                edge_type: EdgeType::Imports,
                symbols: vec![],
            });
        }
    }

    ProjectGraph { nodes, edges }
}

fn bench_scc(c: &mut Criterion) {
    let graph = build_synthetic_graph(3000, 8000);
    c.bench_function("tarjan_scc_3000", |b| {
        b.iter(|| algo::scc::find_sccs(&graph))
    });
}

fn bench_topo_sort(c: &mut Criterion) {
    let graph = build_synthetic_graph(3000, 8000);
    let sccs = algo::scc::find_sccs(&graph);
    c.bench_function("topo_sort_3000", |b| {
        b.iter(|| algo::topo_sort::topological_layers(&graph, &sccs))
    });
}

fn bench_blast_radius(c: &mut Criterion) {
    let graph = build_synthetic_graph(3000, 8000);
    let target = graph.nodes.keys().next().unwrap().clone();
    c.bench_function("blast_radius_3000", |b| {
        b.iter(|| algo::blast_radius::blast_radius(&graph, &target, None))
    });
}

fn bench_centrality(c: &mut Criterion) {
    let graph = build_synthetic_graph(3000, 8000);
    c.bench_function("brandes_centrality_3000", |b| {
        b.iter(|| algo::centrality::betweenness_centrality(&graph))
    });
}

fn bench_pagerank(c: &mut Criterion) {
    let graph = build_synthetic_graph(3000, 8000);
    c.bench_function("pagerank_3000", |b| {
        b.iter(|| algo::pagerank::pagerank(&graph, 0.85, 100, 1e-6))
    });
}

fn bench_combined_importance(c: &mut Criterion) {
    let graph = build_synthetic_graph(3000, 8000);
    let centrality = algo::centrality::betweenness_centrality(&graph);
    let centrality_str: BTreeMap<String, f64> = centrality
        .iter()
        .map(|(k, &v)| (k.as_str().to_string(), v))
        .collect();
    let pr = algo::pagerank::pagerank(&graph, 0.85, 100, 1e-6);
    c.bench_function("combined_importance_3000", |b| {
        b.iter(|| algo::pagerank::combined_importance(&centrality_str, &pr))
    });
}

fn build_synthetic_graph_with_clusters(
    node_count: usize,
    edge_count: usize,
) -> (ProjectGraph, ClusterMap, StatsOutput) {
    let graph = build_synthetic_graph(node_count, edge_count);

    // Build clusters from node cluster assignments
    let mut cluster_files: BTreeMap<ClusterId, Vec<CanonicalPath>> = BTreeMap::new();
    for (path, node) in &graph.nodes {
        cluster_files
            .entry(node.cluster.clone())
            .or_default()
            .push(path.clone());
    }

    let mut clusters_map = BTreeMap::new();
    for (id, files) in cluster_files {
        let file_count = files.len();
        clusters_map.insert(
            id,
            Cluster {
                files,
                file_count,
                internal_edges: 0,
                external_edges: 0,
                cohesion: 0.5,
            },
        );
    }
    let clusters = ClusterMap {
        clusters: clusters_map,
    };

    let centrality = algo::centrality::betweenness_centrality(&graph);
    let sccs = algo::scc::find_sccs(&graph);
    let layers = algo::topo_sort::topological_layers(&graph, &sccs);
    let stats = algo::stats::compute_stats(&graph, &centrality, &sccs, &layers);

    (graph, clusters, stats)
}

fn bench_compression_l0(c: &mut Criterion) {
    let (graph, clusters, stats) = build_synthetic_graph_with_clusters(10000, 25000);
    c.bench_function("compression_l0_10000", |b| {
        b.iter(|| algo::compress::compress_l0(&graph, &clusters, &stats))
    });
}

fn bench_compression_l1(c: &mut Criterion) {
    let (graph, clusters, stats) = build_synthetic_graph_with_clusters(3000, 8000);
    let first_cluster = clusters.clusters.keys().next().unwrap().clone();
    c.bench_function("compression_l1_cluster", |b| {
        b.iter(|| algo::compress::compress_l1(&graph, &clusters, &stats, &first_cluster))
    });
}

fn bench_compression_l2(c: &mut Criterion) {
    let (graph, clusters, stats) = build_synthetic_graph_with_clusters(3000, 8000);
    let first_file = graph.nodes.keys().next().unwrap().clone();
    c.bench_function("compression_l2_3000", |b| {
        b.iter(|| algo::compress::compress_l2(&graph, &clusters, &stats, &first_file, 2))
    });
}

fn bench_spectral(c: &mut Criterion) {
    let graph = build_synthetic_graph(3000, 8000);
    c.bench_function("spectral_3000", |b| {
        b.iter(|| algo::spectral::spectral_analysis(&graph, 200, 1e-6))
    });
}

criterion_group!(
    benches,
    bench_scc,
    bench_topo_sort,
    bench_blast_radius,
    bench_centrality,
    bench_pagerank,
    bench_combined_importance,
    bench_compression_l0,
    bench_compression_l1,
    bench_compression_l2,
    bench_spectral,
);
criterion_main!(benches);
