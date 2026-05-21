---
tier_id: tier-09
title: Component-graph MCP surface, analytics, and framework E2E + SLO
deps: [tier-05, tier-06, tier-08]
exit_criteria:
  - "`blast_radius` and `coupling_report` traverse `Renders`/`UsesHook` edges — verified by a fixture where a leaf component's blast radius includes its parents."
  - "An MCP surface exposes the component graph: `file_summary` lists `Component` symbols, and components carry their rendered children + used hooks (new tool or extended existing tool — golden `insta` fixture)."
  - "`ariadne-e2e` indexes a real React, Vue, Svelte, and Astro repository; each asserts non-zero `Component` symbols and `Renders` edges."
  - "The tier-13 SLO gate is re-run on the framework corpus: cold <60s, incremental p95 <500ms, query p95 <100ms — green or an explicit, sourced escalation."
  - "`cargo nextest run --workspace`, `cargo clippy ... -D warnings`, `cargo test --test architecture` all green."
status: pending
---

<context>
Final tier. The parser (02/03/04), CLI (05), and SCIP (06/07/08) tiers produce
`Component` symbols and `Renders`/`UsesHook` edges; this tier proves the graph
and MCP layers consume them, then validates the whole feature end-to-end on
real repositories under the v1 SLOs. Full context: plan.md `<verification>`.
</context>

<files>
- `crates/ariadne-graph/src/**` — confirm graph algorithms traverse the new `EdgeKind`s; add a `component_graph` use case only if existing tools cannot answer "children + hooks of component X".
- `crates/ariadne-mcp/src/server.rs` — extend `file_summary` (and/or add a `component_graph` tool) to surface `Component` symbols, rendered children, used hooks.
- `crates/ariadne-mcp/tests/` — golden `insta` fixture for the component-graph MCP output.
- `crates/ariadne-graph/tests/` — blast-radius/coupling test over a component-graph fixture.
- `crates/ariadne-e2e/**` — add React/Vue/Svelte/Astro repos to the corpus; per-repo assertions; SLO gate inclusion.
- `crates/ariadne-e2e/fixtures/` or corpus manifest — pinned commits of the four real OSS repos.
</files>

<steps>
1. **Failing test first** (`ariadne-graph` test): build a fixture graph — a leaf
   component rendered by two parents via `Renders` edges — and assert
   `blast_radius(leaf)` includes both parents and `coupling_report` counts the
   `Renders` edges. Red if the graph algorithms filter by `EdgeKind` and drop
   the new variants.
2. Inspect `ariadne-graph`: if blast-radius/coupling iterate edges generically,
   the new `EdgeKind`s flow through with no change — the test goes green by
   construction; keep it as a regression guard. If any algorithm allow-lists
   `EdgeKind`s, add `Renders`/`UsesHook` to that list. Cite the exact site.
3. If "rendered children + used hooks of component X" is not answerable from
   existing edges via existing queries, add a small `component_graph` use case
   to `ariadne-graph` (a typed neighbourhood query over `Renders`/`UsesHook`).
   Do not add it speculatively — only if a real gap is found.
4. MCP: extend `file_summary` in `server.rs` so `Component` symbols are
   labelled as components and carry their rendered children + used hooks; or
   add a dedicated `component_graph` tool. Match the existing tool-definition
   pattern in `server.rs` [src: crates/ariadne-mcp/src/server.rs]. Add the
   golden `insta` fixture; the MCP handshake snapshot tests must stay green.
5. **Failing test first** (`ariadne-e2e`): add a React, a Vue, a Svelte, and an
   Astro real OSS repo (pinned commits) to the corpus; assert each indexes with
   non-zero `Component` symbols and `Renders` edges. Red until the corpus
   manifest + assertions exist.
6. Wire the four framework repos into the existing `slo` gate corpus so the
   tier-13 SLO harness measures cold/incremental/query against a workload that
   includes SFC files [src: .claude/plans/ariadne-core/tier-13-cold-index-slo.md].
7. Run the SLO gate. If a budget is missed, root-cause (likely the multi-region
   SFC parse — R-SLO); apply a non-lossy fix or escalate to the user with the
   measured breakdown. Never weaken an assertion or silently drop SFC files.
8. Per-tier memory probe (plan.md R1 rule): report `memory_report()` deltas for
   any Salsa/graph table the framework langs grow; >256MB per table is a hard fail.
</steps>

<verification>
- `cargo nextest run --workspace` — green: graph component-edge test, MCP
  component-graph golden, e2e per-repo assertions.
- `cargo run -p ariadne-e2e` (or the e2e entry point) indexes all four
  framework repos; the SLO gate reports cold <60s, incremental p95 <500ms,
  query p95 <100ms on the combined corpus.
- Manual MCP session: launch Claude Code with the Ariadne MCP over a Vue
  fixture repo; `file_summary` / `component_graph` returns components with
  their rendered children — output matches the golden fixture.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`,
  `cargo fmt --all --check`, `cargo test --test architecture` — clean.
</verification>

<rollback>
Revert the `server.rs` MCP change and the optional `component_graph` use case;
remove the four repos from the e2e corpus and the SLO gate; delete the new
graph/MCP/e2e tests. The component graph still exists in the index (tiers
02–08) — only its MCP surface and E2E coverage are removed.
</rollback>
