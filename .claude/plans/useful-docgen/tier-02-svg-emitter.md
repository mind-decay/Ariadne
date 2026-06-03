---
tier_id: tier-02
title: Deterministic layered-DAG → SVG emitter
deps: []
exit_criteria:
  - "render_svg(nodes, edges, opts) returns byte-identical output across two calls with the same input (golden test)"
  - "output is well-formed: one <svg> root with viewBox/width/height/xmlns, one <rect>+<text> per node, one arrowhead <marker>, one edge element per edge"
  - "an over-cap input (> opts.max_nodes) yields a deterministic truncated diagram with a visible dropped-count annotation (no silent cap)"
  - "cargo nextest -p ariadne-graph + clippy -D warnings + fmt --check + cargo deny (no new dep) all green"
status: completed
completed: 2026-06-03
---

<context>
The diagram engine. A pure, IO-free `ariadne-graph` module that turns a small directed graph
(≤ opts.max_nodes) into a deterministic SVG string, reusing the `petgraph` layering already
present in `render_layers`. Standalone (no dep on tier-01); tiers 03/04 feed it crate- and
neighbourhood-level node sets [src: plan.md D1-D2; crates/ariadne-graph/src/docgen.rs:391-430].
Full context: plan.md.
</context>

<files>
- crates/ariadne-graph/src/diagram.rs — NEW. `pub struct DiagramNode { id: String, label: String }`,
  `pub struct DiagramEdge { from: String, to: String }`, `pub struct DiagramOpts { max_nodes: usize }`,
  `pub fn render_svg(nodes: &[DiagramNode], edges: &[DiagramEdge], opts: &DiagramOpts) -> String`.
- crates/ariadne-graph/src/lib.rs — re-export `DiagramNode`, `DiagramEdge`, `DiagramOpts`, `render_svg`.
- crates/ariadne-graph/tests/diagram.rs — NEW. determinism, structure, truncation tests.
</files>

<steps>
1. Write failing `tests/diagram.rs`: build a 4-node DAG (a→b, a→c, b→d, c→d); assert
   `render_svg` is byte-identical across two calls; assert the string starts with `<svg ` and
   contains `viewBox=` and `xmlns="http://www.w3.org/2000/svg"`, contains exactly 4 `<rect`,
   4 `<text`, 4 edge elements, and one `<marker id="arrow"` [src:
   https://developer.mozilla.org/en-US/docs/Web/SVG/Element]. Build a 60-node fan with
   `max_nodes = 40` and assert the output contains a `dropped N nodes` annotation and ≤ 40 rects.
2. Layering: condense cycles (`petgraph::algo::condensation(g, true)`) then `toposort`, exactly as
   `render_layers` does; layer index = longest-path rank over the condensation [src:
   crates/ariadne-graph/src/docgen.rs:406-407; https://en.wikipedia.org/wiki/Layered_graph_drawing —
   longest-path layering + toposort are standard Sugiyama steps]. Map every input node to its layer.
3. Positioning (deterministic): sort nodes within a layer by `id`; `x = col * COL_W`,
   `y = layer * ROW_H`, fixed box `W×H`. Format all coordinates with integer precision (`{:.0}` /
   integer types) — no float formatting in output, no map iteration order [src: plan.md risks/tier-02].
4. Emit SVG with `std::fmt::Write`: `<svg viewBox="0 0 {w} {h}" width="{w}" height="{h}"
   xmlns="…">`, a `<defs><marker id="arrow" markerWidth markerHeight refX refY orient="auto">
   <polygon points="…"/></marker></defs>` block, one `<rect rx>`+`<text text-anchor="middle">` per
   node, one `<line … marker-end="url(#arrow)">` per edge [src:
   https://developer.mozilla.org/en-US/docs/Web/SVG/Element — all standard SVG 1.1/2]. XML-escape labels.
5. Cap: if `nodes.len() > opts.max_nodes`, keep the top `max_nodes` by degree (descending degree,
   then `id` ascending), drop the rest, and append a `<text>` annotation `dropped N nodes`.
   The truncation set is deterministic.
6. Re-export from `lib.rs` (façade only). No new crate dependency.
</steps>

<verification>
- `cargo nextest run -p ariadne-graph` → diagram tests green (determinism, structure, truncation).
- One-time human check: pipe a test SVG to a file and open it in a browser / IDE preview to confirm
  it visually draws; the byte assertions are the regression guard.
- `cargo clippy … -D warnings`; `cargo fmt --all --check`; `cargo deny check` (proves pure-Rust,
  no new dependency); `cargo test --test architecture`.
</verification>

<rollback>
Delete `diagram.rs` + `tests/diagram.rs`; revert the `lib.rs` re-export line. No other crate
references the module until tier-03/04, so removal is isolated.
</rollback>
