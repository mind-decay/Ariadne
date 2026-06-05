---
tier_id: tier-03
title: Exclude out-of-scope (test) edges from crate-level afferent so leaf crates read volatile
deps: [tier-01]
exit_criteria:
  - "architecture_section classifies edge endpoints against a membership universe spanning ALL crate modules (source + test), while the reported rows stay the source-only crate set (12 on the dogfood) — a crate's own test→source edge no longer counts as crate afferent"
  - "A graph-crate fixture with an explicit test-module→source-symbol edge asserts the source crate renders volatile-leaf (instability > 0.7); without the fix that same fixture renders Stable foundational (red-first)"
  - "The existing architecture_role_restored_cli_is_volatile_leaf, boundary_violations_listed_qualified_and_bounded, and architecture_rows_pin_each_crate_layer assertions stay green; synopsis/scope/SVG behaviour unchanged"
  - "coupling_report public API and the daemon per-file coupling_report/weak_spots output are byte-unchanged (the fix is internal to architecture_section, not metrics_for)"
  - "cargo nextest run -p ariadne-graph -p ariadne-daemon -p ariadne-mcp, architecture, clippy, fmt all green; this is the held docgen change — NOT committed here (tier-02 lands it)"
status: completed
completed: 2026-06-05
---

<context>
The Role column mislabels leaf crates (`ariadne-cli`, `ariadne-e2e`) as "Stable
foundational" instead of volatile-leaf. Root cause is a coupling-vs-scope artifact,
NOT the resolver: `for_project` builds crate specs from `scoped` (DocScope =
Source-only, tests excluded) but the graph is never filtered [src:
crates/ariadne-graph/src/docgen.rs:304-316]. `metrics_for` counts an incoming edge
as afferent whenever its source's module-id ≠ this crate; an out-of-scope test
symbol is in NO spec, so a crate's own tests calling its own source (e.g.
`ariadne-e2e/tests/slo.rs → src/domain/connect`) count as crate-level afferent →
instability < 0.3 → "Stable foundational" via `purpose` [src:
crates/ariadne-graph/src/coupling.rs:90-114; crates/ariadne-graph/src/docgen.rs:397-407].

Fix at the `architecture_section` layer (graph crate), not `metrics_for`: build the
crate-coupling membership over ALL crate modules so an intra-crate test→source edge
is same-crate (excluded from afferent), while still emitting one row per source
crate. The per-file `coupling_report` tool keeps its current semantics. Full
rationale + alternatives: plan.md. petgraph semantics relied on here: a directed
`edges_directed(n, Incoming)` yields "all edges TO n", `Outgoing` "all edges FROM n"
[src: https://docs.rs/petgraph/latest/petgraph/graph/struct.Graph.html#method.edges_directed].
</context>

<files>
- crates/ariadne-graph/src/docgen_insights.rs — in `architecture_section`, build the
  crate `ModuleSpec` set the coupling pass consumes from a membership universe over
  ALL crate modules (the unscoped `modules`), not only `scoped`; restrict the emitted
  rows to crates that have ≥1 scoped (source) member so the table stays source-only.
  Pass the unscoped modules in from `for_project` [src: docgen_insights.rs:147-193].
- crates/ariadne-graph/src/docgen.rs — `for_project` passes the unscoped `modules`
  slice to `architecture_section` alongside `scoped` (signature change, internal)
  [src: docgen.rs:315-316].
- crates/ariadne-graph/tests/docgen_project.rs — add a fixture carrying a test-module
  (`crates/<c>/tests/…`)→source-symbol edge; assert the source crate renders
  volatile-leaf; keep the existing role/boundary/layer assertions.
- crates/ariadne-graph/tests/support.rs — extend the crate-path fixture (or add one)
  with a `/tests/` module + a test→source `Calls` edge, mirroring the dogfood shape.
- crates/ariadne-graph/tests/snapshots/docgen_fixture__project.snap — re-accept only if
  the golden moves; review, do not blind-accept.
</files>

<steps>
1. RED. In `docgen_project.rs`, add a fixture: a source crate `ariadne-x` with one
   source symbol `x::run` and a sibling test module `crates/ariadne-x/tests/it.rs`
   whose symbol `it::drives` has a `Calls` edge `it::drives → x::run`, and at least
   one outgoing source edge from `x::run` to another crate so Ce>0. Assert the
   `ariadne-x` Architecture row reads the volatile-leaf string. Run → RED: the
   test→source edge inflates `ariadne-x` afferent, so it reads Stable foundational.
2. THREAD. Change `architecture_section(graph, scoped)` →
   `architecture_section(graph, scoped, modules)` where `modules` is the unscoped
   slice; update the `for_project` call site [src: docgen.rs:315-316].
3. FIX. In `architecture_section`, aggregate the coupling-membership specs from
   `modules` (ALL crate modules, keyed by `crate_of`), so a crate's test symbols are
   members of that crate and an intra-crate test→source edge is same-crate (dropped
   from afferent by `metrics_for`'s `member_of.get(&src) == Some(mid)` guard) [src:
   coupling.rs:104-111]. Compute the displayed rows from the SCOPED crate set (a crate
   with ≥1 source member); skip a coupling row whose crate has no scoped member so
   the table stays source-only and row count is unchanged. Keep `layer_votes` over
   `scoped` (layer is a source property).
4. GREEN. Re-run step-1 fixture → volatile-leaf. Confirm
   `architecture_role_restored_cli_is_volatile_leaf` and the boundary/layer tests
   stay green.
5. ISOLATE the metric path. Add an assertion (or reuse warm_analytics) proving the
   daemon per-file `coupling_report` output is unchanged — the fix must not touch
   `metrics_for` or the public `coupling_report`.
6. SUITE. `cargo nextest run -p ariadne-graph -p ariadne-daemon -p ariadne-mcp`;
   review then `cargo insta accept` any reviewed golden churn; `cargo test --test
   architecture`; clippy; fmt. Do NOT commit — tier-02 lands these held changes.
</steps>

<verification>
- `cargo nextest run -p ariadne-graph` → the new test→source leaf-role fixture is
  green; `architecture_role_restored_cli_is_volatile_leaf`,
  `boundary_violations_listed_qualified_and_bounded`,
  `architecture_rows_pin_each_crate_layer`, `synopsis_crate_count_excludes_tools_dir`,
  `for_project_is_deterministic` all green.
- `cargo nextest run -p ariadne-daemon -p ariadne-mcp` → warm==cold +
  `coupling_report_matches_cold` unchanged-green (proves `metrics_for` untouched).
- `cargo test --test architecture`; clippy `-D warnings`; `cargo fmt --all --check`.
- Fail loudly: if the dogfood (run under tier-02) still shows any source crate's row
  driven by its own test edges, or the row count changes from 12, the membership
  universe is wrong — STOP, do not weaken the fixture assertion.
</verification>

<rollback>
`git checkout -- crates/ariadne-graph/src/docgen.rs
crates/ariadne-graph/src/docgen_insights.rs crates/ariadne-graph/tests/`.
Reverts to the held tier-03 rendering state; the resolver work (tier-01/tier-04) and
HEAD are untouched.
</rollback>
</content>
