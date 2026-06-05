---
tier_id: tier-04
audited: 2026-06-05
verdict: PASS
commit: 97f122a01b2ed4b85e1a0dea5e609eabcdd7544f
---

<scope>
Tier-04 "Abstain same-crate Method/Path callees with no same-file definition;
commit tier-01". Two committed changesets reviewed:
- `985116d fix(salsa): gate cross-crate fallback to free call shapes` — step-1
  base commit (tier-01's resolver/parser work).
- `97f122a fix(salsa): abstain method/path callees without a same-file
  definition` — the tier-04 deliverable (steps 4–7).
Files read end-to-end: `crates/ariadne-salsa/src/derive.rs` (`resolve_edges`,
`CallKind`), `crates/ariadne-salsa/tests/scoped_resolution.rs`,
`docs/adr/0025-shape-scoped-same-crate-resolution.md`,
`docs/adr/0024-scoped-call-resolution.md` (status flip). The held docgen graph
working-tree changes (tier-02) are explicitly out of scope and remain
uncommitted.
</scope>

<checks_run>
- **plan_adherence.** Step-1 commit `985116d` is correctly path-scoped: it
  touches parser/salsa/cli-mod/daemon-facts/ADR-0024/parser-tests ONLY — none of
  the held docgen files (`docgen.rs`, `docgen_insights.rs`, `tests/docgen_project.rs`,
  the project snapshot) leaked in (`git show --stat 985116d`). Exit criterion #1
  satisfied. `97f122a` touches exactly the tier-04 `<files>`: `derive.rs`,
  `scoped_resolution.rs`, ADR-0024 (status), ADR-0025 (new). Nothing outside the
  list.
- **correctness.** `resolve_edges` reads the call shape into a single
  `wide_scope` flag = `matches!(kind, CallKind::Free)`. Narrow (Method/Path):
  `in_scope = same_file`, `resolved = in_scope` — same-file only, no same-crate,
  no unambiguous-global. Wide (Free + every render/hook, which pass `true`):
  `same_file.or_else(same_crate)` then `.or_else(unambiguous)` — the full ADR-0024
  ladder, unchanged. `CallKind::from_byte` maps `1→Method`, `2→Path`, `_→Free`
  (recall-preserving default); test seeds use `0/1/2` consistently
  (derive.rs:94-100; scoped_resolution.rs:75-82).
- **security.** N/A surface — internal `pub(crate)` resolver over already-parsed
  facts; no input parsing, secrets, authz, injection, or deserialization. The
  unknown-byte→Free fallback is documented and has only two controlled producers.
- **performance.** No new hot-path allocation; closures are lazy and unused on the
  narrow path; deterministic (sorted candidate lists + `HashSet` dedup). Worktree
  cold index ~0.9s; salsa `memory_probe` budget test green (<256MB/table).
- **architecture.** `ariadne-salsa` keeps its local `CallKind` mirror (no
  `ariadne-parser` dep); `cargo test --test architecture` green; `cargo deny check`
  → advisories/bans/licenses/sources ok (only pre-existing unmatched-license
  *warnings*). No smuggled dependency or pattern.
- **tests.** Two RED spikes
  (`path_/method_shaped_same_crate_different_file_callee_yields_no_edge`), the
  positive control (`method_shaped_same_file_callee_still_resolves`), and the
  recall guards (`unambiguous_global_callee_resolves_cross_crate`,
  `same_crate_call_resolves_within_caller_crate_not_collision`) all assert edge
  presence/absence (behavior, not implementation) with loud messages. RED→GREEN is
  structurally sound: the pre-fix line was `in_scope = same_file.or_else(same_crate)`
  unconditionally, so a same-crate different-file Method/Path callee bound via the
  same-crate tier; the gate now refuses it.
- **docs.** ADR-0025 is complete (status Accepted, supersedes clause, measured
  PROOF block, decision, rationale, three rejected alternatives, consequences,
  cited sources). ADR-0024 status flip is correctly *scoped* to the same-crate
  non-Free clause — the Free ladder explicitly stands. The `resolve_edges` doc
  comment was updated to match the new behavior.
- **exit_criteria.** All six independently verified (below).
</checks_run>

<findings>
| id | category | severity | location | problem | fix |
|----|----------|----------|----------|---------|-----|
| — | — | — | — | No defect found. Zero FAIL, zero INFO. | — |
</findings>

<verdict>
**PASS.** Re-ran the full `<verification>` against HEAD `97f122a`; every command
green.

- `cargo nextest run -p ariadne-parser -p ariadne-salsa -p ariadne-daemon
  -p ariadne-mcp` → **184/184 passed.** Tier-04 spikes green
  (`path_/method_shaped_same_crate_different_file_callee_yields_no_edge`); positive
  control green (`method_shaped_same_file_callee_still_resolves`); recall guards
  green (`unambiguous_global_callee_resolves_cross_crate`,
  `same_crate_call_resolves_within_caller_crate_not_collision`,
  `ambiguous_callee_with_no_in_scope_definition_yields_no_edge`); parity green
  (`equivalence::fresh_vs_incremental_equivalence`,
  `daemon::incremental_warm::warm_apply_equals_fresh_rebuild`); memory probe green
  (`memory_probe::warm_graph_tables_stay_within_the_per_table_budget`).
- `cargo test --test architecture` → 1 passed. `cargo fmt --all --check` → clean.
  `cargo clippy --workspace --all-targets --all-features -- -D warnings` → clean.
  `cargo deny check` → ok (warnings are pre-existing unused license allowances).
- **Determinism / reindex (exit #6).** Indexed a throwaway HEAD worktree (the
  user's live daemon untouched): three consecutive cold indexes each reported
  **2059 edges / 3628 symbols** — identical edge set. The live daemon graph reports
  2064 edges / 3632 symbols; the +5/+4 delta is exactly the uncommitted tier-02
  docgen working-tree edits in the main repo, not a resolver discrepancy. Both
  match the commit/ADR claim of 3339→~2064 (phantom class removed).
- **Phantom gone (exit #6).** `doc_for(run_index)` shows its reference set no
  longer includes the cli adapter `new` (`crates/ariadne-cli/src/adapters/
  daemon_client.rs`); combined with the aggregate edge drop and the green
  Path-same-crate-different-file spike (exactly the `X::new()` shape), the
  `→ *::new` domain→adapter phantom class is gone.
- **Premise measured (exit #2).** The MEASURE step's residual-row classification
  is recorded durably in ADR-0025's PROOF block (committed tier-01 binary, 3339
  edges, four phantom rows enumerated by caller/callee/shape/reason), satisfying
  "measured, not assumed".

Note (scope, not a defect): the *rendered* overview "boundary section near-zero"
is verified here only at the EDGE level (phantom edges gone, count drop). The full
rendered boundary section rides tier-02's still-uncommitted docgen changes and is
that tier's verification gate (plan D4), not tier-04's.
</verdict>

<next_steps>
None. Verdict PASS — proceed to tier-02 (land the held docgen tier-03 rendering on
the now-reliable edge set, flip its `status` blocked→completed, regenerate the
overview).
</next_steps>

<sources>
- crates/ariadne-salsa/src/derive.rs:255-336 (`resolve_edges`), :79-101 (`CallKind`)
- crates/ariadne-salsa/tests/scoped_resolution.rs:84-131,325-397
- docs/adr/0025-shape-scoped-same-crate-resolution.md ; docs/adr/0024-scoped-call-resolution.md (status)
- git show --stat 985116d, 97f122a (commit scoping)
- [OWASP Top 10](https://owasp.org/www-project-top-ten/) — no applicable item (no external input surface)
- [Google eng-practices — reviewer standard](https://google.github.io/eng-practices/review/reviewer/standard.html)
</sources>
