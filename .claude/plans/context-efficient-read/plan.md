---
slug: context-efficient-read
title: Token-efficient whole-file read — a deterministic code skeleton/outline tool for LLM consumers
created: 2026-06-07
owners: [user, claude]
review: [user, codex]
single_tier: false
tiers:
  - tier-01-outline-projection
  - tier-02-outline-mcp
  - tier-03-outline-cli
  - tier-04-outline-adoption
---

<context>
The `ariadne-mcp-adoption` plan shipped `search_code` + `read_symbol` and an
advisory; symbol-targeted reads now cut ~87% of tokens vs grep + whole-file
`Read` [src: .claude/plans/ariadne-mcp-adoption/tier-09-search-read-advisory-eval.md
notes]. The residual gap is exactly the user's complaint: when a consumer LLM
must comprehend a *whole file* (not one already-known symbol), it still calls
native `Read` and pays for every byte — signatures, bodies, comments, blanks.
`read_symbol` answers "show me X"; nothing answers "what's in this file" cheaply.

This plan adds a deterministic **code skeleton / outline** read: one tool that
returns a file as near-valid folded source — imports + doc comments + signatures
kept, bodies elided to a fold marker with an elided-line count — plus a compact
symbol index so the LLM expands only the bodies it needs via `read_symbol`
(progressive disclosure). Precedent: aider's tree-sitter repo map sends
signatures, omits bodies — "GPT doesn't need to see the entire implementation …
it just needs to understand it well enough to use it" [src:
https://aider.chat/2023/10/22/repomap.html]; Anthropic's tool guidance: return
"only high signal information", progressive disclosure, a concise mode at "~⅓ of
the tokens" [src: https://www.anthropic.com/engineering/writing-tools-for-agents].

Scope: a pure outline assembler in the domain, exposed as an MCP tool
(`read_outline`) and a CLI subcommand (`ariadne outline`), across every language
the parser already indexes; the existing advisory escalates whole-file source
`Read`s to name it; discoverability + a deterministic token-delta re-measure.
Out of scope: LLM summarization / embeddings [src:
feedback_no_llm_features]; relevance-ranked budget auto-selection (non-
deterministic); a full-text body index; changing the symbol record schema.
</context>

<constraints>
- Deterministic: outline is a pure projection of bytes + symbol spans; no
  inference, no model call [src: feedback_no_llm_features].
- Hexagonal: the assembler is a domain use case in `ariadne-graph`; both driving
  adapters (`ariadne-mcp`, `ariadne-cli`) depend on it, never on each other —
  driving→driving is a hard fail [src: CLAUDE.md D13; feedback_hexagonal_strict].
- TDD: each tier writes a failing test first; pure assembler unit-tested on byte
  fixtures; adapter tiers tested over real on-disk files [src: plan.md
  `<constraints>`].
- No new runtime dependency; reuse `serde_json`, `clap`, `rmcp` in tree [src:
  .claude/plans/ariadne-mcp-adoption/plan.md `<constraints>`].
- Stale-safe IO: read the live file, clamp out-of-range spans, flag `stale` +
  `revision`, never fail/fabricate [src: crates/ariadne-mcp/src/adapters/source.rs:74-99].
- Query SLO: outline p95 <100ms on the standard workload [src:
  .claude/plans/ariadne-core/plan.md `<constraints>`].
