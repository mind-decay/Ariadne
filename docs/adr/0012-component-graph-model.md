# ADR-0012: Component-graph entity model

<status>
Accepted
Date: 2026-05-21
Decider: user
</status>

<context>
The `js-framework-support` plan indexes five JS-framework families and must
"surface a component graph (which component renders which, which hooks each
uses)" [src: ../../.claude/plans/js-framework-support/plan.md `<context>`].

A component is not just any function — it is the unit UI structure is built
from, and render / hook usage are the edges that structure travels along.
Ariadne's analytics (`blast_radius`, `coupling_report`, `weak_spots`) already
traverse the symbol/edge graph; the question is whether components and their
relationships enter that graph as first-class entities or stay invisible.

Forces: the model must let existing analytics reason about UI structure with
no per-tool special-casing (maintainability); it must not add a parallel graph
or a new storage table (efficiency, reliability); it must fit the fixed
`ariadne-core` hexagonal boundary — domain declares types, adapters populate
them [src: ../../.claude/plans/ariadne-core/plan.md D13].
</context>

<decision>
Model the component graph inside the existing symbol/edge graph. A component
is a symbol whose kind is `"component"`; render and hook usage are two new
edge kinds. Concretely: `EdgeKind` gains `Renders` and `UsesHook`
(`ariadne-core`, this tier), and the parser emits `DeclKind::Component` for
component declarations (`ariadne-parser`, tier-02). `SymbolRecord.kind` is a
free-form `String` today [src: crates/ariadne-core/src/domain/records.rs:25-37],
so no closed symbol-kind enum changes here — components are tagged with the
string `"component"`; if `kind` is later canonicalised to a closed enum, a
`Component` variant is added then.
</decision>

<rationale>
- **Maintainability** — `blast_radius`, `coupling_report`, `plan_assist`, and
  the MCP surface already walk `(SymbolId, EdgeKind, SymbolId)` triples. Adding
  variants to the existing `EdgeKind` means a "what renders this component"
  query is the same traversal as "what calls this function"; no analytics
  tool needs component-aware code paths
  [src: ../../.claude/plans/js-framework-support/plan.md D8].
- **Efficiency** — render/hook edges land in the existing `EDGES` table behind
  the existing `EdgeKey` 17-byte key; the new variants extend `to_byte` /
  `from_byte` (tags `3` and `4`) with no new table, no schema migration, and
  no second graph kept in sync [src: crates/ariadne-core/src/domain/records.rs:52-69].
- **Reliability** — `EdgeKind` is `#[non_exhaustive]`, so every cross-crate
  `match` already carries a wildcard arm; adding variants is additive and
  cannot silently break a consumer. The graph crate's `EdgeKind::from_core`
  wildcard absorbs the new kinds until a later tier widens the graph alphabet
  [src: crates/ariadne-graph/src/build.rs:69-79].
- **Scalability** — because components are ordinary symbols, the incremental
  delta path (`apply_delta`) and the streaming cold-index build need no
  component-specific handling; they scale exactly as the rest of the graph.
</rationale>

<alternatives>
- **Model components as plain functions, render/hook as plain calls** —
  rejected. A React/Solid component *is* a function and a hook call *is* a
  call, so this "works" mechanically, but it erases UI structure: analytics
  could not distinguish a component from a helper, nor a render edge from an
  ordinary call, so "which components does this one render" becomes
  unanswerable. The plan's success criterion is explicitly `Renders`/`UsesHook`
  edges being *present and distinguishable*
  [src: ../../.claude/plans/js-framework-support/plan.md `<context>` Success].
- **A separate component-graph table + dedicated traversal API** — rejected.
  Duplicates the graph machinery, must be kept consistent with the symbol
  graph on every incremental update, and forces every analytics tool to query
  two graphs. Higher coupling and a new failure mode for no analytical gain.
</alternatives>

<consequences>
- `ariadne_core::EdgeKind` gains `Renders = 3` and `UsesHook = 4`; the byte
  alphabet of `EdgeKey` is now `0..=4`. `from_byte` decodes both; older
  indexes simply never carry tags `3`/`4` (forward-compatible, no migration).
- `ariadne-parser` (tier-02) must emit `DeclKind::Component`; the CLI edge
  resolver (tier-05) must map render/hook sites to `EdgeRecord { kind:
  Renders | UsesHook }`. Those tiers own that wiring; this tier ships the
  `ariadne-core` types only.
- New invariant: symbol kind `"component"` is the canonical tag for a
  component declaration across parser, CLI, graph, and MCP. Canonicalising
  `SymbolRecord.kind` into a closed enum later must add a `Component` variant
  preserving this tag — a change that supersedes this ADR.
- `ariadne-graph`'s `EdgeKind` (its own wider in-RAM alphabet) does not yet
  have render/hook variants; `from_core` collapses them onto `Calls` until a
  later tier extends the graph alphabet and its `EdgeKindSet` filters.
</consequences>

<sources>
- `[src: ../../.claude/plans/js-framework-support/plan.md]` — `<context>`, decision D8.
- `[src: ../../.claude/plans/js-framework-support/tier-01-domain.md]` — this tier.
- `[src: crates/ariadne-core/src/domain/records.rs]` — `EdgeKind`, `EdgeKey`, `SymbolRecord` (free-form `kind`).
- `[src: crates/ariadne-graph/src/build.rs]` — graph `EdgeKind` and `from_core` wildcard.
- `[src: ../../.claude/plans/ariadne-core/plan.md]` — D13 hexagonal boundary rule.
</sources>
