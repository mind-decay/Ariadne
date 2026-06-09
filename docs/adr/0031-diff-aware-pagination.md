# ADR-0031: diff-aware pagination

<status>
Accepted
Date: 2026-06-08
Decider: claude
</status>

<context>
ADR-0029 established the response-economy mechanism (an opaque,
revision-stamped `Cursor{revision:u32, offsets:Vec<u64>}`) and ADR-0030
extended it to multi-list tools via `paginate_sublist`/`multi_cursor`. Two
growable tools remained: `affected_tests` (`tests`, `seeds`) and
`diff_blast_radius` (`seeds`, aggregate `must_touch`, aggregate `may_touch`).
Both differ from every other growable tool in one way that breaks the ADR-0029
cursor's validity guard: their result set is derived from the changeset's git
diff (`ariadne_git::diff` at the MCP composition root), not from the index
revision alone [src: crates/ariadne-mcp/src/server.rs diff_blast_radius /
affected_tests; ADR-0023]. Within one index revision, two *different*
working-tree diffs produce two different result sets, so an ADR-0029 cursor
(stamped with the revision only) would silently page rows from changeset A into
a request scoped to changeset B [src: .claude/plans/data-fidelity-arc/block-1/
plan.md D2, BR1, BR5].

`diff_blast_radius` is additionally the only growable tool with a *nested*
shape: a list of seeds, each carrying its own `must_touch`/`may_touch`. A naive
cursor over the nested lists would be combinatorial (one offset per seed per
list) and unbounded as the seed page advances [src: tier-04 context].
</context>

<decision>
Two additions, both in `ariadne_graph::economy`, reused by the cold (MCP) and
warm (daemon) handlers so the JSON stays byte-identical:

1. **Changed-paths fingerprint in the cursor.** A new `DiffCursor{revision:u32,
   fingerprint:u64, offsets:Vec<u64>}` carries an order-independent FNV-1a
   `diff_fingerprint(changed_paths)` alongside the revision. `diff_multi_cursor`
   mints it from each top-level sublist's `(next_offset, remainder)`; `decode`
   re-checks both stamps and returns `CursorError::StaleDiff` when the
   fingerprint mismatches (the working-tree diff changed under it) — mapped to a
   JSON-RPC `invalid_params` (−32602), exactly as `StaleRevision` is. Its wire
   layout prepends the fingerprint word, so a `DiffCursor` and a plain `Cursor`
   never cross-decode. `DiffCursor::window()` projects to the plain `Cursor` the
   existing `paginate_sublist` consumes, so the top-level windowing is unchanged.

2. **Fixed per-seed inner cap, never a nested cursor.** Each seed's inner
   `must_touch`/`may_touch` are sorted by the same stable key and truncated to
   the request `limit`, with the full pre-cap count reported on the seed row
   (`must_touch_total`, `may_touch_total`). Only the three *top-level* lists are
   cursored.
</decision>

<rationale>
- **Reliability (no silent mis-paging across changesets):** the fingerprint
  makes a cursor valid only for the changeset that minted it. A changed diff
  between pages is a different result set, so rejecting the cursor (and steering
  a re-query) is correct, not a degradation — the MCP spec's "stable cursors" +
  "handle invalid cursors gracefully" [src: plan.md D2; MCP pagination spec].
- **Reliability / maintainability (parity):** one shared `DiffCursor` +
  `diff_multi_cursor` + `inner_page` shape; the cold and warm handlers call them
  with identical comparators, and the warm path `From`-projects the core report
  into the MCP wire output, so cold == warm == CLI by construction. Integration
  tests assert the top-level cursor round-trip's union equals the un-capped
  lists; the daemon parity unit tests assert warm == cold-oracle.
- **Efficiency (bounded, never combinatorial):** capping the nested inner lists
  at a fixed `limit` keeps a single seed's contribution bounded without a
  per-seed-per-list cursor explosion; the reported counts keep the truncation
  visible, honoring the arc's "truncation is reported, never silent" constraint
  [src: plan.md AR3; tier-04 exit criteria].
- **Efficiency (no new dep):** the fingerprint is a hand-rolled FNV-1a and the
  codec reuses ADR-0029's hex helpers — no base64/hash crate on the critical
  path [src: crates/ariadne-graph/src/economy.rs].
</rationale>

<alternatives>
- **Revision-only cursor (ADR-0029 as-is) for the diff tools** — rejected: it
  silently pages a stale changeset's rows whenever the working tree changes
  between pages, the exact wrong-rows failure BR1 names. `[src: plan.md D2, BR1]`
- **A combinatorial nested cursor over every seed's inner lists** — rejected:
  unbounded offset vector that grows as the seed page advances, and meaningless
  once the seed page itself moves; the fixed inner cap + reported count is
  bounded and honest. `[src: tier-04 exit criteria]`
- **Fingerprint over `hunks` as well as `changed_paths`** — rejected for this
  tier: the plan scopes the stamp to the changed-paths set (the cheap, stable
  identity of the changeset); a hunk-level stamp is broader than the decision and
  not required by the exit criteria. `[src: plan.md D2; tier-04 step 2]`
- **Hashing crate (e.g. `blake3`) for the fingerprint** — rejected: a 64-bit
  FNV-1a is sufficient for an equality stamp and adds no dependency to
  `ariadne-graph`. `[src: plan.md "no new dep"]`
</alternatives>

<consequences>
- `DaemonQuery::{DiffBlast, AffectedTests}` gain `limit`/`cursor`/`verbosity`;
  `DiffBlastReport`/`AffectedTestsReport` gain `next_cursor`/`note`, and
  `DiffSeed` gains `must_touch_total`/`may_touch_total` — a new protocol
  revision. The protocol is in-workspace and single-binary, and the daemon
  restarts on a revision change, so no old daemon speaks the old shape (BR4).
- The warm daemon paths now `From`-project the core report into the MCP wire
  output (`DiffBlastOutput`/`AffectedTestsOutput`), so the concise
  `skip_serializing_if` omission + `next_cursor`/`note` + per-seed counts take
  effect at the wire boundary, never on the postcard-framed IPC type. The CLI
  `affected-tests` command (and its `query affected_tests` twin) project the same
  way, so the third serving path matches.
- `affected_tests` and `diff_blast_radius` default to concise verbosity; in-repo
  precision consumers pass `verbosity:detailed`.
- A diff-aware serving path that mints a plain `Cursor` (no fingerprint), or that
  adds a nested per-seed cursor, is off-limits without superseding this ADR.
</consequences>

<sources>
- `[src: .claude/plans/data-fidelity-arc/block-1/tier-04-diff-aware-rollout.md]`
- `[src: .claude/plans/data-fidelity-arc/block-1/plan.md D1,D2,D4,D5; BR1,BR5]`
- `[src: docs/adr/0029-response-economy-cursor-verbosity.md ; docs/adr/0030-multi-list-pagination.md]`
- `[src: docs/adr/0023-mcp-git-diff-dependency.md]`
- `[src: https://modelcontextprotocol.io/specification/2025-06-18/server/utilities/pagination]`
- `[src: crates/ariadne-graph/src/economy.rs (DiffCursor, diff_fingerprint, diff_multi_cursor)]`
</sources>
