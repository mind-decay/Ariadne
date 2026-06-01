---
slug: ariadne-mcp-adoption
title: Make Claude use Ariadne over grep/Read — visibility, bootstrap, advisory + graph search & source-read tools
created: 2026-06-01
owners: [user, claude]
review: [user, codex]
single_tier: false
tiers:
  - tier-01-always-load-visibility
  - tier-02-digest-command
  - tier-03-session-start-hook
  - tier-04-pretooluse-advisory
  - tier-05-adoption-eval
  - tier-06-search-read-spike
  - tier-07-search-code-tool
  - tier-08-read-symbol-tool
  - tier-09-search-read-advisory-eval
---

<context>
Ariadne ships 13 read-only graph tools with trigger-phrase descriptions and an
assertive `with_instructions` ("Prefer these tools over grep, Read…"), echoed in
CLAUDE.md. In practice Claude still reaches for native `grep`/`Read`. Root cause,
verified this session:

1. **Deferral hides descriptions.** MCP Tool Search is on by default — MCP tools
   are deferred and discovered on demand, so at decision time only tool *names*
   load, never the trigger-phrase descriptions; `auto` mode defers once schemas
   pass 10% of context [src: https://code.claude.com/docs/en/mcp "Scale with MCP
   Tool Search"]. `grep`/`Read` are native and always loaded → zero friction.
2. **No forcing function.** Context7/devtools get used because they have no
   native substitute; for code navigation, `grep`/`Read` *are* the substitute and
   fire reflexively. Nothing intercepts them.
3. **No session bootstrap.** The original "knows the project at session start"
   intent needs context injected at `SessionStart`; nothing does, so Claude greps
   to orient.

Scope: raise Ariadne tool-use over `grep`/`Read` on codebase questions by (a)
forcing tool visibility, (b) bootstrapping a project digest each session, (c)
advisory steering on symbol-shaped greps — shipped as Ariadne product features so
every consumer project benefits, dogfooded here.

Capability gap (tiers 06–09): the 13 tools navigate and analyze but cannot
*search* (only `list_symbols` name-substring) or *return source* (outputs are
metadata + spans, so Claude still `Read`s whole files). Adds two deterministic
primitives — `search_code` and `read_symbol` (D8/D9), gated by a spike (tier-06).

Out of scope: semantic/embedding retrieval [src: feedback_no_llm_features];
full-text/body content index (symbol-metadata reach only — `grep` stays the
fallback for free text and non-parsed files); the MCP startup-latency fix
(separate plan); rewriting CLAUDE.md prose beyond listing the two new tools.
</context>

<constraints>
- Hexagonal: digest analytics compose existing `ariadne-graph` use cases; the CLI
  driving adapter (`ariadne-cli`) orchestrates; `_meta` flag lives in the existing
  `ariadne-mcp` adapter; no driving→driving dep added [src: CLAUDE.md hexagonal].
- TDD: each tier writes a failing test first; no module-boundary mocks; temp-dir
  fixtures for `setup`/digest IO [src: ariadne-core/plan.md `<constraints>`].
- No new runtime dependency without sign-off; reuse `serde_json`, `clap`, `rmcp`
  already in tree [src: ariadne-mcp/Cargo.toml:31; ariadne-cli main.rs:19].
- Determinism preserved: digest is a pure read projection; no inference
  [src: feedback_no_llm_features].
- Anti-flake: unit tests assert functional wiring (file contents, JSON shape), not
  wall-clock; behavioral tool-use ratio is reported, never a hard CI gate
  [src: feedback_validation_required].
- `additionalContext` ≤10,000 chars; server instructions + each tool description
  truncate at 2KB; MCP tool output default cap 25k tokens
  [src: https://code.claude.com/docs/en/hooks; https://code.claude.com/docs/en/mcp].
</constraints>

<decisions>
- D1 — Primary visibility fix: write `"alwaysLoad": true` into the `ariadne`
  entry of `.mcp.json`, emitted by `ariadne setup`. Exempts the server from
  deferral so all 13 descriptions load every session regardless of
  `ENABLE_TOOL_SEARCH`. Rejected: `ENABLE_TOOL_SEARCH=false` (user-global env, not
  per-project, harms other servers); relying on tool-search discovery (descriptions
  stay invisible at decision time) [src: https://code.claude.com/docs/en/mcp
  "Exempt a server from deferral"; field requires Claude Code v2.1.121+].
- D2 — Secondary visibility: set per-tool `_meta {"anthropic/alwaysLoad": true}`
  in the MCP server via rmcp `Tool::with_meta`, so always-load holds even when a
  consumer's `.mcp.json` is hand-written or comes from a claude.ai connector.
  rmcp 1.7.0 `Tool` exposes `meta: Option<Meta>` + `with_meta()`
  [src: https://docs.rs/rmcp/1.7.0/rmcp/model/struct.Tool.html;
  https://code.claude.com/docs/en/mcp "mark individual tools as always-loaded …
  anthropic/alwaysLoad"]. Build-time spike: confirm rmcp-macros 1.7 lets the
  `#[tool]`-generated tool carry meta, else override `list_tools`.
- D3 — Session bootstrap: a `SessionStart` hook injects a compact digest as
  `hookSpecificOutput.additionalContext`, phrased as factual statements (imperative
  out-of-band text trips prompt-injection defenses). Rejected: inject full
  `doc_for_project` every session (token cost, 10k cap)
  [src: https://code.claude.com/docs/en/hooks;
  https://www.mindstudio.ai/blog/session-start-hooks-claude-code-force-context].
- D4 — Digest source: a new deterministic `ariadne digest` command composing
  `project_status` + `coupling_report` (top modules) + `doc_for_project` through
  the existing daemon/cold query path, emitting bounded agent-friendly markdown
  with a timeout fallback. Rejected: hook shelling raw `ariadne query` JSON (not
  compact, not agent-shaped) [src: ariadne-cli main.rs:76-93 Query/Status;
  https://www.anthropic.com/engineering/writing-tools-for-agents token efficiency].
- D5 — Steering is advisory: `PreToolUse` on `Grep`/`Glob`/`Read` returns
  `permissionDecision: allow` + `additionalContext` for symbol-shaped queries.
  Rejected: `deny` (false positives break legit text search), `ask` (constant
  interruption) [src: https://code.claude.com/docs/en/hooks; user decision].
- D6 — Distribution: extend the existing `ariadne setup` composition root to write
  `alwaysLoad`, the hook scripts, and the `.claude/settings.json` hook entries —
  idempotently, like its current `.mcp.json`/CLAUDE.md merges. Dogfood in this
  repo; ship to all consumers. Rejected: a new `init`-style command (duplicates
  `setup`) [src: ariadne-cli main.rs:39-46; commands/setup.rs:25-86].
- D7 — Measurement: adoption = ratio of `mcp__ariadne__*` calls to `Grep`/`Read`
  calls on a fixed question set in a headless run; reported, not gated (model
  non-determinism). Wiring is asserted deterministically per tier
  [src: https://www.anthropic.com/engineering/writing-tools-for-agents eval-driven;
  CLAUDE.md validate-by-execution].
- D8 — `search_code`: regex-or-substring on symbol name + optional `path` glob /
  `kind` / `lang` / `visibility` / `limit` → ranked `SymbolSummary`. Pure
  projection over the in-RAM `Catalog` (no new domain port), like `list_symbols`
  [src: catalog.rs:60-153; tools/list_symbols.rs:11-32].
- D9 — `read_symbol`: symbol → span, read the live file under `Catalog.root`,
  return mode `signature|full|context(±N)` + file, line range, `revision`,
  `stale:true` (clamp, never fail). Disk IO in new `adapters/source.rs` [src:
  catalog.rs:77-79; CLAUDE.md "IO under src/adapters/"; tables.rs:15-36 no source].
- D10 — Promote transitive `regex` 1.12.3 (linear-time, `size_limit`/`nest_limit`
  bound — no ReDoS) + `glob` 0.3.3 (`matches_path`, `**`) to direct `ariadne-mcp`
  deps; pure-Rust, already in `Cargo.lock` via `ignore`; signed off this session
  [src: Cargo.lock; docs.rs/regex/1.12.3; docs.rs/glob/0.3.3].
- D11 — Spike-gated: tier-06 measures token-delta vs `grep`+whole-file-`Read` on a
  fixed set; tiers 07–09 proceed only if median reduction ≥40%, else cancelled
  [src: user "measure first"; https://milvus.io/blog/why-im-against-claude-codes-
  grep-only-retrieval-it-just-burns-too-many-tokens.md].
</decisions>

<architecture>
- `ariadne-mcp` (driven→driving MCP adapter): per-tool `_meta` alwaysLoad +
  tightened 2KB server instructions [server.rs:184-460,463-478].
- `ariadne-graph` (domain analytics): existing `coupling`, `docgen`,
  `project_status` use cases reused unchanged by the digest projection.
- `ariadne-cli` (composition root): new `digest` subcommand; extended `setup`
  installer. Reuses the `query` daemon/cold plumbing [main.rs:11,146-164].
- Config artifacts (out-of-binary surfaces `setup` owns): `.mcp.json`
  (`alwaysLoad`), `.claude/settings.json` (`SessionStart`, `PreToolUse` hooks),
  `.claude/hooks/ariadne-session-start.sh`, `.claude/hooks/ariadne-grep-advisor.sh`.
- Eval (`ariadne-e2e`): wiring asserts + adoption harness; reused by tier-06/09.
- `ariadne-mcp` (tiers 07–08): `tools/search_code.rs` (filters `Catalog`) +
  `tools/read_symbol.rs` (IO via `adapters/source.rs`), `#[tool]` on `AriadneServer`.
</architecture>

<tech_inventory>
| Tech | Version | Doc fetched this session |
|------|---------|--------------------------|
| Claude Code MCP `alwaysLoad` / Tool Search | CC ≥2.1.121 | https://code.claude.com/docs/en/mcp |
| Claude Code hooks (SessionStart/PreToolUse) | current | https://code.claude.com/docs/en/hooks |
| rmcp (`Tool::meta`/`with_meta`) | =1.7.0 | https://docs.rs/rmcp/1.7.0/rmcp/model/struct.Tool.html |
| Anthropic "Writing tools for agents" | 2025 | https://www.anthropic.com/engineering/writing-tools-for-agents |
| clap / serde_json | in-tree | ariadne-cli main.rs:19; setup.rs:13 |
| regex / glob (search_code) | =1.12.3 / =0.3.3 | docs.rs/regex/1.12.3, docs.rs/glob/0.3.3 |
</tech_inventory>

<risks>
- R1 — `alwaysLoad` blocks session start until the daemon connects (≤5s cap)
  [src: code.claude.com/docs/en/mcp]. Couples to the `mcp-startup-latency` plan.
  Mitigation: land that plan first/concurrently; digest has a timeout fallback to a
  minimal message. Owner: user+claude.
- R2 — Visibility alone may not flip behavior. Mitigation: layered (digest +
  advisory); tier-05 measures and signals whether to escalate the intercept. Owner:
  claude.
- R3 — Digest goes stale within a long session. Mitigation: digest prints revision
  + freshness; `project_status` stays the live check. Owner: claude.
- R4 — `additionalContext` 10k cap truncation. Mitigation: digest bounded well
  under 10k; assert length in test. Owner: claude.
- R5 — Advisory false positives add noise. Mitigation: tight symbol-shaped
  heuristic; advisory is non-breaking. Owner: claude.
- R6 — `search_code` scans `Catalog` linearly. Mitigation: compile regex/glob once,
  substring fast-path, early-exit at `limit`, bench p95 <100ms; index is follow-up.
- R7 — `read_symbol` span stale after an edit. Mitigation: read live file, clamp
  spans, return `stale:true`+`revision`; never fabricate. Owner: claude.
</risks>

<verification>
- `cargo build --workspace`; `cargo nextest run --workspace`; clippy `-D warnings`;
  `cargo fmt --all --check`; `cargo deny check`; `cargo test --test architecture`.
- `ariadne setup` on a temp project then assert: `.mcp.json` ariadne entry has
  `alwaysLoad:true`; `.claude/settings.json` has SessionStart + PreToolUse entries;
  both hook scripts exist and are executable.
- `ariadne digest` on this repo emits non-empty markdown < the 10k cap with the
  current revision; golden-shape test on a fixture.
- Headless adoption harness runs a fixed question set and reports the Ariadne-vs-
  grep call ratio before/after (recorded in tier-05; not a hard gate).
- Search/read: tier-06 records token-delta + go/no-go; tier-07/08 test filters,
  span-slice == on-disk bytes, `stale` flag, search p95; tier-09 re-measures.
</verification>

<sources>
- [Connect Claude Code to tools via MCP](https://code.claude.com/docs/en/mcp)
- [Hooks reference — Claude Code](https://code.claude.com/docs/en/hooks)
- [rmcp Tool struct — docs.rs 1.7.0](https://docs.rs/rmcp/1.7.0/rmcp/model/struct.Tool.html)
- [Writing effective tools for AI agents — Anthropic](https://www.anthropic.com/engineering/writing-tools-for-agents)
- [SessionStart hooks force context — MindStudio](https://www.mindstudio.ai/blog/session-start-hooks-claude-code-force-context)
- [Why I'm against grep-only retrieval — Milvus](https://milvus.io/blog/why-im-against-claude-codes-grep-only-retrieval-it-just-burns-too-many-tokens.md)
- [regex RegexBuilder 1.12.3](https://docs.rs/regex/1.12.3/regex/struct.RegexBuilder.html) · [glob Pattern 0.3.3](https://docs.rs/glob/0.3.3/glob/struct.Pattern.html)
</sources>
