---
tier_id: tier-09
title: Wire search_code/read_symbol into advisory + instructions + CLAUDE.md; re-measure token delta
deps: [tier-05, tier-07, tier-08]
exit_criteria:
  - "The `ariadne-grep-advisor.sh` advisory (tier-04) names `search_code` for symbol-pattern Grep/Glob and `read_symbol` for symbol-targeted whole-file Reads, while still only ever returning `allow` (never `deny`)."
  - "The MCP server `with_instructions` and the CLAUDE.md \"Ariadne code intelligence\" tool list both gain a Search/Read group listing `search_code` + `read_symbol`; server instructions stay within the 2KB cap."
  - "The tier-05 adoption harness tallies the two new tool names, and a recorded re-run reports the real-tool token delta versus the tier-06 spike estimate."
  - "Advisor classification tests, harness wiring, clippy `-D warnings`, fmt, and `cargo test --test architecture` are green; the behavioural ratio is reported, not gated."
status: completed
completed: 2026-06-03
---

<context>
Tiers 07–08 shipped the capability; this tier closes the loop opened by the spike
and connects the new tools to the adoption machinery. The tier-04 `PreToolUse`
advisory currently nudges symbol-shaped greps toward `find_references`/
`list_symbols`; it should now route them to `search_code`, and route symbol-
targeted whole-file `Read`s to `read_symbol` — staying advisory (`allow` +
`additionalContext`), never blocking [src: tier-04; plan.md D5, R5]. The server
`with_instructions` and the CLAUDE.md tool list must advertise the new tools so
they are discoverable (a scope-allowed CLAUDE.md edit: listing tools, not
rewriting prose) [src: plan.md `<context>` out-of-scope clause; server.rs
with_instructions]. Finally, re-measure the token delta with the real tools to
confirm the spike's projection [src: tier-05; tier-06; D11].
</context>

<files>
- `crates/ariadne-cli/src/commands/setup.rs` — update the advisor script template
  (suggestion text + Read classification) [src: tier-04 files].
- `<root>/.claude/hooks/ariadne-grep-advisor.sh` — this repo's installed copy.
- `crates/ariadne-mcp/src/server.rs` — extend `with_instructions` (Search/Read
  group), within the 2KB cap [src: server.rs:593].
- `crates/ariadne-mcp/tests/snapshots/handshake__server_instructions.snap` — accept
  the regenerated instructions snapshot (the `with_instructions` edit changes it).
- `CLAUDE.md` — add a "Search / Read" bullet to the Ariadne tool list.
- `crates/ariadne-e2e/tests/adoption_harness.rs` — extend the `Tally` struct to count
  `search_code`/`read_symbol` + reuse the tier-06 deterministic token-delta method
  against the real tools [src: adoption_harness.rs:62-69].
- `crates/ariadne-cli/tests/` — advisor classification cases for the new routing
  (advisor matcher `Grep|Glob|Read`) [src: adoption_wiring.rs:35].
</files>

<steps>
1. **Failing test.** Feed the advisor representative payloads: (a) a `Grep` for an
   identifier/CamelCase pattern → expect `additionalContext` naming `search_code`;
   (b) a `Read` of a `.rs` file whose path holds a known symbol → expect
   `read_symbol` suggested; (c) a quoted-string `Grep` or `.md` Read → pass-through,
   empty context. Run — fails (advisor still names only the old tools).
2. **Update the advisor.** Extend the symbol-shaped heuristic and suggestion text:
   pattern-shaped Grep/Glob → `search_code` (plus `find_definition`/
   `find_references` as today); whole-file Read of a source file → `read_symbol`.
   Keep `permissionDecision:"allow"` on match, `defer` otherwise; never `deny`
   (D5, R5) [src: tier-04 steps 2–3].
3. **Install via setup.** Update the template string in `setup.rs`; keep the install
   idempotent and the Bash audit-gate PreToolUse entry intact [src: tier-04 step 4].
4. **Server instructions.** Add a concise Search/Read line to `with_instructions`
   [src: server.rs:593]; assert the total stays ≤2KB in a test and accept the
   regenerated `handshake__server_instructions.snap` [src: plan.md `<constraints>`
   2KB cap].
5. **CLAUDE.md list.** Add "Search / Read — `search_code`, `read_symbol`. Use to
   find code by pattern and read a symbol's source without reading whole files."
6. **Re-measure.** Extend the tier-05 `Tally` to count `mcp__ariadne__search_code`/
   `read_symbol`; re-run the tier-06 deterministic token-delta against the real
   tools; record the numbers and compare to the spike estimate in this tier's notes
   [src: adoption_harness.rs:62-69].
7. **Signal.** If the new tools see low real adoption, note escalating the advisory
   from `allow` toward `ask` as a follow-up plan (not this tier) [src: tier-05 step 5].
</steps>

<verification>
- `cargo nextest run -p ariadne-cli` — new advisor classification cases pass; re-run
  `ariadne setup` is idempotent and preserves the audit-gate hook.
- `cargo nextest run -p ariadne-e2e` — the wiring/harness compiles; the ignored
  behavioural harness stays opt-in. Manual run records the real-tool token delta.
- `cargo test --test architecture`; clippy `-D warnings`; fmt check; assert the
  server instructions byte length ≤2KB; `cargo nextest run -p ariadne-mcp -E
  'test(handshake)'` passes with the accepted `handshake__server_instructions.snap`.
- Real run: in a fresh session here, grep for an existing symbol → confirm the
  advisory now names `search_code`; report it. If not runnable in-session, say so
  and report only the deterministic results — never fabricate a ratio.
</verification>

<rollback>
Revert the advisor template + installed script, the `with_instructions` lines, the
CLAUDE.md bullet, and the harness extension. Config + doc + test only; no product
data path changes, so tiers 07–08 remain intact.
</rollback>

<notes>
Real-tool token-delta re-measure (`adoption_harness.rs::real_tool_token_delta_vs_grep`,
`#[ignore]`, run manually over this repo's live index, revision 560). Baseline = Σ grep
matching-line bytes for the symbol across the indexed corpus + whole-file Read of the
defining file; prototype = real `search_code` output bytes + real `read_symbol`
(`context` mode) output bytes; token proxy = bytes/4 (matches the tier-06 spike method).

| Shape | Symbol | Baseline tok | Proto tok | Reduction |
|-------|--------|-------------:|----------:|----------:|
| find-definition | Catalog | 4263 | 945 | 77.8% |
| find-definition | RedbStorage | 6895 | 210 | 96.9% |
| find-definition | SymbolSummary | 6645 | 342 | 94.8% |
| search-by-pattern | doc_for | 1665 | 908 | 45.4% |
| search-by-pattern | summarize | 1439 | 402 | 72.0% |
| search-by-pattern | handle | 76929 | 1932 | 97.4% |
| read-body | find_symbol | 4330 | 210 | 95.1% |
| read-body | build | 7813 | 1536 | 80.3% |

Median real-tool reduction across 8 tasks: **87.5%** — confirms the tier-06 spike's
87.3% projection (≥40% D11 threshold) with the shipped tools rather than the hand-rolled
prototype. The two arms run over the same indexed corpus; targets resolve in the live
catalog (asserted by `read_symbol`).

Signal (step 7): the deterministic delta confirms the value; real behavioural adoption is
measured by the `#[ignore]` `adoption_ratio_baseline_vs_treated` harness (now tallying
`search_code`/`read_symbol`), reported not gated. If a recorded behavioural run shows low
real uptake of the two new tools, escalating the advisory from `allow` toward `ask` is a
follow-up plan, not this tier [src: tier-05 step 5; plan.md D5].
</notes>
