# ADR-0008: C and C++ syntactic indexing via tree-sitter

<status>
Accepted
Date: 2026-05-20
Decider: user
</status>

<context>
plan.md scopes v1 as "syntactic indexing of any tree-sitter language" plus
semantic SCIP indexing for a fixed lang set [src: ../../.claude/plans/ariadne-core/plan.md `<context>`].
tier-03 shipped eight tree-sitter grammars (ts/js/py/rs/go/java/kotlin/c#); C
and C++ were never wired.

The gap surfaced as a release blocker. The tier-10 SLO corpus assembled only
55,527 indexed files against a 100K floor because `lang_for_path` recognised
no C/C++ extension, so `dotnet/runtime`'s native tree was skipped wholesale
[src: ../../.claude/plans/ariadne-core/tier-10-cli-e2e.md `<blockers>`]. Risk
R8's mitigation names this tier explicitly: "tier-11 adds C/C++ so the corpus
reaches a real 100K" [src: ../../.claude/plans/ariadne-core/plan.md `<risks>` R8].

Forces: the indexer must reach C/C++ coverage (scalability of the real
workload) without dragging a non-Rust runtime onto the critical path —
plan.md D5 fixes "no cgo, pure-Rust deps on the critical path"
[src: ../../.claude/plans/ariadne-core/plan.md D5].
</context>

<decision>
Adopt **`tree-sitter-c` 0.24.2** and **`tree-sitter-cpp` 0.23.4** as
`ariadne-parser` grammars for **syntactic-only** C/C++ indexing — declarations
and call sites via per-lang `.scm` queries, no semantic resolution. The `.h`
header extension, ambiguous between C and C++, resolves to **C** by default
with no content sniffing in v1.
</decision>

<rationale>
- **Efficiency / maintainability** — both crates expose `LANGUAGE: LanguageFn`
  through `tree-sitter-language ^0.1`, whose `Into<Language>` conversion is the
  exact pattern the eight tier-03 grammars already use; registry and
  fact-dispatch wiring is two match arms per crate, no new abstraction
  [src: crates/ariadne-parser/src/adapters/treesitter/registry.rs].
- **Reliability (D5 compliance)** — the grammars are pure-Rust bindings; the
  generated `parser.c` is compiled at build time by the `cc` crate, identical
  to all eight existing tree-sitter grammars. No runtime native dependency, no
  libclang, no cgo [src: ../../.claude/plans/ariadne-core/plan.md D5].
- **Scalability** — wiring the two highest-impact missing grammars lets the
  tier-12 corpus assemble a genuine ≥100K-indexed-file workload, which is the
  precondition for re-running the SLO release gate (R8).
- Syntactic-only scope keeps C/C++ off the SCIP critical path: `scip-clang` is
  opt-in per tier-12, and its `ScipDoc`→`Changeset` bridge is unbuilt
  [src: ../../.claude/plans/ariadne-core/tier-10-cli-e2e.md D-A].
</rationale>

<alternatives>
- **libclang / clang-based parsing** — rejected. Links a large external C++
  toolchain (libclang) that must be present at build and run time; that is the
  precise dependency profile D5 forbids on the critical path. `[src: ../../.claude/plans/ariadne-core/plan.md D5]`
- **Defer C/C++ to `scip-clang` alone** — rejected. SCIP is opt-in (tier-12)
  and yields no syntactic graph for headers; the SCIP→graph bridge is unbuilt,
  so this would leave C/C++ with zero indexed symbols. `[src: ../../.claude/plans/ariadne-core/tier-10-cli-e2e.md D-A]`
- **Content-sniffing `.h` headers** — rejected for v1. Reading each header to
  classify C vs C++ costs an extra IO pass on the cold path; C and C++ queries
  share most node types, so a C++ header mis-tagged as C still yields useful
  decls. Recorded as a known limitation, revisitable post-v1.
</alternatives>

<consequences>
- `ariadne_core::Lang` gains `C` and `Cpp` variants; `tag`/`from_tag` carry
  `"c"`/`"cpp"`. `ParserRegistry` now ships ten grammars.
- New invariant: `queries/c.scm` and `queries/cpp.scm` must compile against the
  pinned grammar versions — `cargo test -p ariadne-parser` exercises this via
  the C and C++ fact-extraction tests.
- `.h` → C is a deliberate, documented limitation. A C++-only header named
  `.h` (not `.hpp`/`.hh`/`.hxx`) is parsed with the C grammar. Acceptable for
  v1; superseding this ADR is required to add content sniffing.
- `cargo deny check` must keep passing: `tree-sitter-c` and `tree-sitter-cpp`
  are MIT, matching the existing tree-sitter grammar licences.
- A grammar-version bump for either crate goes through a superseding ADR, per
  the tier-03 pinning rule.
</consequences>

<sources>
- `[src: https://crates.io/crates/tree-sitter-c]` — `tree-sitter-c` 0.24.2, MIT, `LANGUAGE: LanguageFn`.
- `[src: https://docs.rs/tree-sitter-cpp/latest/tree_sitter_cpp/]` — `tree-sitter-cpp` 0.23.4 API.
- `[src: https://github.com/tree-sitter/tree-sitter-c/blob/v0.24.2/src/node-types.json]` — C grammar node types.
- `[src: https://github.com/tree-sitter/tree-sitter-cpp/blob/v0.23.4/src/node-types.json]` — C++ grammar node types.
- `[src: ../../.claude/plans/ariadne-core/plan.md]` — `<context>`, D5, risk R8, `<tech_inventory>`.
- `[src: ../../.claude/plans/ariadne-core/tier-11-c-cpp-indexing.md]` — this tier.
- `[src: ../../.claude/plans/ariadne-core/tier-10-cli-e2e.md]` — `<blockers>`, deviation D-A.
</sources>
</output>
