---
tier_id: tier-12
title: Hotspot + change-coupling metrics — churn x complexity, logical coupling
deps: [tier-10, tier-11]
exit_criteria:
  - A `hotspot_report` ranks files/symbols by normalized churn combined with cyclomatic complexity.
  - A `co_change_report` reports logical-coupling pairs above a configurable support/confidence threshold.
  - Both use cases are deterministic and golden-tested on a fixture repo with known history.
  - `cargo nextest run -p ariadne-graph` + architecture + clippy + fmt all green.
status: pending
---

<context>
tier-10 ingested churn + co-change; tier-11 added cyclomatic complexity. This tier turns that raw signal into two ranked metrics. Hotspots surface code that is both frequently changed and complex — the strongest predictor of defects and maintenance cost; change coupling surfaces files that change together despite no static edge (plan RD7/RD8, Block C). Full context: plan.md.
</context>

<files>
- crates/ariadne-graph/src/hotspot.rs — new: hotspot ranking use case.
- crates/ariadne-graph/src/co_change.rs — new: logical-coupling use case.
- crates/ariadne-core/src/domain/ — modify: `HotspotEntry` + `CoChangeEdge` result types; threshold config.
- crates/ariadne-graph/tests/ — new: hotspot + co-change goldens.
- crates/ariadne-graph/fixtures/ — modify/ensure a fixture repo with known churn and complexity.
</files>

<steps>
1. Failing test first (`ariadne-graph` tests): over a fixture with one known hot file (high commit count + high complexity) and one known co-change pair, assert `hotspot_report` ranks the hot file first and `co_change_report` returns the pair. Red — neither use case exists.
2. Implement `hotspot.rs`: rank each file/symbol by combining commit frequency (the primary hotspot criterion) with cyclomatic complexity — code that changes often *and* is complex scores highest [src: https://docs.enterprise.codescene.io/versions/4.0.16/guides/technical/hotspots.html ; Tornhill, "Your Code as a Crime Scene", 2015]. Normalize each input to [0,1] over the project, then rank by their product so a unit with zero churn or zero complexity is not a hotspot.
3. Implement `co_change.rs`: from the `CO_CHANGE` table, emit a coupling edge for each pair whose co-change count meets a minimum support and whose confidence (co-changes / changes-of-either) meets a threshold; both thresholds are config-driven [src: https://understandlegacycode.com/blog/key-points-of-software-design-x-rays/].
4. Keep both pure and deterministic — no time-of-day, no RNG; the same index yields the same ranking (audit reproducibility).
5. Define `HotspotEntry`/`CoChangeEdge` in `ariadne-core`; goldens with `insta`.
</steps>

<verification>
- `cargo nextest run -p ariadne-graph` — hotspot + co-change goldens green; re-running yields identical output (determinism).
- Manual: `hotspot_report` on the ariadne_v2 self-index; confirm the top entry is a plausibly churn-heavy, complex file (cross-check `git log` frequency).
- `cargo test --test architecture`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check` — green.
</verification>

<rollback>
`git checkout -- crates/ariadne-graph crates/ariadne-core`. The metrics are additive; v1 analytics are untouched.
</rollback>
