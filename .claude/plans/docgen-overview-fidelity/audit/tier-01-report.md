---
tier_id: tier-01
audited: 2026-06-04
verdict: PASS
commit: c54557c1bf319928abefa59ae340c2cae52a4ea6
---

<scope>
Audited tier-01 ("Uniform DocScope across sections; suppress R1-contaminated
output") of plan `docgen-overview-fidelity`. Scoped diff (working tree vs HEAD
`c54557c`):
- `crates/ariadne-graph/src/doc_model.rs` (+19): `layer_of` override + `DOMAIN_INTERIOR_CRATES`.
- `crates/ariadne-graph/src/docgen.rs` (±6): wiring `scope` into `cycle_clusters`/`change_coupling`, drop `graph` from `architecture_section`.
- `crates/ariadne-graph/src/docgen_insights.rs` (±204): scope threading, Role/boundary suppression, cross-crate cycle withhold, `crate_of`-based synopsis count, layer override.
- `crates/ariadne-graph/tests/docgen_project.rs` (+203): 5 new behavioural tests + crate-path fixture.
- `crates/ariadne-graph/tests/support.rs` (+44): `snapshot_from` helper (within the `<files>` `tests/` scope; justified by the new fixture).
- `crates/ariadne-graph/tests/snapshots/docgen_fixture__project.snap` (±16): re-accepted golden.
- `docs/codebase-overview.md` (±63): regenerated overview. `.svg` regenerated identically (no diff — it encodes no layer/role text).
No file outside the tier `<files>` set was touched; no `Cargo.toml`/`Cargo.lock` change (no new dependency).
</scope>

<checks_run>
- `cargo nextest run -p ariadne-graph` → 67/67 pass, including all 5 new tier-01 tests (`overview_leaks_no_test_or_fixture_paths`, `architecture_rows_pin_each_crate_layer`, `synopsis_crate_count_excludes_tools_dir`, `role_and_boundary_sections_are_withheld`, `cross_crate_cycle_is_withheld_intra_crate_listed`).
- `cargo nextest run -p ariadne-daemon -p ariadne-mcp` → 104/104 pass (incl. `refactor_suggestions_lists_findings`, `warm_apply_equals_fresh_rebuild`, memory-probe budget).
- `cargo run -p ariadne-cli -- doc` run twice → `docs/codebase-overview.md` AND `.svg` byte-identical (md5 stable `5cd68132…`; `diff` empty both files). First run also matched the committed working-tree md exactly → committed overview is current.
- `grep '/tests/|.snap|/fixtures/'` on the overview → no match.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` → clean (no warnings; `classify_violation` carries a justified `#[allow(dead_code)]` per step 5; `boundary_violations` params `_`-prefixed).
- `cargo fmt --all --check` → exit 0.
- `cargo test --test architecture` → `architecture_invariants_hold` ok (hexagonal boundaries intact; change is pure within `ariadne-graph`).
- Read every changed file end-to-end; verified `layer_of`, `crate_of`, `DocScope::include`, `classify` semantics against the call sites.
</checks_run>

<exit_criteria_check>
1. scope on every endpoint; no test/snap/fixture path in overview — VERIFIED (grep clean; `change_coupling` filters both endpoints before `structurally_linked`; `cycle_clusters` drops clusters with any out-of-scope member).
2. Architecture keeps Crate+Layer, Role replaced by explicit withheld marker + R1 note — VERIFIED (overview rows show `_withheld (R1)_`; note line "Role withheld — depends on cross-crate edge accuracy (R1)."). No row claims a coupling shape.
3. Boundary section is a single withheld line referencing R1; no `X → new` rows — VERIFIED (overview line 31; old `apply_writes → new` etc. gone).
4. Cross-crate cycle clusters withheld with explicit line; intra-crate still listed — VERIFIED by the synthetic fixture test; in production the prior 22-member cross-crate cluster is dropped (out-of-scope member) and the intra-crate 2-member cluster still renders — plan-conformant (step 3 drops out-of-scope clusters, withholds surviving cross-crate ones).
5. core/graph/salsa render Layer=Domain; test pins each crate's layer — VERIFIED (overview rows 14/18/21 Domain; `architecture_rows_pin_each_crate_layer` pins Domain for the 3 interior crates AND Adapter/Interior fallback for storage/cli).
6. Synopsis counts only `crates/` crates (excludes `tools/`) — VERIFIED (overview "12 crate(s)", down from 13; `synopsis_crate_count_excludes_tools_dir` pins 5).
7. god_modules/refactor + warm==cold green; `ariadne doc` twice byte-identical; clippy/fmt/architecture green — VERIFIED (see checks_run).
</exit_criteria_check>

