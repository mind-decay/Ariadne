# ADR-0020: Cyclomatic Complexity From The Tree-Sitter CST

<status>
Accepted
Date: 2026-06-01
Decider: claude
</status>

<context>
v1 surfaces module-level structural health (`weak_spots`, `coupling_report`)
but carries no per-function density signal, so a god module's worst functions
are invisible. tier-13 wants a per-symbol and per-file complexity factor for its
hotspot ranking [src: .claude/plans/post-v1-roadmap/tier-13-hotspot-cochange-metrics.md].
Ariadne already builds and caches a tree-sitter CST for every file, so the
metric can be computed with no new dependency and no extra parse — the forces
are efficiency (reuse the existing CST), maintainability (one predicate per
grammar, not a second parser), and reliability (deterministic, no inference,
per [no-llm-features]). Plan RD8 fixes the metric as McCabe
`M = decisions + 1` [src: post-v1-roadmap plan.md RD8].
</context>

<decision>
Compute McCabe cyclomatic complexity in one CST walk per parse layer, store it
as a `u32` field on `SymbolRecord` (and the `Decl`/salsa/wire mirrors), counting
strict decision points — `if`/loop/`case`/`catch`/ternary plus every `&&`/`||`
— and attributing each decision to the innermost decl whose span contains it.
Function-like decls (Function / Method / Component) carry `decisions + 1`
(`>= 1`); every other symbol carries `0`.
</decision>

<rationale>
- **`u32`, 0 = N/A (not `Option<u32>`)** — maintainability. Mirrors tier-04's
  all-`u32`/all-defaulted metadata and the postcard prefix-extension migration;
  tier-13 already treats a zero factor as non-hotspot, so no consumer needs
  `Option` handling and the field is a one-migration, forward-only addition
  [src: post-v1-roadmap tier-12 D1; crates/ariadne-core/src/domain/records.rs].
- **Strict McCabe, counting `&&`/`||`** — reliability / fidelity. The standard
  definition is decision points + 1; a compound predicate decomposes to
  sequential branches at machine level, so `a && b` counts two
  [src: McCabe, "A Complexity Measure", IEEE TSE 1976;
  https://en.wikipedia.org/wiki/Cyclomatic_complexity]. Consistent with RD8's
  explicit `if`/`for`/`while`/`case`/`&&`/`||`/`?` list.
- **Decl-span attribution (grammar-agnostic nesting)** — maintainability. A
  captured decision is attributed to the innermost `Decl` whose `def_byte_range`
  contains it, reusing `innermost_containing_decl` from `facts.rs`, so a nested
  captured `fn`/method owns its decisions and the parent never double-counts. No
  per-grammar "function-boundary" node set is needed — only the decision
  predicate is per-grammar [src: crates/ariadne-parser/src/adapters/treesitter/facts.rs].
- **Boolean via the `binary_expression` `operator` field** — reliability. Every
  bundled grammar except Python exposes `&&`/`||` as the `operator` field of a
  `binary_expression` (verified against each `node-types.json` this session);
  Python uses a dedicated `boolean_operator` node. Detecting the operator field
  rather than the bare `&&` token avoids miscounting a Rust reference pattern
  (`&&x`) [src: tree-sitter-{rust,python,javascript,typescript,go,java,
  kotlin-ng,c-sharp,c,cpp} node-types.json].
</rationale>

<alternatives>
- **Control-flow-only count (CodeScene-style, exclude `&&`/`||`)** — rejected:
  a simpler walker but diverges from RD8 and the cited McCabe definition
  [src: post-v1-roadmap tier-12 D2].
- **`rust-code-analysis` crate** — rejected: a heavy multi-grammar dependency
  duplicating the tree-sitter parser Ariadne already owns; violates the
  pure-Rust-critical-path / no-redundant-dependency posture [src: plan.md RD8].
- **`Option<u32>`** — rejected: `Option` handling in every consumer plus a
  migration default, and a one-way door (a later change is another migration)
  [src: tier-12 D1].
</alternatives>

<consequences>
- `SymbolRecord` gains `complexity: u32` after `attributes`; the postcard v7
  layout extends the v6 byte prefix, shipped behind a single redb v6->v7
  `MigrationStep` that re-encodes `SYMBOLS` in place with `complexity = 0` (no
  rebuild) [src: crates/ariadne-storage/src/domain/migration.rs].
- The decision predicate is per-`Lang` and must track grammar drift: a grammar
  bump that renames a decision node silently under-counts. The per-language
  goldens (`crates/ariadne-parser/tests/complexity.rs`) pin a hand-counted
  branchy + nested-function + boolean case and fail loudly on drift.
- **Recorded mapping limitations** (counted as written; not silently dropped):
  - **Arrow-as-variable** — a non-component arrow bound to a `Variable`
    (`const f = () => {…}`) is not function-like, so it reads `0`. Captured
    component arrows (`const Foo = () => <jsx/>`) are reclassified `Component`
    before the walk and do carry complexity [src: tier-12 D4;
    crates/ariadne-parser/src/adapters/treesitter/facts.rs].
  - **`switch`/`when` default arm** — excluded where the grammar gives the
    default its own node (Go `default_case`, JS `switch_default`); counted where
    the grammar folds it into the arm node (C `case_statement`, Kotlin
    `when_entry`, Java `switch_label`). The divergence is bounded to ±1 per
    switch and documented here rather than normalized per grammar.
  - **Rust `loop`** — `loop_expression` is counted: an unconditional loop still
    adds a back edge to the control-flow graph (`M = E - N + 2P`), so +1 is the
    graph-theoretic contribution.
  - **Synthesized SFC component** — the one `kind = Component` node Ariadne
    synthesizes per `.vue`/`.svelte`/`.astro` file lives in the `ariadne-salsa`
    derivation, not in the parser, so `attach_complexity` never sees it; it
    carries `complexity = 0`, not the `>= 1` an empty-body McCabe gives a
    function-like symbol. It owns no measurable body — its `<script>` decls
    carry their own per-decl complexity. Kept at `0` deliberately: tier-13's
    file-grain aggregation sums a file's symbols, so a synthetic `1` would bias
    every SFC file's total by `+1` over a non-SFC file, and tier-13 treats `0`
    as non-hotspot, correctly excluding the synthetic root
    [src: crates/ariadne-salsa/src/derive.rs:92-103].
- Off-limits without superseding: changing the count basis (e.g. dropping
  `&&`/`||`) or moving complexity off `SymbolRecord` — both break tier-13's
  stored factor and require a new ADR + migration.
</consequences>

<sources>
- `[src: McCabe, "A Complexity Measure", IEEE TSE 1976]`
- `[src: https://en.wikipedia.org/wiki/Cyclomatic_complexity]`
- `[src: .claude/plans/post-v1-roadmap/plan.md RD8]`
- `[src: .claude/plans/post-v1-roadmap/tier-12-cyclomatic-complexity.md]`
- `[src: crates/ariadne-parser/src/adapters/treesitter/complexity.rs]`
- `[src: docs/adr/0014-symbol-metadata-enrichment.md]` (postcard prefix-extension precedent)
</sources>
