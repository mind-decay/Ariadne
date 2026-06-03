---
tier_id: tier-01
audited: 2026-06-03
verdict: PASS
commit: 21dcae75da4ea8e57f9ff4d47a380145986c0d29
---

<scope>
Tier-01 "Doc-layer source scoping + crate/layer model" of `useful-docgen`.
Reviewed the working-tree diff scoped to the tier's `<files>`:
- NEW `crates/ariadne-graph/src/doc_model.rs` (110 lines) — `DocKind`, `classify`,
  `DocScope::include`, `crate_of`, `LayerHint`.
- NEW `crates/ariadne-graph/tests/doc_scope.rs` (131 lines) — classify golden table,
  default-scope filter, extra-excludes, crate_of, LayerHint, and the
  filter-vs-graph-unmutated proof.
- MODIFY `crates/ariadne-graph/src/docgen.rs` — `for_project`/`for_module` take a
  trailing `&DocScope`; `for_project` filters `modules` → `scoped` before
  `render_layers`/`ModuleStat`.
- MODIFY `crates/ariadne-graph/src/lib.rs` — `pub mod doc_model` + re-export of
  `DocKind, DocScope, LayerHint, crate_of`.
- MODIFY `docgen_fixture.rs`, daemon `docs.rs`, mcp `doc_project.rs`/`doc_module.rs` —
  thread `&DocScope::default()`.
- MODIFY `crates/ariadne-daemon/tests/support.rs` — NOT in the tier `<files>` list,
  but a compelled mechanical update: the cold-path helpers `cold_doc_module`/
  `cold_doc_project` call `for_module`/`for_project` directly, so the signature
  change forces the new arg or the daemon crate fails to compile. Justified
  out-of-list touch, no behavioral change.
</scope>

<checks_run>
All `<verification>` commands re-run at commit 21dcae7 (index rev 610, fresh):
- `cargo nextest run -p ariadne-graph` → 46/46 pass, incl. new `doc_scope` (6 tests)
  and updated `docgen_fixture` (golden + proptests).
- `cargo nextest run -p ariadne-daemon -p ariadne-mcp` → 103/103 pass; existing
  doc tests green under default scope (incl. `incremental_warm`, `memory_probe`).
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` → exit 0,
  no warnings (`_scope` underscore param does not trip unused-var).
- `cargo fmt --all --check` → exit 0.
- `cargo test --test architecture` → `architecture_invariants_hold` ok (façade +
  hexagon boundary intact; `doc_model` is std-only, no adapter dep).
- `cargo deny check` → advisories/bans/licenses/sources ok (only pre-existing
  `license-not-encountered` warnings; no new dependency introduced — D5 honored).

Manual gates (by assertion, not eye):
- Exit-1 classify golden table: `classify_buckets_paths_by_priority` asserts
  Fixture/Test/Source/Vendored/Generated buckets in priority order — pass.
- Exit-2 fixture omission: `for_project_omits_fixtures_but_graph_keeps_them`
  asserts `jquery.js` absent from rendered Hot-Spots/Coupling while the Source
  module appears — pass.
- Exit-3 `crate_of`: `crate_of_groups_by_prefix` — pass.
- Exit-4 graph-unmutated: same test asserts `graph.fan_in(jquery) == 1` after
  `for_project`; reinforced structurally by `for_project(&graph, …)` taking an
  immutable borrow (mutation impossible by construction) — pass.
- Exit-5 toolchain green: all commands above — pass.

Read end-to-end: `doc_model.rs`, `doc_scope.rs`, full `for_project`/`render_layers`/
`module_stat`/`push_hotspots`/`push_coupling`, and every caller diff.
</checks_run>

<findings>
| id | category | severity | location | problem | fix |
| --- | --- | --- | --- | --- | --- |
| — | — | — | — | No defects found. | — |
</findings>

<verdict>
PASS. Zero FAIL findings; zero INFO findings.

Every exit criterion is independently verified by a re-run command or a dedicated
assertion, not by eye. The implementation matches the plan precisely:

- `classify` priority order (Vendored → Generated → Fixture → Test → Source) and
  the pure-string, no-IO constraint match step 2 and D3. The added root-relative
  `starts_with` guards (`tests/`, `fixtures/`, `benches/`, `target/`) are
  deterministic and harmless; the `/tests.rs` anchor correctly avoids a
  `contests.rs` false positive.
- `for_project` scoping (`modules.iter().filter(scope.include).cloned().collect()`)
  preserves input order (deterministic) and drives only `render_layers` + the
  `ModuleStat` tables from `scoped`; `cycle_report`/`dead_code` stay graph-global,
  honoring D3 "graph never filtered". The Overview count line keeps `modules.len()`
  — intentionally consistent with the whole-graph `symbol_count`/`edge_count` on the
  same line and outside the plan's stated scope (Hot-Spots/Coupling/layer diagram).
- `for_module`'s `_scope` is intentionally unused for API uniformity per the tier
  `<files>` ("take `&DocScope` as the last param"); the `_` prefix is the correct
  idiom and clippy is clean.
- Hexagon intact: `doc_model` consumes only std; `lib.rs` is re-export-only;
  callers in the daemon/mcp driving+driven layers pass `DocScope::default()`.

Two non-defect observations, logged for transparency (neither is an actionable
INFO finding — nothing to fix):
1. The plan `<files>` predicted the `docgen_fixture` golden snapshot would change
   ("fixture/test rows gone from tables"). The `.snap` files are unchanged because
   the `core_fixture` module names are all Source-classified, so scoping is a no-op
   there. Correct outcome — exit-2 is instead proven by the dedicated `doc_scope`
   test on a real fixture path. The proptest snapshots staying byte-identical also
   confirms scoping preserves determinism.
2. `daemon/tests/support.rs` is touched outside the tier `<files>` list; this is a
   compile-forcing mechanical update (see `<scope>`), not scope creep.
</verdict>

<next_steps>
None. Tier-01 is accepted. Proceed to tier-02 (`tier-02-svg-emitter`), which
consumes `crate_of`/`LayerHint` and the `DocScope` threading established here.
</next_steps>

<sources>
- Tier file: `.claude/plans/useful-docgen/tier-01-doc-scope-model.md` (exit_criteria, steps, verification).
- Plan: `.claude/plans/useful-docgen/plan.md` D3 (doc-layer scoping), D5 (no new dep), constraints.
- CLAUDE.md `<architecture>` (façade re-export-only, hexagon boundary), `<rules>` (TDD, validate-by-execution).
- Re-run command output captured in this session (graph 46/46, daemon+mcp 103/103, clippy 0, fmt 0, architecture ok, deny ok).
</sources>
