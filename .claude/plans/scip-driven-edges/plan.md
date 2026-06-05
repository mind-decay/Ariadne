---
slug: scip-driven-edges
title: Activate the stubbed SCIP layer to drive precise, default-on graph edges
created: 2026-06-04
updated: 2026-06-05
owners: [user, claude]
review: [user, codex?]
single_tier: false
tiers:
  - tier-01-occurrence-ingest-reference-edges
  - tier-02-access-role-read-write-edges
  - tier-03-relationship-impl-type-edges
  - tier-04-default-on-out-of-band-ingest
---

<context>
Problem: the `ariadne-scip` adapter runs 13 real indexers, decodes their SCIP,
then DISCARDS it — every graph edge is a tree-sitter heuristic. PROOF: `--scip`
runs `IngestPlan` inline and throws the report away [src: crates/ariadne-cli/src/domain/mod.rs:155,253-258];
`scip_symbols` returns empty [src: crates/ariadne-salsa/src/derived.rs:171-178];
`ScipDocInput.raw_proto` is always built `None` [src: inputs.rs:50-58]. So edges
come from the scoped bare-name resolver [src: crates/ariadne-salsa/src/derive.rs:236-316].

Why now (state as of 2026-06-05). `r1-resolver-completion` (tier-04 PASS) made
that resolver PRECISE by gating the cross-crate tier to free-identifier calls and
abstaining on Method/Path callees with no same-file def [src: ADR-0025;
.claude/plans/r1-resolver-completion/plan.md D1,D6]. That deliberately TRADED AWAY
recall — a genuine `socket.connect()` / `Foo::new()` cross-crate call now yields NO
edge — and the two plans are documented as complementary, with SCIP as the
recall-recovery complement [src: r1-resolver-completion/plan.md:42,48-50]. The gap
is now committed and measurable, not hypothetical. SCIP carries exactly the missing
signal: occurrences with def/ref/read/write roles and impl/type relationships
[src: crates/ariadne-scip/proto/scip.proto:462-542,645-680].

