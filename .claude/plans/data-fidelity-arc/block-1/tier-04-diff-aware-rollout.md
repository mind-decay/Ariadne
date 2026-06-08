---
tier_id: tier-04
title: Roll the economy helper out to the diff-aware tools
deps: [tier-01, tier-03]
exit_criteria:
  - "`affected_tests` (tests/seeds) and `diff_blast_radius` (seeds/must/may) cap their top-level lists with the multi-list cursor from tier-03 and report `next_cursor` + `note` when truncated."
  - "`diff_blast_radius` per-seed inner `must_touch`/`may_touch` are bounded by a documented fixed cap (= `limit`) with a per-seed count note — bounded and reported, never silently dropped, and never a combinatorial nested cursor."
  - "The diff-aware cursor is stamped with the index revision AND a changed-paths fingerprint; a changed working-tree diff between pages yields a graceful invalid-cursor error, not wrong rows."
  - "`affected_tests` parity holds across MCP-cold, MCP-warm, and its dedicated CLI command; `diff_blast_radius` parity holds across MCP-cold and MCP-warm (it has no CLI `query` twin); ADR-0031 records the diff-aware decisions; clippy `-D warnings`, fmt, architecture, dogfood green."
status: pending
---

<context>
The two tools whose changeset comes from `ariadne_git::diff` at the composition root, so their result
set depends on (index revision + working-tree diff), not the revision alone [src:
crates/ariadne-mcp/src/tools/{diff_blast,affected_tests}.rs:1-12; crates/ariadne-cli/src/commands/query.rs:59-64].
`diff_blast_radius` is the only growable tool with a *nested* shape (a list of seeds, each carrying
two inner lists) and is MCP-only (no `ariadne query` twin) [src: query.rs:214-217]. `affected_tests`
has a dedicated CLI command, not the generic dispatch [src: query.rs:59-64;
crates/ariadne-cli/src/commands/affected_tests.rs]. Reuses tier-01/tier-03 mechanism (cite ADR-0029,
0030); the new decisions (diff fingerprint in the cursor; per-seed fixed inner cap) are ADR-0031.
Full context: `plan.md`.

Top-level sublists + stable keys:
- `affected_tests` — `tests`, `seeds`; each `(file, byte_start, name)` asc [src: affected_tests.rs:89-101].
- `diff_blast_radius` — `seeds` (by seed symbol `(file, byte_start, name)`), aggregate `must_touch`,
  aggregate `may_touch` (each `(file, byte_start, name)`) [src: diff_blast.rs:76-102].
</context>

<files>
- `crates/ariadne-graph/src/economy.rs` — add a cursor stamp field for a diff fingerprint (revision +
  a cheap hash/count of `changed_paths`), or a variant of `Cursor` carrying it; reused by both tools.
- `crates/ariadne-core/src/domain/daemon/query.rs` — add `limit`/`cursor`/`verbosity` to the
  `AffectedTests` and `DiffBlast` variants.
- `crates/ariadne-core/src/domain/daemon/response.rs` — add `next_cursor`/`note` to both payloads and a
  per-seed inner-truncation count on `DiffSeedRow`.
- `crates/ariadne-mcp/src/types.rs` — economy params on `DiffBlastInput` + `AffectedTestsInput`;
  `next_cursor`/`note` on both outputs; per-seed inner count on `DiffSeedRow`.
- `crates/ariadne-mcp/src/tools/{diff_blast,affected_tests}.rs` — sort + paginate the top-level lists;
  inner-cap each seed's must/may at `limit` with a count; concise projection on the `SymbolSummary`
  rows.
- `crates/ariadne-mcp/src/server.rs` — both `#[tool]` methods + `project_daemon` arms (git diff still
  runs in the handler before the call) [src: server.rs:300-315 pattern].
- `crates/ariadne-daemon/src/domain/queries/` — warm handlers for both call the same `paginate`.
- `crates/ariadne-cli/src/commands/affected_tests.rs` — thread the params into `run_query` (the
  dedicated CLI path); `diff_blast_radius` needs no CLI change (MCP-only).
- `docs/adr/0031-diff-aware-pagination.md` — NEW (template).
</files>

<steps>
1. **Failing tests first.** `affected_tests`: default caps tests/seeds + cursor; round-trip union ==
   un-capped; cold == warm == CLI. `diff_blast_radius`: top-level seeds/must/may cap + cursor;
   per-seed inner must/may bounded at `limit` with a count; cold == warm. Cursor with a stale diff
   fingerprint → invalid-cursor error. Run — fails.
2. **Economy diff stamp.** Add the changed-paths fingerprint to the cursor (revision + hash/count);
   decode rejects a mismatch gracefully (−32602-style), since a changed diff is a different result set
   [src: plan.md D2,BR1,BR5; MCP "handle invalid cursors gracefully"].
3. **Core protocol + DTOs.** Params on both variants; `next_cursor`/`note` on both payloads; per-seed
   inner count on `DiffSeedRow`.
4. **Cold handlers.** Sort + paginate the top-level lists; inner-cap each seed's must/may at `limit`
   with a reported count (no silent drop); concise projection [src: diff_blast.rs:76-102;
   affected_tests.rs:89-101].
5. **Warm + server + CLI.** Mirror on the warm path; wire both `#[tool]` methods + `project_daemon`;
   thread params into `commands/affected_tests.rs::run_query`. Keep cold == warm byte-identical.
6. **ADR-0031 + dogfood.** Record the fingerprint + per-seed inner-cap decisions; exercise both tools
   on a real working-tree diff in this repo; report tokens + confirm pages reconstruct the full set.
</steps>

<verification>
- `cargo nextest run -p ariadne-mcp -E 'test(diff_blast) | test(affected_tests)'` — top-level cap +
  cursor, nested inner-cap count, stale-fingerprint reject, parity.
- `cargo nextest run -p ariadne-cli -E 'test(affected_tests)'`; `cargo nextest run -p ariadne-daemon`;
  `cargo test --test architecture`; clippy `-D warnings`; `cargo fmt --all --check`.
- Manual: stage a diff, run `ariadne affected-tests` (CLI) and the `diff_blast_radius` MCP tool;
  confirm capping + paging + the stale-fingerprint guard. Report tokens. If not runnable in-session,
  say so — never fabricate [src: CLAUDE.md].
</verification>

<rollback>
Revert both handlers + warm handlers, the protocol/DTO additions, the economy diff-fingerprint
addition, the server wiring, and the `commands/affected_tests.rs` params, plus ADR-0031. Tiers 01–03
stay intact.
</rollback>
