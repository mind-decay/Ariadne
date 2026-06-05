# ADR-0025: Shape-Scoped Same-Crate Call Resolution

<status>
Accepted
Date: 2026-06-05
Decider: claude (r1-resolver-completion tier-04, R1/D6)
Supersedes: ADR-0024's same-crate clause for non-`Free` (Method/Path) call shapes
</status>

<context>
ADR-0024 scoped index-time resolution to **same-file → same-crate →
unambiguous-global**, and r1-resolver-completion tier-01 then gated the
cross-crate `unambiguous-global` tier to `Free` calls. But the **same-crate**
tier stayed shape-blind: a `Method`/`Path` callee still bound by bare name to a
same-crate definition. The callee is captured as a **bare** identifier — the
qualifier is discarded, so `ProgressBar::new()` is captured as bare `new`
[src: queries/rust.scm:36-44 `@call.path`/`@call.method`; derive.rs `CallKind`].
With the receiver/qualifier gone, a same-crate bare-name match is a guess.

PROOF (measured on the committed tier-01 binary, fresh dogfood reindex, 3339
edges, identical over two runs). Every residual boundary-violation row was
same-crate `domain → adapter`. Four were the phantom: `ariadne-cli`'s domain
callers (`run_index`, `parse_one`, `progress_bar`, `walk_repo`) each issue a
`X::new()` Path call (`ProgressBar::new`, `WalkBuilder::new`, …) whose bare `new`
bound to the crate's lone adapter-layer `DaemonClient::new` — a cross-layer edge
that floods the boundary section and blocks tier-02's near-zero gate
[src: r1-resolver-completion tier-02 `<blockers>`; docgen_insights.rs
`classify_violation`]. `new` is globally ambiguous (defined in 9 crates), so this
is purely the same-crate tier — tier-01's cross-crate `Free` gate cannot reach it
[src: derive.rs `resolve_edges`].

Forces: reliability (every emitted edge must be trustworthy), maintainability
(one resolver, no per-language casing), and the no-denylist constraint
(structure, never spelling) [src: god-module-suggestion-fix plan.md D2].
</context>

<decision>
A `Method`/`Path` callee resolves **same-file only**: the same-crate and
unambiguous-global tiers are refused for non-`Free` shapes. A `Free` callee keeps
the full ADR-0024 ladder unchanged (same-file → same-crate → unambiguous-global),
and render/hook sites keep the full ladder too. Implemented by threading the call
shape into `resolve_edges` as a single `wide_scope` flag — `true` for `Free`
calls and every render/hook, `false` for `Method`/`Path` — so
`in_scope = if wide_scope { same_file.or(same_crate) } else { same_file }` and the
unambiguous-global fallback likewise fires only when `wide_scope`
[src: crates/ariadne-salsa/src/derive.rs `resolve_edges`]. The same-file scope is
the one place a bare member/segment name is lexically unambiguous.
</decision>

<rationale>
- **Reliability.** The phantom is a bare member/segment name bound across a file
  boundary it cannot justify. Refusing the same-crate and global tiers for
  non-`Free` shapes removes exactly that class: the post-fix fresh reindex drops
  the four `ariadne-cli::* → ariadne-cli::new` `domain → adapter` rows; the
  boundary section is left with four genuine same-crate **`Free`** edges
  (`ariadne-storage::migrate_* → decode_value/encode_value`, imported via
  `use crate::adapters::codec`) — real cross-layer calls the resolver correctly
  keeps, no `→ *::new` phantom remains.
- **Reliability (recall preserved where the name is unambiguous).** A same-FILE
  Method call still resolves (the lexically-unambiguous scope), and `Free` calls
  keep same-crate and unambiguous-global resolution — the `beta::run →
  alpha::helper` cross-crate `Free` edge and same-crate `Free` edges all survive
  [src: crates/ariadne-salsa/tests/scoped_resolution.rs].
- **Precision/recall trade (accepted).** Same-crate **cross-file** Method/Path
  edges are dropped (dogfood 3339 → 2064 edges). This is the deliberate trade:
  precise cross-file `Foo::new` needs the discarded qualifier and type→impl
  resolution, which is SCIP's job; SCIP-driven edges later RECOVER this recall
  [src: scip-driven-edges plan.md D3; ADR-0024 deferred alternative]. The
  warm==cold and incremental==fresh parity suites stay green (one shared
  `resolve_edges`), so warm/incremental do not diverge from cold/fresh.
- **Determinism / maintainability.** Candidate lists stay sorted by
  `(file, def_start)`; the rule is one boolean derived from the call shape — no
  lexical name list, no import parsing, no per-language receiver analysis. The
  dogfood reindex reports 2064 edges on repeated runs.
</rationale>

<alternatives>
- **Uniqueness-gate the same-crate tier** (bind same-crate only when the name has
  exactly one same-crate definition) — rejected: insufficient. The collision name
  `new` has exactly **one** same-crate definition in `ariadne-cli`
  (`DaemonClient::new`), so a uniqueness gate still binds the phantom
  [src: grep `fn new` → 1 hit in crates/ariadne-cli/src].
- **Qualifier-aware / receiver-type resolution** (resolve `Foo::new` via the
  receiver type and its impl) — deferred: requires the discarded call qualifier
  and type→impl resolution, which is SCIP's job (ADR-0024 deferred alternative);
  the default tree-sitter path stays structural.
- **Lexical name denylist** (`new`/`build`/…) — rejected: non-portable across
  languages; abstention is driven by call shape, never spelling
  [src: god-module-suggestion-fix plan.md D2; ADR-0024].
</alternatives>

<consequences>
- `ariadne-salsa`'s `resolve_edges` renames the per-site `cross_crate_ok` flag to
  `wide_scope` and uses it to gate the same-crate tier in addition to the
  unambiguous-global tier. `pub(crate)`, internal to the crate; no public-API or
  adapter-boundary change. No redb migration — edges are derived; a reindex
  regenerates them.
- The graph trades same-crate cross-file Method/Path recall for precision until
  SCIP edges land. Analytics over those edges (call graphs, blast radius across
  files within a crate via Method/Path calls) see fewer edges on the default
  tree-sitter path.
- Off-limits without superseding: re-adding a same-crate (or global) bare-name
  fallback for a non-`Free` call shape in `resolve_edges`.
</consequences>

<sources>
- `[src: .claude/plans/r1-resolver-completion/plan.md D1, D6; tier-04-same-crate-shape-abstain.md]`
- `[src: crates/ariadne-salsa/src/derive.rs `resolve_edges`, `CallKind`; tests/scoped_resolution.rs]`
- `[src: crates/ariadne-parser/src/adapters/treesitter/queries/rust.scm:36-44]`
- `[src: crates/ariadne-graph/src/docgen_insights.rs `classify_violation` (boundary rows)]`
- `[src: docs/adr/0024-scoped-call-resolution.md (superseded same-crate clause); scip-driven-edges/plan.md D3]`
- `[src: grep `fn new` crates/ariadne-cli/src → 1 (DaemonClient::new), the uniqueness-gate counter-proof]`
</sources>