<findings>
| id | category | severity | location | problem | fix |
| --- | --- | --- | --- | --- | --- |
| I1 | correctness | INFO | docs/codebase-overview.md:74-75; doc_model.rs:30-47 | `DocScope` (Source-only) still admits non-code files — `classify` treats `Cargo.lock`, `Cargo.toml`, `.claude/**/audit-state.json` as `Source`, so they surface as "hidden change-coupling". Pre-existing `classify` limitation made visible now that test/snap pairs are filtered out; no exit criterion violated. | If undesired, extend `classify`/`extra_excludes` to exclude manifest/config/plan files (out of tier-01 scope). |
| I2 | docs | INFO | docs/codebase-overview.md:5 vs :25; docgen_insights.rs synopsis (`crate_of`) vs architecture_section (`crate_key`) | Synopsis reports "12 crate(s)" while the Architecture table lists 13 `Crate` rows (includes `tools`). Internal inconsistency: synopsis uses `crate_of` (crates/-only), architecture uses `crate_key` (first-segment fallback). The `tools` row predates this tier; only the synopsis count changed. | Align architecture crate grouping with `crate_of` (a plan-scope decision; not in tier-01 `<steps>`). |
| I3 | tests | INFO | crates/ariadne-graph/tests/snapshots/docgen_fixture__project.snap:9 | The `docgen_fixture` golden now reads "0 crate(s) · 8 source symbol(s)" — the bare-name fixture (`api`/`core`/…) no longer counts as crates under `crate_of`, baking a degenerate value into the snapshot. Faithful + deterministic, but weakens the golden as a crate-count guard (the new `docgen_project` fixture covers real behaviour). | Prefix the `docgen_fixture` modules with `crates/<name>/` or document why 0 is expected. |
| I4 | correctness | INFO | crates/ariadne-graph/src/docgen_insights.rs:63 | `module_role` (per-module doc) still uses `LayerHint::of`, not `layer_of`, so `doc_for_module` on a flat-`src` interior crate (core/graph/salsa) labels it `Interior layer`, contradicting the project table's `Domain`. Outside tier-01 declared `<files>`; step 6 scoped the override to `dominant_layer`/`architecture_section`. | Switch `module_role` to `layer_of` (follow-up; not gating). |
</findings>

<verdict>
PASS. Zero FAIL findings. All seven exit criteria independently verified against the
regenerated overview and the green test/clippy/fmt/architecture runs. The two
R1-contaminated sections (Boundary violations, Architecture Role) and cross-crate
cycle cuts are suppressed with explicit, reader-visible withheld lines; `DocScope`
is now applied uniformly to `change_coupling` and `cycle_clusters`; the domain-interior
crate layer override is correct and test-pinned; output is byte-deterministic across runs.
The four INFO findings are non-blocking — I1 is a pre-existing `classify` limitation now
surfaced, I2/I4 are inconsistencies the plan deliberately left out of tier-01 scope, I3 is
a synthetic-golden cosmetic.
</verdict>

<next_steps>
None required to pass. Optional, for the user's consideration:
- Decide whether I2 (synopsis 12 vs architecture 13 rows) is acceptable to ship or should be reconciled — it is the only INFO that produces a visible contradiction in the shipped doc.
- I1 (manifest/config files in change-coupling) and I4 (`module_role` layer label) are natural candidates for tier-02/tier-03 or a small follow-up.
</next_steps>

<sources>
- Tier file: .claude/plans/docgen-overview-fidelity/tier-01-scope-and-suppress.md
- Plan: .claude/plans/docgen-overview-fidelity/plan.md (D1, D3)
- Diff: crates/ariadne-graph/src/{doc_model,docgen,docgen_insights}.rs; tests/{docgen_project,support}.rs; tests/snapshots/docgen_fixture__project.snap; docs/codebase-overview.md
- Re-run logs: cargo nextest (graph 67/67, daemon+mcp 104/104), clippy, fmt, `cargo test --test architecture`, `ariadne doc` ×2 (md5 5cd68132…)
- [Reviewer standard — code health over perfection](https://google.github.io/eng-practices/review/reviewer/standard.html)
</sources>
