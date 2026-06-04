---
slug: god-module-suggestion-fix
title: God-module split suggestion names an extractable member, not the hottest callee
created: 2026-06-04
owners: [user, claude]
review: [user, codex?]
single_tier: true
exit_criteria:
  - "god_modules `top_outbound[0]` names a symbol that is a member of the finding's module (regression test asserts containment in `module.members`); never an external callee"
  - "`efferent` (Ce) value and the threshold/cohesion gate are unchanged vs the prior definition (distinct external targets); a unit test pins Ce on a fixture"
  - "regenerated docs/codebase-overview.md God-modules section names member symbols; no `clone`/`new`/`get`/`default` appears as a split target unless that name is itself the top-fan-out member"
  - "golden refactor_cases__god_modules re-accepted; warm==cold parity + refactor_suggestions MCP tests green; overview byte-identical on re-run"
  - "cargo clippy/fmt/deny/test --test architecture green"
status: completed
completed: 2026-06-04
---

<context>
`refactor::god_modules` builds its outbound histogram keyed on the edge **target**
(the external callee) and filtered to non-members, then the split suggestion names
the hottest target [src: crates/ariadne-graph/src/refactor.rs:80-130, key at
`*outbound.entry(graph.graph[er.target()])` ~96-98, guard `!member_ix.contains(
&er.target())`]. So in docs/codebase-overview.md the God-modules section reads
"Consider splitting `clone` out of health.rs" — incoherent, because `clone` is not
a member of `health.rs`; it is an external symbol the module *calls*. The winners
(`clone`/`new`/`get`/`default`) are high-afferent **sinks** that dominate by raw
call count and carry no extraction signal — the symbol-level analogue of the raw
fan-in noise the docgen redesign removed at file level [src:
.claude/plans/useful-docgen/plan.md `<context>`]. The same use case feeds three
surfaces: the project overview, and the warm + cold `refactor_suggestions` MCP
tools [src: crates/ariadne-graph/src/docgen_insights.rs:409-461 `change_coupling`;
crates/ariadne-daemon/src/domain/queries/refactor.rs:21-78;
crates/ariadne-mcp/src/tools/refactor.rs].
Goal: the suggestion names an actually extractable **member**, chosen by a
language-agnostic structural metric. In scope: `god_modules` only. Out of scope:
cycle-break / misplaced-symbol findings, new metrics, sub-cluster/community
extraction (D2 rejected), any rename of `top_outbound`/`OutboundRow`.
</context>

<constraints>
- Deterministic: `BTreeMap`/`BTreeSet` + sorted vecs, integer `pct_of`; same revision
  → identical bytes [src: crates/ariadne-graph/src/docgen.rs:1-8; refactor.rs:215 `pct_of`].