- MCP limits: tool output ≤25k tokens (paginate/cap), server instructions ≤2KB,
  `additionalContext` ≤10k chars [src: https://code.claude.com/docs/en/mcp;
  https://code.claude.com/docs/en/hooks].
</constraints>

<decisions>
- D1 — New capability = deterministic whole-file **skeleton/outline** (imports +
  doc comments + signatures kept; bodies folded to a marker + elided-line count;
  nested symbols rendered under parents). Rejected: relevance-ranked token-budget
  auto-select (opaque, non-deterministic ranking); LLM summary (violates no-LLM)
  [src: https://aider.chat/2023/10/22/repomap.html; feedback_no_llm_features].
- D2 — Output = **code-like folded source** (densest, model-native) + a compact
  symbol index (name, kind, line range, body-line count) advertising
  `read_symbol` expansion. Rejected: JSON tree (heavier per token, less natural);
  markdown outline (less dense, not directly expandable) [src:
  https://www.anthropic.com/engineering/writing-tools-for-agents; user decision].
- D3 — Pure assembler lives in **`ariadne-graph`** (domain use case), reused by
  the MCP tool and the CLI subcommand. Rejected: placing it in `ariadne-mcp`
  beside `read_symbol` — CLI parity would then force `ariadne-cli`→`ariadne-mcp`,
  a banned driving→driving edge [src: CLAUDE.md D13; feedback_hexagonal_strict].
- D4 — **Byte-faithful** slices: signatures + doc comments are sliced from the
  live file bytes (not reconstructed from the graph); body span =
  `[signature_end, byte_end]`, folded. Rejected: reconstructing signatures from
  graph metadata (loses exact source + formatting) [src:
  crates/ariadne-mcp/src/adapters/source.rs:59-118; tier-08 D9].
- D5 — Doc comments + nesting are **derived deterministically**, no schema/parser
  change: doc = contiguous comment lines immediately above `byte_start` per the
  defining file's `Lang` comment syntax; nesting = symbol-span containment.
  Rejected: adding doc-span/parent fields to the symbol record + storage (large
  blast radius for a read-only view) [src:
  crates/ariadne-mcp/src/catalog.rs:26-51; crates/ariadne-core/src/domain/types/lang.rs].
- D6 — Advisory escalation stays **`allow`**: a whole-file source `Read` also
  names `read_outline`; never `deny`/`ask`. Rejected: blocking (false positives
  break legit reads) [src: .claude/plans/ariadne-mcp-adoption/plan.md D5; tier-09].
- D7 — **CLI parity**: `ariadne outline <path>` composes the same graph use case
  through the existing cold-catalog plumbing [src: crates/ariadne-cli — query
  path; user decision].
- D8 — **Measure**: a deterministic token-delta harness (bytes/4 proxy) compares
  whole-file `Read` vs `read_outline` over multi-symbol fixtures; target median
  reduction ≥50%; reported, not hard-gated (parser/format non-determinism is nil,
  but the gate matches the shipped anti-flake convention) [src:
  .claude/plans/ariadne-mcp-adoption/plan.md D11; tier-09 notes].
</decisions>

<architecture>
- `ariadne-graph` (domain): new `outline` use case — pure
  `assemble(req) -> Outline`. Input = file bytes + ordered symbol spans
  (`{name, kind, byte_start, byte_end, visibility}`) + `Lang` + options. Output =
  folded-source string + symbol index. No IO; sibling to `docgen`/`api_surface`.
- `ariadne-mcp` (driving): `tools/read_outline.rs` maps `Catalog` symbols
  (`cat.symbols` filtered by file, sorted by `byte_start` — the `file_summary`
  pattern) → assembler input, reads bytes via `adapters/source.rs`, returns
  `SourceOutline`; `read_outline` `#[tool]` on `AriadneServer`.
- `ariadne-cli` (driving): `outline` subcommand builds the cold catalog (query
  plumbing), enumerates file symbols, reads bytes via `std::fs`, calls the graph
  use case, prints folded source (or `--json`).
- Config surfaces (`ariadne setup` owns): `ariadne-grep-advisor.sh` gains the
  whole-file-Read → `read_outline` nudge; `with_instructions` + CLAUDE.md list it.
- Eval (`ariadne-e2e`): deterministic token-delta harness vs whole-file `Read`.
</architecture>

<tech_inventory>
| Tech | Version | Doc fetched this session |
|------|---------|--------------------------|
| rmcp `#[tool]`/`#[tool_router]`/`#[tool_handler]`, `CallToolResult` | =1.7.0 | https://docs.rs/rmcp/1.7.0/rmcp/index.html (Context7 quota exhausted) |
| clap derive `#[derive(Subcommand)]`/`#[arg]` | in-tree | https://docs.rs/clap/latest/clap/_derive/_tutorial/index.html |
| Anthropic "Writing tools for agents" | 2025 | https://www.anthropic.com/engineering/writing-tools-for-agents |
| aider repo map (tree-sitter signature skeleton) | 2023 | https://aider.chat/2023/10/22/repomap.html |
| Claude Code MCP / hooks (limits, advisory) | current | https://code.claude.com/docs/en/mcp · /hooks |
| In-tree anchors | — | source.rs:59-118 · catalog.rs:26-97 · lang.rs · file_summary.rs:25-102 |
</tech_inventory>

<risks>
- R1 — Per-`Lang` doc-comment lexical capture is imperfect (Python docstrings
  live *inside* the body, not above the decl). Mitigation: capture leading
  comment lines only; document the docstring gap; never block. Owner: claude.
- R2 — Nesting via span containment misfires on overlapping/macro-generated
  spans. Mitigation: nearest-enclosing rule + flat fallback; golden tests per
  language. Owner: claude.
- R3 — `signature_end` heuristic truncates multi-line signatures (generics/where
  clauses spanning lines). Mitigation: reuse the shipped heuristic, add a
  multi-line probe (extend to the first `{`/`:`/`;` across lines), test it.
  Owner: claude.
- R4 — Large files blow the 25k-token output cap / p95. Mitigation: skeleton is
  bytes-bounded by symbol count not file size; cap symbol count, paginate, bench
  p95. Owner: claude.
- R5 — Stale spans after edits. Mitigation: clamp + `stale` flag, reuse R7
  handling [src: source.rs:74-99]. Owner: claude.
- R6 — Advisory false positives add noise. Mitigation: tight heuristic (source
  extensions only), stays `allow`. Owner: claude.
</risks>

<verification>
- `cargo build --workspace`; `cargo nextest run --workspace`; clippy `-D
  warnings`; `cargo fmt --all --check`; `cargo deny check`; `cargo test --test
  architecture` (proves no driving→driving edge from D3).
- Golden-snapshot outline tests per language (rust, typescript, javascript, + any
  framework dialect with a fixture): folded source matches expected; fold counts
  exact; doc comments captured; nesting correct; private filter honoured.
- `read_outline` on this repo: skeleton bytes < whole-file bytes; `stale`/clamp
  on a truncated fixture; p95 <100ms.
- `ariadne outline <path>` prints the same skeleton as the MCP tool (parity).
- Token-delta harness records median reduction vs whole-file `Read` (target
  ≥50%, reported). Advisor names `read_outline` on a real source `Read`.
</verification>

<sources>
- [aider repo map with tree-sitter](https://aider.chat/2023/10/22/repomap.html) · [repomap docs](https://aider.chat/docs/repomap.html)
- [Writing effective tools for AI agents — Anthropic](https://www.anthropic.com/engineering/writing-tools-for-agents)
- [rmcp 1.7.0 — docs.rs](https://docs.rs/rmcp/1.7.0/rmcp/index.html) · [clap derive tutorial](https://docs.rs/clap/latest/clap/_derive/_tutorial/index.html)
- [Connect Claude Code to tools via MCP](https://code.claude.com/docs/en/mcp) · [Hooks reference](https://code.claude.com/docs/en/hooks)
- Prior plan: `.claude/plans/ariadne-mcp-adoption/` (search_code/read_symbol/advisory)
</sources>
