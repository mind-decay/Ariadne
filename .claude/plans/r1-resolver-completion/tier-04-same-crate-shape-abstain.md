---
tier_id: tier-04
title: Abstain same-crate Method/Path callees with no same-file definition; commit tier-01
deps: [tier-01]
exit_criteria:
  - "tier-01's completed-but-uncommitted resolver/parser work (parser .scm + facts CallKind, salsa derive/derived gate, cli/daemon kind_byte threading, its tests, ADR-0024 edit) is committed FIRST as one fix commit, path-scoped so the held docgen graph changes stay uncommitted"
  - "On a fresh daemon-stopped dogfood reindex of the COMMITTED binary, the residual boundary-violation rows are enumerated and classified by (caller crate, callee crate, call shape, classify reason) BEFORE any resolver edit — the fix premise is measured, not assumed"
  - "A scoped_resolution.rs spike seeds a same-crate Method/Path callee whose bare name is defined same-crate in a DIFFERENT file (not same-file) and asserts NO edge; red on the committed resolver, green after the fix"
  - "resolve_edges resolves a Method/Path callee only via same-file; same-crate and unambiguous-global tiers are refused for non-Free shapes. Free callees keep same-file→same-crate→unambiguous-global unchanged"
  - "Recall guards stay green: the beta→alpha unambiguous-global (Free) edge, a same-file Method call edge, and a same-crate Free call edge all still resolve; warm==cold / incremental==fresh parity suites green"
  - "Post-fix fresh reindex: boundary-violation rows near-zero (the X::new() domain→adapter phantoms gone); index twice → identical edge set; ADR-0025 written superseding ADR-0024's same-crate clause for non-Free shapes; clippy/fmt/architecture/deny green; salsa memory_report() delta < 256MB/table"
status: completed
completed: 2026-06-05
---

<context>
After tier-01, the cross-crate unambiguous-global fallback is gated to Free shapes,
but the SAME-CRATE tier still binds a Method/Path callee by bare name. The parser
discards the qualifier — `ProgressBar::new()` is captured as bare `new` [src:
ADR-0024 context; queries/rust.scm `@call.path`]. With the receiver/qualifier gone,
a same-crate bare-name match is a guess: a cli-domain caller's `X::new()` binds to
cli's lone adapter-layer `new`, and `classify_violation` flags the intra-crate
cross-layer edge domain→adapter — flooding the boundary section so tier-02's
near-zero gate is blocked [src: r1-resolver-completion/tier-02 `<blockers>` secondary;
crates/ariadne-graph/src/docgen_insights.rs:259-268 `classify_violation`]. `new` is
defined in 9 workspace crates (globally ambiguous), so this is purely the same-crate
tier; tier-01's cross-crate gate cannot reach it [src: derive.rs:277,279-283].

Rule: a Method/Path callee resolves only when its definition is in the caller's own
FILE (the one scope where the bare name is lexically unambiguous); else abstain. Free
callees are unchanged. Precise cross-crate `Foo::new` stays SCIP's job (ADR-0024
deferred). Full rationale + alternatives: plan.md. Determinism / candidate-sort
invariants unchanged [src: derive.rs:276-283].
</context>

