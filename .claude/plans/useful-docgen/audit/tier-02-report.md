---
tier_id: tier-02
audited: 2026-06-03
verdict: PASS
commit: 21dcae75da4ea8e57f9ff4d47a380145986c0d29
---

<scope>
Tier-02 "Deterministic layered-DAG → SVG emitter". Scoped diff = the three files
in the tier `<files>`:
- `crates/ariadne-graph/src/diagram.rs` — NEW (252 lines).
- `crates/ariadne-graph/src/lib.rs` — `mod diagram;` + `pub use diagram::{…}`.
- `crates/ariadne-graph/tests/diagram.rs` — NEW (2 tests).

The working tree also carries uncommitted changes from sibling tiers (tier-01
`doc_model.rs`/`doc_scope.rs`; later-tier `docgen.rs`, daemon/mcp threading).
Those are out of scope for this audit — the tier-02 `lib.rs` hunk adds only the
`diagram` mod + re-export; the `doc_model` lines on the same diff belong to
tier-01 (audited PASS). No file outside tier-02 `<files>` was touched by this
tier's work.
</scope>

<checks_run>
- plan_adherence: exactly the three `<files>` entries changed by tier-02; nothing
  outside the list. `lib.rs` stays a façade (mod decl + re-export, no logic).
- correctness: `render_svg` logic walked against `<steps>` 1-6. Cap → index →
  edge filter → layering → positioning → emit. Edge/empty cases checked.
- architecture: `diagram.rs` is pure, IO-free, depends only on `petgraph` + std;
  no `ariadne-daemon`/`ariadne-mcp` import — hexagon intact (plan constraint).
- tests: both tier-02 tests assert behaviour (determinism, well-formed
  structure, truncation w/ visible drop count), fail loud.
- determinism: only `BTreeMap`/`BTreeSet`/sorted `Vec`/integer coords; no float
  formatting, no `HashMap`, no RNG, no map-iteration-order in output.
- exit_criteria: all four verified independently (below).

Re-ran every `<verification>` command (full output captured):
- `cargo nextest run -p ariadne-graph` → 48 passed, 0 skipped. Both diagram
  tests pass (`render_svg_is_deterministic_and_well_formed`,
  `over_cap_input_truncates_with_visible_dropped_count`).
- `cargo clippy -p ariadne-graph --all-targets --all-features -- -D warnings`
  → exit 0 (workspace clippy also Finished clean, no warnings).
- `cargo fmt --all --check` → exit 0.
- `cargo deny check` → `advisories ok, bans ok, licenses ok, sources ok`
  (only benign `license-not-encountered` allowance warnings) — no new dependency.
- `cargo test --test architecture` → `architecture_invariants_hold ... ok`.
</checks_run>

<exit_criteria_check>
1. Byte-identical across two calls — `render_svg` builds output purely from
   sorted/integer state; test asserts `first == second` (and a second equality
   in the truncation test). PASS.
2. Well-formed: one `<svg>` root w/ viewBox+width+height+xmlns
   (diagram.rs:115-119), one arrow `<marker>` (120-124), one `<rect>`+`<text>`
   per node (138-156), one `<line>` per edge (126-136). Test asserts 4 rect /
   4 text / 4 line / 1 marker on the 4-node DAG. PASS.
3. Over-cap → deterministic truncation + visible `dropped N nodes` annotation
   (diagram.rs:62-65, 158-166); `select_kept` is degree-desc / id-asc, no silent
   cap. Test asserts `dropped 20 nodes` and `<rect> ≤ 40` on a 60-node fan. PASS.
4. nextest + clippy -D warnings + fmt --check + cargo deny all green; no new
   dep. PASS.
</exit_criteria_check>

<findings>
| id | category | severity | location | problem | fix |
| --- | --- | --- | --- | --- | --- |
| F1 | correctness | INFO | diagram.rs:75-82 | Duplicate input edges with the same `(from,to)` collapse via `BTreeSet`, so emitted `<line>` count can be below `edges.len()` — "one edge per edge" holds only for distinct edges. | Intended dedup; document for tier-03/04 callers that edge sets are deduped. Non-blocking. |
| F2 | correctness | INFO | diagram.rs:181-188 | A self-loop edge (`from == to`, both present) increments that node's degree twice in `select_kept`, nudging cap priority; self-loops are then dropped from rendering at line 78. | Standard graph-degree semantics; acceptable. Note for tier-03/04 if self-edges enter crate/layer graphs. Non-blocking. |
</findings>

<verdict>
PASS. Zero FAIL findings. The emitter is pure, deterministic, well-formed, and
caps without silence — matching every `<step>`, `<decision>` (D1/D2 layering via
`condensation`+`toposort`, integer grid, façade re-export), and exit criterion.
The longest-path rank in `layer_assignment` (diagram.rs:202-235) is a correct
strengthening of `render_layers`' topo-index layering, exactly as `<step>` 2
requested; the `condensation(make_acyclic=true)` + `toposort` + `expect` message
mirror the committed `render_layers` (docgen.rs:405-444), whose output is already
under a passing golden byte test (`golden_project_doc`), corroborating
cross-run condensation determinism. Two INFO notes are downstream-facing and do
not gate.
</verdict>

<next_steps>
None for tier-02. Proceed to tier-03 (project content) per plan order; it will
be the first consumer of `render_svg` via `architecture_svg`, so honour the F1/F2
notes when building the crate-level node/edge set.
</next_steps>

<sources>
- repo: crates/ariadne-graph/src/diagram.rs; crates/ariadne-graph/tests/diagram.rs;
  crates/ariadne-graph/src/lib.rs; crates/ariadne-graph/src/docgen.rs:405-444
  (`render_layers` mirrored pattern).
- [petgraph 0.8.0 algo (condensation, toposort)](https://docs.rs/petgraph/0.8.0/petgraph/algo/)
- [MDN — SVG Element reference](https://developer.mozilla.org/en-US/docs/Web/SVG/Element)
- [Layered graph drawing — Wikipedia](https://en.wikipedia.org/wiki/Layered_graph_drawing)
- plan.md D1/D2, `<constraints>`, `<risks>`/tier-02.
</sources>
