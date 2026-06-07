---
slug: data-fidelity-arc
title: Data-fidelity arc — serve leaner (token economy) + parse deeper + resolve completer + reason deeper (dataflow)
created: 2026-06-07
owners: [user, claude]
review: [user, codex?]
single_tier: false
blocks: [block-1-serve-leaner, block-2-parse-deeper, block-3-resolve-completer, block-4-reason-deeper]
next_step: run /spec-plan on each block seed file (1 first or ∥ 2, then 3, then 4) to expand it into detailed, audited tiers
---

<context>
`context-efficient-read` (in build) made ONE path — whole-file read — token-lean by
returning a folded skeleton instead of every byte [src:
.claude/plans/context-efficient-read/plan.md `<context>`]. The user asks what else to
bring to a qualitatively new level so the system is **more efficient at working with
data** and **parses that data with higher quality**. This arc answers both with two
levers across four seed blocks.

- **Serve leaner (efficiency).** Outline's win is isolated. Of the ~22 MCP tools, only
  `list_symbols` + `search_code` cap output [src:
  crates/ariadne-mcp/src/tools/list_symbols.rs:13; search_code.rs:73,137]; the rest —
  `find_references` (returns an unbounded `Vec<ReferenceSite>` [src:
  crates/ariadne-mcp/src/tools/find_references.rs:24]), `blast_radius`,
  `coupling_report`, `weak_spots`, `co_change`, `hotspots` — emit full JSON with no
  cursor, no concise mode, no 25k-token budget. A hot symbol (jquery `Ca 681`) dumps
  681 full-span rows. Block 1 generalizes the token-economy pattern to the whole surface.
- **Parse deeper / resolve completer / reason deeper (fidelity).** `SymbolRecord`
  carries name/kind/visibility/attributes only [src:
  crates/ariadne-core/src/domain/records.rs:28-37], so `context-efficient-read` derives
  doc-spans, nesting and signature boundaries *heuristically at read time* — its own R1
  (Python docstrings missed), R2 (nesting misfires), R3 (multi-line sig truncation)
  [src: context-efficient-read/plan.md `<risks>`]. Block 2 captures those as exact parse
  facts. Block 3 closes the edge **recall** the default tree-sitter resolver trades for
  precision (it abstains on method/path cross-crate callees) [src:
  r1-resolver-completion/plan.md D1,D6; crates/ariadne-salsa/src/derive.rs:240-247,276-283].
  Block 4 adds a net-new analysis dimension — intra-procedural data-flow — the stretch
  `intelligence-platform` block A explicitly deferred [src:
  intelligence-platform/block-a-deepen-brain.md:46].

This file is the **arc master**: shared context, constraints, cross-cutting decisions,
the shared tech inventory, risks, and pointers to the four block seed files. Neither this
file nor the block files commit tiers — `/spec-plan` per block does that (the
`intelligence-platform` arc precedent) [src: intelligence-platform/plan.md AD1].
In scope: the four block seed files below. Out of scope (this arc): in-product
LLM/embeddings [src: feedback_no_llm_features]; relevance ranking that is
non-deterministic; the products/platform surfaces owned by `intelligence-platform`
blocks B/C; cross-repo federation.
</context>

<constraints>
- Inherits every v1 + post-v1 invariant: pure-Rust critical path, no cgo/Node/JVM in the
  `ariadne` binary; single static binary; hexagonal (`ariadne-core` declares ports,
  adapters implement, adapters never depend on each other); TDD failing-test-first [src:
  .claude/plans/ariadne-core/plan.md D5,D13; CLAUDE.md `<rules>`; tests/architecture.rs].
- Deterministic — no inference; identical input → identical output; sorted/`BTree`
  containers; any truncation/ranking uses a stable key and is reported, never silent
  [src: feedback_no_llm_features; r1-resolver-completion/plan.md `<constraints>`].
