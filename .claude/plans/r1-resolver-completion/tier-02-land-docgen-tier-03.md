---
tier_id: tier-02
title: Land docgen-overview-fidelity tier-03 on the now-reliable edge set
deps: [tier-01, tier-03, tier-04]
exit_criteria:
  - "Prerequisites landed: tier-03 (crate-role test-edge scope) held graph changes restored, tier-04 (same-crate Method/Path abstention) resolver fix committed and reindexed — without both, the dogfood role/boundary gates below cannot pass"
  - "docgen-overview-fidelity tier-03 <verification> re-run passes verbatim: cli/e2e render volatile-leaf, boundary-violation set near-zero with qualified `crate::name` rows, cross-crate cycle clusters listed qualified"
  - "Its fixture assertions are present and green: cross-crate violation upper-bound (not merely non-empty), cli/e2e leaf-role string, boundary rows contain `::` (never bare `new`)"
  - "`ariadne doc` (or `ariadne-cli -- doc`) run twice → docs/codebase-overview.{md,svg} byte-identical; the committed overview reflects the reliable edges"
  - "docgen-overview-fidelity/tier-03 frontmatter status flipped blocked→completed; audit-state updated"
  - "cargo nextest run -p ariadne-graph -p ariadne-daemon -p ariadne-mcp, architecture, clippy, fmt all green"
status: completed
completed: 2026-06-05
---

<context>
tier-01 made the edge set trustworthy. The docgen tier-03 rendering changes are
already written and verified-green but held uncommitted, with
docs/codebase-overview.{md,svg} reverted to the tier-01-honest state, because the
edges were not yet reliable [src: .claude/plans/docgen-overview-fidelity/
tier-03-reenable-on-reliable-edges.md `<blockers>`]. This tier restores those held
changes, re-runs that tier's `<verification>` on the corrected edges, regenerates
+ commits the overview, and flips its status. It does not re-derive the rendering
logic — that work exists; it validates and lands it (plan D4). Authoritative
detail for the rendering changes lives in the docgen tier-03 file; this tier is
the unblock + verify + commit wrapper.
</context>

<files>
- (restore from stash) crates/ariadne-graph/src/docgen.rs,
  crates/ariadne-graph/src/docgen_insights.rs,
  crates/ariadne-graph/tests/docgen_project.rs,
  crates/ariadne-graph/tests/snapshots/docgen_fixture__project.snap — the held
  tier-03 rendering + fixture changes [src: docgen tier-03 `<files>`].
- docs/codebase-overview.md, docs/codebase-overview.svg — regenerated.
- .claude/plans/docgen-overview-fidelity/tier-03-reenable-on-reliable-edges.md —
  `status: blocked` → `completed`.
- .claude/plans/docgen-overview-fidelity/audit-state.json — reflect tier-03 land
  if the audit flow updates it.
</files>

<steps>
1. Confirm tier-01 landed (resolver fix committed, dogfood e2e/cli read
   volatile-leaf). If not, STOP — this tier depends on tier-01.
2. Restore the held docgen tier-03 changes (`git stash pop`, or re-apply if they
   were committed-as-WIP). Resolve any conflict against tier-01's edge-driven
   snapshot re-acceptance.
3. Run `cargo nextest run -p ariadne-graph`. The tier-03 fixture assertions
   (cross-crate-violation upper bound; cli/e2e volatile-leaf role string;
   boundary rows contain `::`) must now PASS on the reliable edges — where they
   could not before. Review (do not blind-accept) any remaining goldens, then
   `cargo insta accept`.
4. Regenerate the committed overview: `cargo run -p ariadne-cli -- doc <repo>
   --out docs/codebase-overview.md --svg docs/codebase-overview.svg`. Read every
   section: Architecture Role shows `ariadne-cli`/`ariadne-e2e` as volatile leaves;
   Boundary violations is short and names qualified members; cycle clusters
   qualified. Run twice → diff empty (determinism).
5. Run `cargo nextest run -p ariadne-daemon -p ariadne-mcp` (warm==cold + refactor
   suites unchanged-green), `cargo test --test architecture`, clippy, fmt.
6. Flip docgen tier-03 `status: blocked` → `completed`; note in its `<blockers>`
   that R1 shipped under `r1-resolver-completion`. Commit the docgen-graph
   changes + regenerated overview + the status flip.
</steps>

<verification>
- `cargo nextest run -p ariadne-graph` → tier-03 upper-bound, leaf-role, and
  qualified-render assertions green; goldens re-accepted after review.
- `cargo nextest run -p ariadne-daemon -p ariadne-mcp` → warm==cold + refactor
  suites unchanged-green.
- `cargo run -p ariadne-cli -- doc` twice → docs/codebase-overview.{md,svg}
  byte-identical; Architecture Role: cli/e2e volatile-leaf (I > 0.7); Boundary
  violations near-zero and qualified.
- `cargo test --test architecture`; `cargo clippy --workspace --all-targets
  --all-features -- -D warnings`; `cargo fmt --all --check`.