Best-practice anchor: this is the canonical dual model. Sourcegraph runs precise
(SCIP, opt-in, batch, auto-indexed) AND search-based (syntactic), "automatically
uses Precise whenever available, search-based as fallback" — verbatim D4/D6 below
[src: https://sourcegraph.com/docs/code-search/code-navigation/precise_code_navigation;
https://sourcegraph.com/blog/announcing-scip]. The hybrid is specifically
documented to fix "method receivers, pointer indirection, package-qualified
identifiers" — the exact gap r1 left [src: same].

In scope: SCIP fact extraction (occurrences + relationships) at the composition
root; a pure salsa-input fact type; range-mapped edge resolution; the precise
`References`/`Imports` recall recovery (T1); `Reads`/`Writes` (T2);
`Implements`/`TypeOf` + EdgeKindFilter honesty-by-production (T3); SCIP default-on,
run OUT-OF-BAND, with the precise tree-sitter resolver as live fallback (T4).
Out of scope: changing the `SymbolId` scheme or letting SCIP create/override
symbols (Strategy A — rejected D1); LLM/embeddings [src: post-v1-roadmap plan.md];
docgen re-enablement — already landed on tree-sitter edges 2026-06-05
[src: .claude/plans/docgen-overview-fidelity/tier-03-reenable-on-reliable-edges.md
status: completed].
</context>

<constraints>
- `ariadne-salsa` may depend only on `ariadne-core` + `ariadne-storage`; SCIP is a
  driven adapter no use-case crate may import [src: tests/architecture.rs:13-14,31-43].
  SCIP proto is decoded to a pure type at the composition root, never in salsa (D2).
- Pure-Rust critical path; no new dependency — prost + the SCIP types are vendored
  [src: crates/ariadne-scip/src/proto.rs; indexer/mod.rs:85].
- Hexagonal + TDD: a failing test precedes implementation each tier [src: CLAUDE.md].
- Determinism: same input → identical edge set; sort occurrences by (file, range);
  BTree/sorted containers only [src: derive.rs; ADR-0024 rationale].
- `SymbolId` stability (RD12/ADR-0017) must not regress — SCIP feeds edges only,
  symbols untouched (D1) [src: derived.rs:187-211].
- Parity holds: cold==warm and incremental==fresh over the one shared derivation
  [src: post-v1-roadmap plan.md RD11; db.rs:329-336].
- SLOs hold: cold <60s, incremental p95 <500ms, query p95 <100ms; SCIP NEVER runs
  on the synchronous index/incremental path (D6); per-tier `memory_report()` delta,
  >256MB/table is a hard fail (R7) [src: ariadne-core plan.md `<risks>`].
- No symbol-name denylist; abstention is structural [src: ADR-0024; ADR-0025].
</constraints>

<decisions>
**D1 — SCIP drives edges; tree-sitter keeps symbols + identity (Strategy B, user).**
The `SymbolId` scheme, goldens, and warm-graph node identity stay unchanged; SCIP
occurrences map to existing tree-sitter symbols by source range. *Rejected:*
SCIP-authoritative symbols (Strategy A) — re-baselines every golden and revisits
RD12 stability [src: derived.rs:187-211; ADR-0017].

**D2 — SCIP facts cross into salsa as a pure core type at the composition root.**
A new `ariadne-scip::extract_facts(&IngestReport) -> Vec<(path, ScipFacts)>`
runs at the cli/daemon root; `ScipFacts` (pure core type: per occurrence
`{symbol, byte_range, roles:u32}`, per relationship `{from, to, flags}`, plus the
indexed content hash) is fed via a salsa input replacing
`ScipDocInput.raw_proto: Option<Vec<u8>>` — mirroring `SyntacticFactsInput` (RD11;
the salsa input field is the `ScipFactsRaw` mirror, per the
`SyntacticFacts`/`SyntacticFactsRaw` precedent)
[src: inputs.rs:50-58,60-71]. *Rejected:* decode proto in salsa (arch violation).

**D3 — Occurrence→symbol resolution is by source range, not name.** A `Definition`
occurrence (role `0x1`) maps to the innermost enclosing tree-sitter symbol; its
normalized SCIP symbol string keys a global `scip_symbol → SymbolId` map. A
non-def occurrence resolves `src`=enclosing ts symbol, `dst`=key→`SymbolId`; an
unmapped `dst` (std/external) drops the edge — recovering the cross-crate calls
ADR-0025 abstained on, with zero name collision (SCIP strings are globally unique)
[src: scip.proto:645-680; crates/ariadne-scip/src/normalize/mod.rs:160-162].

**D4 — SCIP edges supersede tree-sitter per covered+current file; the PRECISE
resolver is the live fallback.** A file is "covered" only while its content hash
matches the hash its SCIP facts were indexed at; covered ⇒ edges from SCIP, the
tree-sitter `resolve_edges` pass is skipped for it; else (no indexer, hash moved
off the index, or SCIP not yet run) ⇒ the shape-gated `resolve_edges` (now precise,
not phantom-prone) [src: derive.rs:236-316; ADR-0025]. So a live edit never shows a
stale resolved edge. Hash already tracked [src: inputs.rs; db.rs]. *Rejected:* union
both (double-counts, conflicting `dst`); keep stale SCIP edges past an edit.

**D5 — Edge kinds track SCIP's signal; the filter is honest BY PRODUCTION (user:
all-tiers).** TWO `EdgeKind` enums exist: the derivation/storage one
(`References,Imports,Defines,Renders,UsesHook`) [src: crates/ariadne-core/src/domain/records.rs:161-172]
and the in-RAM graph one (`Calls,Imports,TypeOf,Defines,Overrides,Reads,Writes,Inherits`)
[src: crates/ariadne-graph/src/build.rs:30-47]. `from_core` collapses every kind
but Defines/Imports to `Calls` [src: build.rs:66-79], so the graph's
`TypeOf/Overrides/Reads/Writes/Inherits` — advertised by `EdgeKindFilter`
[src: core daemon/query.rs:14-31] and `EdgeKindSet` [src: build.rs:82-95] — are
NEVER produced (5 of 8 filters return empty). Widen the derivation enum + `from_core`
so SCIP populates `Reads`/`Writes`/`Implements`(→Overrides)/`TypeOf`; assert the
filter maps 1:1 to producible kinds (T3). Emit a kind only on its present SCIP
bit/flag (no fabrication). *Rejected:* prune unproducible filters (loses real SCIP
signal); fabricate from tree-sitter (untrustworthy — the failure this fixes).