- Graph-pure, IO-free use case; no daemon/adapter dependency [src: CLAUDE.md D13; memory `hexagonal-strict`].
- No new dependency; reuse the `edges_directed(ix, Outgoing)` traversal already in the
  function (pinned petgraph 0.8) [src: refactor.rs ~94; Cargo.toml petgraph 0.8 pin;
  https://docs.rs/petgraph/latest/petgraph/graph/struct.EdgeReference.html].
- Proportional: name one member (highest external fan-out), matching the current
  single-symbol output shape; no clustering [src: CLAUDE.md `<rules>` "no features beyond the tier"].
</constraints>

<decisions>
**D1 — Re-key the histogram on the source member, ranked by external fan-out.**
For each member node `ix`, count its edges whose target is outside the module and
attribute that count to `graph.graph[ix]` (the member), not to the target. The
hottest member is the best extraction candidate: removing it most reduces the
module's outbound coupling, and the suggestion "split `<member>` out" is now
coherent because `<member>` is in the module. `ix` is already the source of its
own outgoing edges, so no `EdgeReference::source()` call is needed
[src: https://docs.rs/petgraph/latest/petgraph/graph/struct.EdgeReference.html —
edges_directed(_, Outgoing) yields edges from `ix`; refactor.rs:94-101].
*Rejected:* keep target-keying + filter — the suggestion still names a callee that
lives in another module; wrong noun.

**D2 — Noise exclusion is structural, not a name list.** A name denylist
(`clone`/`new`/`default`…) is English/Rust-specific and breaks across the languages
Ariadne indexes (TS/Python/Go/Java/C#) [src: crates/ariadne-parser fixtures cover
multiple langs]. The trivial methods dominate today *because* they are high-afferent
sinks; ranking members by **efferent** fan-out (D1) structurally excludes them in
any language — a sink has near-zero outbound edges, so it can never top the member
ranking. No list, no per-language config. *Rejected:* const denylist (lexical,
non-portable); kind-tag filter (derived impls are not even symbols, manual ones tag
as `function`, so kind cannot identify them) [src: mcp list_symbols probe this session].

**D3 — Fix the shared use case; keep field names.** Patch `refactor::god_modules`
so all three consumers correct at once (user decision). Keep `GodModuleFinding.
top_outbound` and `OutboundRow{symbol,edges}` names — they still read as "top
members by outbound traffic"; only their doc comments and the `symbol` meaning
change. Avoids churning the `refactor_suggestions` tool's JSON keys
[src: crates/ariadne-core/src/domain/daemon/rows.rs:96-118]. *Rejected:* rename to
`top_members`/`MemberRow` — changes a shipped tool contract for cosmetic honesty.
</decisions>

<architecture>
- `ariadne-graph::refactor::god_modules` — sole logic change: two accumulators per
  module. `efferent` (Ce) must keep counting **distinct external targets**; the new
  member→edge-count map drives `top_outbound` + the suggestion. Conflating them (the
  current single map) would make re-keying silently redefine Ce.
- `ariadne-core` rows.rs — doc-comment-only updates on `OutboundRow`/`GodModuleRow`.
- Daemon/MCP refactor handlers + their tests — no change (field names stable; tests
  assert structure, not symbol identity) [src: crates/ariadne-mcp/tests/tools_refactor.rs:21;
  crates/ariadne-daemon/tests/warm_analytics.rs:136,300].
</architecture>

<tech_inventory>
| Tech | Version | Doc fetched this session |
| --- | --- | --- |
| petgraph (edges_directed / Direction::Outgoing / EdgeReference) | 0.8 (pinned) | https://docs.rs/petgraph/latest/petgraph/graph/struct.EdgeReference.html (Context7 quota exhausted → WebSearch/docs.rs fallback) |
</tech_inventory>

<files>
- crates/ariadne-graph/src/refactor.rs — `god_modules`: split the accumulator into a
  `BTreeSet<SymbolId>` of external targets (for `efferent`/Ce) and a
  `BTreeMap<SymbolId,u32>` member→external-edge-count (for `top_outbound`); rebuild
  `top`/`suggestion` from the member map; update wording + `GodModuleFinding.top_outbound`
  doc. Add a unit test asserting the named symbol ∈ `module.members` and Ce unchanged.
- crates/ariadne-graph/tests/snapshots/refactor_cases__god_modules.snap — re-accept (insta).
- crates/ariadne-core/src/domain/daemon/rows.rs — doc comments on `OutboundRow.symbol`
  ("module member") and `GodModuleRow.top_outbound` ("members ranked by external fan-out").
- docs/codebase-overview.md + docs/codebase-overview.svg — regenerate via `ariadne doc`.
</files>

<steps>
1. Write/extend a failing test in `crates/ariadne-graph/tests/refactor_cases.rs` (or a
   `#[cfg(test)]` mod in refactor.rs): on the god-module fixture, assert
   `gods[0].top_outbound[0].0` is contained in the matching `ModuleSpec.members`, and
   assert `gods[0].efferent` equals the count of distinct external targets (pins Ce).
2. In `god_modules`, replace the single target-keyed `outbound` map with: `external:
   BTreeSet<SymbolId>` (insert `graph.graph[er.target()]` per external edge) and
   `by_member: BTreeMap<SymbolId,u32>` (per member `ix`, increment by its external-edge
   count, insert only when >0). Keep `total_out` as the sum of external edges
   [src: refactor.rs:90-104].
3. Set `efferent = external.len()` (Ce unchanged); keep the `efferent <= threshold ||
   cohesion >= COHESION_FLOOR` gate verbatim [src: refactor.rs:104-106].
4. Build `top` from `by_member`, sort `b.1.cmp(&a.1).then(a.0.cmp(&b.0))`, truncate
   `TOP_OUTBOUND`; suggestion = `format!("Consider extracting `{}` — it accounts for
   {pct}% of this module's outbound coupling.", table.name(sym))` with
   `pct = pct_of(cnt, total_out)` [src: refactor.rs:108-118,215].
5. Update doc comments in refactor.rs (`top_outbound`) and rows.rs (`OutboundRow`,
   `GodModuleRow.top_outbound`) to the member semantics.
6. `cargo nextest run -p ariadne-graph`; review the new golden with `cargo insta review`
   (or `cargo insta accept` after confirming members are named); confirm warm/cold +
   MCP refactor tests still pass.
7. Regenerate `cargo run -p ariadne-cli -- doc`; read the God-modules section and confirm
   it names members (no bare `clone`/`new`/`get`/`default` split target); run twice → diff empty.
</steps>

<verification>
- `cargo nextest run -p ariadne-graph` → new containment + Ce tests green; god_modules golden re-accepted.
- `cargo nextest run -p ariadne-daemon -p ariadne-mcp` → `refactor_suggestions_matches_cold`,
  `refactor_suggestions_lists_findings`, warm_analytics green (unchanged).
- `cargo run -p ariadne-cli -- doc` twice → `docs/codebase-overview.{md,svg}` byte-identical;
  God-modules lines name member symbols.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`; `cargo fmt --all
  --check`; `cargo deny check` (no new dep); `cargo test --test architecture`.
</verification>

<risks>
| Risk | Likelihood | Mitigation | Owner |
| --- | --- | --- | --- |
| Re-keying silently redefines Ce (efferent) | High if naive | D1/step 2-3: separate `BTreeSet` for targets; step 1 test pins Ce | build |
| A genuine hand-written member named `clone`/`new` tops a module | Low | Acceptable — it is the real top coupler; structural metric, not a name | build |
| Golden churn hides a regression | Med | Review (not blind-accept) the new snapshot; containment test guards the actual bug | build/audit |
</risks>

<rollback>
`git checkout -- crates/ariadne-graph/src/refactor.rs crates/ariadne-core/src/domain/daemon/rows.rs
crates/ariadne-graph/tests/ docs/codebase-overview.md docs/codebase-overview.svg`. No
schema/field rename means no cross-crate revert; daemon/mcp untouched.
</rollback>

<sources>
- repo: crates/ariadne-graph/src/refactor.rs:23-130,215; docgen_insights.rs:409-461;
  crates/ariadne-core/src/domain/daemon/rows.rs:96-118; crates/ariadne-daemon/src/domain/queries/refactor.rs:21-78;
  crates/ariadne-mcp/src/tools/refactor.rs; tests: refactor_cases.rs, tools_refactor.rs:21, warm_analytics.rs:136,300.
- [petgraph EdgeReference — docs.rs](https://docs.rs/petgraph/latest/petgraph/graph/struct.EdgeReference.html)
- [useful-docgen plan — raw-fan-in noise precedent](.claude/plans/useful-docgen/plan.md)
</sources>