- Fail loudly: if cli/e2e still render "Stable foundational", or the
  boundary-violation set is not near-zero, tier-01's gate is incomplete — STOP
  and reopen tier-01 (do not weaken the tier-03 assertion to pass).
</verification>

<rollback>
`git checkout -- crates/ariadne-graph docs/codebase-overview.md
docs/codebase-overview.svg
.claude/plans/docgen-overview-fidelity/tier-03-reenable-on-reliable-edges.md`.
Reverts to the tier-01 state (reliable edges committed, overview honest-but-
suppressed); tier-01's resolver fix is untouched.
</rollback>

<blockers>
BLOCKED at step 4 — the plan's premise is wrong: the R1 resolver fix alone does
NOT make `ariadne-cli`/`ariadne-e2e` render volatile-leaf. On a fresh in-place
re-index with the R1-fixed + tier-03-rendering binary (daemon stopped, rev 1:
383 files, 3626 symbols, 3330 edges), the regenerated overview still shows
`ariadne-cli` and `ariadne-e2e` (and ~11 of 12 crates) as "Stable foundational
module — many dependents", never the volatile-leaf string. tier-02
`<verification>` "fail loudly" fired; not weakened, not committed.

Root cause is NOT tier-01's resolver (its gate is correct + complete — the
`socket.connect()` cross-crate method phantom is gone; `connect`'s 4 refs are all
same-crate post-fix). It is a coupling-vs-scope artifact in
`architecture_section` [crates/ariadne-graph/src/docgen_insights.rs:147] +
`GraphIndex::coupling_report` [crates/ariadne-graph/src/coupling.rs:90-114]:
`for_project` builds the crate specs from `scoped` modules (DocScope = Source-
only, tests excluded) but "the graph itself is never filtered [D3]"
[docgen.rs:303-309]. `metrics_for` counts any incoming edge whose source is not
in the same spec as afferent — and out-of-scope test symbols are in NO spec, so a
crate's OWN tests calling its OWN source (e.g. e2e `tests/slo.rs` →
`src/domain/connect`) count as crate-level afferent, inflating Ca until
instability < 0.3. The tier-03 fixture (`support::core_fixture`, an efferent-only
cli with no test edges) never exercised this, so its `architecture_role_restored_
cli_is_volatile_leaf` assertion passes while the real dogfood does not.

Secondary: dogfood boundary rows are not near-zero — dominated by intra-crate
`→ ariadne-cli::new` rows (`run_index/parse_one/progress_bar/walk_repo → new`),
same-crate `X::new()` Path-call mis-resolutions to an arbitrary cli `new` (a
same-crate resolution tier-01's cross-crate gate does not address).

Unblock requires a decision OUTSIDE this tier's land-and-verify scope: fix the
crate-level coupling so out-of-scope (test) edges do not count as crate afferent
(e.g. `architecture_section` builds `member_of` over ALL modules, not just
`scoped`, so intra-crate test→source edges are excluded) — a docgen design change
needing its own plan/tier. Escalated to the user 2026-06-05; tier left blocked,
held tier-03 changes preserved in working tree + stash@{0}, overview reverted to
the committed honest state.

RESOLUTION (2026-06-05): the two root causes are now sibling tiers this tier
depends on — `tier-03-crate-role-test-scope` (graph: crate-coupling membership over
ALL crate modules so test→source edges are same-crate, fixing the volatile-leaf
Role) and `tier-04-same-crate-shape-abstain` (salsa: a Method/Path callee with no
same-file definition abstains, killing the `X::new()` domain→adapter boundary
flood). `deps` updated to `[tier-01, tier-03, tier-04]` so the gate enforces
ordering; spec-build refuses this tier until both are `completed`. On entry, confirm
both landed, then re-run the verification below — the role + boundary gates now pass
because their causes are fixed, not because the assertions were weakened.

LANDED (2026-06-05). All three deps `completed`. The held rendering changes were
already committed as WIP (`02d26ce`) with the snapshot re-accepted; the stale
`stash@{0}` was dropped. Re-ran `<verification>` on a fresh daemon-stopped re-index
of the committed binary (2064 edges): graph 70/70 (tier-03 upper-bound, leaf-role,
qualified-render assertions green), daemon+mcp 104/104 (warm==cold, memory probe),
architecture + clippy(`-D warnings`) + fmt clean, `ariadne doc` twice byte-identical.
Role: `ariadne-cli` → "Volatile leaf"; the false "Stable foundational" mislabel is
gone for every leaf crate (fail-loud gate did NOT fire). `ariadne-e2e` → "Isolated"
(Ca=0, Ce=0) rather than literal "volatile-leaf" — its cross-crate calls are
method/path shape abstained by D6 and nothing depends on a test crate; the documented
recall/precision boundary (SCIP recovers it). User accepted e2e=Isolated as honoring
the "never falsely foundational" intent. Boundary near-zero (4 qualified
intra-`ariadne-storage` domain→adapter rows; `→ *::new` flood gone), cycles qualified.
The regenerated overview + both status flips committed; docgen tier-03 flipped to
`completed`.
</blockers>
</content>
