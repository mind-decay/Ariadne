---
block_id: block-3
title: Block 3 — resolve completer (edge recall without phantoms)
arc: data-fidelity-arc
order: 3
deps: [block-2]
status: seed   # seed → expand via /spec-plan into tiers
expand_with: /spec-plan .claude/plans/data-fidelity-arc/block-3-resolve-completer.md
---

<context>
This is a **seed plan**, not a tier set. Shared constraints/tech live in the arc master:
`.claude/plans/data-fidelity-arc/plan.md`.

Problem: the default tree-sitter resolver buys precision by abstaining on recall. A
method/path callee whose qualifier is discarded gets no edge unless it is same-file/
same-crate Free — `socket.connect()` and a same-crate `Foo::new()` resolve to nothing
rather than risk a phantom [src: r1-resolver-completion/plan.md D1,D6;
crates/ariadne-salsa/src/derive.rs:240-247,276-283]. SCIP recovers the cross-crate /
receiver-typed recall this trades away [src: scip-driven-edges/plan.md D3] and now drives
impl/type edges out-of-band [src: commit 0af641e; scip-driven-edges tier-03,tier-04], but
the default-path graph still under-connects, so `blast_radius`/`find_references`/
`coupling_report` under-report on method-heavy code.

Success: method/path/cross-crate callees resolve via type/qualifier evidence (SCIP
default-on, or qualifier-aware tiers) so recall rises measurably on the 15-language
fixtures while the phantom-edge precision `r1-resolver-completion` won stays held (no
regression on its recall/precision controls).
Scope (in): make the precise (SCIP-backed / qualifier-aware) resolution the default path;
close the residual recall classes `r1` documented as SCIP territory. Scope (out): a new
edge *kind*; the `SymbolId` scheme; cross-repo resolution; any name denylist (abstention
stays structural) [src: r1-resolver-completion/plan.md `<constraints>`].
</context>

<candidate_capabilities>
Each bullet is a likely tier the `/spec-plan` expansion will detail. General terms only.

**R1 — Promote SCIP-driven edges to the default resolution source.** SCIP already yields
precise reference/access/relationship/impl/type edges and runs out-of-band [src:
scip-driven-edges/plan.md D4, tier-04; commit 0af641e]; make it the default with the
per-file tree-sitter path as the fallback where SCIP is absent, so the committed/dogfood
graph rides precise edges. Resolves the recall `r1` deferred to SCIP [src:
r1-resolver-completion/plan.md:48-50].

**R2 — Qualifier/receiver-type-aware tiers in the tree-sitter resolver (SCIP-absent
languages).** Use Block 2's captured params/return + the call qualifier (no longer
discarded) to bind a Method/Path callee to the receiver's type's method, instead of
abstaining — the resolution ADR-0024 deferred [src: docs/adr/0024-scoped-call-resolution.md;
r1-resolver-completion D6]. Synergy with Block 2 is why `deps: [block-2]`.

**R3 — Recall/precision measurement harness.** A deterministic edge-count + classification
harness over the fixtures reports recall (resolved method/path callees) and precision (zero
phantom cross-crate), so each change is proven, not asserted — extending the `r1` spike's
shape-classification approach [src: r1-resolver-completion/plan.md R1, `<verification>`].
</candidate_capabilities>

<existing_assets>
- `r1-resolver-completion` shape-gated tiers (`Free`/`Method`/`Path`) + the resolver tier
  ladder [src: crates/ariadne-salsa/src/derive.rs:220-283].
- `scip-driven-edges` occurrence/access/relationship/impl/type ingest, default-on
  out-of-band [src: .claude/plans/scip-driven-edges/plan.md; commits bc82fbd, ad59c2f, 0af641e].
- Block 2's captured qualifier/params/return facts (the inputs R2 needs).
- Warm==cold / incremental==fresh parity guards [src: post-v1-roadmap RD11].
</existing_assets>

<open_questions>
Resolve in the `/spec-plan` expansion (do not guess now):
- Default-on SCIP: cost vs the cold <60s SLO; behaviour when an indexer is absent for a
  language; how the fallback boundary is drawn deterministically [src: scip-driven-edges R1].
- R2: which languages lack a SCIP indexer and thus need qualifier-aware tree-sitter tiers;
  how far same-crate type resolution can go without full type inference.
- Does promoting SCIP change the committed docgen/overview snapshots (edge-set churn)? Plan
  the re-baseline like `r1` tier-01 did [src: r1-resolver-completion R4].
- Recall target: "measurably higher" needs a fixture baseline number set in the expansion.
</open_questions>

<verification_intent>
Deterministic harness on the 15-language fixtures: method/path/cross-crate callees that
abstained now resolve (recall up against a recorded baseline); `r1`'s phantom-edge and
`beta::run → alpha::helper` controls stay green (precision held, no over-binding); warm==
cold and incremental==fresh parity hold; a fresh re-index twice is byte-identical (edge
determinism). Each tier TDD: failing test first [src: CLAUDE.md `<rules>`].
</verification_intent>

<sources>
- Recall boundary + shape gate: .claude/plans/r1-resolver-completion/plan.md ; crates/ariadne-salsa/src/derive.rs:220-283
- SCIP edges: .claude/plans/scip-driven-edges/plan.md ; commits bc82fbd, ad59c2f, 0af641e
- Deferred qualifier resolution: docs/adr/0024-scoped-call-resolution.md
- Parity guards: .claude/plans/post-v1-roadmap/plan.md RD11
- Arc master + inherited constraints: .claude/plans/data-fidelity-arc/plan.md
</sources>
