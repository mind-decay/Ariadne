//! Synthetic graph invariants. Step 1 of tier-07:
//! chain A→B→C→D, `blast_radius(D, depth=10)` returns {A,B,C}; cycle A→B→A
//! produces SCC {A,B}; expected `fan_in` for D == 1 [src:
//! .claude/plans/ariadne-core/tier-07-graph-analytics.md step 1].
//!
//! Also covers the property-check from `<verification>`: graph build is
//! order-insensitive — analytics outputs do not depend on the insertion
//! sequence of nodes and edges.

#![allow(
    clippy::many_single_char_names,
    clippy::similar_names,
    clippy::unreadable_literal
)]

use std::collections::{BTreeSet, HashSet};

use ariadne_core::{EdgeKind as CoreEdgeKind, SymbolId};
use ariadne_graph::{BlastRadius, EdgeKind, EdgeKindSet, GraphIndex};
use proptest::prelude::*;

fn sid(n: u64) -> SymbolId {
    SymbolId::new(n).expect("non-zero")
}

#[test]
fn chain_blast_radius_reaches_all_predecessors() {
    let mut g = GraphIndex::new();
    let (a, b, c, d) = (sid(1), sid(2), sid(3), sid(4));
    for s in [a, b, c, d] {
        g.add_symbol(s);
    }
    // Chain A → B → C → D.
    g.add_edge(a, b, EdgeKind::Calls);
    g.add_edge(b, c, EdgeKind::Calls);
    g.add_edge(c, d, EdgeKind::Calls);

    let br = g.blast_radius(d, 10, EdgeKindSet::ALL).expect("d present");
    let reached: BTreeSet<SymbolId> = br
        .must_touch
        .iter()
        .chain(br.may_touch.iter())
        .copied()
        .collect();
    assert_eq!(reached, BTreeSet::from([a, b, c]));
}

#[test]
fn cycle_two_node_scc_detected() {
    let mut g = GraphIndex::new();
    let (a, b) = (sid(1), sid(2));
    g.add_symbol(a);
    g.add_symbol(b);
    g.add_edge(a, b, EdgeKind::Calls);
    g.add_edge(b, a, EdgeKind::Calls);

    let report = g.cycle_report();
    assert_eq!(report.cycles.len(), 1, "expected exactly one SCC ≥2");
    let members: BTreeSet<SymbolId> = report.cycles[0].members.iter().copied().collect();
    assert_eq!(members, BTreeSet::from([a, b]));
}

#[test]
fn chain_fan_in_for_tail_is_one() {
    let mut g = GraphIndex::new();
    let (a, b, c, d) = (sid(1), sid(2), sid(3), sid(4));
    for s in [a, b, c, d] {
        g.add_symbol(s);
    }
    g.add_edge(a, b, EdgeKind::Calls);
    g.add_edge(b, c, EdgeKind::Calls);
    g.add_edge(c, d, EdgeKind::Calls);
    assert_eq!(g.fan_in(d), 1);
    assert_eq!(g.fan_in(a), 0);
}

#[test]
fn edge_kind_filter_excludes_other_kinds() {
    let mut g = GraphIndex::new();
    let (a, b) = (sid(1), sid(2));
    g.add_symbol(a);
    g.add_symbol(b);
    g.add_edge(a, b, EdgeKind::Imports);

    let br_calls = g
        .blast_radius(b, 10, EdgeKindSet::CALLS)
        .expect("b present");
    assert!(
        br_calls.must_touch.is_empty() && br_calls.may_touch.is_empty(),
        "Imports edge must not be reachable under a Calls-only filter"
    );
    let br_imports = g
        .blast_radius(b, 10, EdgeKindSet::IMPORTS)
        .expect("b present");
    assert_eq!(br_imports.must_touch.len() + br_imports.may_touch.len(), 1);
}

