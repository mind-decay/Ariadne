---
block_id: block-4
title: Block 4 — reason deeper (intra-procedural data-flow)
arc: data-fidelity-arc
order: 4
deps: [block-2, block-3]
status: seed   # seed → expand via /spec-plan into tiers
expand_with: /spec-plan .claude/plans/data-fidelity-arc/block-4-reason-deeper.md
---

<context>
This is a **seed plan**, not a tier set. Shared constraints/tech live in the arc master:
`.claude/plans/data-fidelity-arc/plan.md`.

Problem: Ariadne answers structural questions (who-calls, impact, coupling) but not
*value-flow* ones — "where does this value reach", "does an input flow to a sink unchecked",
"which parameters does a return depend on". This is the data-flow/taint dimension
`intelligence-platform` block A explicitly named as the deferred stretch "unless asked"
[src: intelligence-platform/block-a-deepen-brain.md:46]. The user asked, so this block owns
it. The inputs already exist or land earlier in this arc: the tree-sitter CST (for an
intra-procedural CFG), the call graph, and Block 2's param/return facts + Block 3's
complete edges — hence `deps: [block-2, block-3]`.

Success: a deterministic intra-procedural data-flow use-case answers a seeded
source→sink/def→use query correctly on the fixtures, framed as IFDS graph reachability so
it is precise (finite fact set, distributive functions) and polynomial [src:
Reps/Horwitz/Sagiv, "Precise Interprocedural Dataflow Analysis via Graph Reachability",
POPL'95 — https://research.cs.wisc.edu/wpis/abstracts/popl95.abs.html]. Ships behind a
feature flag; intra-procedural first.
Scope (in): a per-function CFG from the CST; classic gen/kill facts (reaching defs / live
vars / def-use) solved as reachability; an MCP/CLI surface. Scope (out, this block): full
interprocedural/path-sensitive analysis (later tier); a security/taint *scanner* product
(future); any LLM. [src: AD6 in arc master.]
</context>

<candidate_capabilities>
Each bullet is a likely tier the `/spec-plan` expansion will detail. General terms only.

**F1 — Intra-procedural CFG from the CST.** Walk the per-function tree-sitter CST into a
basic-block control-flow graph — the same CST the cyclomatic-complexity counter already
traverses for branch nodes [src: post-v1-roadmap RD8;
crates/ariadne-graph/src/hotspot.rs]. Deterministic, no external dependency.

**F2 — Gen/kill data-flow facts solved as reachability.** Compute reaching-definitions /
def-use / live-variables over the CFG — the separable "bit-vector" problems IFDS subsumes —
as graph reachability, giving exact intra-procedural value flow [src: IFDS POPL'95;
https://en.wikipedia.org/wiki/Reaching_definition]. Reuses Block 2's param/return facts to
seed sources/sinks at the function boundary.

**F3 — Surface: `dataflow`/def-use query.** An `ariadne-graph` use-case behind a feature
flag, surfaced as MCP `data_flow`/CLI `ariadne dataflow <symbol>`, answering "which defs
reach this use" / "does this param flow to this return". Output respects Block 1's response
budget (this query can be large).

**F4 — (stretch, defer) interprocedural extension.** Extend F2 across call edges via the
full IFDS exploded-supergraph; this is the path to a future taint/security scanner — named
here, planned only if the expansion confirms intra-procedural lands cleanly first [src: IFDS POPL'95].
</candidate_capabilities>

<existing_assets>
- The CST is already walked per symbol for cyclomatic complexity (branch nodes) — F1
  extends the same traversal to a CFG [src: post-v1-roadmap RD8; crates/ariadne-graph/src/hotspot.rs].
- Call graph + (Block 3) complete edges — the supergraph spine for the F4 interprocedural stretch.
- Block 2 param/return facts — the function-boundary sources/sinks F2 seeds from.
- `ariadne-graph` use-case + MCP/CLI surface pattern — the F3 home [src: existing analytics tools].
</existing_assets>

<open_questions>
Resolve in the `/spec-plan` expansion (do not guess now):
- CFG coverage per language — which control constructs per grammar; where the CFG builder
  lives (`ariadne-parser` CST walk vs an `ariadne-graph` use-case over stored facts).
- Which data-flow problem ships first (reaching-defs/def-use is the smallest useful one).
- Fact representation + the salsa/warm-graph storage story (is flow recomputed per query,
  or memoized per function?) against the query p95 <100ms SLO.
- Feature-flag + memory budget (a CFG per function is new state) [src: ariadne-core R1].
- F4 interprocedural: confirm IFDS exploded-supergraph is tractable on 100K files before committing.
</open_questions>

<verification_intent>
Golden tests on seeded fixtures: a reaching-def/def-use query returns the hand-verified set
for a function with branches and reassignment; "does param X flow to return" is correct on
positive and negative cases; output is deterministic (identical twice) and bounded by Block
1's budget; the feature flag gates it cleanly (off = no new cost). Each tier TDD: failing
test first [src: CLAUDE.md `<rules>`].
</verification_intent>

<sources>
- IFDS (precise polynomial dataflow as reachability): https://research.cs.wisc.edu/wpis/abstracts/popl95.abs.html
- Reaching definitions / gen-kill: https://en.wikipedia.org/wiki/Reaching_definition
- Deferred-stretch origin: .claude/plans/intelligence-platform/block-a-deepen-brain.md:46
- CST traversal precedent (complexity): .claude/plans/post-v1-roadmap/plan.md RD8 ; crates/ariadne-graph/src/hotspot.rs
- Arc master + inherited constraints: .claude/plans/data-fidelity-arc/plan.md
</sources>
