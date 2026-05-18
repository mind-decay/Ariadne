---
tier_id: tier-07
title: Live graph + analytics (blast-radius, coupling/cohesion, cycles, dead code)
deps: [tier-01, tier-02, tier-04, tier-05]
exit_criteria:
  - `GraphIndex` builds an in-RAM `petgraph::stable_graph::StableDiGraph` over symbols (nodes) and typed edges (Calls, Imports, TypeOf, Defines, Overrides, Reads, Writes).
  - blast_radius(symbol, depth, edge_kinds) computes reverse-reachable set + immediate dominators using Cooper et al. simple_fast [src: https://docs.rs/petgraph/latest/petgraph/algo/dominators/index.html].
  - coupling_report(scope) returns afferent (Ca), efferent (Ce), instability I = Ce/(Ca+Ce), abstractness, distance-from-main-sequence per module.
  - cycle_report() returns Tarjan SCCs of size ≥2 [src: https://docs.rs/petgraph/latest/petgraph/algo/fn.tarjan_scc.html].
  - dead_code() returns symbols with fan_in == 0 AND not exported AND not a test/main entry; configurable via project config.
  - Proptest: on synthetic graphs with known invariants (chain, cycle, complete bipartite), analytics return exact expected results.
  - Criterion: blast_radius depth=3 on 1M-edge graph in <100ms p95.
status: pending
---

<context>
This is the analytics core that powers most MCP tools. petgraph is the de-facto Rust graph lib: Tarjan + Kosaraju SCC, Cooper simple_fast dominators, BFS/DFS [src: https://docs.rs/petgraph]. Static graph metrics (afferent/efferent coupling, instability index) are textbook software-architecture quality indicators [src: https://win.tue.nl/~aserebre/2IS55/2009-2010/10.pdf].
</context>

<files>
- crates/ariadne-graph/Cargo.toml — petgraph, fxhash, smallvec, rayon, workspace deps.
- crates/ariadne-graph/src/lib.rs — re-exports `GraphIndex`, `EdgeKind`, analytics structs, `GraphError`.
- crates/ariadne-graph/src/build.rs — builds GraphIndex from `ariadne-storage::ReadSnapshot`.
- crates/ariadne-graph/src/blast.rs — `blast_radius(symbol, depth, kinds)` BFS + dominator tree.
- crates/ariadne-graph/src/coupling.rs — module-level Ca/Ce/I + abstractness.
- crates/ariadne-graph/src/cycles.rs — Tarjan SCC, returns components of size ≥2.
- crates/ariadne-graph/src/dead.rs — fan_in==0 detector with exporter heuristic.
- crates/ariadne-graph/src/plan_assist.rs — "what files must I touch to change symbol X" — ranked by reverse-reachable weight.
- crates/ariadne-graph/tests/synthetic.rs — proptest on hand-crafted graphs.
- crates/ariadne-graph/tests/golden_repo.rs — on a fixture mini-repo, insta snapshots of each analytic.
- crates/ariadne-graph/benches/blast.rs — criterion on a synthetic 1M-edge graph.
</files>

<steps>
1. **Failing test first** (tests/synthetic.rs): chain A→B→C→D, blast_radius(D, depth=10, kinds=All) returns {A,B,C}; cycle A→B→A produces SCC {A,B}; expected fan_in for D == 1. Fails until steps 3-8 implemented.
2. Define `EdgeKind`: Calls, Imports, TypeOf, Defines, Overrides, Reads, Writes, Inherits. Bitflag for filter sets via `bitflags` crate.
3. GraphIndex internal: `petgraph::stable_graph::StableDiGraph<SymbolId, EdgeMeta>` plus `FxHashMap<SymbolId, NodeIndex>` index. StableDiGraph chosen so node indices survive removals (incremental updates do remove) [src: https://docs.rs/petgraph/latest/petgraph/stable_graph/struct.StableGraph.html].
4. build_from_snapshot(&ReadSnapshot) -> GraphIndex: streams symbols + edges from redb in batches; uses rayon::par_iter to populate the petgraph from sharded chunks then merges; constant-time finalization.
5. blast_radius(symbol, depth, kinds):
   - Step 1: reverse-BFS up to `depth` filtered by `kinds`; collect predecessors.
   - Step 2: compute immediate dominators with `petgraph::algo::dominators::simple_fast(&reversed_subgraph, root=symbol)` to rank "must touch" vs "may touch" [src: https://docs.rs/petgraph/latest/petgraph/algo/dominators/fn.simple_fast.html].
   - Return: BlastRadius { must_touch: Vec<SymbolId>, may_touch: Vec<SymbolId>, depth_used: u8 }.
6. coupling_report(scope: Module):
   - Ca (afferent) = count of distinct symbols outside the module pointing into it.
   - Ce (efferent) = count of distinct outside symbols pointed to from inside it.
   - I = Ce / (Ca + Ce) (0 = max stable, 1 = max unstable).
   - Abstractness A = abstract_decls / total_decls (per module).
   - Distance from main sequence: |A + I - 1| (textbook Martin metric) [src: https://en.wikipedia.org/wiki/Software_package_metrics].
7. cycle_report(): `petgraph::algo::tarjan_scc(&graph)`, filter size ≥2; return list with member symbols sorted [src: https://docs.rs/petgraph/latest/petgraph/algo/fn.tarjan_scc.html].
8. dead_code(): for each node with `incoming_edges_directed(node, Incoming).count() == 0`, exclude if SymbolKind in (Main, Test, ExportPublic, EntryPoint as configured); return remaining list with reason.
9. plan_assist(symbol, max_files):
   - blast_radius depth=∞ filtered to Calls + Imports + TypeOf + Inherits.
   - Group by FileId; rank files by sum of (1 / dominance_depth) of contained symbols.
   - Truncate to max_files; return PlanFile { path, why: Vec<&str>, certainty: f32 } list.
10. **Failing tests** in tests/golden_repo.rs: fixture mini-repo with 5 files, golden insta snapshot of each analytic output. Update snapshots via `cargo insta review` only with audit approval.
11. Criterion bench (benches/blast.rs): synthesize a 100K-node 1M-edge graph (preferential-attachment random); blast_radius depth=3 from 100 random seeds; assert p95 <100ms. Gate in CI.
12. Incremental update API: `GraphIndex::apply_delta(added: Vec<SymbolId>, removed: Vec<SymbolId>, edge_diff: EdgeDelta)`; preserves StableDiGraph indices; called from tier-04 when Salsa derives a fresh `edges_for_file`.
</steps>

<verification>
- `cargo nextest run -p ariadne-graph` green; synthetic + golden snapshots stable.
- `cargo bench -p ariadne-graph` reports blast_radius p95 ≤100ms on 1M edges.
- Manual: on the ariadne_v2 self-index (after tier-05 ingests Rust), run plan_assist for `ariadne_storage::WriteTxn::apply_changeset` — expect the listed files to include all callers + the storage crate itself.
- Property check: random insertion order produces identical analytics outputs (graph build is order-insensitive).
</verification>

<rollback>
`git rm -r crates/ariadne-graph` + workspace member removal.
</rollback>
