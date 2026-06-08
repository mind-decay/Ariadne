---
tier_id: tier-02
title: Roll the economy helper out to the single-list tools
deps: [tier-01]
exit_criteria:
  - "`coupling_report`, `co_change`, `hotspots`, `complexity` each cap at the default page, return `next_cursor` + `note` when truncated, and round-trip to the full set across pages (completeness test per tool)."
  - "Each tool's truncation uses the documented stable sort key (below), so the default page is a meaningful top-N, not arbitrary."
  - "Concise is the default: hotspots/complexity symbol-grain rows omit the embedded symbol `id` + byte offsets; metric-only tools (coupling, co_change) have concise == detailed (the cap is their win) — a test records this per tool."
  - "Cold == warm == CLI `query` JSON for all four tools; clippy `-D warnings`, fmt, `cargo test --test architecture`, dogfood green."
status: completed
completed: 2026-06-08
---

<context>
Mechanical application of the tier-01 contract to the four single-`Vec` tools that share the warm +
cold + CLI surface [src: crates/ariadne-mcp/src/tools/{coupling_report,co_change,hotspots,complexity}.rs;
crates/ariadne-daemon/src/domain/queries/analytics.rs]. No new mechanism — reuses
`economy::paginate` + `Verbosity` from tier-01 (cite ADR-0029). The only per-tool decisions are the
canonical stable sort key and the concise field-set, recorded inline (no new ADR). These tools are
the worst offenders measured: `co_change` 733k tok, `hotspots` symbol-grain 311k, `complexity`
symbol-grain 291k, `coupling_report` 20k [src: this-session measurements; plan.md `<context>`]. Full
context: `plan.md`.

Stable sort keys (truncation order):
- `coupling_report` — afferent (Ca) desc, then `module` asc (most-depended-on first) [src:
  crates/ariadne-mcp/src/tools/coupling_report.rs:52-61].
- `co_change` — `degree` desc, then `(a, b)` asc [src: co_change.rs:25-36].
- `hotspots` — `score` desc, then file path / symbol id asc (the use case already ranks by score)
  [src: crates/ariadne-graph/src/hotspot.rs; hotspots.rs:53-77].
- `complexity` — `complexity` desc, then `key` asc (already implemented) [src: complexity.rs:54-66].
</context>

<files>
- `crates/ariadne-core/src/domain/daemon/query.rs` — add `limit`/`cursor`/`verbosity` to the
  `CouplingReport`, `CoChange`, `Hotspots`, `Complexity` variants.
- `crates/ariadne-core/src/domain/daemon/response.rs` — wrap the `Coupling`/`CoChange`/`Hotspots`/
  `Complexity` payloads with `next_cursor`/`note`; mark the embedded `SymbolSummary` cryptic fields
  skip-able (shared with tier-01).
- `crates/ariadne-mcp/src/types.rs` — add the economy params to `CoChangeInput` + `GrainScopeInput`,
  and to `coupling_report`'s input (a dedicated paginated input or a flattened `EconomyInput`, so the
  non-paginated `doc_for_project` that shares `ScopeInput` is untouched — verify schemars supports the
  chosen shape); add `next_cursor`/`note` to the four outputs.
- `crates/ariadne-mcp/src/tools/{coupling_report,co_change,hotspots,complexity}.rs` — sort by the key
  above, then `economy::paginate`; concise projection on symbol-grain rows.
- `crates/ariadne-mcp/src/server.rs` — the four `#[tool]` methods take the new params; `project_daemon`
  handles the wrapped responses.
- `crates/ariadne-daemon/src/domain/queries/analytics.rs` (+ coupling handler) — same `economy::paginate`
  call on the warm path.
- `crates/ariadne-cli/src/commands/query.rs` — thread the params in `build_query`/`dispatch`/`project`
  for the four tools; pin `detailed` where `digest` consumes them.
</files>

<steps>
1. **Failing tests first (per tool).** For each of the four: (a) a default call on the self-index caps
   at 50 + returns a cursor; (b) cursor round-trip union == the un-capped sorted set, no gap/dup; (c)
   cold == warm JSON; (d) concise field-set assertion (symbol-grain drops id/offsets; metric tools
   concise == detailed). Run — fails.
2. **Core protocol.** Add the three params to the four `DaemonQuery` variants; wrap the four responses
   with `next_cursor`/`note` [src: crates/ariadne-core/src/domain/daemon/{query,response}.rs].
3. **Cold handlers.** In each `tools/*.rs`, replace the unbounded `collect()` with: build rows → sort
   by the documented key → `economy::paginate(rows, …, revision, sublist=0, budget)` → concise
   projection [src: the four tool files]. Keep the float/metric values byte-identical (no rounding —
   determinism + "don't change what they compute", plan.md scope-out).
4. **Warm handlers.** Apply the identical `economy::paginate` call in `analytics.rs` (hotspots,
   complexity, co_change) and the coupling warm handler, so the JSON matches the cold path byte-for-byte
   [src: crates/ariadne-daemon/src/domain/queries/analytics.rs:32-175].
5. **Server + CLI.** Wire the `#[tool]` inputs + `project_daemon`; thread `query.rs`
   `build_query`/`dispatch`/`project`; pin `detailed` for any `digest` consumption [src: server.rs;
   query.rs:141-186,205-318].
6. **Dogfood.** Confirm each tool's default output is now well under 25k and pages cleanly; record the
   before/after token counts for the tier notes (the worst three were 733k/311k/291k).
</steps>

<verification>
- `cargo nextest run -p ariadne-mcp -E 'test(coupling) | test(co_change) | test(hotspots) | test(complexity)'`
  — cap, cursor round-trip, concise, parity per tool.
- `cargo nextest run -p ariadne-daemon` — warm-path analytics tests stay green.
- `cargo test --test architecture`; clippy `-D warnings`; `cargo fmt --all --check`.
- Manual: `ariadne query co_change '{"min_revs":1,"min_shared_commits":1,"min_degree":0.0}'` no longer
  emits ~733k tokens — it caps + offers a cursor; page through and confirm the union matches the
  un-capped set. Report the measured reduction. If not runnable in-session, say so [src: CLAUDE.md].
</verification>

<rollback>
Revert the four tool handlers, their warm handlers, the protocol/DTO additions for these four
variants, and the server/cli wiring. Tier-01 (`economy` + find_references) stays intact.
</rollback>
