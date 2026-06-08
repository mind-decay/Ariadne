---
tier_id: tier-03
title: Roll the economy helper out to the multi-list tools
deps: [tier-01]
exit_criteria:
  - "`blast_radius` (must/may), `weak_spots` (cycles/god/dead), `refactor_suggestions` (god/cycle/misplaced) each cap every sublist independently and carry one `next_cursor` (present when ANY sublist has a remainder) + a `note` naming which lists were truncated."
  - "A round-trip over the multi-list cursor returns the remaining items of each non-exhausted sublist and the per-list unions equal the un-capped lists (completeness across sublists)."
  - "Each sublist truncates by its documented stable key (below); concise drops embedded `SymbolSummary` id/offsets (blast_radius, weak_spots dead) and equals detailed for name/metric-only lists (refactor, cycles, god_modules)."
  - "Cold == warm == CLI `query` JSON for all three tools; ADR-0030 records multi-list cursor semantics; clippy `-D warnings`, fmt, `cargo test --test architecture`, dogfood green."
status: pending
---

<context>
Extends tier-01's cursor to tools that return several lists at once. The one mechanism change is the
cursor's `offsets` vector carrying a per-sublist offset (length = number of lists), recorded in
ADR-0030 (next free after tier-01's 0029) [src: .claude/plans/data-fidelity-arc/block-1/plan.md D2,
BR5]. `weak_spots.dead_symbols` already hard-caps at `MAX_DEAD=16` with no cursor — this replaces it
with the helper's cap + cursor so the remainder is reachable [src:
crates/ariadne-mcp/src/tools/weak_spots.rs:20,92-98]. `blast_radius` measured at 45–46k tok (1.8×
over) is the headline fix here [src: this-session measurements]. Full context: `plan.md`.

Sublists + stable sort keys:
- `blast_radius` — `must_touch`, `may_touch`; each `(file, byte_start, name)` asc [src:
  crates/ariadne-mcp/src/tools/blast_radius.rs:48-61].
- `weak_spots` — `cycles` (by first member then size), `god_modules` (efferent desc, module asc),
  `dead_symbols` (`(file, byte_start, name)` asc) [src: weak_spots.rs:43-105].
- `refactor_suggestions` — `god_modules` (efferent desc), `cycle_breaks` (score desc, then `(from,to)`),
  `misplaced_symbols` (ratio desc, then symbol) [src: refactor.rs:33-71].
</context>

<files>
- `crates/ariadne-graph/src/economy.rs` — extend (or confirm tier-01 already shaped) `paginate` /
  `Cursor` for a multi-sublist call: each list paginated against its own `offsets[i]`; helper to
  assemble one `next_cursor` set when any sublist has a remainder.
- `crates/ariadne-core/src/domain/daemon/query.rs` — add `limit`/`cursor`/`verbosity` to the
  `BlastRadius`, `WeakSpots`, and `RefactorSuggestions` variants.
- `crates/ariadne-core/src/domain/daemon/response.rs` — add `next_cursor`/`note` to the three
  payloads.
- `crates/ariadne-mcp/src/types.rs` — economy params on `BlastRadiusInput` + the refactor/weak_spots
  inputs (dedicated paginated input or flatten, keeping non-paginated `ScopeInput` consumers
  untouched); `next_cursor`/`note` on the three outputs.
- `crates/ariadne-mcp/src/tools/{blast_radius,weak_spots,refactor}.rs` — sort each sublist by its key,
  paginate each against its offset, drop `weak_spots`'s ad-hoc `MAX_DEAD`, apply concise projection.
- `crates/ariadne-mcp/src/server.rs` — three `#[tool]` methods + `project_daemon` arms.
- `crates/ariadne-daemon/src/domain/queries/` — warm `blast_radius`/`weak_spots`/`refactor` handlers
  call the same multi-list `paginate`.
- `crates/ariadne-cli/src/commands/query.rs` — thread params for the three tools.
- `docs/adr/0030-multi-list-pagination.md` — NEW (template): one cursor, per-sublist offsets.
</files>

<steps>
1. **Failing tests first.** For each tool: a default call caps each sublist at 50 and yields one
   cursor when any sublist overflows; a round-trip returns the remaining items per sublist and each
   per-list union equals the un-capped list; cold == warm; concise field-set assertion. Run — fails.
2. **Extend `economy`.** Confirm/extend `Cursor.offsets` indexing and a multi-list `paginate` (or N
   single-list calls sharing one cursor). A sublist whose `offsets[i]` is past its end yields no rows
   and no remainder; `next_cursor` is emitted iff at least one sublist has a remainder [src: plan.md
   D2; MCP "missing nextCursor = end", https://modelcontextprotocol.io/specification/2025-06-18/server/utilities/pagination].
3. **Core protocol + DTOs.** Add the params + `next_cursor`/`note` to the three variants/payloads.
4. **Cold handlers.** Sort each sublist by its documented key, paginate per offset, remove
   `weak_spots`'s `MAX_DEAD` constant (superseded), apply concise [src: weak_spots.rs:20,92-98;
   blast_radius.rs:48-61; refactor.rs:33-71].
5. **Warm handlers + server + CLI.** Mirror the cold calls on the warm path; wire `#[tool]` inputs +
   `project_daemon` + `query.rs`. Keep cold == warm byte-identical.
6. **ADR-0030 + dogfood.** Record the cursor semantics; confirm `blast_radius` on a hot symbol
   (was ~46k) now caps + pages; report before/after tokens.
</steps>

<verification>
- `cargo nextest run -p ariadne-mcp -E 'test(blast_radius) | test(weak_spots) | test(refactor)'` —
  per-sublist cap, multi-list cursor round-trip, concise, parity.
- `cargo nextest run -p ariadne-daemon`; `cargo test --test architecture`; clippy `-D warnings`;
  `cargo fmt --all --check`.
- Manual: `ariadne query blast_radius '{"symbol":"FileId"}'` caps `must_touch`/`may_touch` and offers
  a cursor; page to exhaustion and confirm the union equals the prior un-capped lists. Report tokens.
  If not runnable in-session, say so — never fabricate [src: CLAUDE.md].
</verification>

<rollback>
Revert the three handlers + warm handlers, the protocol/DTO additions, the economy multi-list
extension, the server/cli wiring, and ADR-0030. Restore `weak_spots`'s `MAX_DEAD`. Tiers 01–02 stay
intact.
</rollback>
