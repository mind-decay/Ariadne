# ADR-0030: multi-list pagination

<status>
Accepted
Date: 2026-06-08
Decider: claude
</status>

<context>
ADR-0029 established the response-economy mechanism — an opaque,
revision-stamped `Cursor{revision:u32, offsets:Vec<u64>}` and a generic
`paginate` — and tiers 01–02 applied it to the single-list growable tools
(`find_references`, `coupling_report`, `hotspots`, `complexity`, `co_change`).
Three growable tools return *several* lists at once: `blast_radius`
(`must_touch`, `may_touch`), `weak_spots` (`cycles`, `god_modules`,
`dead_symbols`), and `refactor_suggestions` (`god_modules`, `cycle_breaks`,
`misplaced_symbols`). `blast_radius` was measured at 45–46k tokens (1.8× over
the 25k MCP cap) and `weak_spots.dead_symbols` carried an ad-hoc `MAX_DEAD=16`
hard cap with no cursor, so its remainder was unreachable
[src: .claude/plans/data-fidelity-arc/block-1/plan.md D2, BR5;
crates/ariadne-mcp/src/tools/weak_spots.rs (pre-tier `MAX_DEAD`)]. The output
must stay byte-identical across the three serving paths (MCP cold, MCP
warm/daemon, CLI `query`) — the tier-07 parity invariant.
</context>

<decision>
Reuse the tier-01 `Cursor.offsets` vector as a *per-sublist* offset: a
multi-list tool sorts and windows each list independently against its own
`offsets[i]` via the new `economy::paginate_sublist`, then assembles ONE
`next_cursor` over all sublists via `economy::multi_cursor` — emitted iff any
sublist still has a remainder, carrying every sublist's next offset so an
exhausted sublist re-pages to empty rather than past its end. A single `note`
(`economy::multi_truncation_note`) names which lists were truncated. The
economy cap + cursor supersede `weak_spots`'s `MAX_DEAD` constant, making its
dead-code remainder reachable.
</decision>

<rationale>
- **Reliability / maintainability (parity):** one shared helper keeps cold ==
  warm == CLI true by construction. The cold and warm handlers call the same
  `paginate_sublist`/`multi_cursor`/`multi_truncation_note` with identical
  comparators, so their JSON cannot drift; per-tool integration tests assert the
  cursor round-trip's union equals the un-capped lists, and the daemon parity
  tests assert warm == cold-oracle [src: plan.md D1, BR3, BR5].
- **Reliability (completeness, no silent mis-paging):** one cursor carries every
  sublist's offset and the catalog revision; within a revision each per-sublist
  offset into a stable sort is deterministic and complete, and across a re-index
  the revision mismatch rejects the cursor (ADR-0029). A `limit:0` sublist is
  terminal (no remainder), mirroring `paginate`'s liveness guard.
- **Efficiency:** `blast_radius` drops from 45–46k tokens to one bounded page;
  concise verbosity additionally omits the embedded `SymbolSummary` cryptic
  fields on `must_touch`/`may_touch` and `dead_symbols`, while the name/metric-
  only lists (cycles, god modules, every refactor row) are unchanged by
  verbosity [src: plan.md D3, D4].
- **Efficiency (no new dep):** reuses ADR-0029's hand-rolled hex cursor codec
  unchanged [src: crates/ariadne-graph/src/economy.rs].
</rationale>

<alternatives>
- **A distinct cursor per sublist** — rejected: a multi-list result would carry
  N cursors, breaking the MCP opaque single-`nextCursor` model and forcing the
  client to thread several tokens. `[src: https://modelcontextprotocol.io/specification/2025-06-18/server/utilities/pagination]`
- **Keep `MAX_DEAD` alongside the cursor** — rejected: a silent hard cap leaves
  the remainder unreachable, contradicting the "truncation is reported, never
  silent" arc constraint. `[src: data-fidelity-arc/plan.md AR3; plan.md BR5]`
- **One flat concatenated list across the sublists** — rejected: it would erase
  the must/may and cycle/god/dead distinctions the tools exist to draw, and a
  single sort key cannot order heterogeneous row types. `[src: plan.md D4]`
</alternatives>

<consequences>
- `DaemonQuery::{BlastRadius, WeakSpots, RefactorSuggestions}` gain
  `limit`/`cursor`/`verbosity`; their reports gain `next_cursor`/`note` — a new
  protocol revision. The protocol is in-workspace and single-binary, and the
  daemon restarts on a revision change, so no old daemon speaks the old shape
  (BR4) [src: ADR-0029 consequences].
- `weak_spots`'s `MAX_DEAD` constant is removed (both cold and warm handlers);
  the dead-code list is now page-capped + cursored.
- `blast_radius`/`weak_spots`/`refactor_suggestions` take dedicated MCP input
  types (`BlastRadiusInput` gains the economy fields; `WeakSpotsInput`,
  `RefactorInput` are new), leaving the shared `ScopeInput` consumer
  (`doc_for_project`) untouched.
- The warm daemon paths now `From`-project the core report into the MCP wire
  output (like the tier-02 single-list tools), so the concise `skip_serializing_if`
  omission takes effect on the wire boundary, never on the postcard-framed IPC type.
- Tier-04 (`diff_blast_radius`) reuses the same `paginate_sublist`/`multi_cursor`
  shape for its nested seed lists.
- Adding a multi-list serving path that bypasses `paginate_sublist`/`multi_cursor`
  is off-limits without superseding this ADR.
</consequences>

<sources>
- `[src: .claude/plans/data-fidelity-arc/block-1/plan.md D1,D2,D4,D5; BR5]`
- `[src: docs/adr/0029-response-economy-cursor-verbosity.md]`
- `[src: https://modelcontextprotocol.io/specification/2025-06-18/server/utilities/pagination]`
- `[src: crates/ariadne-graph/src/economy.rs (paginate_sublist, multi_cursor, multi_truncation_note)]`
</sources>
</output>
