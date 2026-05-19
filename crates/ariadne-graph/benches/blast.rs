//! Criterion bench for `blast_radius` depth=3 on a synthetic
//! ~1M-edge Barabási–Albert preferential-attachment graph. Tier-07
//! step 11 / SLO: p95 ≤100ms [src: tier-07 `exit_criteria` +
//! <https://en.wikipedia.org/wiki/Barab%C3%A1si%E2%80%93Albert_model>].
//!
//! Preferential attachment produces the heavy-tailed in-degree
//! distribution that exercises `simple_fast` on hub seeds — the worst
//! case the plan intends to gate. Uniform Erdős–Rényi would mask that
//! cost.
//!
//! The graph builds once (outside the timed region). Sample seeds are
//! biased toward high-in-degree nodes so per-call timings reflect the
//! hub-heavy reality the SLO targets.

#![allow(clippy::cast_possible_truncation)]

use ariadne_core::SymbolId;
use ariadne_graph::{EdgeKind, EdgeKindSet, GraphIndex};
use criterion::{Criterion, criterion_group, criterion_main};
use std::hint::black_box;

const N_NODES: usize = 100_000;
const TARGET_EDGES: usize = 1_000_000;
const SAMPLE_SEEDS: usize = 100;
/// Edges per newly-attached node — Barabási–Albert "m" parameter.
/// `TARGET_EDGES / N_NODES ≈ 10` puts the average degree where the SLO
/// is meant to bite.
const M_PER_NODE: usize = TARGET_EDGES / N_NODES;
const M0_SEED: usize = M_PER_NODE + 1;

fn sid(n: u64) -> SymbolId {
    SymbolId::new(n).expect("non-zero")
}

/// Linear congruential generator — deterministic, no extra crate
/// dependency, matches Numerical Recipes constants.
fn lcg(state: &mut u64) -> u64 {
    *state = state
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1_442_695_040_888_963_407);
    *state
}

/// Build a Barabási–Albert graph: each new node `i ≥ M0_SEED` attaches
/// to `M_PER_NODE` existing nodes drawn with probability proportional
/// to current in-degree. Implementation uses the standard cumulative
/// endpoint list (Newman 2003): every time an edge lands on node `v`,
/// `v` is appended to `endpoints`; sampling a uniform index into
/// `endpoints` is then equivalent to sampling by in-degree.
fn build_pa_graph() -> (GraphIndex, Vec<SymbolId>) {
    let mut g = GraphIndex::new();
    #[allow(clippy::cast_possible_truncation)]
    let nodes: Vec<SymbolId> = (1..=N_NODES as u64).map(sid).collect();
    for s in &nodes {
        g.add_symbol(*s);
    }
    // Seed a small clique so preferential attachment has anchors with
    // non-zero degree to attract new edges.
    let mut endpoints: Vec<usize> = Vec::with_capacity(TARGET_EDGES * 2);
    for i in 0..M0_SEED {
        for j in 0..i {
            g.add_edge(nodes[i], nodes[j], EdgeKind::Calls);
            endpoints.push(i);
            endpoints.push(j);
        }
    }
    let mut rng_state: u64 = 0x9E37_79B9_7F4A_7C15;
    let mut emitted = endpoints.len() / 2;
    let mut new_node = M0_SEED;
    while emitted < TARGET_EDGES && new_node < N_NODES {
        let mut targets: smallvec::SmallVec<[usize; 16]> =
            smallvec::SmallVec::with_capacity(M_PER_NODE);
        // Sample `M_PER_NODE` distinct existing targets weighted by
        // current degree via the endpoint list.
        while targets.len() < M_PER_NODE {
            let pick = (lcg(&mut rng_state) as usize) % endpoints.len();
            let candidate = endpoints[pick];
            if candidate == new_node || targets.contains(&candidate) {
                continue;
            }
            targets.push(candidate);
        }
        for &t in &targets {
            if g.add_edge(nodes[new_node], nodes[t], EdgeKind::Calls) {
                endpoints.push(new_node);
                endpoints.push(t);
                emitted += 1;
            }
        }
        new_node += 1;
    }
    // Bias seeds toward high-degree nodes by sampling from `endpoints`
    // directly — same proportional-to-degree distribution.
    let mut seeds: Vec<SymbolId> = Vec::with_capacity(SAMPLE_SEEDS);
    let mut seen = fxhash::FxHashSet::default();
    while seeds.len() < SAMPLE_SEEDS {
        let pick = (lcg(&mut rng_state) as usize) % endpoints.len();
        let node_ix = endpoints[pick];
        if seen.insert(node_ix) {
            seeds.push(nodes[node_ix]);
        }
    }
    (g, seeds)
}

fn bench_blast(c: &mut Criterion) {
    let (graph, seeds) = build_pa_graph();
    eprintln!(
        "[blast bench] built PA graph: {} nodes, {} edges",
        graph.symbol_count(),
        graph.edge_count()
    );
    c.bench_function("blast_radius_depth_3_1m_edges", |b| {
        b.iter(|| {
            for s in &seeds {
                let br = graph.blast_radius(*s, 3, EdgeKindSet::ALL);
                black_box(br);
            }
        });
    });
}

criterion_group!(benches, bench_blast);
criterion_main!(benches);
