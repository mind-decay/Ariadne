# ADR-0021: Hotspot Scoring And Change-Coupling Metrics

<status>
Accepted
Date: 2026-06-02
Decider: claude
</status>

<context>
tier-11/11a persisted file-level churn (`FileChurn`) + co-change (`CoChangePair`),
tier-11b per-symbol churn (`SymbolChurn`), tier-12 per-symbol `complexity` —
all raw signal. tier-13 turns it into two ranked analytics: hotspots (churn ×
complexity) and change coupling (logical coupling). Both are pure
`ariadne-graph` use cases over inputs that already exist in `ariadne-core`, so
the forces are reliability (deterministic, no inference, per [no-llm-features]),
maintainability (one operationalization, recorded here, not scattered), and
fidelity to the established references. The metric basis is plan RD7 (git
history → churn/co-change) and RD8 (McCabe complexity)
[src: post-v1-roadmap plan.md RD7, RD8].
</context>

<decision>
Hotspot score is the product of project-max-normalized churn and complexity:
each factor is `x / max(x)` over the input set (`0` when `max == 0`),
`score = norm_churn * norm_complexity` ∈ [0, 1]. Change coupling uses code-maat's
degree `shared / mean(revs_a, revs_b)`, gated by code-maat's three filters —
`min_revs` (5), `min_shared_commits` (5), `min_degree` (0.30). Both are free
functions over owned inputs, output total-ordered (score/degree descending, key
ascending), so a re-run is byte-identical.
</decision>

<rationale>
- **Product, not sum (hotspot)** — fidelity. A hotspot is code that is *both*
  changed often *and* complex; the product forces a zero in either factor to a
  zero score, encoding the AND that a weighted sum would let a single high axis
  defeat [src: https://docs.enterprise.codescene.io/versions/4.0.16/guides/technical/hotspots.html;
  Tornhill, "Your Code as a Crime Scene", 2015].
- **Project-max normalization** — reliability / comparability. `x / max(x)` maps
  each axis to [0, 1] over the analyzed set so churn (commit counts) and
  complexity (McCabe) compose despite incomparable raw units; `max == 0` ⇒ `0`
  keeps an all-zero factor inert. f64 arithmetic over integer inputs narrowed to
  f32 is reproducible and matches the `coupling.rs` ratio precedent
  [src: crates/ariadne-graph/src/coupling.rs:117-120].
- **code-maat degree (coupling)** — fidelity. The canonical reference
  implementation computes `coupling = shared-revs / average(revs_a, revs_b)`;
  Ariadne mirrors it exactly rather than inventing a formula. `degree ∈ [0, 1]`
  since `shared ≤ min(revs) ≤ mean(revs)`
  [src: https://github.com/adamtornhill/code-maat/blob/master/src/code_maat/analysis/logical_coupling.clj].
- **code-maat's three default thresholds** — reliability. `min_revs`/
  `min_shared_commits`/`min_degree` (5/5/0.30) are code-maat's published
  defaults, suppressing low-support noise; exposed as `CoChangeConfig` so a
  caller can retune without a code change
  [src: https://github.com/adamtornhill/code-maat/blob/master/README.md].
- **Pure free functions, total order (both)** — reliability. No clock, no RNG;
  ties broken by key ascending (path, then `SymbolId`) so goldens are stable,
  mirroring `attribute_symbol_churn` [src: crates/ariadne-graph/src/symbol_churn.rs:56-106].
</rationale>

<alternatives>
- **Weighted sum of churn + complexity (hotspot)** — rejected: a unit high on
  one axis alone ranks as a hotspot, violating the AND CodeScene/Tornhill
  describe [src: tier-13 D2].
- **Single-metric ranking (churn only, or complexity only)** — rejected: ignores
  one of the two axes the hotspot definition requires [src: tier-13 D2].
- **Jaccard `shared / (a + b − shared)` (coupling)** — rejected: the draft text,
  but not what code-maat — the reference the source cites — computes
  [src: tier-13 D3].
- **Directional confidence `shared / revs_a` (coupling)** — rejected: asymmetric,
  needing two values per unordered `CoChangePair` [src: tier-13 D3].
</alternatives>

<consequences>
- All result + config types (`HotspotGrain`/`HotspotEntry`/`HotspotReport`,
  `CoChangeConfig`/`CoChangeEdge`/`CoChangeReport`) live in `ariadne-graph`,
  following the analytics-output convention (`CouplingReport`, `DeadCodeReport`,
  `BlastRadius`); `ariadne-core` is unchanged. No new crate, no new dependency,
  no schema change — purely additive, so `tests/architecture.rs` is unchanged
  [src: crates/ariadne-graph/src/coupling.rs; crates/ariadne-graph/src/dead.rs].
- File-grain complexity is `Σ SymbolRecord.complexity` over a file's symbols
  (complexity stands in for LOC, which Ariadne does not store); tier-13 builds
  that map in tests, and the composition root aggregates it when the report is
  exposed as an MCP tool (tier-15). The daemon IPC wire DTOs are added then.
- CodeScene discloses no exact hotspot formula, so the product-of-normalized
  score is Ariadne's deterministic operationalization, fixed here. Off-limits
  without superseding: changing the score basis (product → sum), the coupling
  degree formula, or moving the types to `ariadne-core` — each breaks the
  goldens or the analytics-output convention and needs a new ADR.
</consequences>

<sources>
- `[src: https://docs.enterprise.codescene.io/versions/4.0.16/guides/technical/hotspots.html]`
- `[src: Tornhill, "Your Code as a Crime Scene", 2015]`
- `[src: https://github.com/adamtornhill/code-maat/blob/master/src/code_maat/analysis/logical_coupling.clj]`
- `[src: https://github.com/adamtornhill/code-maat/blob/master/README.md]`
- `[src: .claude/plans/post-v1-roadmap/plan.md RD7, RD8]`
- `[src: .claude/plans/post-v1-roadmap/tier-13-hotspot-cochange-metrics.md D2, D3]`
- `[src: crates/ariadne-graph/src/coupling.rs]` (ratio + analytics-output precedent)
</sources>
