---
tier_id: tier-02
title: ariadne digest — compact, deterministic project digest for session bootstrap
deps: []
exit_criteria:
  - "`ariadne digest [root]` prints agent-friendly markdown to stdout: revision + counts, top coupled modules, a question→tool cheat-sheet."
  - "Output is bounded well under 10,000 chars and ends non-empty even when the graph is empty (minimal fallback)."
  - "The command resolves through the same daemon/cold path as `ariadne query`; warm daemon answers fast, cold falls back in-process."
  - "A golden-shape test asserts the markdown sections and the length bound on a fixture repo."
status: completed
completed: 2026-06-03
---

<context>
The SessionStart hook (tier-03) needs a single fast command that yields a compact,
agent-shaped project summary to inject as `additionalContext` (≤10k chars)
[src: https://code.claude.com/docs/en/hooks]. Raw `ariadne query` JSON is neither
compact nor agent-shaped, so this tier adds a dedicated projection composing
existing analytics — no new domain logic, no inference [src: plan.md D4;
feedback_no_llm_features]. Token-efficient, high-signal output follows Anthropic's
tool-output guidance [src: https://www.anthropic.com/engineering/writing-tools-for-agents].
</context>

<files>
- `crates/ariadne-cli/src/main.rs` — add `Cmd::Digest { root }` + dispatch
  [main.rs:32-105, 146-164].
- `crates/ariadne-cli/src/commands/digest.rs` — new; compose tool queries, format
  markdown.
- `crates/ariadne-cli/src/commands/query.rs` — factor out a `run_tool(root, tool,
  args) -> Result<serde_json::Value>` helper reused by `query` (prints) and
  `digest` (formats) [main.rs:152-156].
- `crates/ariadne-cli/src/commands/mod.rs` — register `digest`.
- `crates/ariadne-cli/tests/` — golden-shape + length-bound test on a fixture.
</files>

<steps>
1. **Failing test.** Build a small fixture repo (or reuse an existing test
   fixture), run `digest`, assert the output contains an `## Ariadne` heading, a
   `revision` line, a "Top modules" section, a "When to use which tool" cheat-sheet,
   and `output.len() < 10_000`. Run — fails (command absent).
2. **Refactor query.** Extract `commands::query::run_tool(root, tool, args_json)`
   returning the tool's `serde_json::Value` (daemon-first, cold fallback), and make
   the existing `query::run` format+print its result. No behavior change to `query`
   [src: main.rs:152-156].
3. **Implement digest.** `commands::digest::run(root)` calls `run_tool` for
   `project_status`, `coupling_report` (cap to top N modules), and
   `doc_for_project` (truncate to a short overview), then renders compact markdown:
   header with revision/counts, top-N coupled modules, and a fixed question→tool
   cheat-sheet (factual phrasing). Bound the assembled string under the cap; if any
   query errors or the graph is empty, emit a minimal "graph ready — run
   project_status" fallback [src: plan.md D4, R1; hooks 10k cap].
4. **Wire CLI.** Add `Cmd::Digest { root: PathBuf (default ".") }` and dispatch to
   `commands::digest::run` [src: main.rs:32-105,146-164].
5. **Bound + timeout.** Guard the daemon round-trips so a slow/cold daemon cannot
   hang session start: on timeout, return the minimal fallback (R1).
</steps>

<verification>
- `cargo nextest run -p ariadne-cli` — golden-shape + length test passes.
- `cargo clippy … -D warnings`; `cargo fmt --all --check`;
  `cargo test --test architecture` (CLI stays the only multi-adapter crate).
- Real run: `ariadne digest` in this repo prints the digest; eyeball that revision
  matches `ariadne status`, the module list is non-empty, length < 10k. Compare
  against the stated expectation; report the actual byte length.
- Fail loudly: if the digest exceeds the cap, the test fails — do not silently
  truncate mid-section; shorten the projection.
</verification>

<rollback>
Remove `commands/digest.rs`, the `Cmd::Digest` arm, and the `mod` registration;
revert the `query.rs` refactor (inline the helper back). Pure additive code.
</rollback>

<audit_followups>
- F1 (INFO, audit/tier-02-report.md) — resolved 2026-06-03. The original refactor
  routed `query` output through an order-less `serde_json::Value`, re-sorting keys
  alphabetically and contradicting step 2's "No behavior change to query".
  Fix: the shared `run_tool` now returns pretty JSON **text**, serialized straight
  from each tool's typed output struct (`serde_json::to_string_pretty`), so keys
  stay in declaration order (`revision` first); `query` prints it verbatim and
  `digest::fetch` re-parses it into a `Value` (field-name reads, order immaterial).
  Restores the exact pre-refactor `query` output and drops the redundant
  double-serialize on the hot path. Chosen over global `serde_json/preserve_order`
  (would churn ~10 MCP/graph snapshots, out of tier scope) and over merely softening
  the prose. Regression pinned by `tests/query.rs::query_preserves_struct_declaration_key_order`;
  warm/cold byte-parity re-confirmed by `cli_daemon_parity`.
</audit_followups>
