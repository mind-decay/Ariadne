---
slug: useful-docgen
title: Ariadne docgen redesign — insight-driven docs + render-safe pure-Rust SVG
created: 2026-06-02
owners: [user, claude]
review: [user, codex?]
single_tier: false
tiers:
  - tier-01-doc-scope-model
  - tier-02-svg-emitter
  - tier-03-project-content
  - tier-04-module-content
  - tier-05-symbol-enrich
  - tier-06-cli-doc-command
---

<context>
Problem: the generated docs dump raw graph numbers, not insight. `docgen::for_project`
ranks a vendored fixture (`jquery.js`, dead-code 855) as the #1 hot-spot, fills the
glossary with language noise (`new`, `map`, `clone`), tables 351 file-rows of Martin
metrics, and renders the layer diagram as a 351-node Mermaid flowchart that exceeds
Mermaid render limits and does not draw in a bare IDE Markdown preview at all
[src: crates/ariadne-graph/src/docgen.rs:192-232, 365-430; docs/codebase-overview.md:9-983].
Solution: redesign all three doc surfaces (project, module, symbol) to emit deterministic,
system-only insight — symbol-edge boundary violations, cycle clusters, churn×complexity
risk, hidden change-coupling — and replace the unrenderable Mermaid with a committed,
pure-Rust SVG that opens in any Markdown preview without an extension.
In scope: `docgen` content + a deterministic layered-DAG→SVG emitter + source-scoping
(doc-layer filter) + a CLI command that writes `.md`+`.svg`.
Out of scope: LLM/template-engine synthesis (D11), SVG→raster (PNG), mutating the graph,
new analytics metrics — tier-03/04 consume the existing tier-12/13 hotspot/co-change/
complexity use cases [src: .claude/plans/post-v1-roadmap/plan.md tiers 12-13].
</context>

<constraints>
- Deterministic render: same revision → identical bytes, for Markdown *and* SVG; no
  timestamps, no map iteration order, no RNG [src: crates/ariadne-graph/src/docgen.rs:1-8].
- No LLM, no template engine — `std::fmt::Write` synthesis only [src: CLAUDE.md D11; memory `no-llm-features`].
- Pure-Rust, no new dependency on the critical path; reuse pinned `petgraph 0.8`
  [src: Cargo.toml:64-69; CLAUDE.md D5].
- Hexagonal: the SVG/Markdown *bytes* are produced by the `ariadne-graph` use case (pure,
  no IO); file writes happen only in the `ariadne-cli` driving adapter [src: CLAUDE.md D13;
  memory `hexagonal-strict`].
- Scoping is a doc-layer filter, never a graph mutation: `find_references`/`blast_radius`
  on a fixture symbol must still resolve [src: crates/ariadne-daemon/src/domain/queries/docs.rs:16-50].
- TDD: each tier writes a failing golden/structured test before implementation [src: CLAUDE.md `<rules>`].
- ≤200-line authored files; source `.rs` files are exempt (project rule covers plan/skill/audit only).
</constraints>

