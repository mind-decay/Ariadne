---
tier_id: tier-14
title: Analytics MCP tools — hotspots, complexity, co_change, diff_blast_radius
deps: [tier-12, tier-13]
exit_criteria:
  - Four MCP tools — `hotspots`, `complexity`, `co_change`, `diff_blast_radius` — are registered and discoverable.
  - Each tool is exercised end-to-end against a spawned MCP server with a stable insta golden.
  - Each tool routes through the daemon client with the v1 cold-path fallback (tier-08 pattern).
  - `cargo nextest run -p ariadne-mcp` + architecture + clippy + fmt all green.
status: pending
---

<context>
Closes Block C. tier-12/tier-13 added the hotspot, co-change, complexity, and diff-aware blast-radius use cases to `ariadne-graph`. This tier exposes them to Claude as MCP tools, matching the discoverability conventions v1 tier-15 set for the existing 13 tools. Full context: plan.md.
</context>

<files>
- crates/ariadne-mcp/src/ — modify: four new `#[tool]` handlers + their input/output types.
- crates/ariadne-core/src/domain/ — modify (if needed): the protocol variants for the new queries (tier-06 `DaemonRequest`/`DaemonResponse`).
- crates/ariadne-daemon/src/domain/ — modify: dispatch the four new queries to `ariadne-graph`.
- crates/ariadne-mcp/tests/ — new: integration goldens for the four tools.
- docs/ — modify: regenerate the MCP tool list / discoverability doc (v1 tier-15 surface).
</files>

<steps>
1. Failing test first (`ariadne-mcp` tests): spawn the MCP server, call each of the four tools, assert each returns a stable golden. Red — the tools are not registered.
2. Add the four `DaemonRequest`/`DaemonResponse` variants (tier-06 protocol) and their daemon-side dispatch to `hotspot_report`, the complexity query, `co_change_report`, and `diff_blast` [src: tier-12, tier-13].
3. Implement the four `#[tool]` handlers: `hotspots` (scope-prefix input, like `weak_spots`), `complexity` (file or symbol scope), `co_change` (scope + thresholds), `diff_blast_radius` (`DiffSpec` input). Each handler routes through the tier-08 daemon client with the cold-path fallback.
4. Write tool descriptions following v1 tier-15's discoverability rules — third-person, with trigger phrases — so Claude selects them reliably [src: .claude/plans/ariadne-core/tier-15-mcp-discoverability.md].
5. Integration goldens via the spawned-server harness (v1 tier-08/10 pattern).
6. Regenerate the discoverability doc so the tool count and catalog are current.
</steps>

<verification>
- `cargo nextest run -p ariadne-mcp` — four new tool goldens green; all v1 tool goldens still green.
- Manual: in a Claude Code session, ask "what are the hotspots in this repo" and "blast radius of my current diff"; confirm Claude selects `hotspots` / `diff_blast_radius` and the output is reasonable.
- `cargo test --test architecture`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check` — green.
</verification>

<rollback>
`git checkout -- crates docs`. The four tools are additive; the v1 tool surface is unaffected.
</rollback>
