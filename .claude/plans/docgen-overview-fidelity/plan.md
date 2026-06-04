---
slug: docgen-overview-fidelity
title: Docgen project-overview fidelity — honest sections now, real edges later
created: 2026-06-04
owners: [user, claude]
review: [user, codex?]
single_tier: false
tiers: [tier-01-scope-and-suppress, tier-02-edge-resolution, tier-03-reenable-on-reliable-edges]
---

<context>
`ariadne doc` renders `docs/codebase-overview.md` from `ariadne-graph` docgen.
A bug-hunt this session (same family as the shipped god-module-suggestion-fix)
found the overview ships misleading signal from two root causes:
- **R1 — phantom cross-crate edges.** Index-time call resolution binds a callee
  *name* to a workspace symbol, so ubiquitous calls (`Vec::new()`, `.build()`)
  collapse onto one arbitrary same-named symbol. PROOF: `apply_writes`
  (`crates/ariadne-storage/src/adapters/redb/apply.rs`) calls only std
  `Vec::new()`, yet the graph carries `apply_writes → new` flagged
  "adapter → adapter cross-crate"; `cargo test --test architecture` is green, so
  no Cargo-level adapter→adapter dep exists and the edge is provably spurious.
- **R2 — inconsistent doc-scope + raw rendering.** Some sections apply
  `DocScope`, others do not; symbols render as bare names; dedup keys on rendered
  strings [src: crates/ariadne-graph/src/docgen_insights.rs:191-230,409-461].

Scope: fix the docgen layer to stop shipping false claims (T1), fix R1 at the
indexer (T2), then re-enable the suppressed sections on reliable edges (T3).
Out of scope: Risk hot-spots (verified correct this session); new metrics; any
rename of shipped `refactor_suggestions`/coupling JSON contracts.
</context>

<constraints>
- Deterministic: `BTreeMap`/`BTreeSet` + sorted vecs; same revision → identical
  bytes [src: crates/ariadne-graph/src/docgen.rs:1-8].
- Docgen stays graph-pure, IO-free; the only IO layer is `ariadne doc`
  [src: CLAUDE.md D13; crates/ariadne-cli/src/commands/doc.rs:1-10].
- No name denylist. Excluding `new`/`build`/… by lexical list is non-portable
  across TS/Python/Go/Java/C# and was rejected in god_modules D2
  [src: .claude/plans/god-module-suggestion-fix/plan.md D2].
