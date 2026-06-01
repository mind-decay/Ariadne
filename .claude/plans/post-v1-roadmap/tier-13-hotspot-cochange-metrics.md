---
tier_id: tier-13
title: Hotspot + change-coupling metrics — churn x complexity, logical coupling
deps: [tier-11, tier-11b, tier-12]
exit_criteria:
  - "`file_hotspots` + `symbol_hotspots` rank each unit by the product of project-max-normalized churn and complexity; the known hot unit (high commits AND high complexity) ranks first, a unit with zero churn or zero complexity scores 0."
  - "`co_change_report` emits a coupling edge per file pair whose shared-commit count meets `min_shared_commits` and whose degree `shared / mean(revs_a, revs_b)` meets `min_degree`, with entities below `min_revs` excluded (code-maat formula + thresholds)."
  - All result + config types live in `ariadne-graph`; `ariadne-core` is unchanged (inputs already exist there).
  - Both use cases are pure and deterministic — re-running on the same inputs yields byte-identical reports (no clock, no RNG); insta goldens pin the ranking.
  - ADR-0021 records the hotspot scoring, the coupling formula + thresholds, and the rejected alternatives.
  - "`cargo nextest run -p ariadne-graph` + `cargo test --test architecture` + clippy + fmt all green."
status: completed
completed: 2026-06-02
---

<context>
tier-11 persisted file-level churn (`CHURN` → `FileChurn`) + co-change (`CO_CHANGE` → `CoChangePair`); tier-11b persisted per-symbol churn (`SYMBOL_CHURN` → `SymbolChurn`); tier-12 added `SymbolRecord.complexity` (McCabe). This tier turns that raw signal into two ranked analytics, both as pure `ariadne-graph` use cases (no new dependency, no crate). Hotspots surface code that is both frequently changed AND complex — the strongest predictor of defect/maintenance cost — at file grain (`CHURN` x aggregate complexity) and symbol grain (`SYMBOL_CHURN` x per-symbol complexity). Change coupling surfaces files that change together despite no static edge (plan RD7/RD8, Block C). MCP/daemon exposure of these reports is tier-15. Full context: plan.md.
</context>

