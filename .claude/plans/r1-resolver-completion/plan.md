---
slug: r1-resolver-completion
title: Complete R1 — kill residual phantom cross-crate edges via call-shape-gated resolution
created: 2026-06-04
owners: [user, claude]
review: [user, codex?]
single_tier: false
tiers: [tier-01-call-shape-gate, tier-03-crate-role-test-scope, tier-04-same-crate-shape-abstain, tier-02-land-docgen-tier-03]
---

<context>
Problem (one sentence): the index-time call resolver still binds a method/path
callee cross-crate by bare name to its single workspace definition, so leaf
crates accrue phantom afferent edges and render as "stable foundational".

Success (one measurable sentence): after a fresh re-index, `ariadne-e2e` and
`ariadne-cli` carry near-zero cross-crate afferent edges and render volatile-leaf
(instability > 0.7), while the legitimate same-crate and bare-free-call
cross-crate edges (the `beta::run → alpha::helper` shape) are preserved.

Root cause. ADR-0024 scoped resolution to `same-file → same-crate →
unambiguous-global`; the `unambiguous-global` tier fires for ALL call shapes
[src: crates/ariadne-salsa/src/derive.rs:240-247 `resolve_edges`]. A callee is
captured as a BARE identifier with its qualifier discarded — `socket.connect()`,
`Foo::new()` and a free `helper()` all flatten to `@call.callee`
[src: crates/ariadne-parser/.../queries/rust.scm:35-44; facts.rs:122-128]. So a
method/associated callee whose bare name has exactly one workspace definition
(e.g. `connect` defined only in `ariadne-e2e`) binds cross-crate, the phantom.

The discriminating fact already exists in every grammar: free-identifier calls
sit in a different query sub-pattern than member/scoped calls
[src: queries/{rust,typescript,python,go,csharp,cpp,…}.scm]. The fix captures
that shape and refuses the cross-crate `unambiguous-global` tier for non-free
shapes — a structural rule, no name denylist. This completes the partial R1 fix
(ADR-0024) and unblocks docgen-overview-fidelity tier-03 (`status: blocked`)
[src: .claude/plans/docgen-overview-fidelity/tier-03-reenable-on-reliable-edges.md].

In scope: a `CallKind` on captured call sites (parser); its u8 mirror on the
salsa fact input; the cross-crate gate in `resolve_edges`; re-index + the held
docgen tier-03 landing. Out of scope: capturing the call qualifier or
receiver-type/path resolution (deferred to SCIP, ADR-0024 alternatives;
`scip-driven-edges` plan owns it) [src: docs/adr/0024-scoped-call-resolution.md];
the `SymbolId` scheme; any new edge kind.

Relationship to `scip-driven-edges` (complementary, user decision). SCIP edges
are opt-in (`--scip`, default off) with per-file tree-sitter fallback
[src: .claude/plans/scip-driven-edges/plan.md D4, R1]; the committed/dogfood
overview rides tree-sitter edges, so this resolver fix is the default-path fix
and unblocks tier-03 now. SCIP later RECOVERS the cross-crate recall this trades
away [src: scip-driven-edges plan.md D3].

Follow-up (2026-06-05). Rendering the full Role column on the dogfood for the
first time (tier-02 step 4) surfaced TWO causes tier-01 did not address, in two
different crates: (1) **Role mislabel (graph).** `metrics_for` counts an edge from
a symbol in NO spec as afferent, so a crate's own test→source edge inflates crate
Ca and leaf crates read "Stable foundational" [src: docgen.rs:304-316;
coupling.rs:90-114] → D5 / tier-03. (2) **Boundary flood (salsa).** A same-crate
`X::new()` Path call binds by bare name to an unrelated same-crate `new`; the
intra-crate cross-layer edge is flagged domain→adapter, blocking tier-02's
near-zero gate [src: tier-02 `<blockers>`; docgen_insights.rs:259-268] → D6 /
tier-04. Both added IN scope (user 2026-06-05); per-file role and qualifier-aware
`Foo::new` stay out of scope (SCIP).
</context>

