---
tier_id: tier-02
title: Deterministic layered-DAG → SVG emitter
deps: []
exit_criteria:
  - "render_svg(nodes, edges) returns byte-identical output across two calls with the same input (golden test)"
  - "output is well-formed: one <svg> root with viewBox, one <rect>+<text> per node, one arrowhead <marker>, one edge element per edge"
  - "an over-cap input (> MAX_NODES) yields a deterministic truncated diagram with a visible dropped-count annotation (no silent cap)"
  - "cargo nextest -p ariadne-graph + clippy -D warnings + fmt --check + cargo deny (no new dep) all green"
status: pending
---

<context>
The diagram engine. A pure, IO-free `ariadne-graph` module that turns a small directed graph
(≤ MAX_NODES) into a deterministic SVG string, reusing `petgraph` layering already present in
`render_layers`. Standalone (no dep on tier-01); tiers 03/04 feed it crate- and neighbourhood-
level node sets [src: plan.md D1-D2; crates/ariadne-graph/src/docgen.rs:391-430].
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
   `render_svg` is byte-identical across two calls; assert the string starts with
   `<svg ` and `viewBox=`, contains exactly 4 `<rect`, 4 `<text`, 4 edge elements, and one
   `<marker id="arrow"`. Build a 60-node fan and assert the output contains a `dropped N nodes`
   annotation and ≤ MAX_NODES rects.
2. Layering: condense cycles (`petgraph::algo::condensation`, make_acyclic=true) then `toposort`,
   exactly as `render_layers` does; layer index = longest-path rank over the condensation
   [src: crates/ariadne-graph/src/docgen.rs:406-408; https://en.wikipedia.org/wiki/Layered_graph_drawing].
   Map every input node to its condensation layer.
3. Positioning (deterministic): sort nodes within a layer by `id`; `x = col * COL_W`,
   `y = layer * ROW_H`, fixed box `W×H`. Format all coordinates with integer precision
   (`{:.0}`) — no floats in output, no map iteration order [src: plan.md risks/tier-02].
4. Emit SVG with `std::fmt::Write`: `<svg viewBox="0 0 {w} {h}" width height xmlns>`, a
   `<defs><marker id="arrow" …><polygon …></marker></defs>` block, one `<rect rx>`+`<text
   text-anchor="middle">` per node, one `<line … marker-end="url(#arrow)">` per edge
   [src: https://developer.mozilla.org/en-US/docs/Web/SVG/Element]. XML-escape labels.
5. Cap: if `nodes.len() > opts.max_nodes`, keep the top `max_nodes` by degree (descending degree,
   then `id` ascending), drop the rest, and append a `<text>` annotation `dropped N nodes`.
   Truncation set is deterministic.
6. Re-export from `lib.rs` (façade only). No new crate dependency.
</steps>

<verification>
- `cargo nextest run -p ariadne-graph` → diagram tests green (determinism, structure, truncation).
- Pipe a test SVG to a file and open it in a browser / IDE preview to confirm it visually draws
  (one-time human check; the byte assertions are the regression guard).
- `cargo clippy … -D warnings`; `cargo fmt --all --check`; `cargo deny check` (proves pure-Rust,
  no new dependency); `cargo test --test architecture`.
</verification>

<rollback>
Delete `diagram.rs` + `tests/diagram.rs`; revert the `lib.rs` re-export line. No other crate
references the module until tier-03/04, so removal is isolated.
</rollback>
