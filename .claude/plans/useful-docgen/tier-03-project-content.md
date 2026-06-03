---
tier_id: tier-03
title: Project overview content redesign (for_project + architecture_svg)
deps: [tier-01, tier-02]
exit_criteria:
  - "for_project emits the insight sections (Synopsis, Architecture, Boundary violations, Cycle clusters, Risk hot-spots, Refactor & change-coupling); no per-file Martin dump, no language-noise glossary, no Mermaid block"
  - "architecture_svg returns a deterministic crate-level SVG (~12 nodes) reachable from the Markdown via ![architecture](codebase-overview.svg)"
  - "Cycle clusters names the largest SCC with its member count and >=1 suggested cut edge; Risk hot-spots rank by churn x complexity over source-scoped files only"
  - "git-history vectors threaded through; empty history degrades to an explicit 'history unavailable' line, deterministically"
  - "golden Markdown test green; render twice -> identical bytes; cargo clippy/fmt/deny/architecture green"
status: pending
---

<context>
Replace the project overview's raw metric dumps with deterministic, system-only insight, and
swap the unrenderable condensation-node Mermaid for a crate-level SVG (tier-02) referenced as a
sidecar. Consumes tier-01 scoping + crate grouping and the existing graph-pure use cases
`coupling`/`cycles`/`dead`/`hotspot`/`co_change`/`refactor`/`heuristics`. Risk + change-coupling
need git-history vectors threaded into `for_project` (D6) [src: plan.md D2/D4/D5/D6;
crates/ariadne-graph/src/hotspot.rs:102-120; co_change.rs:74-107; refactor.rs:80]. Full context: plan.md.
</context>

<files>
- crates/ariadne-graph/src/docgen.rs — MODIFY `for_project`: extend signature to
  `for_project(graph, snap, modules, churn: &[FileChurn], co_change: &[CoChangePair], scope: &DocScope)`;
  rewrite the section assembly.
- crates/ariadne-graph/src/docgen.rs — ADD `pub fn architecture_svg(graph, modules, scope) -> String`
  (crate-aggregated node/edge set via `crate_of` → `diagram::render_svg`).
- crates/ariadne-graph/src/docgen_insights.rs — NEW. Deterministic helpers: `synopsis`,
  `architecture_section`, `boundary_violations`, `cycle_clusters`, `risk_hotspots`, `change_coupling`,
  `file_complexity_map` (fold `SymbolRecord.complexity` per file from the snapshot). Pure `std::fmt::Write`.
- crates/ariadne-graph/src/lib.rs — re-export `architecture_svg` (façade only).
- crates/ariadne-daemon/src/domain/queries/docs.rs — MODIFY `doc_for_project`: pass `&cat.churn`,
  `&cat.co_change`, `&DocScope::default()` [src: catalog.rs:147-152].
- crates/ariadne-mcp/src/tools/doc_project.rs — MODIFY: pass `&cat.churn`, `&cat.co_change`,
  `&DocScope::default()` (cold `Catalog` already carries them) [src: crates/ariadne-mcp/src/catalog.rs:84-92].
- crates/ariadne-graph/tests/docgen_fixture.rs — MODIFY golden to the new sections.
- crates/ariadne-graph/tests/docgen_project.rs — NEW. asserts each section + SVG determinism + empty-history path.
</files>

<steps>
1. Write failing `tests/docgen_project.rs` over the test fixture graph: assert the Markdown contains
   headers `## Architecture`, `## Boundary violations`, `## Cycle clusters`, `## Risk hot-spots`,
   `## Refactor & change-coupling`; assert it references the sidecar SVG; assert `architecture_svg` is
   byte-identical across two calls; assert an empty-`churn` call emits the "history unavailable" line.
2. **Synopsis**: one deterministic paragraph — scoped crate count, layer count, languages, source
   symbol/edge totals (`graph.symbol_count`/`edge_count` over the scoped set).
3. **Architecture**: aggregate scoped file-modules into crates via `crate_of` (tier-01); per crate emit
   role from coupling shape (`purpose()` logic, docgen.rs:242-252) + layer (`LayerHint`); embed
   `![architecture](codebase-overview.svg)` and call `architecture_svg` to produce the bytes.
4. **Boundary violations** (D5): iterate symbol edges; flag an edge whose source crate/layer →
   target crate/layer breaks an invariant (domain→adapter, adapter→adapter cross-crate, non-core→core
   rules) [src: CLAUDE.md `<architecture>`; tests/architecture.rs]. List each violating edge
   `src → dst`; if none, state so explicitly.
5. **Cycle clusters**: from `graph.cycle_report()` (CycleReport{Vec<Cycle>}, each `Cycle.members`
   sorted) rank SCCs by member count; for the top clusters list size + representative members + a
   suggested cut edge — the graph edge between two members chosen deterministically by lowest
   (source id, target id) [src: crates/ariadne-graph/src/cycles.rs:27-39].
6. **Risk hot-spots**: build `file_complexity_map` by folding `SymbolRecord.complexity` per file from
   the snapshot (mirror analytics.rs:35-40), call `hotspot::file_hotspots(churn, &map)`, keep entries
   whose file is source-scoped, rank top-N. Replace the old `Ce + cycles + dead` score table
   [src: crates/ariadne-graph/src/hotspot.rs:102-120; analytics.rs:31-55].
7. **Refactor & change-coupling**: god modules from `refactor::god_modules` (graph-pure, refactor.rs:80);
   plus file pairs from `co_change::co_change_report(churn, co_change, &CoChangeConfig::default())` that
   co-change but have **no** structural edge in the graph (hidden coupling) [src: co_change.rs:74-107].
   Drop the language-noise glossary entirely.
8. Update the `docgen_fixture` golden; verify byte-determinism by rendering twice.
</steps>

<verification>
- `cargo nextest run -p ariadne-graph` → docgen_project + docgen_fixture green.
- `cargo nextest run -p ariadne-daemon -p ariadne-mcp` → doc_for_project still returns Markdown.
- Determinism: call `for_project` and `architecture_svg` twice in the test → assert equal.
- `cargo test --test architecture` (no graph→daemon dep introduced; D6 guard).
- `cargo clippy … -D warnings`; `cargo fmt --all --check`; `cargo deny check`.
</verification>

<rollback>
`git checkout -- crates/ariadne-graph/src/docgen.rs crates/ariadne-graph/tests crates/ariadne-daemon
crates/ariadne-mcp`; delete `docgen_insights.rs` + `tests/docgen_project.rs`; revert the `lib.rs`
re-export. tier-01/02 modules remain intact.
</rollback>
