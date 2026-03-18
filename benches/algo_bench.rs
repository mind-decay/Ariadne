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

criterion_group!(
    benches,
    bench_scc,
    bench_topo_sort,
    bench_blast_radius,
    bench_centrality,
);
criterion_main!(benches);
