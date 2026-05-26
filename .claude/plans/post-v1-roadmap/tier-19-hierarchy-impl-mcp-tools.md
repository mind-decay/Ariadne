---
tier_id: tier-19
title: Hierarchy MCP tools — call_hierarchy, type_hierarchy, implementations
deps: [tier-18, tier-09]
exit_criteria:
  - Three MCP tools — `call_hierarchy`, `type_hierarchy`, `implementations` — are registered and discoverable.
  - Each reuses the tier-18 `ariadne-graph` use cases and routes through the daemon client with cold fallback.
  - Each tool is exercised end-to-end against a spawned MCP server with a stable insta golden.
  - `cargo nextest run -p ariadne-mcp` + architecture + clippy + fmt all green.
status: pending
---

<context>
Closes Block D and the roadmap. tier-18 put the call/type-hierarchy and implementations algorithms in `ariadne-graph` and exposed them over LSP. This tier exposes the same three algorithms to Claude as MCP tools — no new graph logic, only the MCP surface (plan RD9). Full context: plan.md.
</context>

<files>
- crates/ariadne-mcp/src/ — modify: three new `#[tool]` handlers + input/output types.
- crates/ariadne-core/src/domain/ — modify: `DaemonRequest`/`DaemonResponse` variants for the three queries.
- crates/ariadne-daemon/src/domain/ — modify: dispatch the three queries to `ariadne-graph::hierarchy`.
- crates/ariadne-mcp/tests/ — new: integration goldens for the three tools.
- docs/ — modify: regenerate the MCP discoverability doc (final tool catalog).
</files>

<steps>
1. Failing test first (`ariadne-mcp` tests): spawn the MCP server, call `call_hierarchy`, `type_hierarchy`, `implementations`, assert stable goldens. Red — the tools are not registered.
2. Add the three `DaemonRequest`/`DaemonResponse` variants and daemon-side dispatch to the tier-18 `ariadne-graph::hierarchy` use cases — no new algorithm, the daemon calls the same functions LSP calls.
3. Implement the three `#[tool]` handlers: `call_hierarchy` (symbol + direction), `type_hierarchy` (symbol + direction), `implementations` (trait/interface symbol). Route each through the tier-09 daemon client with the cold-path fallback.
4. Write tool descriptions per v1 tier-15 discoverability rules — third-person, explicit trigger phrases [src: .claude/plans/ariadne-core/tier-15-mcp-discoverability.md].
5. Integration goldens via the spawned-server harness.
6. Regenerate the discoverability doc; confirm the final tool count (v1 13 + tier-15 4 + this 3 = 20) is documented.
</steps>

<verification>
- `cargo nextest run -p ariadne-mcp` — three new tool goldens green; all prior tool goldens still green.
- Manual: in a Claude Code session, ask "who calls X" and "who implements trait Y"; confirm Claude selects `call_hierarchy` / `implementations` and the output matches the tier-18 LSP result for the same symbol.
- `cargo nextest run --workspace` — full workspace suite green (final roadmap gate).
- `cargo test --test architecture`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check` — green.
</verification>

<rollback>
`git checkout -- crates docs`. The three tools are additive; LSP hierarchy (tier-18) and the v1 tool surface are unaffected.
</rollback>
