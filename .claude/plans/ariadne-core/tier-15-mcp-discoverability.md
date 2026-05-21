---
tier_id: tier-15
title: MCP tool discoverability — server instructions + 13 tool descriptions rewritten for when-to-use + trigger phrases
deps: [tier-14]
exit_criteria:
  - "`AriadneServer::get_info` instructions string carries workflow guidance, explicit trigger phrases, and an explicit `prefer Ariadne tools over grep/Read` nudge naming those tools; a new `handshake.rs` test snapshots it as `server_instructions`."
  - "All 13 `#[tool(description=...)]` strings follow the `<spec>` template — what + `Use when ...` clause + at least one quoted trigger phrase, each ≤320 chars; a new `handshake.rs` test snapshots the name→description map as `tools_descriptions`."
  - "A failing-first `handshake.rs` test asserts every tool description contains the literal `Use when ` and the server instructions string contains the literal token `grep`; it fails against current tier-08 strings before the rewrite."
  - "`handshake__tools_list.snap` is byte-unchanged — `handshake.rs:17` snapshots `tool.input_schema` only, never `tool.description`; editing description strings cannot touch it."
  - "tier-15 audit records one real Claude Code MCP session: given a code-structure task whose lazy default is grep/Read, the agent calls an Ariadne tool; the tool name + transcript excerpt is pasted into the audit report."
  - "`cargo build --workspace`, `clippy -D warnings`, `fmt --check`, `cargo test --test architecture`, `cargo nextest run --workspace`, `RUSTDOCFLAGS=-D warnings cargo doc` all green."
status: pending
---

<context>
Post-v1 discoverability tier. `ariadne-mcp` ships 13 `#[tool]`s, but an
agent defaults to `grep`/`Read` instead of calling them — the two surfaces
that steer tool selection are both weak [src: crate-read 2026-05-21]:

W1 — server instructions [src: crates/ariadne-mcp/src/server.rs:227-232]
describe the tools but give no when-to-use and no "prefer over text
search" nudge. The `instructions` field exists precisely to bias the model
[src: https://modelcontextprotocol.io/specification/2025-06-18/basic/lifecycle —
`instructions` in the initialize result].