#[test]
fn blast_radius_filters_reads_and_writes_independently() {
    // SCIP access roles cross into the graph alphabet through `from_core`
    // (scip-driven-edges T2): `Reads`/`Writes` are dedicated kinds, so a
    // `blast_radius` query can isolate readers from writers of a binding.
    assert_eq!(
        EdgeKind::from_core(CoreEdgeKind::Reads),
        EdgeKind::Reads,
        "core Reads must map to graph Reads, not collapse to Calls",
    );
    assert_eq!(
        EdgeKind::from_core(CoreEdgeKind::Writes),
        EdgeKind::Writes,
        "core Writes must map to graph Writes, not collapse to Calls",
    );

    let reach = |br: &BlastRadius| -> BTreeSet<SymbolId> {
        br.must_touch
            .iter()
            .chain(br.may_touch.iter())
            .copied()
            .collect()
    };

    let mut g = GraphIndex::new();
    let (field, reader, writer) = (sid(1), sid(2), sid(3));
    for s in [field, reader, writer] {
        g.add_symbol(s);
    }
    // `reader` reads `field`; `writer` writes `field`. Build through `from_core`
    // so the storage→graph access-role mapping is exercised end-to-end here.
    g.add_edge(reader, field, EdgeKind::from_core(CoreEdgeKind::Reads));
    g.add_edge(writer, field, EdgeKind::from_core(CoreEdgeKind::Writes));

    let reads = g
        .blast_radius(field, 10, EdgeKindSet::READS)
        .expect("field present");
    assert_eq!(
        reach(&reads),
        BTreeSet::from([reader]),
        "a READS filter reaches only the read-edge source",
    );

    let writes = g
        .blast_radius(field, 10, EdgeKindSet::WRITES)
        .expect("field present");
    assert_eq!(
        reach(&writes),
        BTreeSet::from([writer]),
        "a WRITES filter reaches only the write-edge source",
    );

    let both = g
        .blast_radius(field, 10, EdgeKindSet::READS | EdgeKindSet::WRITES)
        .expect("field present");
    assert_eq!(
        reach(&both),
        BTreeSet::from([reader, writer]),
        "READS|WRITES reaches both access sources",
    );

    let calls = g
        .blast_radius(field, 10, EdgeKindSet::CALLS)
        .expect("field present");
    assert!(
        reach(&calls).is_empty(),
        "neither access edge is reachable under a Calls-only filter",
    );
}

proptest! {
    /// Build the same graph in two different insertion orders; assert
    /// SCC + cycle outputs agree. Covers the order-insensitive property
    /// from tier-07 `<verification>`.
    #[test]
    fn graph_build_order_insensitive(
        seed in any::<u64>(),
        n_nodes in 3u64..16,
        n_edges in 0usize..40,
    ) {
        let nodes: Vec<SymbolId> = (1..=n_nodes).map(sid).collect();
        let mut rng_state = seed;
        let edges: Vec<(SymbolId, SymbolId)> = (0..n_edges)
            .map(|_| {
                rng_state = rng_state.wrapping_mul(6364136223846793005).wrapping_add(1);
                let i = (rng_state >> 33) as usize % nodes.len();
                rng_state = rng_state.wrapping_mul(6364136223846793005).wrapping_add(1);
                let j = (rng_state >> 33) as usize % nodes.len();
                (nodes[i], nodes[j])
            })
            .collect();

        let mut a = GraphIndex::new();
        for s in &nodes { a.add_symbol(*s); }
        for (s, d) in &edges { a.add_edge(*s, *d, EdgeKind::Calls); }

        let mut b = GraphIndex::new();
        for s in nodes.iter().rev() { b.add_symbol(*s); }
        for (s, d) in edges.iter().rev() { b.add_edge(*s, *d, EdgeKind::Calls); }

        let cycles_a: HashSet<BTreeSet<SymbolId>> = a
            .cycle_report()
            .cycles
            .into_iter()
            .map(|c| c.members.into_iter().collect())
            .collect();
        let cycles_b: HashSet<BTreeSet<SymbolId>> = b
            .cycle_report()
            .cycles
            .into_iter()
            .map(|c| c.members.into_iter().collect())
            .collect();
        prop_assert_eq!(cycles_a, cycles_b);
    }

    /// Complete bipartite K_{m,n}: every left-side node has an edge to
    /// every right-side node. No cycles, fan_in(right) == m, and
    /// blast_radius(right_j) returns the full left side. Closes
    /// tier-07 exit_criterion #6 (chain, cycle, **complete bipartite**).
    #[test]
    fn complete_bipartite_invariants(
        m in 1u64..8,
        n in 1u64..8,
    ) {
        let mut g = GraphIndex::new();
        let left: Vec<SymbolId> = (1..=m).map(sid).collect();
        let right: Vec<SymbolId> = (m + 1..=m + n).map(sid).collect();
        for s in left.iter().chain(right.iter()) {
            g.add_symbol(*s);
        }
        for l in &left {
            for r in &right {
                g.add_edge(*l, *r, EdgeKind::Calls);
            }
        }

        // No cycles — bipartite graph with edges left→right is acyclic.
        prop_assert!(g.cycle_report().cycles.is_empty());

        let left_set: BTreeSet<SymbolId> = left.iter().copied().collect();
        for r in &right {
            // Every right node has exactly m callers.
            prop_assert_eq!(g.fan_in(*r), usize::try_from(m).unwrap());

            // blast_radius(right_j) reaches the entire left side.
            let br = g
                .blast_radius(*r, 10, EdgeKindSet::ALL)
                .expect("right node present");
            let reached: BTreeSet<SymbolId> = br
                .must_touch
                .iter()
                .chain(br.may_touch.iter())
                .copied()
                .collect();
            prop_assert_eq!(reached, left_set.clone());
        }
    }
}
