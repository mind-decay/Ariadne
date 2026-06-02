---
tier_id: tier-03
title: Project overview content redesign (for_project + architecture_svg)
deps: [tier-01, tier-02]
exit_criteria:
  - "for_project emits the six insight sections (Synopsis, Architecture, Boundary Violations, Cycle Clusters, Risk Hot-Spots, Refactor & Change-Coupling); no raw 351-row coupling dump, no language-noise glossary"
  - "architecture_svg returns a deterministic crate-level SVG (~12 nodes) reachable from the Markdown via ![architecture](codebase-overview.svg)"
  - "Cycle Clusters names the largest SCC with its member count and ≥1 suggested cut edge; Risk Hot-Spots rank by churn×complexity over source-scoped files only"
  - "golden Markdown test green; render twice → identical bytes; cargo clippy/fmt/deny/architecture green"
status: pending
---

<context>
Replace the project overview's raw metric dumps with deterministic, system-only insight, and
swap the unrenderable 351-node Mermaid for a crate-level SVG (tier-02) referenced as a sidecar.
Consumes tier-01 scoping + crate grouping and the existing `coupling`/`cycles`/`dead`/`hotspot`/
`co_change`/`heuristics` use cases [src: plan.md D2/D4/D5; crates/ariadne-graph/src/co_change.rs:555;
crates/ariadne-graph/src/hotspot.rs]. Full context: plan.md.
</context>

<files>
- crates/ariadne-graph/src/docgen.rs — MODIFY `for_project`: rewrite section assembly.
- crates/ariadne-graph/src/docgen_insights.rs — NEW. Deterministic helpers: `synopsis`,
  `architecture_section` (crate roles), `boundary_violations`, `cycle_clusters`,
  `risk_hotspots`, `change_coupling`. Pure `std::fmt::Write`.
- crates/ariadne-graph/src/docgen.rs — ADD `pub fn architecture_svg(graph, modules, scope) -> String`
  (crate-aggregated node/edge set → `diagram::render_svg`).
- crates/ariadne-graph/src/lib.rs — re-export `architecture_svg`.
- crates/ariadne-graph/tests/docgen_fixture.rs — MODIFY golden to the new sections.
- crates/ariadne-graph/tests/docgen_project.rs — NEW. asserts each section + SVG determinism.
</files>

<steps>
1. Write failing `tests/docgen_project.rs` over the test fixture graph: assert the Markdown
   contains headers `## Architecture`, `## Boundary violations`, `## Cycle clusters`,
   `## Risk hot-spots`, `## Refactor & change-coupling`; assert it references the sidecar SVG;
   assert `architecture_svg` is byte-identical across two calls.
2. **Synopsis**: one deterministic paragraph — scoped crate count, layer count, languages,
   source symbol/edge totals (reuse `graph.symbol_count`/`edge_count` over scoped set).
3. **Architecture**: aggregate scoped file-modules into crates via `crate_of` (tier-01); per crate
   emit role from coupling shape (`purpose()` logic) + layer (domain/adapter/interior); embed
   `![architecture](codebase-overview.svg)` and call `architecture_svg` to produce the bytes.
4. **Boundary violations** (D5): iterate symbol edges; flag edge whose source crate/layer →
   target crate/layer breaks an invariant (domain→adapter, adapter→adapter cross-crate,
   non-core→core-only rules) [src: CLAUDE.md `<architecture>`; tests/architecture.rs]. List each
   violating edge `src → dst`; if none, state so explicitly.
5. **Cycle clusters**: from `graph.cycle_report`, rank SCCs by member count; for the top clusters
   list size + representative members + a suggested cut edge (the edge whose removal reduces the
   SCC, chosen deterministically by lowest endpoint id) [src: crates/ariadne-graph/src/cycles.rs].
6. **Risk hot-spots**: rank source-scoped files by churn×complexity using the existing tier-12/13
   use cases; confirm exact public fn signatures in `crate::hotspot`/`crate::co_change`/complexity
   source in-session before calling [src: plan.md risks]. Replace the old Ce+dead score table.
7. **Refactor & change-coupling**: god modules from `weak_spots`/`heuristics`; plus file pairs from
   `co_change_report` that co-change but have no structural edge (hidden coupling)
   [src: crates/ariadne-graph/src/co_change.rs:555]. Drop the language-noise glossary.
8. Update the `docgen_fixture` golden; verify byte-determinism by rendering twice.
</steps>

<verification>
- `cargo nextest run -p ariadne-graph` → docgen_project + docgen_fixture green.
- `cargo nextest run -p ariadne-daemon -p ariadne-mcp` → doc_for_project still returns Markdown.
- Determinism: call `for_project` and `architecture_svg` twice in the test → assert equal.
- `cargo clippy … -D warnings`; `cargo fmt --all --check`; `cargo deny check`; `cargo test --test architecture`.
</verification>

<rollback>
`git checkout -- crates/ariadne-graph/src/docgen.rs crates/ariadne-graph/tests`; delete
`docgen_insights.rs` + `tests/docgen_project.rs`; revert the `lib.rs` re-export. tier-01/02
modules remain intact.
</rollback>
