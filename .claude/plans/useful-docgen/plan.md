---
slug: useful-docgen
title: Ariadne docgen redesign — insight-driven docs + render-safe pure-Rust SVG
created: 2026-06-02
revised: 2026-06-03
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
ranks a vendored fixture (`crates/ariadne-parser/fixtures/javascript/jquery.js`, Ca 571)
as the #1 hot-spot — its score is `efferent + cycles + dead`, and the fixture's dead-code
count dominates — fills the glossary with language noise via raw fan-in, tables one Martin
row per indexed file-module (hundreds, no cap), and renders the layer diagram as a
condensation-node Mermaid `flowchart TD` that exceeds Mermaid limits and does not draw in a
bare IDE Markdown preview at all [src: crates/ariadne-graph/src/docgen.rs:192-232 (`for_project`),
326-347 (`push_hotspots`), 350-362 (`push_coupling`), 365-386 (`push_glossary`),
391-430 (`render_layers`); docs/codebase-overview.md (committed, 53 KB)].
Solution: redesign all three doc surfaces (project, module, symbol) to emit deterministic,
system-only insight — symbol-edge boundary violations, cycle clusters, churn×complexity
risk, hidden change-coupling — and replace the unrenderable Mermaid with a committed,
pure-Rust SVG that opens in any Markdown preview without an extension.
In scope: `docgen` content + a deterministic layered-DAG→SVG emitter + source-scoping
(doc-layer filter) + a CLI command that writes `.md`+`.svg`.
Out of scope: LLM/template-engine synthesis (D11), SVG→raster (PNG), mutating the graph,
new analytics metrics — tier-03/04 consume the existing graph-pure churn/complexity/
co-change use cases, not new ones [src: crates/ariadne-graph/src/hotspot.rs, co_change.rs].
Index at revision: 563 — 372 files, 3495 symbols, 5072 edges [src: mcp project_status].
</context>

<constraints>
- Deterministic render: same revision → identical bytes, for Markdown *and* SVG; no
  timestamps, no map iteration order, no RNG [src: crates/ariadne-graph/src/docgen.rs:1-8].
- No LLM, no template engine — `std::fmt::Write` synthesis only [src: CLAUDE.md D11; memory `no-llm-features`].
- Pure-Rust, no new dependency on the critical path; reuse pinned `petgraph 0.8`
  [src: Cargo.toml:64-69; CLAUDE.md D5].
- Hexagonal: the SVG/Markdown *bytes* are produced by the `ariadne-graph` use case (pure,
  no IO); file writes happen only in the `ariadne-cli` driving adapter. `ariadne-graph`
  must never call into `ariadne-daemon`/`ariadne-mcp` analytics handlers (domain→adapter
  inversion); it consumes only core types + its own graph-pure use cases [src: CLAUDE.md D13;
  memory `hexagonal-strict`].
- Scoping is a doc-layer filter, never a graph mutation: `find_references`/`blast_radius`
  on a fixture symbol must still resolve [src: crates/ariadne-daemon/src/domain/queries/docs.rs:18-50].
- TDD: each tier writes a failing golden/structured test before implementation [src: CLAUDE.md `<rules>`].
- ≤200-line authored files; source `.rs` files are exempt (project rule covers plan/skill/audit only).
</constraints>

