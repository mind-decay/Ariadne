---
tier_id: tier-08
title: MCP server (rmcp 1.7.0 stdio) exposing analytics as tools to Claude Code
deps: [tier-01, tier-02, tier-04, tier-05, tier-06, tier-07]
exit_criteria:
  - `ariadne-mcp serve` runs an rmcp 1.7.0 stdio server with #[tool] handlers for: list_symbols, find_definition, find_references, blast_radius, file_summary, plan_assist, coupling_report, weak_spots, doc_for, project_status.
  - JSON-Schema for each tool's input/output is auto-derived via the #[tool] macro and validated by an integration test that spawns the server, sends `tools/list` over stdio, and asserts the schema set matches a golden snapshot.
  - Concurrent reads: 8 simultaneous tool calls on a fixture project succeed in <100ms p95 each (criterion).
  - Cold start: from `ariadne-mcp serve` to first tools/list response <100ms on a fully-built 10K-file index.
  - Graceful shutdown on SIGINT/EOF; no orphaned threads.
status: completed
completed: 2026-05-20
---

<context>
Claude Code natively spawns MCP stdio servers per session, listing them in mcp.json [src: https://docs.claude.com/en/docs/claude-code]. rmcp 1.7.0 provides #[tool_router]/#[tool] macros that minimize boilerplate [src: https://docs.rs/rmcp]. We expose read-only analytics — write operations stay in CLI / watcher.
</context>

<files>
- crates/ariadne-mcp/Cargo.toml — `rmcp = "=1.7.0"` with features ["server","macros","transport-io"], `tokio` (rt-multi-thread), serde_json, schemars, workspace deps.
- crates/ariadne-mcp/src/lib.rs — re-exports `serve_stdio`, `AriadneServer`.
- crates/ariadne-mcp/src/server.rs — #[tool_router]/#[tool_handler] impl on `AriadneServer { db: Arc<RwLock<AriadneDb>>, snapshot_cache: ... }`.
- crates/ariadne-mcp/src/tools/{list_symbols,find_def,find_refs,blast_radius,file_summary,plan_assist,coupling,weak_spots,doc_for,status}.rs — one module per tool.
- crates/ariadne-mcp/src/types.rs — public input/output structs with `JsonSchema` derive.
- crates/ariadne-mcp/tests/handshake.rs — spawns binary, drives stdio MCP handshake (initialize → tools/list), asserts golden tools-list.
- crates/ariadne-mcp/tests/tools_<name>.rs — per-tool integration test against a fixture project.
- crates/ariadne-mcp/benches/concurrent.rs — criterion with 8 concurrent tool calls.
- bin: `crates/ariadne-mcp/src/bin/ariadne-mcp.rs` — entrypoint that calls `serve_stdio`.
</files>

<steps>
1. **Failing test first** (tests/handshake.rs): spawn `cargo run -p ariadne-mcp --bin ariadne-mcp -- serve --root tests/fixtures/tiny` via std::process; write JSON-RPC `initialize` then `tools/list` on stdin; read stdout; assert returned tools list matches insta golden. Fails until step 4.
2. Add rmcp pinned `= "1.7.0"` to ensure no minor-version churn (R5) [src: https://docs.rs/rmcp].
3. Define input/output types in src/types.rs with `#[derive(serde::Deserialize, serde::Serialize, schemars::JsonSchema)]`. Each tool gets `<Name>Input` and `<Name>Output` structs. Example:
   ```rust
   #[derive(Deserialize, JsonSchema)]
   pub struct BlastRadiusInput { pub symbol: String, pub depth: Option<u8>, pub kinds: Option<Vec<EdgeKindFilter>> }
   #[derive(Serialize, JsonSchema)]
   pub struct BlastRadiusOutput { pub must_touch: Vec<SymbolSummary>, pub may_touch: Vec<SymbolSummary>, pub depth_used: u8 }
   ```
4. Implement `AriadneServer` with `#[tool_router]`. Each `#[tool]` async fn looks up `AriadneDb`, opens a read snapshot, calls the corresponding ariadne-graph analytic, returns the serialized output [src: https://docs.rs/rmcp].
5. Tool catalog (each its own module):
   - list_symbols(query, lang?, kind?, limit) → top-K matching symbols.
   - find_definition(symbol) → SymbolSummary with file + line.
   - find_references(symbol, scope?) → list of CallSite/Reference.
   - blast_radius(symbol, depth, kinds) → tier-07 output.
   - file_summary(path) → symbols, fan-in/out, top dependencies.
   - plan_assist(symbol, max_files) → ranked PlanFile list.
   - coupling_report(scope) → Ca/Ce/I/A/D table.
   - weak_spots(scope?) → cycles ∪ god-modules (Ce > N) ∪ dead-code top-N, with reasons.
   - doc_for(symbol) → structured: signature, kind, file, brief, public refs.
   - project_status() → revision, file_count, symbol_count, edge_count, last_index_ms.
6. `serve_stdio(opts: ServeOpts)`: builds `tokio` rt, opens Storage + AriadneDb, optionally starts watcher (configurable), creates `AriadneServer`, calls `rmcp::serve::stdio(server)` [src: https://docs.rs/rmcp/latest/rmcp/transport/index.html].
7. Concurrency model: `AriadneDb` is `RwLock` — tools take read locks via Salsa snapshot (cheap; multiple readers concurrent). Writer is the watcher loop (or `ariadne-cli index` one-shot).
8. Cancellation: on Claude session shutdown, rmcp sends EOF on stdin → tokio task ends → server drops db handle → no orphan threads. Use `tokio::signal::ctrl_c` for SIGINT.
9. **Failing tests** per tool (tests/tools_<name>.rs): for each tool spawn the bin, send `initialize`+`tools/call`, assert response matches golden insta snapshot on the tiny fixture.
10. Concurrency bench (benches/concurrent.rs, `harness = false`): 8 tokio tasks each calling blast_radius+list_symbols 100 times on a 10K-symbol fixture index; assert per-call p95 <100ms; gate in CI. Custom percentile harness rather than the `criterion` crate — child-process spawn-per-iter doesn't fit criterion's sampling model and the SLO is a single p95 gate, not regression detection.
10b. Cold-start bench (benches/cold_start.rs, `harness = false`): seed a redb fixture with 10 000 files (one symbol each, sparse references), spawn `ariadne-mcp serve`, drive `initialize` → `notifications/initialized` → `tools/list`, measure wall-clock from `Command::spawn` to the `tools/list` response. Assert <100ms; gate exit_criteria 4. Sibling test `tests/shutdown.rs` covers exit_criteria 5 (EOF → clean exit, no zombie).
11. Document mcp.json snippet in README:
    ```json
    { "mcpServers": { "ariadne": { "command": "ariadne-mcp", "args": ["serve", "--root", "."] } } }
    ```
12. Note: rmcp 1.7.0 specifies MCP protocol version `2024-11-05` [src: per WebFetch summary]. If Claude Code requires a newer protocol version at integration time, bump rmcp + re-run handshake test.
</steps>

<verification>
- `cargo nextest run -p ariadne-mcp` green; handshake + 10 per-tool tests pass.
- `cargo bench -p ariadne-mcp` p95 ≤100ms per tool call under 8-way concurrency.
- `cargo bench -p ariadne-mcp --bench cold_start` wall-clock ≤100ms on a 10 000-file fixture (exit_criteria 4).
- `cargo nextest run -p ariadne-mcp --test shutdown` confirms EOF → clean exit + reaped child (exit_criteria 5).
- Manual: add `ariadne` to a local Claude Code project's `.mcp.json`, restart session, run `/mcp` and confirm tools listed; invoke "blast radius of …" on the ariadne_v2 self-index; output reasonable. Compare against tier-07 golden.
- Manual (fulfills tier-07 `<verification>` bullet 3 — deferred while SCIP→storage commit pipeline was being wired): run `plan_assist` for `ariadne_storage::WriteTxn::apply_changeset` against the ariadne_v2 self-index via the MCP `plan_assist` tool. Expect every direct caller's file plus the storage crate itself in the returned `PlanFile` list. Document outcome in this tier's audit report.
- Negative: kill the binary mid-request; client receives error frame, no zombie processes (`ps -ax | grep ariadne-mcp` empty).
</verification>

<rollback>
`git rm -r crates/ariadne-mcp` + workspace member removal. Remove ariadne entry from any local mcp.json files.
</rollback>

<deferred>
The manual `<verification>` bullets 3-5 (Claude Code `.mcp.json` round-trip, `plan_assist` for `ariadne_storage::WriteTxn::apply_changeset` against the ariadne_v2 self-index, mid-request kill check) are deferred to tier-10. They require the SCIP→storage commit pipeline + `ariadne index` CLI, both stubbed until tier-09/10 (see `ariadne-salsa::AriadneDb::commit_revision` returning an empty `Changeset`, and `ariadne-cli::main` printing a tier-10 stub). Tier-10 already gates "All MCP tools exercised end-to-end via spawned client; insta golden outputs stable" against real OSS fixtures, which subsumes these checks.
</deferred>
