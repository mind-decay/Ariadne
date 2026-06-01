---
tier_id: tier-01
title: Force Ariadne tool visibility — alwaysLoad in .mcp.json + per-tool _meta + 2KB instructions
deps: []
exit_criteria:
  - "`ariadne setup` writes `\"alwaysLoad\": true` into the `ariadne` entry of `.mcp.json`, idempotently, leaving foreign entries untouched."
  - "The MCP server attaches `_meta {\"anthropic/alwaysLoad\": true}` to every one of the 13 tools (verified by a handshake/list_tools test)."
  - "Server `with_instructions` ≤2KB and frames when to search for the tools; descriptions unchanged in shape."
  - "This repo's `.mcp.json` re-run through `setup` now carries `alwaysLoad`; a fresh session shows Ariadne tools loaded without a ToolSearch step."
status: pending
---

<context>
Deferral is the dominant cause: MCP Tool Search defers tools by default, so only
names load and the trigger-phrase descriptions never reach the decision point
[src: https://code.claude.com/docs/en/mcp "Scale with MCP Tool Search"]. This tier
makes Ariadne tools always-loaded by two independent means (belt + suspenders) and
tightens the server instructions, which under tool search are the discovery signal
"like skills" and truncate at 2KB [src: same, "For MCP server authors"]. See
plan.md `<decisions>` D1, D2 for rationale and rejected alternatives.
</context>

<files>
- `crates/ariadne-cli/src/commands/setup.rs` — extend `merge_mcp_json` to add
  `"alwaysLoad": true` to the `ariadne` entry [setup.rs:44-86].
- `crates/ariadne-mcp/src/server.rs` — attach per-tool `_meta` alwaysLoad; tighten
  `with_instructions` body [server.rs:184-460, 463-478].
- `crates/ariadne-cli/tests/` (new or existing setup test) — assert `.mcp.json`
  carries `alwaysLoad` after `setup`.
- `crates/ariadne-mcp/tests/handshake.rs` — assert every listed tool carries the
  `_meta` flag.
</files>

<steps>
1. **Failing test (setup).** In the CLI setup test, run `setup` on a temp project
   and assert the parsed `.mcp.json` `mcpServers.ariadne.alwaysLoad == true` and a
   second pre-existing foreign server entry is preserved. Run — it fails.
2. **Implement (setup).** In `merge_mcp_json`, add `"alwaysLoad": true` to the
   `entry` JSON object alongside `command`/`args`/`env` [src: setup.rs:57-62;
   https://code.claude.com/docs/en/mcp "Exempt a server from deferral"]. Keep the
   merge non-destructive: only the `ariadne` key is inserted/replaced.
3. **Failing test (server _meta).** In `handshake.rs`, after `initialize`+
   `tools/list`, assert each returned tool's `_meta` contains
   `"anthropic/alwaysLoad": true`. Run — it fails.
4. **Spike + implement (server _meta).** Confirm how rmcp 1.7 lets a
   `#[tool]`-generated `Tool` carry `meta`: prefer a macro attribute; if absent,
   override `ServerHandler::list_tools` to map each tool through
   `Tool::with_meta(Meta::from(json!({"anthropic/alwaysLoad": true})))`
   [src: https://docs.rs/rmcp/1.7.0/rmcp/model/struct.Tool.html — `meta:
   Option<Meta>`, `with_meta()`]. No tool name/description/schema changes.
5. **Tighten instructions.** Rewrite `with_instructions` (≤2KB) to lead with the
   one-line trigger ("for any question about symbols, references, impact, or
   architecture, search for and call these tools instead of grep/Read"), then the
   navigate/impact/architecture/docs grouping, then the freshness note. Keep it
   factual [src: https://code.claude.com/docs/en/mcp 2KB truncation; server.rs:464].
6. **Dogfood.** Run `ariadne setup` in this repo to rewrite `.mcp.json` with
   `alwaysLoad`. Record the resulting entry in the verification notes.
</steps>

<verification>
- `cargo nextest run -p ariadne-cli -p ariadne-mcp` — the two new asserts pass.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`;
  `cargo fmt --all --check`; `cargo test --test architecture`.
- Inspect this repo's `.mcp.json`: `mcpServers.ariadne.alwaysLoad == true`.
- Real run: open a fresh Claude Code session in this repo and confirm an Ariadne
  tool is callable without a preceding `ToolSearch` (tools loaded upfront). State
  the observation explicitly; if it cannot be run in-session, say so.
- Fail loudly: if rmcp 1.7 cannot attach `_meta`, do NOT fake it — keep D1 as the
  shipped fix, mark the `_meta` assert `#[ignore]` with a cited reason, and report.
</verification>

<rollback>
Revert `setup.rs` and `server.rs`; re-run `ariadne setup` to drop `alwaysLoad`
from `.mcp.json` (or hand-delete the key). No data migration; config-only.
</rollback>