W2 — each `#[tool(description=...)]` [src: server.rs:85-218] is a one-line
"what" with zero "when" and zero trigger phrases. Tools are model-controlled
— the model "discover[s] and invoke[s] tools automatically based on its
contextual understanding"
[src: https://modelcontextprotocol.io/specification/2025-06-18/server/tools],
so the description is the lever. This also violates the repo convention
that a description states what AND when with explicit trigger phrases
[src: CLAUDE.md `<conventions>`].

Scope: `description` string literals + the `with_instructions` argument in
`server.rs`, plus new `handshake.rs` test coverage. No new dependency, no
port, no signature change, no ADR — `description` is tool metadata only
[src: https://docs.rs/rmcp/1.7.0/rmcp/attr.tool.html]. Brief correction:
the premise that `handshake__tools_list.snap` changes is wrong — that
golden snapshots `tool.input_schema` only [src: handshake.rs:17], which no
string edit touches; tier-15 instead ADDS two snapshots. Full context:
plan.md.
</context>

<files>
- crates/ariadne-mcp/src/server.rs — rewrite all 13 `#[tool(description=...)]`
  strings (L85-218) and the `with_instructions(...)` argument (L227-232).
- crates/ariadne-mcp/tests/handshake.rs — NEW tests: a failing-first content
  assertion, plus `tools_descriptions` + `server_instructions` snapshots.
- crates/ariadne-mcp/tests/snapshots/ — NEW `handshake__tools_descriptions.snap`
  and `handshake__server_instructions.snap`; `handshake__tools_list.snap`
  stays byte-unchanged.
</files>

<trigger_map>
Per-tool when-to-use anchor + trigger phrases the executor feeds the template.

| tool | use when | trigger phrases |
|---|---|---|
| list_symbols | locating a symbol by name/kind before opening files | "where is the X function", "list the structs in" |
| find_definition | you need the canonical definition site of a named symbol | "where is X defined", "go to definition of" |
| find_references | you need every use site of a symbol | "who calls X", "where is X used", "find usages of" |
| blast_radius | assessing what a change to a symbol could break | "what breaks if I change X", "impact of changing", "is it safe to edit" |
| file_summary | orienting in an unfamiliar file before reading it | "what is in this file", "summarize src/X.rs" |
| plan_assist | scoping which files a change touches before editing | "what files do I touch for X", "where do I start to change" |
| coupling_report | assessing module dependency / architecture health | "how coupled is", "Martin coupling metrics for" |
| weak_spots | hunting cycles, god modules, dead code | "what is wrong with this codebase", "find tech debt", "any cycles" |
| doc_for | you need a structured explanation of one symbol | "what does X do", "explain the symbol X" |
| project_status | checking index freshness/coverage before trusting results | "is the index current", "how big is the project" |
| doc_for_module | you need a doc-style summary of a file/module | "document this module", "overview of src/X.rs" |
| doc_for_project | you need a whole-project architecture overview | "explain the architecture", "how is this project structured" |
| refactor_suggestions | you want concrete static refactor candidates | "how should I refactor", "cleanup suggestions for" |
</trigger_map>

<spec>
**Per-tool description template** (one line, ≤320 chars):
`<what — keep within one clause of the current wording>. Use when <anchor>; triggers: "<phrase>", "<phrase>".`
The literal `Use when ` and at least one double-quoted trigger phrase are
mandatory — they are the machine-checked contract (step 1).

**Server instructions contract** — the `with_instructions` argument, one
paragraph, ≤900 chars (it is injected into context every session — keep
tight). It MUST contain, verbatim where quoted:
1. Identity: a read-only semantic graph of the local project.
2. The nudge — explicitly name `grep` and `Read`: prefer these tools over
   `grep`/`Read`/file-walking for any question about symbols, references,
   impact, or architecture; the graph answers in one call where text search
   needs many and misses cross-file edges.
3. Workflow map: navigate (`list_symbols`/`find_definition`/`find_references`),
   impact (`blast_radius`/`plan_assist`), architecture
   (`coupling_report`/`weak_spots`/`refactor_suggestions`), docs
   (`doc_for`/`doc_for_module`/`doc_for_project`), freshness
   (`project_status`).
4. Aggressive framing, modelled on Context7's server instructions ("Use
   even when you think you know the answer") [src: Context7 MCP server
   instructions, observed this session]: call these even when the answer
   seems known — the graph reflects the current code, assumptions may not.
</spec>

<steps>
1. **Failing test first.** In `handshake.rs` add `handshake_descriptions_carry_when_and_triggers`:
   spawn the client, `list_all_tools()`, assert all 13 tools present and
   every `tool.description` is `Some`, non-empty, and contains the literal
   `Use when `; read the server `InitializeResult` via the rmcp client
   peer-info accessor (`peer_info()` on the `RunningService`/`Peer` —
   confirm the exact accessor against rmcp 1.7.0 docs) and assert
   `.instructions` is `Some` and contains the token `grep`. Run
   `cargo nextest run -p ariadne-mcp` → MUST FAIL: current descriptions
   [src: server.rs:85-218] carry no `Use when ` clause; current
   instructions [src: server.rs:227-232] never mention `grep`.

2. **Rewrite 13 tool descriptions.** Edit each `#[tool(description = ...)]`
   in `server.rs` to the `<spec>` template, drawing the when-clause +
   triggers from `<trigger_map>`. Keep the leading "what" within one clause
   of the current wording; each string ≤320 chars. `description` literals
   only — no signature, no `Parameters<T>` type, no logic touched.

3. **Rewrite server instructions.** Replace the `with_instructions("...")`
   argument in `get_info` [src: server.rs:227-232] with a block meeting
   every clause of the `<spec>` instructions contract. One paragraph,
   ≤900 chars.

4. **Add regression snapshots.** In `handshake.rs` add a test that builds a
   `BTreeMap<String,String>` of name→`tool.description` and calls
   `insta::assert_snapshot!("tools_descriptions", ...)`, and a test that
   calls `insta::assert_snapshot!("server_instructions", instructions)`.
   Review each generated `.snap.new` by eye against `<spec>`, then
   `cargo insta accept`. These lock the strings against silent drift.

5. **Confirm input-schema golden unchanged.** `handshake__tools_list.snap`
   MUST stay byte-identical — `handshake.rs:17` snapshots
   `tool.input_schema`, never `tool.description` [src: handshake.rs:17;
   tier-14 step 10]. If `cargo insta` reports drift on `tools_list`, a
   non-string change leaked — stop and root-cause; do not `accept`.

6. **Verify + manual session.** Run the full gate (`<verification>`). Then
   run one real Claude Code session against a fixture repo (or the repo's
   own `.mcp.json`) with Ariadne configured: give a code-structure task
   whose lazy default is grep/Read (e.g. "what would break if I change
   `WriteTxn::apply`"), observe the agent call an Ariadne tool, paste the
   tool name + transcript excerpt into the tier-15 audit. If the agent
   still defaults to grep, the wording is too weak — strengthen and re-run;
   do not mark the tier done on a failed session.
</steps>

<verification>
- `cargo nextest run -p ariadne-mcp` — green: the step-1 content assertion
  now passes; the step-4 snapshots are accepted; all prior MCP tests pass.
- `cargo nextest run --workspace` — green: no other crate is touched.
- `cargo test --test architecture` — green: no new dependency or
  cross-crate edge; `ariadne-mcp` wiring is unchanged.
- `cargo build --workspace`, `cargo clippy --workspace --all-targets
  --all-features -- -D warnings`, `cargo fmt --all --check`,
  `RUSTDOCFLAGS=-D warnings cargo doc --workspace --no-deps
  --document-private-items` — clean (string literals only; no new public
  item, so `#![deny(missing_docs)]` is unaffected).
- Snapshots: `handshake__tools_descriptions.snap` and
  `handshake__server_instructions.snap` are NEW and reviewed;
  `handshake__tools_list.snap` is byte-unchanged. Any drift on `tools_list`
  fails loud — never `cargo insta accept` it without root cause.
- Manual session recorded in the tier-15 audit per step 6: a real
  agent-over-grep observation, not a snapshot diff. A failed session is
  root-caused, not silenced.
</verification>

<rollback>
`git revert` the `server.rs` description + instructions string edits, the
new `handshake.rs` tests, and the two new `.snap` files. All changes are
string literals + test-only — no on-disk format, no MCP input schema, no
API signature is touched, so reverting needs no migration and restores
tier-14's behaviour exactly.
</rollback>
