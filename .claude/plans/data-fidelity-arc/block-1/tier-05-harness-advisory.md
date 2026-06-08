---
tier_id: tier-05
title: Token-delta harness + discoverability + 25k-cap dogfood
deps: [tier-02, tier-03, tier-04]
exit_criteria:
  - "A deterministic `#[ignore]` token-delta harness (bytes/4 proxy, no clock/model) drives every one of the 10 capped tools at the default budget (concise, cap 50) vs an un-capped baseline (detailed, unbounded limit) and records the per-tool reduction in `economy-token-delta.md`."
  - "The harness asserts every tool's default output is ≤25k tokens on the ariadne_v2 self-index (BR6); a failure means lower that tool's default `limit`, not weaken the assertion."
  - "The MCP server `with_instructions` names the `verbosity` (concise default) + `cursor` affordances and stays ≤2KB; the regenerated handshake snapshot is accepted."
  - "CLAUDE.md's Ariadne tool-list notes the `verbosity`/`cursor` affordances (listing only, no prose rewrite); clippy `-D warnings`, fmt, `cargo test --test architecture` green."
status: pending
---

<context>
Cross-cutting proof + discoverability, mirroring `context-efficient-read` tier-04 (which shipped a
bytes/4 token-delta harness + a `with_instructions`/CLAUDE.md note) [src:
.claude/plans/context-efficient-read/tier-04-outline-adoption.md;
.claude/plans/context-efficient-read/outline-token-delta.md]. No new per-tool logic and no new
architectural decision: this tier measures the block's effect and advertises the affordances, so it
records the validated default cap as an addendum to ADR-0029 rather than a new ADR. Confirms the
seed's success metric: every growable tool deterministically bounded, no tool over 25k [src:
.claude/plans/data-fidelity-arc/block-1/plan.md `<verification>`; seed `<verification_intent>`]. Full
context: `plan.md`.
</context>

<files>
- `crates/ariadne-e2e/tests/economy_token_delta.rs` — NEW `#[ignore]` deterministic harness: for each
  of the 10 tools, drive the cold `tools::*::handle` twice — baseline (`verbosity:detailed`,
  `limit:u32::MAX`, no cursor) and default (`verbosity:concise`, default cap) — record
  `bytes`/`bytes/4` and the reduction; assert default ≤25k tokens.
- `.claude/plans/data-fidelity-arc/block-1/economy-token-delta.md` — the recorded report artifact
  (deterministic; index revision noted, no timestamp), the sibling of `outline-token-delta.md`.
- `crates/ariadne-mcp/src/server.rs` — extend the `with_instructions` text to name the `verbosity`
  (concise default) + `cursor` paging affordances, within the 2KB cap [src:
  crates/ariadne-mcp/src/server.rs:745-761].
- `crates/ariadne-mcp/tests/snapshots/handshake__server_instructions.snap` — accept the regenerated
  snapshot.
- `CLAUDE.md` — in the "Ariadne code intelligence" tool list, note that the growable tools take
  `verbosity` (concise default) + page with an opaque `cursor` (listing/affordance note, the
  scope-allowed edit precedent — not a prose rewrite) [src: context-efficient-read tier-04 step 5].
- `docs/adr/0029-response-economy-cursor-verbosity.md` — append the measured default-cap validation
  addendum.
</files>

<steps>
1. **Failing harness first.** Write `economy_token_delta.rs` asserting every default-budget tool
   output is ≤25k tokens. Run (`--ignored`) — it exercises the real handlers; any tool still over 25k
   fails, pinpointing a too-high default `limit` to lower (BR6) [src: plan.md D4,BR6].
2. **Record the deltas.** For each tool, baseline (detailed, unbounded) vs default (concise, cap):
   record per-tool bytes + bytes/4 + reduction and the median in `economy-token-delta.md`. Deterministic
   — fixed index, no wall-clock, no model [src: outline-token-delta.md method].
3. **Server instructions.** Add a clause to `with_instructions` naming `verbosity`/`cursor`; assert
   the total stays ≤2KB in the existing handshake test; accept the regenerated snapshot [src:
   server.rs:745-761; context-efficient-read tier-04 step 4].
4. **CLAUDE.md note.** Extend the tool-list bullets to mention `verbosity`/`cursor` on the growable
   tools — listing only, no prose rewrite [src: context-efficient-read tier-04 step 5].
5. **ADR addendum.** Append the measured default-cap=50 ≤25k validation to ADR-0029.
6. **Dogfood.** Re-run the worst pre-block offenders (`co_change` low-threshold 733k, `hotspots`
   symbol 311k, `complexity` symbol 291k, `blast_radius` 46k) at default and confirm each is now
   ≤25k + pageable; report the reductions in the tier notes.
</steps>

<verification>
- `cargo nextest run -p ariadne-e2e -E 'test(economy_token_delta)' --run-ignored all` — the harness
  runs and the ≤25k assertion passes for all 10 tools; record the median reduction (reported).
- `cargo nextest run -p ariadne-mcp -E 'test(handshake)'` — instructions ≤2KB; snapshot accepted.
- `cargo test --test architecture`; clippy `-D warnings`; `cargo fmt --all --check`.
- Manual: in a fresh MCP session, confirm the server instructions name `verbosity`/`cursor` and a
  growable tool returns a concise, capped page with a usable cursor. Report the measured before/after.
  If not runnable in-session, report only the deterministic harness numbers — never fabricate a ratio
  [src: CLAUDE.md validate-by-execution].
</verification>

<rollback>
Revert the harness + report artifact, the `with_instructions` clause + snapshot, the CLAUDE.md note,
and the ADR-0029 addendum. Config/doc/test only — tiers 01–04 (the capability) stay intact.
</rollback>