<decisions>
**D1 — Hand-rolled deterministic layered-DAG→SVG emitter; reject `layout-rs`.** A new pure
`ariadne-graph` module assigns layers by longest-path / topological order (reuse
`petgraph` `condensation`+`toposort`, already used by `render_layers`), positions nodes by a
fixed deterministic rule, and emits SVG via `std::fmt::Write` — mirroring the existing Mermaid
emitter [src: crates/ariadne-graph/src/docgen.rs:391-430; https://en.wikipedia.org/wiki/Layered_graph_drawing].
SVG element syntax (`svg viewBox`, `rect`, `text`, `line`, `path`, `defs/marker`) per
[src: https://developer.mozilla.org/en-US/docs/Web/SVG/Element]. *Rejected:* `layout-rs` 0.1.3 —
a general Graphviz-dot engine with **no documented output determinism**, violating the
same-bytes guarantee, and a new dependency for what is a ≤40-node layered DAG
[src: https://docs.rs/crate/layout-rs/latest].

**D2 — SVG renders at crate/layer granularity, not file granularity.** `ModuleSpec.name` is a
file path (351 file-modules) [src: crates/ariadne-graph/src/coupling.rs:28; daemon docs.rs build_modules].
The architecture diagram aggregates files into crates by the `crates/<name>/` path prefix,
collapsing 351 nodes to ~12. Determinism: prefix grouping + sorted node order. *Rejected:*
file-level diagram with node caps — still a hairball, still hides the big SCC.

**D3 — Source-scoping by deterministic path classifier, configurable.** `classify(path)` →
{Source, Test, Fixture, Vendored, Generated} from path heuristics (`/tests/`, `/fixtures/`,
`/benches/`, `node_modules/`, `target/`, `*.min.js`). Docs default to Source-only; the CLI
exposes extra excludes. Applied at the doc layer (which modules are *reported* + aggregate
metrics), never to the graph [src: per Q3 answer]. *Rejected:* separate test/fixture buckets
(keeps noise visible but still ranks it).

**D4 — SVG is a sidecar file, referenced from Markdown; the CLI writes both.** Markdown
references `![architecture](codebase-overview.svg)` — GitHub and IDE previews render committed
linked SVG without an extension; raw inline `<svg>` is stripped by GitHub and bloats the MCP
payload [src: https://developer.mozilla.org/en-US/docs/Web/SVG/Element]. The read-only MCP tool
returns Markdown text only; the CLI `doc` command (cold path, like `query`) writes `.md`+`.svg`
[src: crates/ariadne-cli/src/commands/query.rs:255-275].

**D5 — Boundary-violation insight is symbol-edge level, complementing the cargo test.**
`tests/architecture.rs` enforces *crate-dependency* invariants; it cannot see a domain *symbol*
referencing an adapter symbol. The project doc reports those finer edges (domain→adapter,
adapter→adapter cross-crate, →core-only) deterministically [src: CLAUDE.md `<architecture>` invariants;
tests/architecture.rs].
</decisions>

<architecture>
- `ariadne-graph` (domain use case, pure): `doc_model` (classify + crate/layer grouping +
  `DocScope`), `diagram` (layered-DAG→SVG), `docgen` (rewritten `for_project`/`for_module` +
  new `architecture_svg`). Insight helpers consume existing `coupling`/`cycles`/`dead`/`hotspot`/
  `co_change`/`heuristics` modules.
- `ariadne-core` (types): additive fields on `DocForReport` for symbol enrichment.
- `ariadne-daemon` (driven query layer): `docs::doc_for*` thread `DocScope`.
- `ariadne-cli` (driving adapter): new `commands/doc.rs` writes `.md`+`.svg`, owns the
  configurable exclude globs. Only this layer touches the filesystem.
- `ariadne-mcp` (driving adapter): renders enriched `DocForReport`; Markdown unchanged in shape
  (now references the sidecar SVG by relative path).
</architecture>

<tech_inventory>
| Tech | Version | Doc fetched this session |
| --- | --- | --- |
| petgraph (layering: condensation, toposort) | 0.8 (pinned) | Cargo.toml:64-69 (reused, not new) |
| SVG 1.1 element syntax | W3C/MDN | https://developer.mozilla.org/en-US/docs/Web/SVG/Element |
| Layered graph drawing (Sugiyama / longest-path) | n/a | https://en.wikipedia.org/wiki/Layered_graph_drawing |
| layout-rs (rejected alternative) | 0.1.3 (2025-04-24) | https://docs.rs/crate/layout-rs/latest |
</tech_inventory>

<risks>
| Risk | Likelihood | Mitigation | Owner |
| --- | --- | --- | --- |
| SVG layout non-determinism (HashMap order, float fmt) | Med | BTreeMap/sorted vecs only; fixed-precision `{:.0}` coords; golden byte test | tier-02 |
| Path heuristics misclassify real source as fixture | Med | golden `classify()` table + config override; assert a known source file is Source | tier-01 |
| Crate-level SVG still over node cap on huge repos | Low | deterministic top-N by degree + annotated drop count (no silent cap) | tier-02 |
| `DocForReport` field add ripples to mcp+cli renderers | Med | additive optional fields; update all renderers in one tier | tier-05 |
| Insight helpers depend on tier-12/13 APIs not yet wired into `GraphIndex` | Low | builder confirms public fn signatures in-session before use | tier-03/04 |
</risks>

<verification>
- Each tier: `cargo nextest run -p <crate>` golden tests green; `cargo clippy --workspace
  --all-targets -- -D warnings`; `cargo fmt --all --check`; `cargo test --test architecture`;
  `cargo deny check` (proves no new dependency).
- End-to-end (tier-06): run `ariadne doc` on this repo, regenerate `docs/codebase-overview.md`
  + `docs/codebase-overview.svg`, open the `.svg` in the IDE Markdown preview and confirm it
  draws; confirm `jquery.js` is absent from hot-spots and the ~100-file SCC is named.
- Determinism: run the generator twice → `diff` is empty for both `.md` and `.svg`.
</verification>

<sources>
- [MDN — SVG Element reference](https://developer.mozilla.org/en-US/docs/Web/SVG/Element)
- [Layered graph drawing — Wikipedia](https://en.wikipedia.org/wiki/Layered_graph_drawing)
- [Sugiyama's algorithm (Eiglsperger et al.) — Springer](https://link.springer.com/chapter/10.1007/978-3-540-31843-9_17)
- [layout-rs crate — docs.rs (rejected)](https://docs.rs/crate/layout-rs/latest)
- crates/ariadne-graph/src/docgen.rs; coupling.rs:28; daemon docs.rs; CLAUDE.md D5/D11/D13
</sources>
