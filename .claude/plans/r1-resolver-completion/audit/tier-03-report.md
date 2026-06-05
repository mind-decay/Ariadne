---
tier_id: tier-03
audited: 2026-06-05
verdict: PASS
commit: a2f6b45f8e984731b2eb1e14a7277db27c441504
---

<scope>
Audit of `tier-03-crate-role-test-scope` (r1-resolver-completion): exclude a
crate's own out-of-scope test edges from crate-level afferent so leaf crates
read volatile rather than stable-foundational.

Tier-03's changes are HELD in the working tree (uncommitted — tier-02 lands
them), so the scoped diff is `git diff HEAD` over the tier's `<files>`. HEAD is
the tier-01 commit `a2f6b45`. Files reviewed end-to-end:
- `crates/ariadne-graph/src/docgen.rs` — `for_project` threads the unscoped
  `modules` slice into `architecture_section` (lines 315-318, 289-310).
- `crates/ariadne-graph/src/docgen_insights.rs` — `architecture_section`
  signature `(scoped)` → `(graph, scoped, modules)`; crate-coupling membership
  built over ALL crate modules, rows filtered to scoped-source crates.
- `crates/ariadne-graph/tests/docgen_project.rs` — new `test_scope_fixture` +
  `architecture_excludes_test_edges_from_crate_afferent`; `section_of` helper.
- `crates/ariadne-graph/tests/snapshots/docgen_fixture__project.snap` — Role
  column / boundary / cycle move, reviewed below.
- `crates/ariadne-graph/tests/support.rs` — listed in `<files>` as a may-touch
  ("or add one"); UNtouched. The fixture reuses existing `support::sid` /
  `support::snapshot_from` helpers, so no extension was required. Not a defect.

Out of scope (noted, not audited here): the working-tree diff to
`docgen_insights.rs` also re-enables `boundary_violations` and `cycle_clusters`
and removes the `ROLE_WITHHELD` const. These are the HELD docgen-overview-
fidelity tier-03 changes that r1 tier-02 lands (plan.md D4); this tier's
exit-criteria name `boundary_violations_listed_qualified_and_bounded` as an
EXISTING test that must "stay green", confirming that re-enable predates
tier-03. They are covered by docgen-overview-fidelity's own audit
(`audit/tier-02-report.md`). The documented intermingling is expected (plan.md
R4/R8) and gates neither this verdict.
</scope>

<checks_run>
All `<verification>` commands re-run from the held working tree:
- `cargo nextest run -p ariadne-graph` → 70/70 pass. The new
  `architecture_excludes_test_edges_from_crate_afferent` is green;
  `architecture_role_restored_cli_is_volatile_leaf`,
  `boundary_violations_listed_qualified_and_bounded`,
  `architecture_rows_pin_each_crate_layer`,
  `synopsis_crate_count_excludes_tools_dir`, `for_project_is_deterministic`
  all green (re-run explicitly: 6/6).
- `cargo nextest run -p ariadne-daemon -p ariadne-mcp` → 104/104 pass, incl.
  `warm_apply_equals_fresh_rebuild` (warm==cold) and `memory_probe` within
  budget.
- `coupling_report_matches_cold` (daemon) + `coupling_arm_matches_cold_output`
  (mcp) green → per-file coupling output byte-unchanged (exit #4).
- `cargo test --test architecture` → 1/1 pass (hexagonal invariants hold).
- `cargo fmt --all --check` → clean (exit 0).
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` →
  clean.

Code-path verification:
- `coupling_report` sorts rows by name (coupling.rs:74); `layer_votes` is a
  `BTreeMap`; the row loop skips rows with no scoped layer-vote — so displayed
  order and crate set equal the prior scoped set. Snapshot keeps 4 rows
  (cli/core/salsa/storage), row count unchanged (R6).
- `metrics_for` afferent guard `member_of.get(&src) != Some(mid)`
  (coupling.rs:104): with membership over ALL crate modules, an intra-crate
  test→source edge has the same `mid` for both endpoints → dropped from
  afferent. Confirmed by the fixture: fixed Ca=0,Ce=1→I=1.0→Volatile leaf;
  naive scoped-only Ca=3,Ce=1→I=0.25→Stable foundational (genuine red-first,
  exit #2) against `purpose` thresholds (docgen.rs:399-409).
- `crate_of` maps `crates/ariadne-x/tests/it.rs` and
  `crates/ariadne-x/src/lib.rs` to the same `ariadne-x` key — proven by the
  passing fixture (otherwise the test edges stay afferent and it renders
  stable-foundational).
- `for_project` passes the genuinely unscoped `modules` (docgen.rs:292) plus
  the filtered `scoped` (306-310); `coupling.rs` is unmodified (git status) →
  public `coupling_report`/`metrics_for` byte-unchanged.
- No dangling `ROLE_WITHHELD` / withheld-marker references remain
  (`grep crates/ariadne-graph/src` → none); clippy `-D warnings` confirms.
- Held changes are uncommitted (` M` in git status; HEAD = tier-01 a2f6b45),
  satisfying exit #5 "NOT committed here".
</checks_run>

<findings>
| id | category | severity | location | problem | fix |
| --- | --- | --- | --- | --- | --- |
| — | — | — | — | No FAIL or INFO findings. | — |
</findings>

<verdict>
PASS. The membership-universe fix is correct, deterministic, and scoped to
`architecture_section`: crate-coupling specs aggregate over all crate modules
(source + test) keyed by `crate_of`, dropping intra-crate test→source edges
from afferent, while displayed rows stay the source-only crate set via the
`layer_votes` skip. `coupling.rs`/`metrics_for`/public `coupling_report` are
byte-unchanged (exit #4). Every exit criterion is independently verified and
all `<verification>` commands pass on re-run. The new fixture is genuinely
red-first by construction. The broader docgen-insights diff (boundary/cycle
re-enable) is the held sibling-plan work landed by tier-02, audited elsewhere,
and does not regress any tier-03 criterion.
</verdict>

<next_steps>
None required for tier-03. The held changes proceed to tier-02 for commit;
the dogfood "12 rows / leaf crates volatile, boundary near-zero" assertion is
a tier-02 verification (re-index of the committed binary), not testable in this
tier's fixture suite — confirm it there.
</next_steps>

<sources>
- coupling membership / afferent guard: crates/ariadne-graph/src/coupling.rs:67-138
- role thresholds: crates/ariadne-graph/src/docgen.rs:399-409
- threaded call site: crates/ariadne-graph/src/docgen.rs:288-318
- petgraph edges_directed semantics: https://docs.rs/petgraph/latest/petgraph/graph/struct.Graph.html#method.edges_directed
- code-review standard (ship-if-satisfies-plan): https://google.github.io/eng-practices/review/reviewer/standard.html
</sources>
