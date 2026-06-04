---
tier_id: tier-01
title: Uniform DocScope across sections; suppress R1-contaminated output
deps: []
exit_criteria:
  - "`change_coupling` and `cycle_clusters` apply `scope.include` to every endpoint; no `tests/`/`*.snap`/`fixtures/` path appears in the regenerated overview"
  - "Architecture table keeps Crate + Layer; the Role column is replaced by an explicit withheld marker and one line stating it depends on R1 — no row claims a coupling shape"
  - "Boundary violations section body is a single explicit withheld line referencing R1; no `X → new` rows"
  - "Cross-crate cycle clusters (members spanning >1 crate) are withheld with an explicit line; intra-crate clusters still listed"
  - "Domain-interior crates (ariadne-core, ariadne-graph, ariadne-salsa) render Layer = Domain; a test pins each crate's expected layer"
  - "Synopsis crate count counts only crates under `crates/` (excludes the `tools/` directory)"
  - "god_modules/refactor MCP + warm==cold tests still green; `ariadne doc` twice → byte-identical; clippy/fmt/architecture green"
status: completed
completed: 2026-06-04
---

<context>
Make every history/graph section honour `DocScope` and stop shipping the two
R1-contaminated sections as if trustworthy. Pure docgen edits in `ariadne-graph`;
no indexer change (that is tier-02). Full rationale: see `plan.md` D1/D3
[src: .claude/plans/docgen-overview-fidelity/plan.md].
`DocScope::include` keeps only `DocKind::Source` paths
[src: crates/ariadne-graph/src/doc_model.rs:60-71]. `LayerHint::of` infers
Domain/Adapter from `src/domain`/`src/adapters` segments, else Interior
[src: doc_model.rs:100-108].
</context>

<files>
- crates/ariadne-graph/src/docgen_insights.rs — thread `&DocScope` into
  `change_coupling` (filter both co-change endpoints via `scope.include`) and
  `cycle_clusters` (filter members to source; withhold cross-crate clusters);
  in `architecture_section` replace the Role cell with a withheld marker + note;
  in `boundary_violations` return a single explicit withheld line; in `synopsis`
  count only `crate_of(path).is_some()` crates [src: docgen_insights.rs:68,135,191,259,409].
- crates/ariadne-graph/src/doc_model.rs — add a test-pinned crate→layer override
  for the project's domain-interior crates, sourced from CLAUDE.md `<architecture>`;
  path heuristic stays the fallback [src: doc_model.rs:100; CLAUDE.md `<architecture>`].
- crates/ariadne-graph/src/docgen.rs — pass `scope` to the updated section calls
  in `for_project`; no logic beyond wiring [src: docgen.rs `for_project`].
- crates/ariadne-graph/tests/ (docgen_project.rs + snapshots) — update goldens;
  add the crate→layer assertion and a "no test/fixture path leaks" assertion.
- docs/codebase-overview.{md,svg} — regenerate via `ariadne doc`.
</files>

<steps>
1. Write/extend failing tests in `crates/ariadne-graph/tests/docgen_project.rs`:
   (a) the rendered overview contains no `/tests/`, `.snap`, or `/fixtures/`
   substring; (b) `ariadne-graph`'s Architecture row shows Layer `Domain`;
   (c) the Boundary-violations and Role outputs contain the withheld marker;
   (d) a cross-crate 2-node cycle fixture is withheld. Run → red.
2. Thread `scope: &DocScope` into `change_coupling`; after `co_change_report`,
   drop any edge where `!scope.include(&e.a) || !scope.include(&e.b)` before the
   `structurally_linked` filter [src: docgen_insights.rs:442-447]. Source-only
   scope removes test/snap endpoints and the trivial "source ⇄ own test" pairs.
3. Thread `scope` into `cycle_clusters`; keep a cluster only if all members map
   to a `scope.include` path; then withhold (explicit line, do not list a cut)
   any surviving cluster whose members span >1 crate via `crate_key`
   [src: docgen_insights.rs:259-301; plan.md D1]. Intra-crate clusters render as
   today.
4. In `architecture_section`, drop the `purpose(row)` Role cell; render
   `| crate | layer | _withheld (R1)_ |` and emit one line under the table:
   "Role withheld — depends on cross-crate edge accuracy (R1)."
   [src: docgen_insights.rs:163-172; plan.md D1].
5. In `boundary_violations`, replace the body with a single explicit line:
   "_Withheld — symbol-edge boundary checks depend on cross-crate edge accuracy
   (R1); re-enabled after the resolver fix._" Keep the function + scope param so
   tier-03 reverts cleanly [src: docgen_insights.rs:191-230; plan.md D1].
6. Add the crate→layer override in `doc_model.rs` (core/graph/salsa → Domain),
   applied in `dominant_layer`/`architecture_section`; fallback to the path
   heuristic [src: doc_model.rs:100; CLAUDE.md `<architecture>`].
7. In `synopsis`, compute crate count from members whose path yields
   `crate_of(..).is_some()`, excluding non-`crates/` dirs like `tools/`
   [src: docgen_insights.rs:68-128; doc_model.rs:77-83].
8. `cargo nextest run -p ariadne-graph`; review (do not blind-accept) the new
   goldens with `cargo insta review`; confirm refactor/warm==cold MCP tests pass.
9. Regenerate `cargo run -p ariadne-cli -- doc`; read every section; run twice →
   diff empty.
</steps>

<verification>
- `cargo nextest run -p ariadne-graph` → new scope/layer/withheld tests green;
  docgen_project goldens re-accepted after review.
- `cargo nextest run -p ariadne-daemon -p ariadne-mcp` → refactor_suggestions +
  warm_analytics unchanged-green.
- `cargo run -p ariadne-cli -- doc` twice → `docs/codebase-overview.{md,svg}`
  byte-identical; grep the `.md` for `/tests/`, `.snap`, `/fixtures/` → no match;
  Architecture shows `ariadne-graph` Layer Domain; Boundary + Role withheld.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`;
  `cargo fmt --all --check`; `cargo test --test architecture`.
</verification>

<rollback>
`git checkout -- crates/ariadne-graph/src/docgen_insights.rs
crates/ariadne-graph/src/doc_model.rs crates/ariadne-graph/src/docgen.rs
crates/ariadne-graph/tests/ docs/codebase-overview.md docs/codebase-overview.svg`.
No cross-crate signature leaves `ariadne-graph`; daemon/mcp untouched.
</rollback>