- All v1 SLOs hold: cold full-index <60s, incremental p95 <500ms, query p95 <100ms, warm
  query p95 <10ms, <4GB RAM on 100K files [src: post-v1-roadmap/plan.md `<constraints>`].
- Schema changes ship behind one redb `MigrationRegistry` step (in-place re-encode, no
  rebuild), the path RD10 used to add `visibility`/`attributes` [src: post-v1-roadmap
  RD2,RD10].
- MCP output limits respected: tool result ≤25k tokens by default; outputs that can grow
  paginate/cap with sensible defaults [src: https://www.anthropic.com/engineering/writing-tools-for-agents;
  https://code.claude.com/docs/en/mcp].
- Each authored tier (per-block expansion) ships an ADR on an architectural decision;
  audit-gated per `.claude/hooks/audit-gate.sh` [src: CLAUDE.md `<workflow>`].
</constraints>

<decisions>
**AD1 — Two levers, four blocks; recommended sequence 1 → {2 ∥ 1} → 3 → 4, Block 1
independent.** Block 1 (serve-leaner) has no hard dependency and saves tokens across ~20
tools immediately, so it front-loads value and may run in parallel with Block 2. Block 2
(parse-deeper) is the fidelity foundation: exact facts make Block 1's concise outputs and
`context-efficient-read`'s outline exact for free, and Block 4 reasons over them. Block 3
(edges) exploits Block 2's type facts; Block 4 (dataflow) reasons over complete edges +
rich facts — hence its `deps`. Listed `deps` are recommended-prerequisites for maximal
reuse, not hard blockers; each block's own `/spec-plan` fixes tier-level deps [src:
spec-plan SKILL `<step id=4>`; intelligence-platform/plan.md AD1].

**AD2 — Each block is a SEED that commits NO tiers; the per-block `/spec-plan` produces
audited tiers.** Mirrors the `intelligence-platform` arc so the master stays general and
each block expands in a fresh session [src: intelligence-platform/plan.md AD1; spec-plan
`<anti_patterns>` — no tiers depending on un-built state].

**AD3 — No overlap with `intelligence-platform`: that arc adds *analyses* (test-impact,
api-diff, fitness, products, platform); this arc upgrades the *data plane* — how data is
delivered (Block 1), parsed (Block 2), resolved (Block 3) and reasoned over (Block 4).**
The data-flow capability is the stretch `intelligence-platform` block A deferred "unless
asked" — the user just asked, so Block 4 owns it [src:
intelligence-platform/block-a-deepen-brain.md:15,46].