**D6 — SCIP is default-on but runs OUT-OF-BAND; fast index unblocked (user: most
effective/quality).** Today `--scip` runs `IngestPlan` INLINE, costing cold-index
time [src: domain/mod.rs:253-258]. Invert to default-on (`--no-scip` opt-out) using
the existing `IngestPlan::detect`/degraded-never-fail orchestration
[src: crates/ariadne-scip/src/indexer/plan.rs:1-39]: the fast tree-sitter index
commits first; SCIP runs AFTER as a separate pass (CLI follow-up; daemon idle/
background pass — daemon has zero SCIP wiring today) and re-commits covered edges.
A missing indexer/stale hash degrades to the precise resolver (D4). This reaches the
committed/dogfood/MCP graph the LLM consumes without touching the cold<60s /
incr-p95<500ms paths — Sourcegraph's auto-index endpoint [src: precise_code_navigation
doc]. New ADR-0026. *Rejected:* keep opt-in (value stranded behind a flag the
daemon never sets); run inline default-on (blows cold SLO with 13 subprocesses).
</decisions>

<architecture>
Composition root (CLI cold, daemon warm) — both may import `ariadne-scip` +
`ariadne-salsa`: after the fast index, decode the `IngestReport`, call
`extract_facts` → `ScipFacts`, set `ScipFactsInput` per covered file (D2,D6).
Derivation (`ariadne-salsa`, the one shared path): a memoized `scip_facts_for_file`
query; a driver pass (sibling to `resolve_edges`) builds the global
`scip_symbol → SymbolId` map from Definition occurrences (D3), resolves the rest to
typed `EdgeRecord`s, and for covered files replaces the tree-sitter pass (D4).
Output merges into the same `Changeset` the committer writes [src: db.rs:329-336].
Core: `ScipFacts` pure type (salsa input mirror `ScipFactsRaw`); derivation `EdgeKind` gains `Reads,Writes,Implements,
TypeOf` with stable byte tags; `from_core` + `EdgeKindFilter` reconciled (D5).
Storage encodes the new tags (round-trip; old DBs hold tags 0–4). No new component.
</architecture>

<tech_inventory>
| tech | version | role | tier | source verified this session |
|---|---|---|---|---|
| SCIP schema | proto/SCIP_COMMIT pin | SymbolRole 0x1/0x2/0x4/0x8 + Relationship flags = the edge signal | 01,02,03 | crates/ariadne-scip/proto/scip.proto:488-542,645-680 (this session) |
| prost (vendored SCIP types) | workspace pin | decode `proto::Index` at the root; no new dep | 01 | crates/ariadne-scip/src/indexer/mod.rs:85 |
| salsa | 0.26.2 (v1 pin) | new pure `ScipFactsInput`; memoized query | 01 | inputs.rs:60-71 (SyntacticFactsInput precedent) |
| redb | 4.1.0 (v1 pin) | edge tag round-trip for widened EdgeKind; no data migration | 02,03 | records.rs:161-172 |
| Sourcegraph precise nav | docs 2026-06 | dual precise+syntactic model; opt-in/batch/auto-index/fallback | 04 | https://sourcegraph.com/docs/code-search/code-navigation/precise_code_navigation |
</tech_inventory>

