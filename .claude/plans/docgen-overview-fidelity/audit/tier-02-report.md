---
tier_id: tier-02
audited: 2026-06-04
verdict: PASS
commit: a2f6b45f8e984731b2eb1e14a7277db27c441504
---

<scope>
Tier-02 "R1 — scope index-time callee resolution; stop name-collision phantom
edges" of plan `docgen-overview-fidelity`. Scoped diff = commit `a2f6b45`
(the sole tier-02 commit; tier-01 work landed in `63adc4b`):
- `docs/adr/0024-scoped-call-resolution.md` (new, Accepted) — the ADR the tier
  required under `docs/adr/NNNN-scoped-call-resolution.md`.
- `crates/ariadne-salsa/src/derive.rs` — `package_of`, `ResolvedCandidate`, the
  scoped `resolve_edges` precedence.
- `crates/ariadne-salsa/src/db.rs` — per-file `package` key threaded into
  `SymbolCandidate`/`FileFacts`.
- `crates/ariadne-salsa/tests/scoped_resolution.rs` (new) — repro + std-callee +
  recall-guard.
- the tier file itself (`status: pending → completed`).

Locus note: the `<files>` block guessed the resolver in `ariadne-scip` /
`ariadne-parser`; the spike correctly pinned it to `ariadne-salsa`
(`derive.rs::resolve_edges`) and recorded the path in ADR-0024. The plan
explicitly deferred the exact locus to the spike (`<risks>`: "R1 resolver locus
mis-identified"), so the salsa locus is plan-adherent. `ariadne-core` was not
touched (the "only if needed" clause did not trigger — scope context is derived
from the path via `package_of`). No public-API or adapter-boundary change.
</scope>

<checks_run>
- `cargo fmt --all --check` → clean.
- `cargo test --test architecture` → 1 passed (hexagonal invariants hold; no new
  adapter→adapter dep).
- `cargo nextest run --workspace` → 432 passed, 0 failed, 19 skipped. Includes
  `ariadne-salsa::incremental incremental_sequence_equals_fresh_rebuild`
  (determinism / incremental==fresh) and `ariadne-daemon::incremental_warm
  warm_apply_equals_fresh_rebuild` (warm==cold parity).
- Tier tests `crates/ariadne-salsa/tests/scoped_resolution.rs` → 3 passed.
- RED→GREEN proof: checked out the pre-fix `derive.rs`+`db.rs` (parent of
  `a2f6b45`), kept the new test → `same_crate_call_resolves_within_caller_crate
  _not_collision` and `ambiguous_callee_with_no_in_scope_definition_yields_no_
  edge` FAILED (collision bound to crate B; `new` bound to a global); restored.
  The regression tests are genuine, not tautological.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` →
  Finished, no warnings.
- `cargo deny check` → advisories ok, bans ok, licenses ok, sources ok (no new
  dependency; warnings are pre-existing unmatched license allowances).
- Dogfood reindex (clean HEAD git worktree, fresh binary): cold index → 3953
  edges, matching ADR-0024's claimed post-fix count exactly (live pre-fix daemon
  still shows 5420). Re-index of same input → 3953 again (determinism). On that
  fresh graph `doc_for apply_writes` reports blast_may=0 with no outbound `new`,
  and `find_references new` has 1 caller, `apply_writes` absent → the
  `apply_writes → new` phantom is gone (exit criterion 4 on the real graph, not
  only the fixture). Worktree removed; tree clean.
- Completeness: `resolve_edges` (`derive.rs:260`) is the only production
  `EdgeRecord` construction site; `ariadne-scip` emits no edges. One fix covers
  cold index, warm daemon, and incremental — ADR claim verified by grep.
</checks_run>

<findings>
| id | category | severity | location | problem | fix |
| --- | --- | --- | --- | --- | --- |
| F1 | docs | INFO | docs/adr/0024-scoped-call-resolution.md (context + sources) | ADR cites `resolve_edges` at `derive.rs:177-227` / `156-227`, but the fn spans 220-278 at HEAD — stale line numbers (symbol is named, so still unambiguous). | Update the line ranges to 220-278. |
| F2 | tests | INFO | commit a2f6b45 (git history) | Exit criterion 1 says "committed red → green"; the regression test was committed together with the fix in one commit, not as a separate RED commit. Verified genuinely RED against pre-fix code, so the TDD guarantee holds. | None required; note for process traceability. |
| F3 | correctness | INFO | crates/ariadne-salsa/src/derive.rs:236-247 | Third scoping tier is "unambiguous-global", not the "import-visible" named in exit criterion 3 / plan step 2. Substitution is documented in ADR-0024 with proof import-visible reintroduces the phantom (`apply.rs` `use ariadne_core`; `ariadne_core` defines `new`). Both testable clauses of criterion 3 (in-scope resolution; std-callee no-edge) hold, and the recall guard (step 5) holds. | None required; the deviation is sound and recorded via the plan's own ADR mechanism. |
</findings>

<verdict>
PASS. Zero FAIL findings. All five exit criteria independently verified:
1. Synthetic multi-crate collision repro exists and is genuinely red→green
   (proven by reverting the resolver). [F2: single-commit packaging, non-block.]
2. Resolver located + recorded in ADR-0024 (`derive.rs::resolve_edges`,
   `db.rs::build_changeset`), status Accepted. [F1: stale line refs, non-block.]
3. Resolution scoped same-file → same-crate → unambiguous-global; std `Vec::new`
   yields no edge — both asserted by tests. [F3: "unambiguous-global" substitutes
   the literally-named "import-visible", justified + documented, non-block.]
4. Post-reindex `apply_writes` has no outbound cross-crate `new` edge — confirmed
   on the real dogfood graph AND the fixture.
5. architecture green; warm==cold parity green; navigation/reference/blast tests
   unchanged-green; same input → identical 3953-edge set (deterministic).
The fix is correct, complete (sole edge producer scoped), deterministic, adds no
dependency, and honors the no-denylist and hexagonal-boundary constraints.
</verdict>

<next_steps>
None blocking. Optional cleanup (any future tier-03 session may fold these in):
- F1: refresh the ADR-0024 line citations for `resolve_edges` (220-278).
- Tier-03 unblocked: re-enable the T1-suppressed sections on the now-reliable
  (3953-edge) set and assert the fixture-backed upper bound on cross-crate
  violations.
</next_steps>

<sources>
- Tier file: .claude/plans/docgen-overview-fidelity/tier-02-edge-resolution.md
- Plan: .claude/plans/docgen-overview-fidelity/plan.md (R1, D1, D2)
- Diff: commit a2f6b45 — crates/ariadne-salsa/src/{derive.rs,db.rs},
  crates/ariadne-salsa/tests/scoped_resolution.rs, docs/adr/0024-scoped-call-resolution.md
- Edge-producer grep: crates/ariadne-salsa/src/derive.rs:260 (sole site);
  crates/ariadne-scip (none).
- Dogfood: fresh HEAD worktree cold index → 3953 edges (×2), apply_writes
  blast_may=0, find_references `new` excludes apply_writes.
- [Google eng-practices — reviewer standard](https://google.github.io/eng-practices/review/reviewer/standard.html)
</sources>