**AD4 — Token-economy mechanism = a self-implemented opaque cursor + token budget INSIDE
each `tools/call` payload, plus a `verbosity` enum and ranked truncation with steering
text.** MCP spec pagination covers only *list* operations (`resources/list`,
`tools/list`), NOT `tools/call` results, so growable tool outputs must carry their own
`nextCursor`/`cursor` mirroring the spec's opaque-cursor model; a concise mode lands ≈⅓
of the tokens of a detailed one; truncation steers the agent toward narrower follow-ups
[src: https://modelcontextprotocol.io/specification/2025-06-18/server/utilities/pagination;
https://www.anthropic.com/engineering/writing-tools-for-agents].

**AD5 — Parse-depth + edge facts thread core→storage→parser→scip→salsa behind a redb
migration; delivery economy lives in `ariadne-graph` use-cases + the MCP/CLI adapters,
never driving→driving.** The RD10 threading precedent and the `context-efficient-read` D3
home (assembler in `ariadne-graph`, both adapters depend on it) keep the hexagonal
invariant [src: post-v1-roadmap RD10; context-efficient-read/plan.md D3; CLAUDE.md D13].

**AD6 — Block 4 data-flow is IFDS-shaped (finite fact set, distributive functions, solved
as graph reachability), intra-procedural first, behind a feature flag.** The Reps-
Horwitz-Sagiv framework gives a precise, polynomial, deterministic basis that reuses the
CST + call graph Ariadne already holds; full interprocedural/taint is a later tier [src:
Reps/Horwitz/Sagiv, "Precise Interprocedural Dataflow Analysis via Graph Reachability",
POPL'95 — https://research.cs.wisc.edu/wpis/abstracts/popl95.abs.html].
</decisions>

<architecture>
The arc layers onto the current hexagonal system; no interior rewrite.
- **Block 1** = a delivery-layer pattern in `ariadne-graph`/`ariadne-mcp`: a shared
  response-budget + opaque-cursor + `verbosity` helper applied to the growable tools,
  reusing the `limit`/`truncate` shape `list_symbols`/`search_code` already prove [src:
  crates/ariadne-mcp/src/tools/search_code.rs:90-137]. No parse change, no new crate.
- **Block 2** = widen `SymbolRecord` with exact parse facts (doc-span, signature-span,
  parent/nesting, params/return) threaded core→storage→parser→scip→salsa behind a redb
  vN→vN+1 step; `context-efficient-read`'s outline + `read_symbol` + `docgen` read facts
  instead of re-deriving heuristics. No new crate.
- **Block 3** = the resolver (`ariadne-salsa` `resolve_edges`) gains qualifier/type-aware
  recall (SCIP default-on + shape-aware tiers) so method/path/cross-crate callees resolve;
  builds on `r1-resolver-completion` + `scip-driven-edges`. No new crate.
- **Block 4** = a new `ariadne-graph` data-flow use-case over the CST + call graph (IFDS
  reachability), surfaced as MCP/CLI, behind a flag. Possibly one new analysis module; no
  new external dependency on the critical path.
Dataflow is unchanged: watcher → daemon invalidates salsa → warm petgraph → clients query
the warm catalog. Blocks read/extend that graph; Block 1 reshapes only the delivered view.
</architecture>

<tech_inventory>
| tech | version pinned | block | role | source verified this session |
|---|---|---|---|---|
| rmcp `#[tool]`/`CallToolResult` | =1.7.0 (repo pin) | 1 | cursor/verbosity params on tool calls | https://docs.rs/rmcp/1.7.0/rmcp/index.html (in-repo pin) |
| MCP pagination model | 2025-06-18 spec | 1 | opaque `nextCursor`/`cursor` pattern to mirror in payloads | https://modelcontextprotocol.io/specification/2025-06-18/server/utilities/pagination |
| Anthropic "writing tools for agents" | 2025 | 1 | concise mode ≈⅓ tokens, paginate/filter/truncate, steer-on-truncate, 25k cap | https://www.anthropic.com/engineering/writing-tools-for-agents |
| tree-sitter | 0.26.x (repo pin) | 2,4 | capture params/return/sig spans by field; CST for CFG/dataflow | https://tree-sitter.github.io/tree-sitter/using-parsers/queries/1-syntax.html |
| redb `MigrationRegistry` | 4.1.0 (repo pin) | 2,3 | in-place vN→vN+1 re-encode of widened records, no rebuild | post-v1-roadmap RD2; https://docs.rs/redb/4.1.0/redb/struct.WriteTransaction.html |
| salsa | =0.26.2 (repo pin) | 2,3 | `Update`-safe fact inputs (byte/u8 tags) for new parse facts | https://docs.rs/salsa/0.26.2/salsa/trait.Update.html |
| SCIP layer | repo (default-on, out-of-band) | 3,4 | precise reference/type/relationship edges feeding recall + dataflow | scip-driven-edges/plan.md; commit 0af641e |
| IFDS framework | POPL'95 (method) | 4 | precise polynomial dataflow as graph reachability | https://research.cs.wisc.edu/wpis/abstracts/popl95.abs.html |
| (deferred) per-block libraries | — | all | fixed by each block's `/spec-plan` (intelligence-platform precedent) | intelligence-platform/plan.md AR1 |
</tech_inventory>

<risks>
| id | risk | likelihood | mitigation |
|---|---|---|---|
| AR1 | a block seed drifts into committed tiers, violating "general sense only" | medium | each block lists *candidate capabilities*, not tiers; tier cuts come only from the per-block `/spec-plan` (AD2) |
| AR2 | overlap/duplication with `intelligence-platform` block A | medium | AD3 fixes the boundary (data plane vs analyses); Block 4 owns the dataflow stretch that arc deferred |
| AR3 | Block 1 ranked truncation reintroduces non-determinism | medium | rank by a stable key (e.g. path/byte order), stable opaque cursor, truncation reported with a "N more, call with cursor" steer — never silent [src: r1 `<constraints>`; Anthropic guidance] |
| AR4 | Block 2 widening `SymbolRecord` is a 5-crate thread + migration (large blast radius) | medium | reuse the RD10 thread + a frozen `SymbolRecordVN` round-trip migration test; ship behind one redb step (AD5) |
| AR5 | Block 4 dataflow scope explodes (interprocedural/path-sensitive) | medium | last block; intra-procedural + IFDS-bounded finite facts first; feature-flagged; interprocedural is a later tier (AD6) |
| AR6 | any block regresses an SLO (cold/incr/warm/query, RAM) | low | per-tier SLO + `memory_report()` probe in each expansion; >256MB/table hard fail [src: ariadne-core plan.md R1] |
</risks>

<verification>
Arc-level: every v1 + post-v1 audit stays green and the ariadne_v2 self-index dogfood
stays green throughout; the arc is "done" when all four blocks have audited PASS tiers.
Per-block proof intent (detailed in each block's `<verification_intent>`): Block 1 — a
deterministic token-delta harness shows a measured reduction on the growable tools and a
cursor round-trip returns the full set across pages; Block 2 — outline/`read_symbol`/
docgen read exact doc/sig/nesting/param facts (golden) and the migration preserves prior
records byte-faithfully; Block 3 — method/path/cross-crate callees resolve with no
phantom regression (recall up, precision held) on the 15-language fixtures; Block 4 —
a seeded intra-procedural flow (source→sink) is reported correctly and deterministically.
No block is "done" on type-check alone [src: CLAUDE.md `<rules>` validation-by-execution].
</verification>

<blocks>
Run in order (1 first or ∥ 2, then 3, then 4); each opens a fresh planning session that
designs deep tiers for one block:
- 1: `/spec-plan .claude/plans/data-fidelity-arc/block-1-serve-leaner.md`
- 2: `/spec-plan .claude/plans/data-fidelity-arc/block-2-parse-deeper.md`
- 3: `/spec-plan .claude/plans/data-fidelity-arc/block-3-resolve-completer.md` (after Block 2)
- 4: `/spec-plan .claude/plans/data-fidelity-arc/block-4-reason-deeper.md` (after Blocks 2,3)
</blocks>

<sources>
- Sibling seed (the read win this arc generalizes): .claude/plans/context-efficient-read/plan.md
- Arc-pattern precedent + boundary: .claude/plans/intelligence-platform/plan.md ; block-a-deepen-brain.md
- Resolver recall boundary: .claude/plans/r1-resolver-completion/plan.md ; .claude/plans/scip-driven-edges/plan.md
- Symbol record + migration precedent: .claude/plans/post-v1-roadmap/plan.md RD2, RD10
- Writing effective tools for agents — Anthropic: https://www.anthropic.com/engineering/writing-tools-for-agents
- MCP pagination: https://modelcontextprotocol.io/specification/2025-06-18/server/utilities/pagination
- Claude Code MCP (25k cap): https://code.claude.com/docs/en/mcp
- IFDS dataflow: https://research.cs.wisc.edu/wpis/abstracts/popl95.abs.html
- tree-sitter query syntax: https://tree-sitter.github.io/tree-sitter/using-parsers/queries/1-syntax.html
- Hexagonal Architecture (Cockburn, 2005): https://alistair.cockburn.us/hexagonal-architecture/
</sources>
