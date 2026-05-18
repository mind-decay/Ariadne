---
tier_id: tier-09
title: Static doc-gen + refactor suggestion engine (graph-driven, deterministic)
deps: [tier-01, tier-02, tier-04, tier-05, tier-07]
exit_criteria:
  - `docgen::for_module(module_id)` produces Markdown summary: purpose (inferred from imports/exports), public API table, fan-in/out, top dependents, cycles touched.
  - `docgen::for_project()` emits a top-level Markdown architecture overview (modules, layers via SCC condensation, hot-spots).
  - `refactor::god_modules(threshold)` returns modules with Ce > threshold AND cohesion < 0.3 with specific suggestion entries.
  - `refactor::cycle_break_proposals(scc)` returns ranked edge-removal candidates: lowest-fan_in edges first; references CWE / DDD anti-pattern names where applicable.
  - `refactor::misplaced_symbols()` finds symbols whose primary callers all live in a different module → suggest moving.
  - All output deterministic (same revision → same bytes); golden insta tests on a fixture repo.
  - Tools exposed via MCP (tier-08 update): doc_for_module, doc_for_project, refactor_suggestions.
status: pending
---

<context>
Closes the "weak-spots + refactor + doc-gen" promise of v1. Pure static computation, no LLM dependency (D11). Builds atop tier-07 analytics — does not introduce new graph algorithms, only synthesizes existing metrics into actionable structured output.
</context>

<files>
- crates/ariadne-graph/src/docgen.rs — Markdown rendering for module + project.
- crates/ariadne-graph/src/refactor.rs — god_modules, cycle_break_proposals, misplaced_symbols.
- crates/ariadne-graph/src/heuristics.rs — shared scoring helpers (cohesion proxy, edge-weight ranking).
- crates/ariadne-graph/tests/docgen_<name>.rs — insta golden Markdown on fixture repos.
- crates/ariadne-graph/tests/refactor_<name>.rs — insta golden JSON of suggestion lists.
- crates/ariadne-mcp/src/tools/{doc_module,doc_project,refactor}.rs — new MCP tool wrappers.
- crates/ariadne-mcp/tests/tools_doc.rs, tests/tools_refactor.rs — integration tests.
</files>

<steps>
1. **Failing test first** (tests/docgen_fixture.rs): take a 5-file fixture mini-repo; call `docgen::for_module(M)`; assert insta snapshot matches expected Markdown — H1=module name, H2 sections "Purpose", "Public API", "Inbound coupling", "Outbound coupling", "Cycles", "Dead Code". Fails until step 4.
2. cohesion proxy: ratio of intra-module edges to total edges incident to the module. cohesion = intra_edges / (intra_edges + cross_edges). Document choice + simplicity vs LCOM4 [src: https://en.wikipedia.org/wiki/Cohesion_(computer_science)].
3. docgen::for_module(module_id):
   - Inputs: GraphIndex + ariadne-storage::ReadSnapshot.
   - Compute: public symbols (kind=ExportPublic), top-10 callers (sorted by edge count), top-10 callees, cohesion, instability I, abstractness A, cycles intersecting module, dead symbols.
   - Render via templated Markdown writer (no external template engine; small `std::fmt::Write` helpers).
4. docgen::for_project():
   - Tarjan SCC condensation gives the module DAG; topo-sort yields a layer ordering rendered as a Mermaid `flowchart TD` block.
   - Hot-spots = top-5 modules by combined (Ce + cycle membership + dead-code count).
   - Output Markdown with sections: Overview, Layers (Mermaid), Hot-Spots, Coupling table, Glossary (auto-listed top symbols).
5. refactor::god_modules(threshold: f32):
   - For each module: god = (Ce > threshold) && (cohesion < 0.3).
   - Output `GodModuleFinding { module, ce, cohesion, top_outbound: Vec<(SymbolId, count)>, suggestion: &str }` where suggestion text references the highest-Ce sub-cluster: "Consider splitting {top_outbound[0].symbol} out into its own module — currently {pct}% of outbound traffic flows through it".
6. refactor::cycle_break_proposals(scc):
   - For each edge inside the SCC, score = 1 / max(fan_in_src, fan_out_dst).
   - Return edges sorted by score descending — the highest score = lowest-traffic edge, cheapest to invert/remove.
   - Cite "Dependency-Inversion Principle" + Martin's I metric [src: https://win.tue.nl/~aserebre/2IS55/2009-2010/10.pdf].
7. refactor::misplaced_symbols():
   - For each symbol S in module M: compute caller_modules histogram from incoming Calls/Imports edges.
   - If `max(other_module_calls) > 2 * own_module_calls` AND visibility allows movement, emit `MisplacedSymbol { symbol, current_module, target_module, ratio }`.
8. Determinism guarantee: all iterations over hashmaps use sorted-by-key views to avoid order non-determinism in output. Add a proptest asserting `bytes(output) == bytes(output)` across 50 reruns with random insertion orders.
9. **Failing tests** (tests/refactor_*.rs): per-suggestion-type golden JSON on a fixture chosen to trigger that finding (one cycle fixture, one god-module fixture, one misplaced-symbol fixture).
10. MCP wrappers (tier-08 extension): three new tools doc_for_module, doc_for_project, refactor_suggestions returning their respective outputs as serde-serialized structures with `markdown: String` for doc tools.
11. Update tier-08 tests/handshake.rs golden snapshot to include the 3 new tools.
12. Document that refactor suggestions are *hints*, not authoritative — note in MCP tool descriptions so Claude does not present them as commands.
</steps>

<verification>
- `cargo nextest run -p ariadne-graph -p ariadne-mcp` green.
- Manual: run doc_for_project on ariadne_v2 self-index; output Markdown renders correctly on GitHub and Claude Code; Mermaid diagram visualizes layers.
- Determinism: re-run docgen 10x on same revision; assert byte-identical output (sha256 stable).
- Negative: empty project → doc_for_project returns Markdown with "no modules" placeholder, not an error.
</verification>

<rollback>
Module-level additions inside ariadne-graph + ariadne-mcp. Rollback = `git revert` of this tier's commits; or remove `mod docgen; mod refactor; mod heuristics;` from lib.rs and the three new tool wrappers.
</rollback>
