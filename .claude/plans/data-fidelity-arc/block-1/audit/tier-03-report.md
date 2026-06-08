---
tier_id: tier-03
audited: 2026-06-08
verdict: PASS
commit: 99e0f3a11b2f6d8034b3822a4e7a76f95ab540f4
---

<scope>
Tier-03 rolls the `ariadne_graph::economy` helper (ADR-0029) out to the three
*multi-list* growable tools: `blast_radius` (`must_touch`/`may_touch`),
`weak_spots` (`cycles`/`god_modules`/`dead_symbols`), and
`refactor_suggestions` (`god_modules`/`cycle_breaks`/`misplaced_symbols`). The
one new mechanism is `economy::{paginate_sublist, multi_cursor,
multi_truncation_note}` — each sublist windows against its own `offsets[i]`, one
shared `next_cursor` is emitted iff any sublist has a remainder, and `MAX_DEAD`
is removed so the dead-code remainder is reachable (ADR-0030).

Diff is the uncommitted working tree on HEAD 99e0f3a (which bundles the
already-PASSED tiers 01–02). Every tier `<files>` entry is touched as intended:
`economy.rs` (+4 helpers, +5 tests), core `query.rs`/`response.rs` (3 variants +
3 reports), `mcp/types.rs` (3 inputs/outputs + 6 `From` impls), the 3 cold
handlers, `mcp/server.rs` (`#[tool]` params + `project_daemon` + parity tests),
daemon `impact.rs`/`health.rs`/`refactor.rs` warm handlers + `dispatch.rs`
wiring, `cli/query.rs`, and NEW `docs/adr/0030-multi-list-pagination.md`.
Justified out-of-`<files>` edits: `ariadne-graph/src/lib.rs` (façade re-export),
two benches + three daemon test files (signature/oracle updates), and the
`handshake__tools_list.snap` schema snapshot. All in scope or mechanically
required by the protocol change.
</scope>

<checks_run>
- Read every changed file end-to-end (3 cold handlers, 3 warm handlers, core
  DTOs, `types.rs` `From` impls, server/cli wiring, ADR-0030, the 3 integration
  test files, daemon oracle updates, schema snapshot).
- `cargo fmt --all --check` → clean (exit 0).
- `cargo nextest run -p ariadne-mcp -E 'test(blast_radius)|test(weak_spots)|test(refactor)'`
  → 15/15 passed (per-sublist cap, single multi-list cursor round-trip
  completeness, concise field-set, cold==warm arm parity).
- `cargo nextest run -p ariadne-daemon` → 30/30 passed, incl. the three cold
  oracles `blast_radius_matches_cold_golden`, `weak_spots_matches_cold`,
  `refactor_suggestions_matches_cold` (warm == cold, now over the sorted/paged
  shape), plus the memory probe and incremental-rebuild.
