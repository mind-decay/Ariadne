---
block_id: block-2
title: Block 2 — parse deeper (exact parse facts in the symbol record)
arc: data-fidelity-arc
order: 2
deps: []
status: seed   # seed → expand via /spec-plan into tiers
expand_with: /spec-plan .claude/plans/data-fidelity-arc/block-2-parse-deeper.md
---

<context>
This is a **seed plan**, not a tier set. Shared constraints/tech live in the arc master:
`.claude/plans/data-fidelity-arc/plan.md`.

Problem: `SymbolRecord` carries `canonical_name`/`kind`/`visibility`/`attributes` only
[src: crates/ariadne-core/src/domain/records.rs:28-37], so every richer view re-derives
structure heuristically at read time. `context-efficient-read` lexically guesses doc-spans
(R1 — Python docstrings, inside the body, are missed), nesting via span containment (R2 —
misfires on overlapping/macro spans), and the signature boundary (R3 — multi-line
generics/where-clauses truncate) [src: context-efficient-read/plan.md `<risks>`]. The
discriminating facts already exist in the tree-sitter CST — parameters, return type, and
the signature node are addressable by field; nesting is the parent decl node — but they
are discarded after symbol extraction.

Success: `SymbolRecord` (or a sibling fact table) carries exact spans for doc, signature,
params and return, plus a parent link, captured at parse time; outline / `read_symbol` /
docgen read these instead of re-deriving — turning context-efficient-read R1/R2/R3 from
"document the gap" into "exact". Migration preserves all prior records byte-faithfully.
Scope (in): grammar capture of the new fields; threading core→storage→parser→scip→salsa;
a redb migration step; rewiring the heuristic readers onto the facts. Scope (out): a
full-text body index; type *inference* (Block 3/SCIP territory); any LLM summary.
</context>

<candidate_capabilities>
Each bullet is a likely tier the `/spec-plan` expansion will detail. General terms only.

**P1 — Capture exact structural facts per language.** Relabel/extend each `.scm` to
capture the signature node, `parameters`/`return_type` fields, and the nesting parent —
the same capture-by-field mechanism the resolver already uses [src:
crates/ariadne-parser/.../queries/*.scm; https://tree-sitter.github.io/tree-sitter/using-parsers/queries/1-syntax.html].
Doc-span = the grammar's preceding comment sibling where exposed, else the existing lexical
rule (honest boundary, no regression).

**P2 — Widen the record + redb migration.** Add doc-span / signature-span / params /
return / parent fields to `SymbolRecord` (or a parallel `SymbolDetail` table) behind one
`MigrationRegistry` vN→vN+1 step that re-encodes in place, no rebuild — the exact path
RD10 used for `visibility`/`attributes` [src: post-v1-roadmap RD2, RD10;
crates/ariadne-core/src/domain/records.rs:28-37].

**P3 — Thread facts core→storage→parser→scip→salsa.** Mirror the RD10 thread and the
`Update`-safe salsa fact pattern (byte/u8 tags, never a bare enum) [src:
crates/ariadne-salsa/src/derived.rs:53-58; post-v1-roadmap RD10].

**P4 — Rewire heuristic readers onto exact facts.** `context-efficient-read`'s outline
assembler, `read_symbol`, and docgen read the stored spans; R1/R2/R3 heuristics become
exact lookups (lexical doc-span kept only as the documented fallback) [src:
context-efficient-read/plan.md D4,D5,`<risks>`].
</candidate_capabilities>

<existing_assets>
- RD10 already threaded `visibility`+`attributes` core→…→salsa behind a redb migration —
  the exact precedent P2/P3 follow [src: post-v1-roadmap RD10; tier-04].
- The parser already captures decl + signature-end heuristically (the basis to make exact)
  [src: crates/ariadne-mcp/src/adapters/source.rs:59-118; context-efficient-read D4].
- `.scm` capture-by-field + negated-field mechanism is in use for call shapes [src:
  r1-resolver-completion D2; queries/*.scm].
- redb `MigrationRegistry` contiguity-checked step framework [src:
  crates/ariadne-storage/src/domain/migration.rs:67-87].
</existing_assets>

<open_questions>
Resolve in the `/spec-plan` expansion (do not guess now):
- Widen `SymbolRecord` in place vs a parallel `SymbolDetail` table — blast radius vs query
  cost; which keeps warm==cold parity cleanest [src: post-v1-roadmap RD11].
- Per-language signature-node + params/return field names (the grammar table); doc-comment
  sibling availability per grammar (Rust `///` vs Python docstring-in-body).
- Store spans (byte ranges) only, or also the sliced text — spans keep it byte-faithful
  and small [src: context-efficient-read D4].
- Migration: frozen `SymbolRecordVN` decoder + round-trip test asserting prior fields
  survive byte-identical [src: post-v1-roadmap R-A2].
- Memory delta of the wider record on 100K files (>256MB/table hard fail) [src: ariadne-core R1].
</open_questions>

<verification_intent>
Golden tests on the 15-language fixtures: doc-span, signature-span, params, return and
parent are captured exactly for seeded symbols (incl. the multi-line-signature and
nested-symbol cases context-efficient-read R2/R3 flagged); outline/`read_symbol`/docgen
render from the facts and match expected; the redb migration opens a prior-schema file and
yields records with prior fields byte-identical + new fields populated; warm==cold and
incremental==fresh parity hold. Each tier TDD: failing test first [src: CLAUDE.md `<rules>`].
</verification_intent>

<sources>
- Record + migration precedent: .claude/plans/post-v1-roadmap/plan.md RD2, RD10 ; crates/ariadne-storage/src/domain/migration.rs:67-87
- Heuristics this block makes exact: .claude/plans/context-efficient-read/plan.md D4,D5,`<risks>`
- Capture-by-field mechanism: .claude/plans/r1-resolver-completion/plan.md D2 ; https://tree-sitter.github.io/tree-sitter/using-parsers/queries/1-syntax.html
- Salsa `Update`-safe facts: crates/ariadne-salsa/src/derived.rs:53-58 ; https://docs.rs/salsa/0.26.2/salsa/trait.Update.html
- Arc master + inherited constraints: .claude/plans/data-fidelity-arc/plan.md
</sources>
