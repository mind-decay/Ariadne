---
slug: data-fidelity-arc/block-1
title: Block 1 — serve leaner (response economy across the growable MCP tools)
created: 2026-06-07
owners: [user, claude]
review: [user, codex?]
single_tier: false
tiers: [tier-01-economy-helper, tier-02-single-list-rollout, tier-03-multi-list-rollout, tier-04-diff-aware-rollout, tier-05-harness-advisory]
---

<context>
Expands the Block 1 seed [src: .claude/plans/data-fidelity-arc/block-1-serve-leaner.md] into
audited tiers. Inherits every arc constraint/decision [src: .claude/plans/data-fidelity-arc/plan.md].

Problem: `context-efficient-read` made the whole-file read path lean, but the rest of the MCP
surface bleeds context. Only `list_symbols` + `search_code` cap output [src:
crates/ariadne-mcp/src/tools/search_code.rs:73,90-137; list_symbols.rs:13]. The growable tools
return everything. **Measured on this 415-file repo (token proxy = bytes/4; Claude Code caps tool
results at 25k [src: https://www.anthropic.com/engineering/writing-tools-for-agents]):**
`co_change` (low thresholds) 733k tok (29× over), `hotspots` symbol-grain 311k (12×),
`complexity` symbol-grain 291k (11×), `blast_radius` 45–46k (1.8×), `hotspots` file-grain 34k
(1.4×), `coupling_report` 20k (80%), `find_references` up to 12k. Several tools already blow the
cap on a *medium* repo, before the 100K-file SLO scale. Capping is overdue, not speculative.

Success (measurable): each of the 10 growable tools bounds its own output deterministically — a
default page cap, an opaque cursor for the remainder, a `verbosity` knob (concise default), and a
steer line — proven by a cursor round-trip (page-union = the un-capped set) and a deterministic
token-delta harness; no tool exceeds 25k tokens at the default cap on the self-index.

In scope: a shared response-economy helper (`ariadne-graph`) + its application to the 10 growable
tools across all serving paths (MCP cold, MCP warm/daemon, CLI `query`), with CLI parity where a
twin exists. Out of scope: non-deterministic relevance ranking; changing what the tools *compute*;
the read path (owned by `context-efficient-read`); the read-only navigation tools that already cap.
</context>

<constraints>
- Deterministic — no inference; identical input → identical output; truncation/cursor use a stable
  sort key and are reported, never silent [src: .claude/plans/data-fidelity-arc/plan.md AD4,AR3;
  feedback_no_llm_features].
- Hexagonal + TDD: `ariadne-core` declares the shared protocol/DTOs; `ariadne-graph` holds the pure
  economy use case; adapters (`ariadne-mcp`, `ariadne-daemon`, `ariadne-cli`) call the use case,
  never each other. A failing test precedes implementation [src: CLAUDE.md `<rules>`;
  crates/ariadne-mcp/tests stays green; tests/architecture.rs].
- Byte-identical parity across the three serving paths (cold `tools::*::handle`, warm daemon
  `DaemonResponse`, CLI `query`/`dispatch`) — the economy logic lives in one shared helper both
  paths call [src: crates/ariadne-mcp/src/server.rs:770-795; crates/ariadne-cli/src/commands/query.rs:197-321;
  crates/ariadne-daemon/src/domain/queries/analytics.rs:1-10].
- No redb schema change — economy is a delivery-layer projection over already-computed results, not
  a stored fact (unlike Blocks 2/3) [src: arc plan.md AD5].
- No new workspace dependency: the cursor codec is hand-rolled (no base64/cbor crate is on the
  graph/core critical path; `bincode` lives only in `ariadne-parser`) [src: crates/*/Cargo.toml];
  a new dep stops and asks [src: CLAUDE.md `<rules>`].
- MCP output limits respected: ≤25k tokens at default cap; pagination/filter/truncation with
  sensible defaults; steer the agent on truncation [src:
  https://www.anthropic.com/engineering/writing-tools-for-agents].
- v1 SLOs hold (cold <60s, incr p95 <500ms, query p95 <100ms, warm p95 <10ms, <4GB/100K files);
  capping only shrinks work, so no regression is expected — still re-checked per tier [src: arc
  plan.md `<constraints>`].
- Each tier ships an ADR on its architectural decision; audit-gated [src: CLAUDE.md `<workflow>`].
</constraints>

<decisions>
**D1 — One pure economy helper in `ariadne-graph`, both adapters call it (parity for free).** A new
≤200-line module (the façade's one-analytic-per-module convention) exposes `Verbosity`,
`Budget{limit,cursor,verbosity}`, the opaque `Cursor` codec, and a generic `paginate` over a
caller-supplied stable comparator. The MCP cold handler and the daemon warm handler both call it, so
their JSON stays byte-identical [src: crates/ariadne-graph/src/lib.rs:1-49; arc plan.md AD5;
.claude/plans/context-efficient-read/plan.md D3 (assembler home)]. *Rejected:* an `ariadne-mcp`-only
formatting layer — it would skip the warm/CLI paths and break parity (driving→driving is forbidden)
[src: CLAUDE.md D13].

**D2 — Cursor = opaque, hand-rolled, revision-stamped `{revision:u32, offsets:[u64]}`.** MCP spec
pagination covers only *list* operations (`resources/list`, `tools/list`, …), **not `tools/call`**,
so a growable result carries its own cursor mirroring the spec's opaque model [src:
https://modelcontextprotocol.io/specification/2025-06-18/server/utilities/pagination]. The cursor is
opaque to the client (encoded, MUST-NOT-parse); `offsets` is a per-sublist offset vector (length 1
for single-list tools, N for multi-list), making one cursor type serve every tool. It carries the
catalog `revision`: within a revision an offset-into-stable-sort is deterministic and complete;
across a re-index the `revision` mismatches → a graceful invalid-cursor error steering a re-query
(the spec's "stable cursors" + "handle invalid cursors gracefully", −32602) [src: same spec; outputs
already carry `revision`, crates/ariadne-mcp/src/tools/read_outline.rs:99]. *Rejected:* index-only
offset with no revision (silently wrong after an edit); a content watermark (heavier, no gain over a
revision stamp on a deterministic sort).

**D3 — Concise verbosity is the default; `detailed` is a lossless superset.** Concise omits the
cryptic fields the LLM reasons about *worse* (raw `u64` symbol `id`, `byte_start`/`byte_end`),
keeping `name`/`kind`/`file`; this lands ≈⅓ the tokens [src:
https://www.anthropic.com/engineering/writing-tools-for-agents (concise ≈⅓; resolve cryptic ids →
semantic names)]. No data leaves the system: `detailed` returns everything, and in-repo precision
consumers (`ariadne digest`, snapshot tests) pin `verbosity:detailed`. Mechanism: optional fields +
`#[serde(skip_serializing_if)]` populated only in detailed; a "concise ⊂ detailed" test guards it.
Quality (lossless detailed) and efficiency (concise on every default call) are both preserved [src:
user directive 2026-06-07]. *Rejected:* default detailed (efficiency win only opt-in); dedicated
concise row types (more types + schema surface, harder parity).

**D4 — Per-sublist cap + per-tool stable sort key; default page = 50, harness-verified.** Each list a
tool returns is sorted by an explicit stable key (not the graph's incidental order) and truncated to
`limit` (default 50, tunable per tool), so the page is a meaningful top-N. 50 keeps every measured
tool well under 25k (rows are 45–80 tok each; even a 3-list tool ≈ 9–12k) [src: this-session
measurements; existing precedents search_code 64, weak_spots `MAX_DEAD` 16
crates/ariadne-mcp/src/tools/weak_spots.rs:20]. The default is verified by the tier-05 harness, never
assumed.

**D5 — Truncation is reported via `next_cursor` (machine) + `note` (human steer).** Each capped
output gains `next_cursor:Option<String>` and `note:Option<String>` ("Showing 50 of 407 — call again
with this cursor for the next page, or narrow with `prefix`/filter"), mirroring `SourceOutline.note`
[src: crates/ariadne-mcp/src/types.rs (SourceOutline); Anthropic steer-on-truncate guidance].

**D6 — Scope = 10 growable tools.** find_references, blast_radius, coupling_report, weak_spots,
co_change, hotspots, complexity (the seed's 7) plus diff_blast_radius, affected_tests,
refactor_suggestions (also unbounded) [src: user directive 2026-06-07; tool files under
crates/ariadne-mcp/src/tools/]. Tools that already cap (list_symbols, search_code, file_summary,
plan_assist) are untouched.
</decisions>

<architecture>
A delivery-layer pattern; no interior rewrite, no parse change.
- `ariadne-graph::economy` (new module) — pure: `Verbosity`, `Budget`, `Cursor` codec, `paginate`.
- `ariadne-core` — shared input params (`limit`/`cursor`/`verbosity`) on the relevant `DaemonQuery`
  variants + `next_cursor`/`note` on the wire DTOs [src: crates/ariadne-core/src/domain/daemon/].
- `ariadne-mcp` — `types.rs` mirrors the new params/fields; each tool's cold `handle` + `#[tool]`
  method calls `economy::paginate`; concise default applied.
- `ariadne-daemon` — warm handlers call the same `economy::paginate` → identical bytes.
- `ariadne-cli` — `query.rs` threads the new params; `digest` pins `detailed`.
Dataflow unchanged: watcher → daemon invalidates salsa → warm petgraph → clients query. Block 1
reshapes only the delivered view of an already-computed result.
</architecture>

<tech_inventory>
| tech | version pinned | role | source verified this session |
|---|---|---|---|
| MCP pagination model | 2025-06-18 | opaque cursor shape mirrored in-payload; list-ops-only confirms self-cursor | https://modelcontextprotocol.io/specification/2025-06-18/server/utilities/pagination |
| Anthropic "writing tools for agents" | 2025 | concise ≈⅓ tokens; paginate/truncate defaults; steer-on-truncate; 25k cap; semantic>cryptic ids | https://www.anthropic.com/engineering/writing-tools-for-agents |
| rmcp `#[tool]`/`CallToolResult` | =1.7.0 (repo pin) | cursor/limit/verbosity params on tool calls | repo pin; crates/ariadne-mcp/src/server.rs |
| serde `skip_serializing_if` | repo pin | concise field omission (D3) | crates/ariadne-mcp/src/types.rs derives |
| (no new dep) | — | hand-rolled cursor codec | crates/*/Cargo.toml (no base64/cbor on critical path) |
</tech_inventory>

<risks>
| id | risk | likelihood | mitigation |
|---|---|---|---|
| BR1 | cursor stale after a live re-index returns wrong/partial rows | medium | revision-stamped cursor; mismatch → graceful invalid-cursor error, never silent (D2) |
| BR2 | concise default breaks an in-repo consumer (digest/snapshots/tests) | medium | detailed is lossless; precision consumers pin `detailed`; "concise ⊂ detailed" test; snapshots re-accepted per tier (D3) |
| BR3 | three-path parity drifts (cold vs warm vs CLI) | medium | single shared `economy::paginate`; per-tool parity test asserts cold == warm JSON (the existing tier-07 parity pattern) |
| BR4 | daemon protocol enum change breaks an old running daemon | low | protocol is in-workspace, single binary; daemon restarts on revision change; note in tier-01 ADR |
| BR5 | multi-list / nested cursor (diff_blast seeds) mis-pages | medium | one `{revision, offsets[]}` cursor with a per-sublist offset; tier-04 round-trip test over the nested shape |
| BR6 | default cap too high → a 3-list tool still >25k | low | tier-05 harness asserts every tool ≤25k at default; lower `limit` if a tool fails |
</risks>

<verification>
Arc-level invariants stay green and the ariadne_v2 self-index dogfood stays green throughout. Block
is "done" when all five tiers have audited PASS. Per-tier proof intent:
- Cursor round-trip: page-1 (default cap) + cursor → page-2 …; the union equals the un-capped set,
  in stable order, with no duplicate or gap (completeness).
- Concise ⊂ detailed: every concise field is a subset of detailed; concise token count < detailed.
- Parity: cold `tools::*::handle` JSON == warm daemon JSON == CLI `query` JSON for each tool.
- Token-delta harness (bytes/4 proxy, `#[ignore]`, deterministic): records per-tool reduction vs the
  un-capped baseline and asserts ≤25k at the default cap [src: .claude/plans/context-efficient-read/
  tier-04-outline-adoption.md harness precedent].
No tier is "done" on type-check alone [src: CLAUDE.md `<rules>` validation-by-execution].
</verification>

<sources>
- Block 1 seed + arc master: .claude/plans/data-fidelity-arc/block-1-serve-leaner.md ; plan.md
- Anthropic writing tools for agents: https://www.anthropic.com/engineering/writing-tools-for-agents
- MCP pagination: https://modelcontextprotocol.io/specification/2025-06-18/server/utilities/pagination
- Claude Code MCP 25k cap: https://code.claude.com/docs/en/mcp
- Proven cap/sort/truncate shape: crates/ariadne-mcp/src/tools/search_code.rs:90-137
- Three serving paths: crates/ariadne-mcp/src/server.rs:770-795 ; crates/ariadne-cli/src/commands/query.rs ; crates/ariadne-daemon/src/domain/queries/analytics.rs
- Harness + advisory precedent: .claude/plans/context-efficient-read/tier-04-outline-adoption.md
- Hexagonal (Cockburn 2005): https://alistair.cockburn.us/hexagonal-architecture/
</sources>