<files>
- (commit only) crates/ariadne-parser/**, crates/ariadne-salsa/src/{derive,derived,db}.rs,
  crates/ariadne-salsa/tests/**, crates/ariadne-cli/src/domain/mod.rs,
  crates/ariadne-daemon/src/domain/facts.rs, docs/adr/0024-scoped-call-resolution.md,
  crates/ariadne-parser/tests/call_shape.rs — tier-01's uncommitted resolver/parser
  work [src: tier-01 `<files>`]. Path-scoped commit; graph/docgen held files excluded.
- crates/ariadne-salsa/src/derive.rs — in `resolve_edges`, restrict the same-crate
  (and the already-Free-gated unambiguous-global) tiers to Free shapes: a Method/Path
  callee uses `same_file` only [src: derive.rs:276-283,308-316].
- crates/ariadne-salsa/tests/scoped_resolution.rs — add the same-crate-different-file
  Method/Path no-edge spike + a same-file Method recall test; keep existing recall.
- docs/adr/0025-shape-scoped-same-crate-resolution.md — new ADR (template), supersedes
  ADR-0024's same-crate clause for non-Free shapes; set ADR-0024 status
  "Superseded by ADR-0025".
</files>

<steps>
1. COMMIT BASE. Verify the working tree carries tier-01's completed resolver/parser
   work (facts.rs `CallKind`, derive.rs Free-gate). `git add` ONLY the tier-01 paths
   above (NOT docgen.rs/docgen_insights.rs/graph tests — those are the held tier-03
   work tier-02 lands), then commit `fix(salsa): gate cross-crate fallback to free
   call shapes`. Run the workspace build + tier-01 `<verification>` to confirm the
   committed base is green.
2. MEASURE (premise gate). Stop the daemon; `cargo run -p ariadne-cli -- index <repo>`
   twice (confirm identical edge count). Render the overview; for EVERY boundary row,
   print (caller crate, callee crate, caller file layer, callee file layer, call shape
   of the edge's source span, classify reason). Record the table in the audit notes.
   Confirm the dominant residual class is same-crate Method/Path → unrelated same-crate
   def (the `X::new()` domain→adapter shape). If the dominant class is something else,
   STOP and revise this tier — do not apply a fix to a wrong premise.
3. RED. In `scoped_resolution.rs`, seed crate `crate_a` with caller `a::run` in
   `crates/crate_a/src/run.rs` (Path call `T::make()`) and a definition `make` in a
   DIFFERENT same-crate file `crates/crate_a/src/other.rs`; assert `a::run` has no
   `References` edge to `make`. Add the Method twin. Run → RED (current same-crate tier
   binds it). Add a positive control: a same-FILE Method call that MUST still resolve.
4. FIX. In `resolve_edges`, thread the call shape into the per-callee resolution so a
   non-Free callee resolves via `same_file` only — drop `same_crate` and the
   unambiguous-global tier for Method/Path. Implement by passing a `free: bool` (or
   reusing the existing `cross_crate_ok` plumbing extended to also gate `same_crate`)
   so `in_scope = if free { same_file.or_else(same_crate) } else { same_file }`
   [src: derive.rs:276-283]. Renders/hooks keep their current `true` path.
5. GREEN. Step-3 spikes green; same-file Method control green; the beta→alpha
   unambiguous-global (Free) and same-crate Free recall tests stay green.
6. ADR. Write `docs/adr/0025-shape-scoped-same-crate-resolution.md` (decision,
   rationale, the rejected alternatives: uniqueness-gate same-crate — insufficient
   when the collision name has one same-crate def; qualifier-aware resolution —
   SCIP-deferred), and flip ADR-0024 status to "Superseded by ADR-0025" for the
   same-crate-non-Free clause only.
7. VERIFY + REINDEX. `cargo nextest run -p ariadne-parser -p ariadne-salsa
   -p ariadne-daemon -p ariadne-mcp`; architecture; clippy; fmt; `cargo deny check`;
   salsa `memory_report()` delta. Fresh reindex twice → identical edge set; re-render
   overview → boundary rows near-zero (no `→ *::new` domain→adapter phantoms). Commit
   `fix(salsa): abstain same-crate method/path callees without a same-file definition`
   + the ADRs (still NOT the docgen graph changes — tier-02 lands those).
</steps>

<verification>
- `cargo nextest run -p ariadne-salsa` → same-crate-different-file Method/Path no-edge
  spikes green; same-file Method control green; beta→alpha + same-crate Free recall
  green; ambiguous-no-edge (tier-01) green.
- `cargo nextest run -p ariadne-daemon -p ariadne-mcp` → warm==cold / incremental==fresh
  parity green (no legitimate-edge regression).
- Fresh `cargo run -p ariadne-cli -- index <repo>` twice → identical edge set; rendered
  overview Boundary violations near-zero and every row crate-qualified; no row whose
  callee is a bare `new`/`build` bound to an unrelated same-crate definition.
- `cargo test --test architecture`; clippy `-D warnings`; `cargo fmt --all --check`;
  `cargo deny check`. Salsa `memory_report()` delta < 256MB/table.
- Fail loudly: a dropped beta→alpha or same-file Method edge = over-gating (hard fail,
  narrow the rule); a surviving `X::new()` domain→adapter phantom = under-gating
  (hard fail). Do NOT weaken a recall test to pass.
</verification>

<rollback>
The two commits are isolated. `git revert` the abstention commit to restore tier-01's
edge set (the base commit from step 1 stays — it is tier-01's already-completed work).
`git checkout -- docs/adr/0025-*.md docs/adr/0024-scoped-call-resolution.md` to drop
the ADRs. The held docgen graph changes are never touched here.
</rollback>
</content>
