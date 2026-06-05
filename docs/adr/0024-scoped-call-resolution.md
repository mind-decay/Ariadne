# ADR-0024: Scoped Index-Time Call Resolution

<status>
Accepted
Date: 2026-06-04
Decider: claude (docgen-overview-fidelity tier-02, R1)
</status>

<context>
Index-time call resolution bound a callee *name* to an arbitrary workspace
symbol, so ubiquitous calls collapsed onto one same-named symbol and the graph
shipped false cross-crate edges (R1). PROOF: `apply_writes`
[src: crates/ariadne-storage/src/adapters/redb/apply.rs:14] calls only std
`Vec::new()` yet the graph carried `apply_writes → new` flagged
"adapter → adapter cross-crate"; `cargo test --test architecture` is green, so
no Cargo-level dep exists and the edge is provably spurious
[src: .claude/plans/docgen-overview-fidelity/plan.md R1].

Spike findings (locus pinned before any edit):
- The callee is captured as **bare** identifier text — the qualifier is
  discarded: `Vec::new()` is captured as `new` via `call_expression
  function: (scoped_identifier name: (identifier) @call.callee)`
  [src: crates/ariadne-parser/src/adapters/treesitter/queries/rust.scm:38-40;
  crates/ariadne-parser/src/adapters/treesitter/facts.rs:122-128].
- The resolver is the pure driver pass
  [src: crates/ariadne-salsa/src/derive.rs:220-278 `resolve_edges`], fed from
  [src: crates/ariadne-salsa/src/db.rs:239-324 `build_changeset`]. Pre-fix it
  preferred a same-file match, **else `candidates.first()`** — the arbitrary
  same-name global that is the phantom.
- It is the **sole** edge producer; SCIP ingestion emits no edges (symbol
  naming only) [src: crates/ariadne-salsa/src/db.rs:461; grep: no `EdgeRecord`
  outside derive.rs]. One fix covers the cold index
  [src: crates/ariadne-cli/src/domain/mod.rs:236 `commit_revision`], the daemon
  warm graph, and incremental updates.

Forces: reliability (every emitted edge must be trustworthy — the overview is
read as ground truth), maintainability (one resolver, no per-language special
casing), and the no-denylist constraint (excluding `new`/`build`/… lexically is
non-portable, rejected in god_modules D2)
[src: .claude/plans/god-module-suggestion-fix/plan.md D2].
</context>

<decision>
Resolve a callee to a definition by scope precedence **same-file → same-crate →
unambiguous-global**, where same-crate is keyed by `package_of(path)` (the
`crates/<name>/` segment, else `""`) and *unambiguous-global* means the name has
exactly one definition workspace-wide. A callee with no in-scope definition that
is also ambiguous globally — the std `Vec::new()` shape — binds to no symbol and
drops the edge. `SymbolId` derivation and SCIP-precise naming are untouched
[src: crates/ariadne-salsa/src/derive.rs `resolve_edges`, `package_of`].
</decision>

<rationale>
- **Reliability.** The phantom is a bare name with multiple definitions bound
  to one arbitrary global. Abstaining when a name is both out-of-scope and
  ambiguous removes exactly that class: the dogfood full index drops from 5378
  to 3953 edges (−1425), all cross-crate name-collisions; `apply_writes` no
  longer resolves `new` because `ariadne-storage` defines no `new` and `new` is
  globally ambiguous.
- **Reliability (recall preserved).** A name with exactly one workspace
  definition is unambiguous, so a genuine cross-crate call still resolves — the
  `beta::run → alpha::helper` shape from the `ariadne doc` fixture, which has no
  import statement, keeps its edge [src: crates/ariadne-cli/tests/doc_command.rs:19-22].
  The full suite (index_parity cold==warm, incremental==fresh, blast/refs/doc)
  stays green, so legitimate same-crate edges are not dropped.
- **Maintainability / portability.** The rule is purely structural — a package
  key plus a definition count. No lexical name list, no import-path parsing, no
  per-language receiver-type analysis. `package_of` mirrors
  `ariadne_graph::doc_model::crate_of` so resolution scope matches docgen's
  crate attribution; it is replicated in `ariadne-salsa` because the crate may
  not depend on `ariadne-graph` [src: tests/architecture.rs lines 30-35].
- **Determinism.** Candidate lists stay sorted by `(file, def_start)`, so each
  `find`/`first` is order-independent; the dogfood index reports 3953 edges on
  repeated runs [src: crates/ariadne-graph/src/docgen.rs:1-8].
</rationale>

<alternatives>
- **Naive package-level import-visible** ("caller imports package P, candidate
  lives in P") — rejected: `apply.rs` has `use ariadne_core::Changeset` and
  `ariadne_core` defines `new`, so it reintroduces the exact
  `apply_writes → ariadne_core::new` phantom. A correct import-visible needs the
  discarded call qualifier. `[src: crates/ariadne-storage/src/adapters/redb/apply.rs:4]`
- **Same-crate only (drop the global fallback entirely)** — rejected: drops the
  legitimate `beta::run → alpha::helper` cross-crate edge (beta has no import),
  violating "scoping must not drop legitimate resolutions."
  `[src: crates/ariadne-cli/tests/doc_command.rs:19-22]`
- **Lexical name denylist** (`new`/`build`/…) — rejected: non-portable across
  TS/Python/Go/Java/C#; abstention is driven by ambiguity (definition count),
  not by the spelling of the name. `[src: .claude/plans/god-module-suggestion-fix/plan.md D2]`
- **Qualifier-aware / receiver-type resolution** — deferred: precise
  cross-crate resolution of `Foo::new` requires the discarded qualifier and
  type→impl resolution, which is SCIP's job; SCIP-driven edges are the
  long-term answer (plan D2). `[src: .claude/plans/docgen-overview-fidelity/plan.md D2]`
</alternatives>

<consequences>
- `ariadne-salsa` gains `derive::package_of` and a `package` scoping key on
  `SymbolCandidate` / `FileFacts`; `sort_candidates` returns
  `ResolvedCandidate { id, package }`. All `pub(crate)`, internal to the crate;
  no public-API or adapter-boundary change.
- The graph trades cross-crate recall for precision: a genuine cross-crate call
  to an *ambiguous* name (defined in ≥2 crates) now yields no edge. This is the
  R1-contaminated signal T1 already suppressed in docgen; tier-03 re-enables
  those sections on the now-reliable edge set, and may revisit cross-crate
  recall via SCIP edges.
- Off-limits without superseding: re-adding an arbitrary same-name global
  fallback in `resolve_edges`, or introducing a lexical name denylist.
</consequences>

<sources>
- `[src: .claude/plans/docgen-overview-fidelity/plan.md R1, D2; tier-02-edge-resolution.md]`
- `[src: crates/ariadne-salsa/src/derive.rs:39-110,220-278 ; src/db.rs:239-324]`
- `[src: crates/ariadne-parser/src/adapters/treesitter/queries/rust.scm:35-44 ; facts.rs:122-128]`
- `[src: crates/ariadne-storage/src/adapters/redb/apply.rs (proof case)]`
- `[src: crates/ariadne-cli/tests/doc_command.rs:19-22 ; crates/ariadne-salsa/tests/scoped_resolution.rs]`
- `[src: crates/ariadne-graph/src/doc_model.rs:95-105 `crate_of`]`
- `[src: docs/adr/0016-shared-per-file-derivation.md ; .claude/plans/god-module-suggestion-fix/plan.md D2]`
</sources>
