---
block_id: block-1
title: Block 1 — serve leaner (tool-wide token economy)
arc: data-fidelity-arc
order: 1
deps: []
status: seed   # seed → expand via /spec-plan into tiers
expand_with: /spec-plan .claude/plans/data-fidelity-arc/block-1-serve-leaner.md
---

<context>
This is a **seed plan**, not a tier set. It scopes Block 1 at the general level so a later
`/spec-plan` designs deep, audited tiers. Shared constraints/tech live in the arc master:
`.claude/plans/data-fidelity-arc/plan.md`.

Problem: `context-efficient-read` made whole-file reads token-lean, but the rest of the
MCP surface still bleeds context. Only `list_symbols` + `search_code` cap output [src:
crates/ariadne-mcp/src/tools/list_symbols.rs:13; search_code.rs:73,137]; the growable
tools return everything — `find_references` an unbounded `Vec<ReferenceSite>` [src:
crates/ariadne-mcp/src/tools/find_references.rs:24], and `blast_radius`, `coupling_report`,
`weak_spots`, `co_change`, `hotspots`, `complexity` full JSON with no cursor, no budget,
no concise mode. Anthropic's guidance: paginate/filter/truncate with sensible defaults,
offer a concise mode (≈⅓ the tokens), keep tool results ≤25k tokens, and steer the agent
when truncating [src: https://www.anthropic.com/engineering/writing-tools-for-agents].

Success: every growable tool bounds its own output deterministically — a default cap, an
opaque cursor for the rest, a `verbosity` knob, and a steering line — with a token-delta
harness proving the reduction and a cursor round-trip proving completeness.
Scope (in): a shared response-economy helper (`ariadne-graph`/`ariadne-mcp`) + its
application to the growable tools; CLI parity where the tool has a CLI twin. Scope (out):
relevance ranking that is non-deterministic; changing what the tools *compute*; the read
path (owned by `context-efficient-read`).
</context>

<candidate_capabilities>
Each bullet is a likely tier the `/spec-plan` expansion will detail. General terms only.

**S1 — Shared response-budget + opaque-cursor helper.** One deterministic helper: a
default item cap, a stable rank/sort key, a `nextCursor` opaque token (mirroring MCP's
list-pagination model, since `tools/call` results are NOT spec-paginated [src:
https://modelcontextprotocol.io/specification/2025-06-18/server/utilities/pagination]),
and a "N more — call with cursor" steer line. Reuses the `truncate`-after-sort shape
`search_code` already proves [src: crates/ariadne-mcp/src/tools/search_code.rs:90-137].

**S2 — Apply the budget to growable tools.** `find_references`, `blast_radius`,
`coupling_report`, `weak_spots`, `co_change`, `hotspots`, `complexity` — each gets a cap +
cursor + the steer line, default-bounded so a hot symbol (jquery `Ca 681`) no longer dumps
681 rows.

**S3 — `verbosity` enum (concise | detailed).** Concise drops low-signal fields (raw
ids/byte offsets) for semantically meaningful ones, landing ≈⅓ the tokens of detailed; a
per-tool default leans concise [src: https://www.anthropic.com/engineering/writing-tools-for-agents].

**S4 — Token-delta harness + advisory.** A deterministic bytes/4-proxy harness records the
reduction per tool vs the current unbounded output (reported, like the
`context-efficient-read` D8 harness [src: context-efficient-read/plan.md D8]); the existing
advisory/`with_instructions` names the new cursor/verbosity affordances.
</candidate_capabilities>

<existing_assets>
- `list_symbols`/`search_code` `limit` + sort-then-truncate — the proven shape S1 generalizes [src: list_symbols.rs:13-27; search_code.rs:73-137].
- `file_summary` `TOP_DEPS` truncation — a per-tool cap precedent [src: file_summary.rs:92].
- `context-efficient-read` token-delta harness (bytes/4 proxy) — reuse for S4 [src: context-efficient-read/plan.md `<verification>`].
- Advisory + `with_instructions` surface for discoverability [src: context-efficient-read/plan.md `<architecture>`].
</existing_assets>

<open_questions>
Resolve in the `/spec-plan` expansion (do not guess now):
- S1: cursor encoding — index-into-stable-sort vs a content watermark; opacity + stability
  across a re-index (MCP requires stable, opaque, non-persistent cursors) [src: MCP
  pagination spec].
- S2: per-tool default caps and the canonical stable sort key for each (references by
  caller order? coupling by Ca desc then path?) so truncation is meaningful, not arbitrary.
- S3: exact concise vs detailed field sets per tool; which is the default per tool.
- Helper home: a pure `ariadne-graph` projection vs an `ariadne-mcp` formatting layer —
  pick the side that keeps CLI parity without a driving→driving edge [src: CLAUDE.md D13].
- Does any tool already approach the 25k cap on this repo (measure before capping)?
</open_questions>

<verification_intent>
Deterministic golden + harness tests: a capped tool returns the default page + a cursor;
paging with the cursor returns the remaining items and the union equals the un-capped set
(completeness); concise output is a strict, lower-token projection of detailed; the
token-delta harness records a measured reduction per tool; no tool exceeds 25k tokens on
the self-index. Each tier TDD: failing test first [src: CLAUDE.md `<rules>`].
</verification_intent>

<sources>
- Writing effective tools for agents — Anthropic: https://www.anthropic.com/engineering/writing-tools-for-agents
- MCP pagination model: https://modelcontextprotocol.io/specification/2025-06-18/server/utilities/pagination
- Claude Code MCP (25k cap): https://code.claude.com/docs/en/mcp
- Proven cap/truncate shape: crates/ariadne-mcp/src/tools/search_code.rs:90-137 ; list_symbols.rs:13-27
- Sibling read win + harness: .claude/plans/context-efficient-read/plan.md
- Arc master + inherited constraints: .claude/plans/data-fidelity-arc/plan.md
</sources>
