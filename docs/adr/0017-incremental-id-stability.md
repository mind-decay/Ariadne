# ADR-0017: Edit-Stable SymbolId And Stale-Record Removal

<status>
Accepted
Date: 2026-05-30
Decider: claude (post-v1 tier-07b, RD12)
</status>

<context>
ADR-0016 moved the per-file derivation into `ariadne-salsa` but kept the cold
`SymbolId` scheme `blake3("{path}#{name}@{offset}")`
[src: crates/ariadne-salsa/src/derive.rs:115-123 @ tier-07a] for byte-parity.
That offset dependence is unsafe for incremental updates: an edit anywhere
above a symbol shifts its `def_byte_range.0`, so the symbol is re-keyed and
every edge to it is severed — a benign edit produces a *maximal* warm-graph
delta instead of a minimal one. Separately, tier-07a's `commit_revision` was
upsert-only [src: crates/ariadne-salsa/src/db.rs @ tier-07a]: it never emitted
the `Changeset` delete vectors, so a re-derivation that dropped a symbol/edge/
file left the stale record behind. tier-08's watcher re-derives single files
and depends on an incremental update equalling a full rebuild (divergence 0).
Forces: reliability (stable node identity, no orphaned records), efficiency
(minimal deltas), maintainability (one derivation, ADR-0016).
</context>

<decision>
Make the `SymbolId` offset-independent: `blake3("{path}#{kind}#{name}#{nth}")`,
where `nth` is the 0-based occurrence index among same-`(name, kind)`
declarations in that file in source order (the synthesized SFC component uses
`kind = "component"`, `nth = 0`). Make `commit_revision` diff-aware: it reads
the prior committed file/symbol/edge sets and fills `Changeset.file_deletes` /
`symbol_deletes` / `edges_removed` for every prior id this revision does not
re-derive, alongside the upserts. Add `rederive_file` / `forget_file` so the
tier-08 watcher applies a single-file delta over this same diff-aware path.
</decision>

<rationale>
- **Reliability (stable identity).** Keying on `(path, kind, name, nth)` instead
  of a byte offset makes an unchanged symbol's id invariant under edits
  elsewhere in its file, so the edges to it survive. Proven by the stability
  test: a prepended blank line leaves the callee's id and the caller→callee
  edge intact [src: crates/ariadne-salsa/tests/incremental.rs].
- **Reliability (no orphans).** Diffing the freshly derived set against the
  prior committed set (`ReadSnapshot::iter_files` / `iter_symbols` /
  `iter_edges` [src: crates/ariadne-core/src/domain/ports.rs:108-131]) emits the
  exact stale-removal set, so an incremental sequence of edits/creates/deletes
  yields storage byte-identical to a fresh full rebuild — the divergence-0
  proptest asserts this over 100 random sequences
  [src: crates/ariadne-salsa/tests/incremental.rs]. A deleted file sheds its
  symbols and incident edges; edges from *other* files that referenced a now-
  deleted symbol also drop, because re-resolution finds no candidate and the
  diff removes the unreproduced edge.
- **Efficiency (minimal delta).** A benign edit now churns only the symbols and
  edges that actually changed, not every edge in the file. `rederive_file`
  mutates one file's inputs through the salsa setter chain so only that file's
  `symbols_for_file` recomputes (others hit the salsa cache).
- **Maintainability.** Both `rederive_file` and `forget_file` funnel through the
  one diff-aware `commit_revision`, and the cold CLI full index reuses it
  unchanged, so there is a single commit path. Cold goldens are re-baselined to
  the new ids; only the `SymbolId` literals changed (counts, names, kinds,
  spans, edge kinds, and file records are byte-identical)
  [src: crates/ariadne-cli/tests/goldens/].
</rationale>

<alternatives>
- **Keep the offset id** — rejected: every edit re-keys downstream symbols and
  severs their edges, so warm deltas are maximal and node identity is unstable
  across edits. `[src: post-v1-roadmap plan.md RD12]`
- **Content-hash ids** (hash the symbol body) — rejected: collide across
  renamed-but-identical bodies and still churn on any body edit, defeating the
  stability goal. `[src: post-v1-roadmap plan.md RD12]`
- **Partition the prior-set diff by changed names** — deferred: the full
  prior-set scan is O(total) per commit, the accepted R-B4 trade alongside the
  global edge-resolution pass; partitioning is a future tier if the p95 <500ms
  SLO is missed on 100K files. `[src: post-v1-roadmap plan.md R-B4]`
</alternatives>

<consequences>
- `derive::symbol_id` takes `(path, kind, name, nth)`; the `nth` disambiguator
  is computed in `commit_revision`'s per-file loop with a `(name, kind)→count`
  map over the deterministic `symbols_for_file` order, so it matches between an
  incremental commit and a full rebuild.
- **Accepted residual churn (R-B5).** `nth` is occurrence order, so inserting a
  same-`(name, kind)` sibling *before* an existing one in the same file shifts
  the later sibling's `nth` and re-keys it. The churn is bounded to same-named
  siblings within one file and is corrected by the divergence-0 proptest; it is
  accepted, not fixed, this tier. `[src: post-v1-roadmap plan.md R-B5]`
- `commit_revision` now reads a prior snapshot before each commit; the first
  commit (empty prior) emits no deletes, preserving tier-07a behaviour.
- `AriadneDb` gains `rederive_file` / `forget_file`; the tier-08 watcher drives
  the warm graph through them with the divergence-0 guarantee.
- Cold parity goldens are re-baselined under the new id scheme; the parity gate
  [src: crates/ariadne-cli/tests/index_parity.rs] continues to freeze cold
  output, now against the stable ids.
- Off-limits without superseding: re-introducing a byte-offset term in the
  `SymbolId`, or an upsert-only commit path that leaves stale records.
</consequences>

<sources>
- `[src: post-v1-roadmap plan.md RD12, R-B4, R-B5]`
- `[src: crates/ariadne-salsa/src/derive.rs ; src/db.rs]`
- `[src: crates/ariadne-salsa/tests/incremental.rs]`
- `[src: crates/ariadne-core/src/domain/changeset.rs:16-28 ; src/domain/ports.rs:108-131]`
- `[src: crates/ariadne-storage/src/adapters/redb/apply.rs:24-44]`
- `[src: crates/ariadne-cli/tests/index_parity.rs ; tests/goldens/]`
- `[src: docs/adr/0016-shared-per-file-derivation.md]`
</sources>
