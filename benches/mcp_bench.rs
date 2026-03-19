mod helpers;

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use criterion::{criterion_group, criterion_main, Criterion};

use ariadne_graph::algo;
#[cfg(feature = "serve")]
use ariadne_graph::mcp::state::GraphState;
use ariadne_graph::model::*;

/// Build a synthetic GraphState for MCP benchmarks.
#[cfg(feature = "serve")]
fn build_synthetic_state(node_count: usize, edge_count: usize) -> GraphState {
    let mut nodes = BTreeMap::new();
    for i in 0..node_count {
        let path = CanonicalPath::new(format!("src/file_{}.ts", i));
        nodes.insert(
            path,
            Node {
                file_type: FileType::Source,
                layer: ArchLayer::Unknown,
                arch_depth: (i % 5) as u32,
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
        let to_idx = (i * 7 + 13) % node_count;
        if from_idx != to_idx {
            edges.push(Edge {
                from: node_keys[from_idx].clone(),
                to: node_keys[to_idx].clone(),
                edge_type: EdgeType::Imports,
                symbols: vec![],
            });
        }
    }

    let graph = ProjectGraph { nodes, edges };

    let sccs = algo::scc::find_sccs(&graph);
    let layers = algo::topo_sort::topological_layers(&graph, &sccs);
    let centrality = algo::centrality::betweenness_centrality(&graph);
    let stats = algo::stats::compute_stats(&graph, &centrality, &sccs, &layers);

    let mut cluster_map = BTreeMap::new();
    for i in 0..30 {
        let cid = ClusterId::new(format!("cluster_{}", i));
        let files: Vec<CanonicalPath> = graph
            .nodes
            .iter()
            .filter(|(_, n)| n.cluster == cid)
            .map(|(p, _)| p.clone())
            .collect();
        let file_count = files.len();
        cluster_map.insert(
            cid,
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
        clusters: cluster_map,
    };

    GraphState::from_loaded_data(graph, stats, clusters, BTreeMap::new())
}

#[cfg(feature = "serve")]
fn bench_mcp_overview(c: &mut Criterion) {
    let state = build_synthetic_state(3000, 8000);
    let state = Arc::new(arc_swap::ArcSwap::from_pointee(state));
    let rebuilding = Arc::new(AtomicBool::new(false));

    c.bench_function("mcp_overview_3000", |b| {
        b.iter(|| {
            let s = state.load();
            // Simulate overview computation
            let mut lang_counts: BTreeMap<String, usize> = BTreeMap::new();
            for path in s.graph.nodes.keys() {
                let ext = path
                    .as_str()
                    .rsplit('.')
                    .next()
                    .unwrap_or("unknown")
                    .to_string();
                *lang_counts.entry(ext).or_default() += 1;
            }
            let _ = s.stats.sccs.len();
            let _ = s.clusters.clusters.len();
            let _ = rebuilding.load(Ordering::Relaxed);
        })
    });
}

#[cfg(feature = "serve")]
fn bench_mcp_blast_radius(c: &mut Criterion) {
    let state = build_synthetic_state(3000, 8000);
    let target = state.graph.nodes.keys().next().unwrap().clone();

    c.bench_function("mcp_blast_radius_3000", |b| {
        b.iter(|| algo::blast_radius::blast_radius(&state.graph, &target, None))
    });
}

#[cfg(feature = "serve")]
fn bench_freshness_check(c: &mut Criterion) {
    use ariadne_graph::mcp::state::FreshnessState;

    c.bench_function("freshness_check_10_files", |b| {
        b.iter(|| {
            let mut freshness = FreshnessState::new();
            for i in 0..10 {
                freshness
                    .stale_files
                    .insert(CanonicalPath::new(format!("src/file_{}.ts", i)));
            }
            freshness.recompute_confidence(3000);
        })
    });
}

#[cfg(feature = "serve")]
fn bench_server_startup(c: &mut Criterion) {
    c.bench_function("graphstate_build_3000", |b| {
        b.iter(|| build_synthetic_state(3000, 8000))
    });
}

#[cfg(feature = "serve")]
criterion_group!(
    benches,
    bench_mcp_overview,
    bench_mcp_blast_radius,
    bench_freshness_check,
    bench_server_startup,
);

#[cfg(feature = "serve")]
criterion_main!(benches);

// Stub for --no-default-features
#[cfg(not(feature = "serve"))]
fn main() {}
