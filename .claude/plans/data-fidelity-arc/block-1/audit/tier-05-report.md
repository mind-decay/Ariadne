---
tier_id: tier-05
audited: 2026-06-09
verdict: PASS
commit: 4f5855d6a031db99250a8a91381b1b61f9b6a007
---

<scope>
Tier-05 (Token-delta harness + discoverability + 25k-cap dogfood) of
`data-fidelity-arc/block-1`. Reviewed the working-tree diff on top of HEAD
`4f5855d` (the tier-05 changes are uncommitted). Files in scope, all present and
within the tier's `<files>` (as reconciled by the `<deviations>` block that
re-homes the harness from `ariadne-e2e` to `ariadne-mcp`):

- `crates/ariadne-mcp/tests/economy_token_delta.rs` (NEW, 464 lines) — the
  `#[ignore]` deterministic harness.
- `.claude/plans/data-fidelity-arc/block-1/economy-token-delta.md` (NEW) — the
  recorded report artifact.
- `crates/ariadne-mcp/src/server.rs` — `with_instructions` clause naming the
  `verbosity`/`next_cursor` affordances.
- `crates/ariadne-mcp/tests/snapshots/handshake__server_instructions.snap` —
  regenerated snapshot.
- `CLAUDE.md` — the "Economy" tool-list bullet.
- `docs/adr/0029-response-economy-cursor-verbosity.md` — measured-default-cap
  `<validation>` addendum.
- `.claude/plans/data-fidelity-arc/block-1/tier-05-harness-advisory.md` — the
  tier file itself (status/deviation update).

`git status` confirms nothing outside this set was touched.
</scope>

<checks_run>
- `mcp__ariadne__project_status` — index revision 1461, matching the report
  artifact's stamped revision. Graph fresh.
- `cargo fmt --all --check` — exit 0 (clean).
- `cargo nextest run -p ariadne-mcp -E 'test(economy_token_delta)' --run-ignored all`
  — PASS (the deviation's documented command). The ≤25k-token assertion holds
  for all 10 tools.
- Determinism: hashed `economy-token-delta.md` before and after the harness run
  (`shasum` 1e44a2b…) — byte-identical, so the report is reproducible at a fixed
  index revision as claimed.
- `cargo nextest run -p ariadne-mcp -E 'test(handshake)'` — 5/5 PASS, including
  the `server_instructions` snapshot and the `instructions.len() <= 2048`
  assertion (`handshake.rs:161`); measured instructions = 1315 bytes.
- `cargo test --test architecture` — PASS (re-home to `ariadne-mcp/tests/` keeps
  rule 4 green: a crate's own integration test linking itself is not a
  driving→driving workspace dependency).
- `cargo clippy -p ariadne-mcp --all-targets --all-features -- -D warnings` —
  exit 0; the new harness compiles clean under `-D warnings` (its local
  `#![allow]` set is scoped to the throwaway measurement file).
- Manual in-session MCP check: `mcp__ariadne__find_references SymbolId` returned
  a flat, un-paged, fully-detailed array (no `next_cursor`/`note`, raw
  ids/offsets, all ~165 sites) — confirming the deviation's statement that the
  running MCP binary predates the economy work, so the new caps are validated
  through the harness, not the live server (the tier's verification clause
  permits exactly this fallback).
- Re-ran the literal `<verification>` line-64 command verbatim
  (`cargo nextest run -p ariadne-e2e -E 'test(economy_token_delta)' --run-ignored all`):
  it selects 0 tests ("error: no tests to run", exit 0) because the harness was
  re-homed; see INFO-2.
- Reconciled every quoted figure in the ADR addendum and report against the
  freshly regenerated `economy-token-delta.md` (rev 1461): all byte/token
  numbers and the 88.7% median match exactly; the harness's reduction and median
  arithmetic was re-derived by hand and agrees.
</checks_run>

<findings>
| id | category | severity | location | problem | fix |
|---|---|---|---|---|---|
| INFO-1 | docs | INFO | `docs/adr/0029-response-economy-cursor-verbosity.md` (validation block, "revision 1453") | The addendum says the harness ran on "revision 1453", but its cited report and every quoted figure (585k/203k/201k/20.5k, 10.5k, 88.7%) are the revision-1461 numbers — the revision label is stale, not the data. | Change "revision 1453" to "revision 1461" to match the cited report artifact. |
| INFO-2 | docs | INFO | `tier-05-harness-advisory.md:64` | The `<verification>` block still names `-p ariadne-e2e`, contradicting the user-approved `<deviations>` that re-homed the harness to `ariadne-mcp`; run verbatim it selects zero tests ("no tests to run") instead of exercising the harness. | Update the line-64 command to `-p ariadne-mcp` so the documented verification matches the shipped home. |
</findings>

<verdict>
PASS. Zero FAIL findings. All four `exit_criteria` are independently verified:

1. The deterministic `#[ignore]` harness (bytes/4 proxy, no clock/model/network;
   byte-copy of the live index; fixed inputs) drives all 10 capped tools at the
   default budget vs. an un-capped baseline and records per-tool reductions in
   `economy-token-delta.md`. Re-run green; report byte-identical on re-run.
2. The harness asserts every tool's default page ≤25k tokens
   (`economy_token_delta.rs:388`) with a message steering toward lowering the
   tool's `limit` on failure (BR6). All 10 pass; the largest default is
   `refactor_suggestions` at 10 528 tokens.
3. `with_instructions` now names `verbosity` (concise default) + the opaque
   `next_cursor`, stays within 2KB (1315 B, asserted), and the regenerated
   handshake snapshot is accepted (5/5 handshake tests PASS).
4. CLAUDE.md's tool list gains an "Economy" bullet listing the 10 growable tools
   and the `verbosity`/`next_cursor` affordances (listing only); clippy
   `-D warnings`, fmt, and `cargo test --test architecture` are all green.

The dogfood numbers (step 6) check out: the worst pre-block offenders collapse
to low single-digit-k tokens at the default budget (co_change 585k→2.0k,
hotspots 203k→2.0k, complexity 201k→1.7k, blast_radius 20.5k→1.9k), all pageable.

Both findings are documentation-accuracy nits (a stale revision integer and a
stale verification command), each fully reconciled by the user-approved
deviation and the regenerated artifact. Neither violates a non-negotiable, an
exit criterion, or a budget, so neither gates the verdict.
</verdict>

<next_steps>
Optional, non-blocking (do not reopen the tier for these):
- INFO-1: correct "revision 1453" → "revision 1461" in the ADR-0029 addendum.
- INFO-2: update tier-05 `<verification>` line 64 from `-p ariadne-e2e` to
  `-p ariadne-mcp` to match the re-homed harness.
Operational note (out of tier scope): the live MCP/daemon binary is a
pre-economy build; restart it so callers actually receive the capped, paged
responses this block ships.
</next_steps>

<sources>
- Tier file: `.claude/plans/data-fidelity-arc/block-1/tier-05-harness-advisory.md`
- Plan: `.claude/plans/data-fidelity-arc/block-1/plan.md` (D4, D5, BR6)
- ADR-0029: `docs/adr/0029-response-economy-cursor-verbosity.md`
- Report artifact: `.claude/plans/data-fidelity-arc/block-1/economy-token-delta.md`
- 25k tool-result cap + concise/paginate guidance: https://www.anthropic.com/engineering/writing-tools-for-agents
- MCP pagination (opaque cursor, list-ops only): https://modelcontextprotocol.io/specification/2025-06-18/server/utilities/pagination
- Reviewer standard: https://google.github.io/eng-practices/review/reviewer/standard.html
</sources>
</content>
</invoke>
