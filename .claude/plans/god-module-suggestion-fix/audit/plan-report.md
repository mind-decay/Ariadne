---
tier_id: single
audited: 2026-06-04
verdict: PASS
commit: 710f3c80cd069940babab7c42eeb3ff894ff4b76
---

<scope>
Single-tier plan `god-module-suggestion-fix` (`status: completed`). Defect:
`refactor::god_modules` keyed the outbound histogram on the edge *target* (an
external callee), so the split suggestion named a non-member (`clone`/`new`/…) —
incoherent "extract X" where X lives in another module. Fix re-keys the ranking
on the source *member* by external fan-out, keeping Ce as distinct external
targets [plan.md `<context>`, D1].

Diff scoped to `<files>`:
- `crates/ariadne-graph/src/refactor.rs` — `god_modules` logic + doc comments.
- `crates/ariadne-graph/tests/refactor_cases.rs` — new regression test.
- `crates/ariadne-graph/tests/snapshots/refactor_cases__god_modules.snap` — re-accepted.
- `crates/ariadne-core/src/domain/daemon/rows.rs` — doc-comment-only.
- `docs/codebase-overview.{md,svg}` — regenerated.
The rest of the working tree (useful-docgen changes) is out of scope; the
god-module fix is confined to these five files — plan adherence holds.
</scope>

<checks_run>
- plan_adherence: every `<files>` entry touched as intended; nothing outside the
  fix's scope. rows.rs change is `///`-only (verified line-by-line). No
  daemon/MCP code changed (D3 field names kept).
- correctness: re-derived the fixture by hand. `core` members {1,2,3}; external
  edges → targets {5,7,8} ⇒ Ce=3 (unchanged). `by_member`={1:1, 2:2},
  `total_out`=3 (per-edge, identical denominator to old). `top`=[(2,2),(1,1)];
  `core::run`(sym 2) IS a core member; `pct_of(2,3)=67`. Suggestion text matches
  step 4 verbatim. Matches the re-accepted snapshot exactly.
- ce_invariant: `efferent = external.len()` (distinct external targets) ==
  prior `outbound.len()` semantics; gate at refactor.rs:118 byte-unchanged.
- architecture: change isolated to `ariadne-graph` (logic) + `ariadne-core`
  rows.rs (docs). No new dep, no smuggled tech, hexagonal boundary intact
  (`cargo test --test architecture` green).
- tests: new `god_module_split_names_a_member_and_pins_ce` asserts
  `top_outbound[0] ∈ module.members` AND recomputes Ce independently from
  `support::edges()` — pins both D1 and the Ce-redefinition risk. Loud asserts.
- docs: God-modules section names members (build_symbol_lines, run_index,
  dispatch, start, warm, doc_for, weak_spots, file_summary,
  verify_framework_fixture, build) — zero `clone`/`new`/`get`/`default`.
- exit_criteria: all four behavioural criteria + the toolchain criterion
  independently verified (below).

Commands re-run (full `<verification>`):
- `cargo nextest run -p ariadne-graph` → 62/62 pass (new test + re-accepted golden).
- `cargo nextest run -p ariadne-daemon -p ariadne-mcp` → 104/104 pass, incl.
  `refactor_suggestions_matches_cold` (warm==cold full-struct equality),
  `refactor_suggestions_lists_findings`, `warm_analytics`, `memory_probe`.
- `cargo test --test architecture` → 1/1 pass.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` → clean.
- `cargo fmt --all --check` → clean (exit 0).
- `cargo deny check` → advisories/bans/licenses/sources ok (only pre-existing
  unused-license-allowance warnings).
- `ariadne doc` ×2 with identical args → md + svg byte-identical
  (`diff` empty); regenerated md matches committed working tree byte-for-byte.
</checks_run>

<findings>
| id | category | severity | location | problem | fix |
| --- | --- | --- | --- | --- | --- |
| — | — | — | — | No defects found. | — |
</findings>

<verdict>
PASS. Zero FAIL, zero INFO.

Exit criteria — all met:
1. `top_outbound[0]` names a module member: regression test asserts containment;
   snapshot shows `core::run` (a core member), not a callee. ✓
2. Ce + threshold/cohesion gate unchanged: snapshot Ce=3 (was 3); gate line
   verbatim; unit test pins Ce from the fixture independently. ✓
3. Regenerated docs name members; no `clone`/`new`/`get`/`default` split target. ✓
4. Golden re-accepted; warm==cold + refactor MCP tests green; overview
   byte-identical on re-run. ✓
5. clippy/fmt/deny/`--test architecture` green. ✓

The per-edge vs distinct-symbol asymmetry (member ranking counts edges; Ce counts
distinct targets) is intentional and matches D1 — traffic-weighted extraction
candidate vs coupling count. Not a defect.
</verdict>

<next_steps>
None. Ready to commit (audit-state.json updated to PASS at this HEAD).
</next_steps>

<sources>
- repo: crates/ariadne-graph/src/refactor.rs:90-138,227-232; tests/refactor_cases.rs:62-98;
  tests/support.rs:95-127,191-215; crates/ariadne-core/src/domain/daemon/rows.rs:94-118;
  crates/ariadne-daemon/tests/warm_analytics.rs:296-319; docs/codebase-overview.md:63-75.
- [petgraph EdgeReference — docs.rs](https://docs.rs/petgraph/latest/petgraph/graph/struct.EdgeReference.html)
- [Google eng-practices — reviewer standard](https://google.github.io/eng-practices/review/reviewer/standard.html)
</sources>