<risks>
| id | risk | likelihood | mitigation |
|---|---|---|---|
| R1 | SCIP indexers opt-in/absent ⇒ edges stay tree-sitter | medium | per-file fallback (D4) + default-on auto-detect (D6); degraded mode is a warning, never a failure [src: plan.rs:7-13] |
| R2 | SCIP vs tree-sitter disagree on def spans (SFC/macro) ⇒ range map misses | medium | innermost-enclosing match; no enclosing `src` or no mapped `dst` drops — never mis-attributes |
| R3 | Reads/Writes/Implements population is indexer-dependent | medium | emit only on a present bit/flag; absence = missing edge, never a wrong edge |
| R4 | edge-count delta breaks insta goldens / parity / blast-radius | high | re-baseline per tier with the recall delta documented; cold==warm + incremental==fresh stay green |
| R5 | non-determinism from occurrence/map iteration order | medium | sort occurrences by (file, range); BTree maps; determinism test stays green |
| R6 | `SymbolId` stability (RD12) regresses | low | SCIP feeds edges only (D1); stable-id proptest stays green |
| R7 | new input + edges blow per-table memory budget | low | per-tier `memory_report()` delta; >256MB/table hard fail |
| R8 | SCIP is batch (sec–min); edits outrun it ⇒ stale edges or SLO miss | high | D6: SCIP never on the synchronous/incremental path; D4 hash-gates coverage so an edited file drops to the precise resolver until SCIP re-runs |
| R9 | default-on regresses cold/incremental SLO or daemon liveness | high | D6 out-of-band: T4 measures cold/incr p95 before+after; SCIP is a post-commit pass, fast index unblocked |
</risks>

<verification>
- Recall: re-index recovers genuine cross-crate Method/Path calls ADR-0025 dropped
  (recall up, all true); `apply_writes` still has no phantom `new` edge.
- Parity: cold==warm and incremental==fresh edge sets identical; index twice →
  identical edges; `cargo test --test architecture` green (salsa ⊥ ariadne-scip).
- Per kind: fixtures assert a precise `References`/`Imports` (T1), `Reads`/`Writes`
  from access roles (T2), `Implements`/`TypeOf` from a trait-impl/typed-binding (T3);
  `blast_radius` filters each kind; a total-mapping test asserts every
  `EdgeKindFilter` variant → a producible `EdgeKind` (T3).
- Default-on: cold <60s and incr p95 <500ms unchanged with SCIP default-on (T4);
  the committed/dogfood overview rides SCIP edges where an indexer is present, the
  precise resolver elsewhere; degraded mode (no binary) is a warning.
- Whole: `cargo nextest run --workspace`, `cargo clippy … -D warnings`,
  `cargo fmt --all --check`, `cargo deny check` (no new dep) green; per-tier memory
  probe within budget; every tier audit PASS.
</verification>

<sources>
- SCIP schema (Occurrence/SymbolRole/Relationship): crates/ariadne-scip/proto/scip.proto:462-542,645-680 ; https://github.com/sourcegraph/scip/blob/main/scip.proto
- Precise+syntactic dual model: https://sourcegraph.com/docs/code-search/code-navigation/precise_code_navigation ; https://sourcegraph.com/blog/announcing-scip
- SCIP discard (the gap): cli domain/mod.rs:155,253-258 ; derived.rs:171-178 ; inputs.rs:50-58
- Indexer orchestration (T4): crates/ariadne-scip/src/indexer/plan.rs:1-39 ; indexer/mod.rs:85
- Precise shape-gated resolver (the fallback): crates/ariadne-salsa/src/derive.rs:236-316 ; db.rs:329-336 ; docs/adr/0024,0025
- Two EdgeKind enums + lossy mapping + filters: records.rs:161-172 ; graph build.rs:30-47,66-79,82-95 ; core daemon/query.rs:14-31
- Complement to r1: .claude/plans/r1-resolver-completion/plan.md:42,48-50 ; docgen tier-03 completed
- Fact-as-input + parity: post-v1-roadmap plan.md RD11/RD12 ; ADR-0016/0017
</sources>