<decisions>
- **D1 — result + config types live in `ariadne-graph`, not `ariadne-core`.** Computed analytics outputs follow the established convention: `CouplingReport`/`CouplingMetrics` [src: crates/ariadne-graph/src/coupling.rs:38-62], `DeadCodeReport`/`DeadSymbol`/`DeadCodeConfig` [src: crates/ariadne-graph/src/dead.rs:14-45], `BlastRadius` all live in `ariadne-graph`; `ariadne-core` holds only persisted records and the daemon IPC wire DTOs (`CouplingReport` et al. in `core/domain/daemon/response.rs:53-110`), which are added when a tool is exposed = tier-15. All use-case inputs already exist in core: `FileChurn`/`CoChangePair`/`SymbolChurn` [src: crates/ariadne-core/src/domain/records.rs:66-131] and `SymbolRecord.complexity` [src: same:58]. tier-13 therefore touches only `ariadne-graph`. *Rejected:* defining the types in core now (draft) — breaks the analytics-output convention and adds core types nothing consumes until tier-15.
- **D2 — hotspot score = product of project-max-normalized churn and complexity.** A hotspot is code that is *both* changed often *and* complex [src: https://docs.enterprise.codescene.io/versions/4.0.16/guides/technical/hotspots.html — change frequency x complexity-proxy overlap (fetched this session); Tornhill, "Your Code as a Crime Scene", 2015]. Each factor is max-normalized over the input set (`x / max(x)`, → 0 when `max == 0`); `score = norm_churn * norm_complexity` ∈ [0,1]. The product makes zero in either factor ⇒ 0, exactly encoding the AND. *Rejected:* weighted sum — a unit high on one axis alone ranks as a hotspot, violating the AND CodeScene/Tornhill describe; single-metric ranking — ignores one axis. CodeScene discloses no exact formula, so this is Ariadne's deterministic operationalization (ADR-0021).
- **D3 — coupling degree = `shared_commits / mean(revs_a, revs_b)`, with code-maat's three thresholds.** The canonical reference implementation (Tornhill's code-maat, the tool the draft's X-Rays source cites) computes `degree = shared-revs / average(revs_a, revs_b)` [src: https://github.com/adamtornhill/code-maat/blob/master/src/code_maat/analysis/logical_coupling.clj — `coupling (m/as-percentage (/ shared-revs average-revs))` (fetched this session)]. `CoChangeConfig` mirrors code-maat's filters — `min_revs` (default 5: min individual revisions to include an entity), `min_shared_commits` (default 5: support), `min_degree` (default 0.30: min coupling) [src: https://github.com/adamtornhill/code-maat/blob/master/README.md (fetched this session)]. `degree ∈ [0,1]` since `shared ≤ min(revs) ≤ mean(revs)`. *Rejected:* Jaccard `shared / (a + b − shared)` (the draft text, but not what code-maat computes); directional association-rule confidence `shared / revs_a` (asymmetric — needs two values per unordered `CoChangePair`).
- **D4 — pure, deterministic, total-ordered output.** Both use cases are free functions over owned inputs (mirroring `attribute_symbol_churn` [src: crates/ariadne-graph/src/symbol_churn.rs:56-106]); no clock, no RNG. Reports sort by score/degree descending, ties broken by key ascending (path, then `SymbolId`), so re-runs are byte-identical and goldens stable. f32 arithmetic over integer inputs is reproducible.
</decisions>

<files>
- crates/ariadne-graph/src/hotspot.rs — new: `HotspotGrain`, `HotspotEntry`, `HotspotReport`; `file_hotspots` + `symbol_hotspots` use cases.
- crates/ariadne-graph/src/co_change.rs — new: `CoChangeConfig`, `CoChangeEdge`, `CoChangeReport`; `co_change_report` use case.
- crates/ariadne-graph/src/lib.rs — modify: add `mod hotspot;` + `mod co_change;` and re-export the public types/functions from the façade [src: crates/ariadne-graph/src/lib.rs:11-33].
- crates/ariadne-graph/tests/hotspot.rs — new: ranking + zero-factor + determinism asserts; insta snapshot of the full report.
- crates/ariadne-graph/tests/co_change.rs — new: threshold-filter + degree-formula + determinism asserts; insta snapshot.
- crates/ariadne-graph/tests/snapshots/ — new: accepted `hotspot__*`/`co_change__*` snapshots.
- docs/adr/0021-hotspot-cochange-metrics.md — new (authored at build; confirm 0021 is the next free id).
</files>

<steps>
1. Failing test first (`crates/ariadne-graph/tests/hotspot.rs`): build inputs with one hot file (high `FileChurn.commits` + high aggregate complexity) and a cold file; assert `file_hotspots` ranks the hot file first and a zero-complexity file scores `0.0`. Red — `hotspot` does not exist [src: crates/ariadne-graph/tests/symbol_churn.rs:36-54 for the direct-input + determinism-rerun pattern].
2. Implement `hotspot.rs`. `pub fn file_hotspots(churn: &[FileChurn], file_complexity: &BTreeMap<String, u32>) -> HotspotReport`: one entry per `FileChurn` (churn = `.commits`, complexity = `file_complexity` lookup or 0); `pub fn symbol_hotspots(churn: &[SymbolChurn], symbol_complexity: &BTreeMap<SymbolId, u32>) -> HotspotReport`: one entry per `SymbolChurn`. Each function max-normalizes churn and complexity over its own input set, sets `score = norm_churn * norm_complexity` (compute in f64, narrow to f32 — `coupling.rs:117-120` precedent), tags `grain`, sorts per D4 (D2). File complexity is `Σ SymbolRecord.complexity` over the file's symbols (functions dominate; complexity stands in for LOC, which Ariadne does not store) — the composition root aggregates it in tier-15; tier-13 builds the map in tests.
3. Failing test first (`tests/co_change.rs`): a pair above all thresholds is returned with the expected `degree`; a pair below `min_shared_commits`, and an entity below `min_revs`, are excluded. Red — `co_change` does not exist.
4. Implement `co_change.rs`. `pub fn co_change_report(churn: &[FileChurn], pairs: &[CoChangePair], cfg: &CoChangeConfig) -> CoChangeReport`: build `path → commits` from `churn`; for each `CoChangePair`, skip if either endpoint is missing or `< cfg.min_revs`, or `pair.count < cfg.min_shared_commits`; `degree = count as f64 / ((a + b) as f64 / 2.0)`; keep if `degree >= cfg.min_degree`; emit `CoChangeEdge { a, b, shared_commits: count, degree: degree as f32 }`; sort per D4 (D3). `impl Default for CoChangeConfig` = `{ min_revs: 5, min_shared_commits: 5, min_degree: 0.30 }`.
5. `lib.rs`: declare both modules and re-export `HotspotGrain`/`HotspotEntry`/`HotspotReport`/`file_hotspots`/`symbol_hotspots` and `CoChangeConfig`/`CoChangeEdge`/`CoChangeReport`/`co_change_report`. Every public item carries a doc comment (`#![deny(missing_docs)]` is active [src: crates/ariadne-graph/src/lib.rs:9]).
6. Add insta snapshots of each full report on a small realistic fixture; review every value by hand (do not blind-`--accept`). Add a re-run equality assert in each test (`assert_eq!(call(), call())`) pinning determinism [src: crates/ariadne-graph/tests/symbol_churn.rs:49-53].
7. Write `docs/adr/0021-hotspot-cochange-metrics.md` from `docs/adr/_template.md`: D2 hotspot scoring, D3 coupling formula + thresholds, the rejected alternatives; status `Accepted`; cite plan RD7/RD8. No new crate/dependency, so `tests/architecture.rs` is unchanged. Run full verification; step 1 + 3 go green.
</steps>

<verification>
- `cargo nextest run -p ariadne-graph` — hotspot ranking (hot-first, zero-factor=0), co-change threshold + degree-formula goldens, and re-run determinism all green.
- `cargo test --test architecture` (no new dep; `ariadne-graph` deps ⊆ {core} unchanged), `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo fmt --all --check`, `RUSTDOCFLAGS=-D warnings cargo doc -p ariadne-graph --no-deps` — green.
- End-to-end (real, not stub): the goldens run the actual use cases over a fixture encoding a known scenario (one hot file/symbol, one coupled pair) and compare the ranked output to a hand-verified expectation. Live `ariadne index` self-index spot-check (top hotspot vs `git log` frequency) is deferred to tier-15, where the MCP tool makes the report invokable.
</verification>

<rollback>
`git checkout -- crates/ariadne-graph && rm -f crates/ariadne-graph/src/hotspot.rs crates/ariadne-graph/src/co_change.rs docs/adr/0021-hotspot-cochange-metrics.md` plus the new snapshots. The metrics are purely additive (no schema, no core, no migration); v1 analytics and the persisted tables are untouched.
</rollback>
