---
tier_id: tier-01
audited: 2026-06-07
verdict: PASS
commit: ad59c2f6f3ec78c92ad3a402d00e62c46979b239
---

<scope>
A1 — static test-impact reachability (`affected_tests`). Reviewed the working-tree
diff scoped to tier-01 `<files>` against `block-a/plan.md` and the parent arc plan.
HEAD `ad59c2f`; the tier diff is uncommitted (the audit-gate hook gates the commit).

Files reviewed end-to-end:
- new: `ariadne-graph/src/test_impact.rs`, `ariadne-mcp/src/tools/affected_tests.rs`,
  `ariadne-cli/src/commands/affected_tests.rs`, `ariadne-e2e/tests/affected_tests.rs`.
- modified: `ariadne-graph/src/lib.rs`; `ariadne-core` daemon protocol
  (`daemon/{mod,query,response}.rs`, `lib.rs`); `ariadne-daemon`
  (`catalog.rs`, `dispatch.rs`, `queries/impact.rs`); `ariadne-mcp`
  (`server.rs`, `types.rs`, `tools/mod.rs`, `tests/handshake.rs` + 2 snapshots);
  `ariadne-cli` (`commands/{mod,query}.rs`, `main.rs`).

Files modified outside the literal `<files>` list, each justified: `queries/impact.rs`
(warm dispatch handler — where the `diff_blast` warm impl already lives, the home of
the "warm dispatch arm" step 4 names); `commands/query.rs` (routes `affected_tests`
to the dedicated command since it needs the client-side git diff — step 6); the
handshake test + snapshots (purely additive new-tool regen).
</scope>

<checks_run>
- `cargo fmt --all --check` → clean (exit 0).
- `cargo test --test architecture` → `architecture_invariants_hold` ok (no adapter→adapter
  edge; daemon stays git-free).
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` → clean (exit 0).
- `cargo nextest run --workspace` → 481 passed, 0 failed, 19 skipped.
- New tests run + pass (filtered): graph `classifies_a_test_symbol_in_every_language`,
  `does_not_classify_plain_symbols`, `affected_tests_returns_only_the_test_ancestor`;
  daemon `build_projects_test_roots`; mcp `affected_tests_arm_matches_cold_output`;
  e2e `affected_tests_working_tree_returns_the_reachable_test` (6/6 pass).
- End-to-end on a hand-seeded git fixture (edit inside `target()`, test `checks_target`
  calls it):
  - COLD `ariadne affected-tests working_tree` and `ariadne query affected_tests
    '{"spec":"working_tree"}'` → both return `tests=[checks_target]`, `seeds=[target]`,
    `unresolved=[]`.
  - WARM (live daemon pid confirmed via `daemon status`, autospawn off) → both routes
    byte-identical to cold. Warm==cold parity confirmed at the binary level.
- Determinism: repeated cold runs byte-identical; e2e asserts the same.
- Snapshot review: `handshake__tools_list.snap` additive only (no deletions);
  `handshake__tools_descriptions.snap` adds only the `affected_tests` entry;
  `EXPECTED_TOOLS` 19→20 with a documented comment.
- Depth contract: `DEFAULT_DEPTH = 3` and `.max(1)` clamp identical across warm
  (`queries/impact.rs:262`) and cold (`tools/affected_tests.rs:72`), matching `diff_blast`.
- Determinism of outputs: graph `tests` = `reached.intersection(test_roots)` (BTreeSet
  iteration, SymbolId order); `seeds` BTreeSet order; `unresolved` sorted+deduped.
- `apply_changeset` ordering verified: `self.paths` updated (file_upserts) before the
  `test_roots` re-classify block (catalog.rs:253-276), so path-convention classification
  reads the new path.
- Classifier domain: covers all 14 named `Lang` variants; the 15 fixture trees include
  `react`/`solid`, which carry no distinct `Lang` (they parse as Tsx/JS/TS) and so reuse
  the already-asserted JS/TS arms — no classification arm is left untested.
</checks_run>

<findings>
| id | category | severity | location | problem | fix |
|----|----------|----------|----------|---------|-----|
| F1 | tests | INFO | `crates/ariadne-daemon/src/domain/dump.rs:60-73`; `crates/ariadne-daemon/src/domain/catalog.rs:253-276` | The incremental `test_roots` maintenance on `apply_changeset` is not guarded by `warm_apply_equals_fresh_rebuild` because `CatalogDump` omits the `test_roots` field; only the build path is tested (`build_projects_test_roots`). Logic is a pure re-derivation over metadata that *is* compared, so divergence is unlikely, but the maintenance code path is unverified. | Add `test_roots` to `CatalogDump` (one field), giving the divergence-0 proptest free coverage of the apply path. |
| F2 | docs | INFO | `crates/ariadne-graph/src/test_impact.rs` (457 total / 256 impl) | Exceeds the block-a plan `<constraints>` line "one authored file ≤200 lines". Non-gating: the CLAUDE.md ≤200 rule is scoped to doc files, and 5 existing `ariadne-graph` use-cases already exceed 200 lines (docgen_insights 578, docgen 472, build 278, diagram 266, refactor 232) under prior PASS audits; the bulk here is plan-mandated inline per-language `[src:]` citations + plan-mandated inline tests. | Reconcile the plan text (it reads as doc-files-only in CLAUDE.md), or split the inline test module to a sibling if the cap is meant literally. |
</findings>

<verdict>
PASS. Zero FAIL findings. Every `<verification>` command was re-run green, the feature
was exercised end-to-end on both the warm (live daemon) and cold paths with byte-identical,
correct output, and all five `exit_criteria` are independently satisfied:
(1) `nextest` workspace green incl. the new failing-first tests; (2) `classify_test_symbols`
marks a test symbol across every `Lang` arm (all 14 variants; the 15 fixture trees collapse
onto them, react/solid included); (3) the e2e golden returns exactly the hand-verified set,
byte-identical across runs; (4) `ariadne query affected_tests` and `ariadne affected-tests`
print that set on warm and cold; (5) clippy/fmt/architecture all green. Architecture holds —
no adapter→adapter dependency, the daemon stays git-free (diff runs client-side, only hunks
travel), and the surfacing mirrors `diff_blast` site-for-site. The two INFO findings are
non-blocking and need no rework to ship.
</verdict>

<next_steps>
None required for PASS. Optional, at the author's discretion: F1 — add `test_roots` to
`CatalogDump` to extend the divergence-0 guard over the incremental projection; F2 —
reconcile the plan's ≤200-line line with the doc-files-only scope it derives from.
</next_steps>

<sources>
- Tier + plan: `.claude/plans/intelligence-platform/block-a/tier-01-test-impact.md`;
  `.claude/plans/intelligence-platform/block-a/plan.md` (D1/D2, constraints, BR2).
- Test-impact technique: https://martinfowler.com/articles/rise-test-impact-analysis.html
- Precedent: `crates/ariadne-graph/src/diff_blast.rs`; `crates/ariadne-mcp/src/tools/diff_blast.rs`.
- Reviewer standard: https://google.github.io/eng-practices/review/reviewer/standard.html
</sources>