<constraints>
- No symbol-name denylist; abstention is driven by structure (call shape +
  definition count), never spelling [src: ADR-0024; god-module-suggestion-fix
  plan.md D2].
- Deterministic: same input → identical edge set; sorted/`BTree` containers only
  [src: crates/ariadne-graph/src/docgen.rs:1-8; ADR-0024 rationale].
- Hexagonal: `ariadne-salsa` depends only on `ariadne-core` + `ariadne-storage`;
  the `CallKind`→u8 mapping lives at the cli/daemon composition root, never in
  salsa [src: tests/architecture.rs; crates/ariadne-salsa/src/derive.rs:14-18].
- Warm==cold and incremental==fresh parity preserved (one shared derivation)
  [src: post-v1-roadmap plan.md RD11].
- `salsa::Update`-safe fact fields only: `u8` has an auto-impl; a fieldless enum
  does not — mirror the existing `visibility_byte` pattern
  [src: https://docs.rs/salsa/0.26.2/salsa/trait.Update.html; derived.rs:53-58].
- SLOs hold (cold <60s, incr p95 <500ms, query p95 <100ms); per-tier
  `memory_report()` delta reported, >256MB/table hard fail; no new dependency,
  pure-Rust critical path [src: ariadne-core plan.md `<risks>`, D5].
- Spike-first: a failing test reproduces the symptom (phantom edge / mislabeled
  role) and pins the branch before any edit [src: CLAUDE.md workflow].
</constraints>

<decisions>
**D1 — Gate the cross-crate `unambiguous-global` tier to FREE-identifier calls.**
Capture each call site's shape (`Free` | `Method` | `Path`); the
`same-file → same-crate` tiers stay shape-blind, but the `unambiguous-global`
(cross-crate) fallback fires only for `Free`. A `Method`/`Path` callee with no
same-file/same-crate definition yields no edge — the `socket.connect()` phantom
[src: derive.rs:240-247]. *Rejected — require an import binding* (the prompt's
literal phrasing): `beta::run → alpha::helper` has no `use` (drops a legitimate
edge, breaks the recall test + ADR-0024 guarantee), and method/associated calls
never import their callee name; ADR-0024 already rejected package-level
import-visible [src: scoped_resolution.rs:177-199; ADR-0024 alternatives].
*Rejected — drop the global fallback entirely* (same-crate only): also drops
beta→alpha [src: scoped_resolution.rs:180-199].

**D2 — Shape is read from each grammar's existing call sub-patterns, by capture
name.** Relabel `@call.callee` → `@call.free` / `@call.method` / `@call.path`
per grammar so the grammar's own pattern structure declares the shape; `facts.rs`
maps the suffix → `CallKind`. *Rejected — infer shape from the captured node's
parent kind in `facts.rs`*: needs a per-grammar node-kind table (`field_expression`
vs `member_expression` vs `selector_expression` vs `attribute`), duplicating
knowledge the `.scm` already encodes [src: queries/*.scm; tree-sitter capture +
negated-field syntax https://tree-sitter.github.io/tree-sitter/using-parsers/queries/1-syntax.html].

**D3 — `CallKind` crosses into salsa as a `u8` tag on `CallRaw`, mapped at the
composition root.** `ariadne-parser::CallKind` is the parser type; `CallRaw`
gains `kind_byte: u8`; `convert_facts` (cli + daemon) maps it, mirroring
`decl_kind_tag`/`visibility_byte`. Keeps salsa free of an `ariadne-parser` dep
and `Update`-safe [src: derived.rs:79-84,53-58; cli mod.rs:474-522;
daemon facts.rs:76-124].

**D4 — Land docgen-overview-fidelity tier-03 here on the now-reliable edges
(user decision).** After the resolver fix re-indexes, tier-02 re-runs that tier's
`<verification>`, lands its held rendering changes, regenerates the overview, and
flips its `status` blocked→completed [src: docgen tier-03 `<blockers>`].

**D5 — Crate-level coupling membership spans ALL crate modules; rows stay
source-only (tier-03).** `architecture_section` builds the crate-coupling specs
from the unscoped `modules`, so a crate's test symbols are same-crate and an
intra-crate test→source edge is dropped from afferent; a row is emitted only for a
crate with ≥1 scoped (source) member. *Rejected — change `metrics_for`*: that is
the public per-file `coupling_report` contract (warm==cold golden); the artifact
is specific to the crate-aggregation, not the metric [src: coupling.rs:90-114;
docgen.rs:304-316].

**D6 — A Method/Path callee resolves only via same-file; no same-crate or global
tier (tier-04).** Without the discarded qualifier a same-crate bare-name match for
a non-Free shape is a guess (`ProgressBar::new()` → an unrelated same-crate `new`),
so restrict same-crate + unambiguous-global to `Free`. *Rejected — uniqueness-gate
the same-crate tier*: insufficient — the collision name (`new`) has exactly one
same-crate def in the caller crate, so it still binds [src: grep: 1 `fn new` in
ariadne-cli/src]. *Rejected — qualifier/receiver-type resolution*: SCIP (ADR-0024
deferred). Supersedes ADR-0024's same-crate clause for non-Free shapes via ADR-0025
[src: derive.rs:276-283].
</decisions>

<architecture>
Two existing components, no new one. Edge pipeline (tier-01, D1–D3, D6):
`ariadne-parser` adds `CallKind` + `.scm` shape relabel; the cli/daemon
composition root maps `CallKind`→`CallRaw.kind_byte`; `ariadne-salsa`
`resolve_edges` gates same-crate (D6) + cross-crate (D1) tiers by shape. Doc
pipeline (D5): `ariadne-graph` `architecture_section` scopes only its
crate-coupling membership. Edges are derived → re-index, no redb migration; cold /
warm / incremental share the one `resolve_edges` pass [src: db.rs:300-328].
</architecture>

<tech_inventory>
| tech | version | role | doc verified this session |
|---|---|---|---|
| tree-sitter | 0.26.8 (repo pin) | `.scm` capture names + `!field` negation (Java object-less call) | https://tree-sitter.github.io/tree-sitter/using-parsers/queries/1-syntax.html (negated fields confirmed) |
| salsa | 0.26.2 (repo pin) | `u8` is `Update`-auto; fieldless enum is not → `kind_byte: u8` | https://docs.rs/salsa/0.26.2/salsa/trait.Update.html (Update auto-impl list) |
| redb | 4.1.0 (repo pin) | none — edges derived, re-index regenerates, no migration | crates/ariadne-core/.../records.rs (EdgeRecord unchanged) |
| petgraph | repo pin | `edges_directed(n, Incoming)` = "all edges TO n", `Outgoing` = "all edges FROM n" — grounds the afferent/efferent correctness of the D5 membership fix | https://docs.rs/petgraph/latest/petgraph/graph/struct.Graph.html#method.edges_directed (verified this session) |

Context7 quota exhausted this session; web sources are the sanctioned fallback
[src: CLAUDE.md `<rules>` per-session doc fetch].
</tech_inventory>

<risks>
| id | risk | likelihood | mitigation |
|---|---|---|---|
| R1 | residual e2e/cli afferent are FREE-call name collisions, not method/path ⇒ gate alone misses them | medium | spike classifies the residual edges by shape before editing; if free-call collisions remain, document them as the genuine recall/precision boundary (SCIP territory) — never add a denylist; "near-zero" not "exactly zero" |
| R2 | `.scm` relabel drops or double-captures a call site (capture-name typo) ⇒ edge-count regression | medium | parser fact tests assert call counts per shape per language; full `ariadne-parser` suite + dogfood edge-count parity |
| R3 | adding `kind_byte` to the `CallRaw` salsa input breaks warm==cold / incremental==fresh parity | low | both `convert_facts` sites map identically; the equivalence + incremental parity suites stay green |
| R4 | tier-01 edge-set change churns the committed `ariadne-graph` docgen snapshot, overlapping tier-02 | medium | tier-01 stashes the held docgen tier-03 working-tree changes, re-accepts only edge-driven snapshot churn with review; tier-02 lands the rendering changes |
| R5 | per-table memory grows past budget | low | +1 byte/call site (u8); `memory_report()` delta reported in tier-01; >256MB/table hard fail |
| R6 | the D5 membership change accidentally alters the displayed row set (count ≠ 12) or shifts a layer vote | medium | rows computed from the SCOPED crate set + layer_votes kept over `scoped`; tier-03 asserts row count unchanged and the existing layer-pin test stays green |
| R7 | D6 same-crate Method/Path abstention over-drops legitimate cross-file same-crate method edges, shrinking the graph and breaking analytics recall | medium | tier-04 MEASURES the residual boundary classes on a committed reindex BEFORE editing; recall guard = warm==cold + incremental==fresh + a same-file Method control must stay green; if a legitimate-edge test breaks, narrow the rule (hard fail, do not weaken the test) |
| R8 | working tree intermingles tier-01's uncommitted resolver work with the held docgen graph changes ⇒ a wrong-scoped commit | medium | tier-04 step 1 commits ONLY tier-01 paths (parser/salsa/cli-mod/daemon-facts/ADR); graph docgen files stay uncommitted for tier-02 to land |
</risks>

<verification>
- [tier-01] Method/Path cross-crate phantom spike red→green; per-shape parse +
  no-edge tests green; `beta::run → alpha::helper` recall stays green.
- [tier-03] A graph fixture with a test-module→source-symbol edge renders the
  source crate volatile-leaf; red without the D5 membership fix, green with it;
  per-file `coupling_report` golden byte-unchanged.
- [tier-04] A same-crate-different-file Method/Path callee yields NO edge (red on
  the committed resolver, green after D6); same-file Method + beta→alpha Free
  recall controls stay green; residual boundary rows classified BEFORE the edit.
- [tier-04+02] Fresh daemon-stopped re-index of the COMMITTED binary twice →
  identical edge set; cli/e2e render volatile-leaf, boundary near-zero with no
  `→ *::new` domain→adapter phantom; `ariadne doc` twice byte-identical; docgen
  tier-03 `<verification>` re-run passes and its `status` → completed.
- `cargo test --test architecture`, `cargo clippy … -D warnings`,
  `cargo fmt --all --check`, `cargo deny check` green; memory probe within budget.
</verification>

<sources>
- Resolver + branch: crates/ariadne-salsa/src/derive.rs:14-18,68-95,220-278 ; db.rs:239-328
- Parser facts + shape capture: crates/ariadne-parser/src/adapters/treesitter/facts.rs:113-160,342-389 ; queries/{rust,typescript,tsx,javascript,python,go,java,kotlin,csharp,c,cpp}.scm
- Salsa fact input + tests: crates/ariadne-salsa/src/derived.rs:29-102 ; convert_facts cli mod.rs:474-522 ; daemon facts.rs:76-124 ; tests/scoped_resolution.rs ; ariadne-cli/tests/doc_command.rs:19-22
- Precedent + blocked tier: docs/adr/0024-scoped-call-resolution.md ; scip-driven-edges/plan.md ; god-module-suggestion-fix/plan.md D2 ; docgen-overview-fidelity/tier-03-reenable-on-reliable-edges.md
- D5/D6 loci: crates/ariadne-graph/src/{coupling.rs:90-114,docgen.rs:304-316,docgen_insights.rs:147-193,259-268} ; derive.rs:276-283 ; new docs/adr/0025-shape-scoped-same-crate-resolution.md
- External: https://tree-sitter.github.io/tree-sitter/using-parsers/queries/1-syntax.html ; https://docs.rs/salsa/0.26.2/salsa/trait.Update.html ; https://docs.rs/petgraph/latest/petgraph/graph/struct.Graph.html#method.edges_directed
</sources>
