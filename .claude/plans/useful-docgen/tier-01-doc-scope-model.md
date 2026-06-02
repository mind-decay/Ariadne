---
tier_id: tier-01
title: Doc-layer source scoping + crate/layer model
deps: []
exit_criteria:
  - "doc_model::classify returns the correct DocKind for source/test/fixture/vendored paths (golden table test)"
  - "for_project under default DocScope omits crates/ariadne-parser/fixtures/javascript/jquery.js from Hot-Spots and the glossary"
  - "a fixture symbol still resolves via the graph (scope is doc-layer only, graph unmutated)"
  - "cargo nextest -p ariadne-graph + clippy -D warnings + fmt --check + cargo deny + test --test architecture all green"
status: pending
---

<context>
Foundation tier. Introduces the deterministic path classifier and crate/layer grouping that
tiers 02-06 consume, and threads a `DocScope` filter through `docgen` so every aggregate
(hot-spots, coupling table, glossary, layer diagram) reflects real source code, not vendored
fixtures or test scaffolding. The graph itself is never filtered — scoping lives entirely in
the doc render path [src: plan.md D3; crates/ariadne-graph/src/docgen.rs:192-232].
Full context: plan.md.
</context>

<files>
- crates/ariadne-graph/src/doc_model.rs — NEW. `DocKind` enum; `classify(path: &str) -> DocKind`;
  `DocScope { extra_excludes: Vec<String> }` with `include(path) -> bool` (default = Source-only);
  `crate_of(path) -> Option<&str>` (group by `crates/<name>/` prefix) + `LayerHint` (domain/adapter
  /interior from path segment `src/domain` vs `src/adapters`).
- crates/ariadne-graph/src/lib.rs — re-export `DocKind`, `DocScope`, `crate_of`.
- crates/ariadne-graph/src/docgen.rs — MODIFY `for_project`/`for_module` to take `&DocScope`;
  filter `modules` and the glossary/hot-spot/coupling inputs through `scope.include`.
- crates/ariadne-daemon/src/domain/queries/docs.rs — MODIFY pass `&DocScope::default()` into
  `for_project`/`for_module` (config wiring deferred to tier-06).
- crates/ariadne-mcp/src/tools/doc_project.rs, doc_module.rs — MODIFY same default-scope pass-through.
- crates/ariadne-graph/tests/doc_scope.rs — NEW. classify() golden table; scope filter assertions.
- crates/ariadne-graph/tests/docgen_fixture.rs — MODIFY expected output (jquery/test rows gone).
</files>

<steps>
1. Write failing `tests/doc_scope.rs`: assert `classify("crates/ariadne-parser/fixtures/javascript/jquery.js")
   == Fixture`, `classify("crates/ariadne-graph/tests/support.rs") == Test`,
   `classify("crates/ariadne-graph/src/docgen.rs") == Source`, and that `DocScope::default().include`
   is true only for Source. Assert `crate_of("crates/ariadne-mcp/src/server.rs") == Some("ariadne-mcp")`.
2. Implement `doc_model.rs`. `classify` matches path segments in a fixed priority order
   (Vendored `node_modules/`|`*.min.js` → Generated `target/`|`*.pb.rs` → Fixture `/fixtures/` →
   Test `/tests/`|`/benches/`|`_test.`|`tests.rs` → else Source). All matching is deterministic
   string ops; no IO.
3. Thread `&DocScope` into `docgen::for_project` and `for_module`: filter the `modules` slice and
   restrict glossary fan-in ranking + hot-spot/coupling `ModuleStat` collection to scoped modules.
   Keep the function signatures' new param last.
4. Update callers (daemon `docs.rs`, mcp `doc_project.rs`/`doc_module.rs`) to pass
   `&DocScope::default()`. Re-export new types from `lib.rs` (façade re-export only) [src: CLAUDE.md `<architecture>`].
5. Add an assertion (in `tests/doc_scope.rs`) that the graph still contains a fixture symbol after
   scoping — call an existing graph query (e.g. `fan_in`) on a jquery symbol id and expect a hit,
   proving scope did not mutate the graph [src: plan.md constraints].
6. Regenerate the `docgen_fixture` golden expectation and confirm the new bytes are deterministic.
</steps>

<verification>
- `cargo nextest run -p ariadne-graph` → doc_scope + docgen_fixture green.
- `cargo nextest run -p ariadne-daemon -p ariadne-mcp` → existing doc tests green with default scope.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`; `cargo fmt --all --check`.
- `cargo test --test architecture` (façade/boundary intact); `cargo deny check` (no new dep).
- Manual: `for_project` output no longer contains `jquery.js` in Hot-Spots; assert in test, not by eye.
</verification>

<rollback>
`git checkout -- crates/ariadne-graph crates/ariadne-daemon crates/ariadne-mcp` and delete
`doc_model.rs` + `tests/doc_scope.rs`. No schema, storage, or wire-format change to unwind.
</rollback>
