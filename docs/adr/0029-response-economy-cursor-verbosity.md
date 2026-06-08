# ADR-0029: response-economy cursor and verbosity

<status>
Accepted
Date: 2026-06-07
Decider: claude
</status>

<context>
The growable MCP tools return their whole result. Measured on this 415-file
repo (token proxy = bytes/4; Claude Code caps a tool result at 25k tokens),
several already blow the cap — `co_change` 733k, `hotspots` symbol-grain 311k,
`find_references` up to 12k — and the v1 SLO targets a 100K-file workload
[src: .claude/plans/data-fidelity-arc/block-1/plan.md `<context>`;
https://www.anthropic.com/engineering/writing-tools-for-agents]. Capping the
delivered view is overdue. The output travels three serving paths — MCP cold
(`tools::*::handle`), MCP warm (daemon `DaemonResponse`), CLI `query` — that
must stay byte-identical (the tier-07 parity invariant). The hexagonal rule
forbids one adapter calling another, so the economy logic cannot live in
`ariadne-mcp` [src: CLAUDE.md D13]. This tier establishes the mechanism on
`find_references`; tiers 02–04 repeat it across the other nine tools.
</context>

<decision>
Add one pure use case, `ariadne_graph::economy` — `Verbosity`, `Budget`, an
opaque revision-stamped `Cursor{revision:u32, offsets:Vec<u64>}` with a
hand-rolled encode/decode, and a generic `paginate<T>` over a caller-supplied
stable comparator. Every serving path calls it: the MCP cold handler and the
daemon warm handler both sort, `paginate`, and project at the requested
verbosity, so their JSON is identical. `ariadne-core` carries the wire surface
(`limit`/`cursor`/`verbosity` on `DaemonQuery::FindReferences`; a
`ReferencesReport{references, next_cursor, note}`; optional cryptic fields on
`ReferenceSite`). Concise verbosity is the default and omits the cryptic
id/offset fields; detailed is a lossless superset.
</decision>

<rationale>
- **Maintainability / reliability (parity):** one shared helper makes
  cold == warm == CLI true by construction, not by discipline; a per-tool
  parity test guards the duplicated comparator/note wording
  [src: .claude/plans/data-fidelity-arc/block-1/plan.md D1, BR3].
- **Reliability (no silent mis-paging):** the cursor stamps the catalog
  revision. Within a revision an offset into a stable sort is deterministic and
  complete; across a re-index the revision mismatches and `Cursor::decode`
  rejects it (cold → `invalid_params`/−32602; warm → query-level error),
  steering a re-query — the MCP spec's "stable cursors" + "handle invalid
  cursors gracefully" for a self-cursor, since spec pagination covers only list
  ops, not `tools/call`
  [src: https://modelcontextprotocol.io/specification/2025-06-18/server/utilities/pagination; D2].
- **Efficiency:** concise drops the fields the LLM reasons about worse (raw ids,
  byte offsets) for ≈⅓ the tokens, while detailed stays lossless for in-repo
  precision consumers
  [src: https://www.anthropic.com/engineering/writing-tools-for-agents; D3].
- **Efficiency (no new dep):** the codec is hand-rolled hex over a fixed
  little-endian layout — no base64/cbor crate enters the graph/core critical
  path [src: crates/*/Cargo.toml; D2].
</rationale>

<alternatives>
- **An `ariadne-mcp`-only formatting layer** — rejected: it skips the warm/CLI
  paths and would force driving→driving coupling, breaking parity and the
  hexagonal rule. `[src: CLAUDE.md D13; plan.md D1]`
- **Index-only offset cursor (no revision stamp)** — rejected: silently returns
  wrong/partial rows after any edit. `[src: plan.md D2]`
- **Default detailed verbosity** — rejected: the efficiency win would be
  opt-in, leaving every default call over-large. `[src: plan.md D3]`
</alternatives>

<consequences>
- `DaemonQuery::FindReferences` and `DaemonResponse::References` change shape: a
  new protocol revision. The protocol is in-workspace and single-binary, and the
  daemon restarts on a revision change, so no old daemon speaks the old shape
  (BR4) [src: plan.md `<risks>`].
- `ReferenceSite` cryptic fields are now `Option` with
  `skip_serializing_if`; snapshot/JSON consumers that need them pin
  `verbosity:detailed`.
- No redb schema change — economy is a delivery-layer projection over an
  already-computed result [src: data-fidelity-arc/plan.md AD5].
- Tiers 02–04 reuse `economy::paginate` for the remaining growable tools; the
  default page size (50) is verified, not assumed, by the tier-05 harness.
- Lowering `paginate` below the established generic shape, or adding a serving
  path that bypasses it, is off-limits without superseding this ADR.
</consequences>

<sources>
- `[src: .claude/plans/data-fidelity-arc/block-1/plan.md D1,D2,D3,D4,D5]`
- `[src: https://modelcontextprotocol.io/specification/2025-06-18/server/utilities/pagination]`
- `[src: https://www.anthropic.com/engineering/writing-tools-for-agents]`
- `[src: crates/ariadne-graph/src/economy.rs]`
</sources>