- No new dependency. Reuse petgraph 0.8 traversal already in docgen
  [src: refactor.rs:94; https://docs.rs/petgraph/0.8.3/petgraph/graph/struct.EdgeReference.html].
- Suppression must be explicit, not silent: a suppressed section prints why it is
  withheld so the doc never reads as "no problems found".
</constraints>

<decisions>
**D1 — Suppress, don't guess, for R1-contaminated sections (user decision).**
Boundary violations, the Architecture Role column, and cross-crate cycle cuts are
built on the edge set R1 corrupts. Until T2 lands, each emits an explicit
"withheld — depends on cross-crate edge accuracy (see R1)" line. *Rejected:* a
docgen-side heuristic guard (filter to corroborated edges, drop constructor
targets) — it partially reintroduces the non-portable name list and still ships
a number nobody can trust [src: this session; god_modules D2].

**D2 — Fix R1 at the resolver, not the symptom (user decision: follow-up tier).**
The phantom is index-time: `edges_added` arrive with resolved `dst`
[src: crates/ariadne-daemon/src/domain/catalog.rs:235-247], and the callee is
captured as bare text [src: crates/ariadne-parser/src/adapters/treesitter/facts.rs:125].
The query-side `find_symbol` first-match is NOT the cause — `boundary_violations`
reads graph edges directly [src: catalog.rs:253-254; docgen_insights.rs:197-198].
T2 pins the resolver by spike, then scopes resolution (same-file → crate →
import-visible) and leaves std/ambiguous callees unresolved (no edge).

**D3 — DocScope applies to every history/graph section, uniformly.** `synopsis`,
`risk_hotspots`, `boundary_violations` already call `scope.include`
[src: docgen_insights.rs:89,200,344]; `change_coupling` and `cycle_clusters` do
not [src: docgen_insights.rs:442-447,259-301]. T1 makes them consistent and drops
trivial "source ⇄ its own test" co-change pairs. *Rejected:* leave co-change
unscoped — the doc claims Source-only and the trivial pairs (degree 1.00) crowd
out real hidden coupling within `LIST_N`.
</decisions>

<architecture>
- `ariadne-graph::docgen_insights` — section renderers; T1 changes
  `change_coupling`, `cycle_clusters`, `synopsis`, `architecture_section`
  (suppress Role), `boundary_violations` (suppress). T3 reverts the suppression.
- `ariadne-graph::doc_model::LayerHint` — T1 widens layer inference for
  flat-`src` domain crates [src: doc_model.rs:100].
- Indexer (locus pinned in T2 spike; candidates: scip ingestion / tree-sitter
  fact-linking) — T2 only. No docgen dependency on the indexer.
- Golden tests under `crates/ariadne-graph/tests/`; regenerated
  `docs/codebase-overview.{md,svg}` via `ariadne doc`.
</architecture>

<tech_inventory>
| Tech | Version | Doc fetched this session |
| --- | --- | --- |
| petgraph (edges_directed / Direction::Outgoing / EdgeReference::source,target) | 0.8 (pinned) | https://docs.rs/petgraph/0.8.3/petgraph/graph/struct.EdgeReference.html (Context7 monthly quota exceeded → docs.rs fallback, confirmed this session) |
</tech_inventory>

<risks>
| Risk | Likelihood | Mitigation | Owner |
| --- | --- | --- | --- |
| R1 resolver locus mis-identified; T2 fix lands in wrong layer | Med | T2 step 1 is a spike: reproduce phantom in a failing test, pin locus before editing | build |
| R1 fix exceeds one tier | Med | If so, T2 exit narrows to "located + failing test + ADR on approach"; R1 impl spins a new plan; T3 then blocks on it | build/user |
| Suppression mistaken for "clean" by readers | Low | D1: explicit withheld-reason line per section | build |
| Layer widening (D3) mislabels a real interior crate | Low | Structural rule + golden test pins each crate's layer; review snapshot | build/audit |
| Re-enabling (T3) still shows residual false positives | Med | T3 exit asserts a fixture-backed upper bound on cross-crate violations, not just "non-empty" | audit |
</risks>

<verification>
- Per-tier `<verification>` blocks are authoritative. Whole-feature done when:
  T1 ships an overview with no misleading rows (suppressed sections explicit);
  T2 turns the `apply_writes → new` reproduction test from red to green;
  T3 re-enables the sections and a fixture asserts cross-crate violations match
  the real (near-zero) count.
- Every tier: `cargo nextest run -p ariadne-graph`, regenerate `ariadne doc`
  twice → byte-identical, `cargo clippy --workspace --all-targets --all-features
  -- -D warnings`, `cargo fmt --all --check`, `cargo test --test architecture`.
</verification>

<sources>
- repo: crates/ariadne-graph/src/docgen_insights.rs:68,89,135,191-230,259-301,409-461;
  doc_model.rs:30-71,100; docgen.rs:392; coupling.rs:90-134;
  crates/ariadne-daemon/src/domain/catalog.rs:235-254;
  crates/ariadne-parser/src/adapters/treesitter/facts.rs:125;
  crates/ariadne-scip/src/normalize/mod.rs:101,145-149;
  crates/ariadne-storage/src/adapters/redb/apply.rs (proof case).
- [petgraph EdgeReference — docs.rs 0.8.3](https://docs.rs/petgraph/0.8.3/petgraph/graph/struct.EdgeReference.html)
- [god-module-suggestion-fix plan — D2 no-denylist precedent](.claude/plans/god-module-suggestion-fix/plan.md)
</sources>