<decisions>
**D1 — Hand-rolled deterministic layered-DAG→SVG emitter; reject `layout-rs`.** A new pure
`ariadne-graph` module assigns layers by longest-path / topological order (reuse
`petgraph` `condensation`+`toposort`, already used by `render_layers`), positions nodes by a
fixed deterministic rule, and emits SVG via `std::fmt::Write` — mirroring the existing Mermaid
emitter [src: crates/ariadne-graph/src/docgen.rs:391-430; condensation(mg,true)+toposort at
406-407; https://en.wikipedia.org/wiki/Layered_graph_drawing — longest-path layering + toposort
are standard Sugiyama steps, cycles collapsed first]. SVG element syntax (`svg viewBox`, `rect`,
`text`, `line`, `polygon`, `defs/marker`) confirmed standard SVG 1.1/2
[src: https://developer.mozilla.org/en-US/docs/Web/SVG/Element]. *Rejected:* `layout-rs` 0.1.3
(2025-04-24) — a Graphviz-dot engine with **no documented output determinism**, violating the
same-bytes guarantee, and a new dependency for what is a ≤40-node DAG
[src: https://docs.rs/crate/layout-rs/latest — determinism not stated].

**D2 — SVG renders at crate/layer granularity, not file granularity.** `ModuleSpec.name` is a
file path; `build_modules` emits one module per indexed file (hundreds) [src:
crates/ariadne-graph/src/coupling.rs:28-35; crates/ariadne-daemon/src/domain/queries/health.rs:20
(warm) and crates/ariadne-mcp/src/tools/coupling_report.rs:27 (cold)]. The architecture diagram
aggregates files into crates by the `crates/<name>/` prefix, collapsing to ~12 nodes.
Determinism: prefix grouping + sorted node order. *Rejected:* file-level diagram with node
caps — still a hairball, still hides the big SCC.

**D3 — Source-scoping by deterministic path classifier, configurable.** `classify(path)` →
{Source, Test, Fixture, Vendored, Generated} from path heuristics (`/tests/`, `/fixtures/`,
`/benches/`, `node_modules/`, `target/`, `*.min.js`). Docs default to Source-only; the CLI
exposes extra excludes. Applied at the doc layer (which modules are *reported* + aggregate
metrics), never to the graph. *Rejected:* separate test/fixture buckets (keeps noise visible).

**D4 — SVG is a sidecar file, referenced from Markdown; the CLI writes both.** Markdown
references `![architecture](codebase-overview.svg)` — GitHub and IDE previews render committed
linked SVG without an extension; raw inline `<svg>` is stripped by GitHub and bloats the MCP
payload [src: https://developer.mozilla.org/en-US/docs/Web/SVG/Element]. The read-only MCP tool
returns Markdown text only and does no IO [src: crates/ariadne-mcp/src/tools/doc_project.rs:16-26].
The CLI `doc` command runs the cold in-process path (like `query`'s `dispatch`) and writes both
files with `std::fs::write` [src: crates/ariadne-cli/src/commands/query.rs:246-290;
std::fs::write already used at commands/index.rs:303].

**D5 — Boundary-violation insight is symbol-edge level, complementing the cargo test.**
`tests/architecture.rs` enforces *crate-dependency* invariants; it cannot see a domain *symbol*
referencing an adapter symbol. The project doc reports those finer edges (domain→adapter,
adapter→adapter cross-crate, →core-only) deterministically [src: CLAUDE.md `<architecture>`;
tests/architecture.rs].

**D6 — Risk + change-coupling inputs are git-history vectors threaded into the doc use case;
docgen stays graph-pure.** Churn×complexity and co-change need `FileChurn`/`CoChangePair`
(git-derived) which a `(graph, snap, modules)` call lacks. Both are `ariadne-core` types
[src: crates/ariadne-core/src/lib.rs:25] persisted in redb and already loaded by **both**
catalogs as `.churn`/`.co_change` [src: crates/ariadne-daemon/src/domain/catalog.rs:147-152;
crates/ariadne-mcp/src/catalog.rs:84-92, loaded via `storage.all_churn()`]. So `for_project`/
`for_module` take `&[FileChurn]` (+ `&[CoChangePair]` for the project) as params; complexity is
folded in-graph from the snapshot's `SymbolRecord.complexity`; ranking uses the graph-pure
`hotspot::file_hotspots`, `co_change::co_change_report`, and `refactor::god_modules`
[src: crates/ariadne-graph/src/hotspot.rs:102-120; co_change.rs:74-107; refactor.rs:80].
*Rejected:* calling the daemon `queries::analytics`/`health` handlers — they live in a driven
adapter; a domain crate calling them inverts the hexagon [src: CLAUDE.md D13]. The daemon
handlers are the *recipe* to mirror, not a dependency [src: crates/ariadne-daemon/src/domain/queries/analytics.rs:32-55].
</decisions>

<architecture>
- `ariadne-graph` (domain use case, pure): `doc_model` (classify + crate/layer grouping +
  `DocScope`), `diagram` (layered-DAG→SVG), `docgen` (rewritten `for_project`/`for_module` +
  new `architecture_svg`/`module_svg`), `docgen_insights` (deterministic section helpers).
  Consumes existing `coupling`/`cycles`/`dead`/`hotspot`/`co_change`/`refactor`/`heuristics`.
- `ariadne-core` (types): additive fields on `DocForReport` for symbol enrichment.
- `ariadne-daemon` (driven query layer): `docs::doc_for*` thread `DocScope` + `cat.churn`/`cat.co_change`.
- `ariadne-mcp` (driving adapter): cold doc tools thread the same; Markdown shape unchanged
  (now references the sidecar SVG by relative path); renders enriched `DocForReport`.
- `ariadne-cli` (driving adapter): new `commands/doc.rs` writes `.md`+`.svg`, owns the
  configurable exclude globs. Only this layer touches the filesystem.
</architecture>

<tech_inventory>
| Tech | Version | Doc fetched this session |
| --- | --- | --- |
| petgraph (condensation, toposort) | 0.8.0 (pinned) | https://docs.rs/petgraph/0.8.0/petgraph/algo/ (confirmed); repo use docgen.rs:406-407 |
| SVG 1.1/2 element syntax | W3C/MDN | https://developer.mozilla.org/en-US/docs/Web/SVG/Element (confirmed) |
| Layered graph drawing (Sugiyama / longest-path) | n/a | https://en.wikipedia.org/wiki/Layered_graph_drawing (confirmed) |
| layout-rs (rejected alternative) | 0.1.3 (2025-04-24) | https://docs.rs/crate/layout-rs/latest (no determinism documented) |
</tech_inventory>

<risks>
| Risk | Likelihood | Mitigation | Owner |
| --- | --- | --- | --- |
| SVG layout non-determinism (HashMap order, float fmt) | Med | BTreeMap/sorted vecs only; integer `{:.0}` coords; golden byte test | tier-02 |
| Path heuristics misclassify real source as fixture | Med | golden `classify()` table + config override; assert a known source file is Source | tier-01 |
| Crate-level SVG over node cap on huge repos | Low | deterministic top-N by degree + annotated drop count (no silent cap) | tier-02 |
| Git-history vectors empty (history not indexed) | Med | risk/co-change sections degrade to an explicit "history unavailable" line, deterministically | tier-03/04 |
| docgen pulls a daemon analytics dep (hexagon inversion) | Med | use only graph-pure fns (hotspot/co_change/refactor); `cargo test --test architecture` guards | tier-03 |
| `DocForReport` field add ripples to mcp+cli renderers | Med | additive optional fields; update all renderers in one tier; cold `Catalog` already has churn → parity | tier-05 |
</risks>

<verification>
- Each tier: `cargo nextest run -p <crate>` golden tests green; `cargo clippy --workspace
  --all-targets --all-features -- -D warnings`; `cargo fmt --all --check`; `cargo test --test
  architecture`; `cargo deny check` (proves no new dependency).
- End-to-end (tier-06): run `ariadne doc` on this repo, regenerate `docs/codebase-overview.md`
  + `docs/codebase-overview.svg`, open the `.svg` in the IDE Markdown preview and confirm it
  draws; confirm `jquery.js` is absent from hot-spots and the largest SCC is named.
- Determinism: run the generator twice → `diff` is empty for both `.md` and `.svg`.
</verification>

<sources>
- [MDN — SVG Element reference](https://developer.mozilla.org/en-US/docs/Web/SVG/Element)
- [Layered graph drawing — Wikipedia](https://en.wikipedia.org/wiki/Layered_graph_drawing)
- [petgraph 0.8.0 algo module — docs.rs](https://docs.rs/petgraph/0.8.0/petgraph/algo/)
- [layout-rs crate — docs.rs (rejected)](https://docs.rs/crate/layout-rs/latest)
- repo: crates/ariadne-graph/src/{docgen.rs,hotspot.rs:102-120,co_change.rs:74-107,refactor.rs:80,coupling.rs:28};
  crates/ariadne-{daemon,mcp}/src/...catalog.rs; crates/ariadne-cli/src/{main.rs:31,commands/query.rs}; CLAUDE.md D5/D11/D13
</sources>