- `cargo test --test architecture` → 1/1 passed (hexagonal invariants hold; no
  driving→driving, economy stays in `ariadne-graph`).
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` → clean.
- Dogfood (freshly-built `cargo build -p ariadne-cli`, cold path in a
  daemon-less temp copy of the live index, revision 1330): `blast_radius FileId`
  default concise = 13,252 B ≈ 3.3k tok with cursor + note "Showing 3 of 546
  must_touch, 3 of 216 may_touch …"; uncapped detailed = 167,003 B ≈ 41.8k tok
  (1.7× over the 25k cap — the headline overflow the tier targets). ~12.6×
  reduction. A page round-trip advanced the shared cursor offsets [3,3] → [6,6]
  (cursor decodes revision 0x0532 = 1330 = the live catalog revision, len 2); a
  hand-crafted bad cursor returned "malformed pagination cursor", never a panic
  or silent mis-page.
- Verified the parity hinge: the wire `SymbolSummary::from(core)` (types.rs:59)
  is a straight field copy, so the warm handlers' concise `None` propagates
  through postcard → `From` → `skip_serializing_if`, matching the cold path that
  nulls the wire type directly. Confirmed sort runs inside `paginate_sublist`
  *before* `project` nulls `byte_start`, so the `(file, byte_start, name)`
  tie-break reads real offsets on both paths (handler comments + code).
- Cross-checked every comparator against the tier's documented sort keys: blast
  `(file, byte_start, name)`; cycles (first member, size); god (efferent desc,
  module asc); dead `(file, byte_start, name)`; cycle_breaks (score desc,
  `(from,to)`); misplaced (ratio desc, symbol) — cold and warm twins identical.
</checks_run>

<findings>
| id | category | severity | location | problem | fix |
|----|----------|----------|----------|---------|-----|
| I1 | docs | INFO | crates/ariadne-mcp/src/types.rs:482-483 | `ScopeInput`'s doc still says it is "shared by `coupling_report`, `weak_spots`, `doc_for_project`, and `refactor_suggestions`", but after tiers 02–03 only `doc_for_project` consumes it — contradicting the new `WeakSpotsInput`/`RefactorInput` docs that call it "the other `ScopeInput` consumer (`doc_for_project`)" (singular). | Narrow the doc to name only `doc_for_project`. |
</findings>

<verdict>
PASS. Zero FAIL findings; one non-gating INFO.

Every exit criterion is independently verified:
- EC1 (each sublist caps independently; one `next_cursor` iff any sublist has a
  remainder; `note` names the truncated lists): `multi_cursor` emits `Some` iff
  any `(_, remainder)` is true and carries every sublist's offset; the three
  `*_caps_and_round_trips` tests + the dogfood note ("3 of 546 must_touch, 3 of
  216 may_touch") confirm it.
- EC2 (round-trip returns each non-exhausted sublist's remainder; per-list union
  == un-capped): the three round-trip tests reconstruct each list with no
  gap/dup; the economy unit test `multi_list_one_cursor_pages_every_sublist_
  completely` proves an exhausted sublist re-pages to empty; dogfood offsets
  advance [3,3]→[6,6].
- EC3 (documented stable key per sublist; concise drops embedded `SymbolSummary`
  id/offsets on blast/dead, equals detailed for name/metric lists): comparators
  match the documented keys on both paths; `blast_radius_concise_drops_cryptic_
  fields`, `weak_spots_concise_drops_dead_ids_only`, and
  `refactor_concise_equals_detailed` all pass.
- EC4 (cold==warm==CLI; ADR-0030; clippy/fmt/architecture/dogfood green): the
  three daemon cold-oracle parity tests pass over the paged shape; CLI and MCP
  warm both `From`-project the same way; ADR-0030 records the per-sublist-offset
  cursor + `MAX_DEAD` supersession; all toolchain gates and the dogfood green.

No new dependency, no new crate, no redb schema change; the cursor codec is
reused unchanged. `MAX_DEAD` removal is intentional and ADR-recorded — the
remainder is now reachable, satisfying the "truncation reported, never silent"
arc constraint. The `#[allow(clippy::too_many_arguments/lines)]` attributes are
justified inline and clippy passes.
</verdict>

<next_steps>
None block acceptance. Optional: fix INFO I1 (one-line doc edit) opportunistically
on the next `types.rs` touch. For the eventual commit, stage only the
tier-03-attributable files (the working tree also carries the modified tier file
itself); the `.claude/skills/*` edits noted in tier-02 are unrelated. Tier-04
(`diff_blast_radius`) may proceed in a fresh `/spec-build` session and reuses the
`paginate_sublist`/`multi_cursor` shape per ADR-0030.
</next_steps>

<sources>
- Tier file: .claude/plans/data-fidelity-arc/block-1/tier-03-multi-list-rollout.md
- Block plan + decisions: .claude/plans/data-fidelity-arc/block-1/plan.md (D1–D6, BR5)
- Mechanism ADRs: docs/adr/0029-response-economy-cursor-verbosity.md ; docs/adr/0030-multi-list-pagination.md
- MCP opaque-cursor model (list-ops-only → self-cursor): https://modelcontextprotocol.io/specification/2025-06-18/server/utilities/pagination
- Concise≈⅓ tokens, steer-on-truncate, 25k cap: https://www.anthropic.com/engineering/writing-tools-for-agents
- Reviewer standard (ship code-health over perfection): https://google.github.io/eng-practices/review/reviewer/standard.html
</sources>
