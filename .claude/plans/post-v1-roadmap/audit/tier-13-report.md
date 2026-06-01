---
tier_id: tier-13
audited: 2026-06-02
verdict: PASS
commit: 0052cefd91ae2c82790a3fbbb151bccf067b2bfe
---

# Audit — tier-13 Hotspot + change-coupling metrics

<scope>
Reviewed the tier-13 diff against `plan.md` (RD7/RD8) and the tier file's
`<decisions>` (D1–D4), `<steps>`, `<verification>`, `<exit_criteria>`.

Scoped diff (`git diff` + untracked, filtered to `<files>`):
- `crates/ariadne-graph/src/hotspot.rs` (new) — `HotspotGrain`/`HotspotEntry`/`HotspotReport`, `file_hotspots`, `symbol_hotspots`.
- `crates/ariadne-graph/src/co_change.rs` (new) — `CoChangeConfig`/`CoChangeEdge`/`CoChangeReport`, `co_change_report`.
- `crates/ariadne-graph/src/lib.rs` (modify) — `mod` decls + façade re-exports only.
- `crates/ariadne-graph/tests/hotspot.rs`, `tests/co_change.rs` (new) — behaviour + determinism asserts + insta snapshots.
- `crates/ariadne-graph/tests/snapshots/{hotspot__file_hotspots,hotspot__symbol_hotspots,co_change__co_change}.snap` (new).
- `docs/adr/0021-hotspot-cochange-metrics.md` (new).
- `.claude/plans/post-v1-roadmap/tier-13-…md` (modify) — status flip + plan refinement.

Read end-to-end: every file above. `ariadne-core/src/domain/records.rs` read to
confirm inputs (`FileChurn`, `CoChangePair`, `SymbolChurn`, `SymbolRecord.complexity`)
exist there and are unchanged.
</scope>

<checks_run>
All `<verification>` commands re-run from a clean tree:
- `cargo nextest run -p ariadne-graph` → 39 passed, 0 failed (incl. 4 new: 2 hotspot, 2 co_change).
- `cargo test --test architecture` → 1 passed (`ariadne-graph` deps still ⊆ {core}; no new dep).
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` → exit 0, no warnings.
- `cargo fmt --all --check` → exit 0.
- `RUSTDOCFLAGS="-D warnings" cargo doc -p ariadne-graph --no-deps` → exit 0 (`#![deny(missing_docs)]` satisfied).

Hand-verification of every snapshot value (not blind-accepted):
- file hotspots: max churn 10, max cx 25 → hot.rs 1.0·0.8=0.8000; complex_stable 0.1·1.0=0.1000; cold 0.2·0.2=0.0400; churned_simple 0.8·0=0.0000. Order + values match snapshot.
- symbol hotspots: max churn 10, max cx 12 → sid1 1.0; sid2 0.3·0.333=0.1000; sid3 0.6·0=0.0000. Match.
- co_change: a–b 6/9=0.6667; b–d 7/14=0.5000; c–d 5/16=0.3125 kept; a–c (count<5), a–rare (rare revs<5), big1–big2 (0.1429<0.30), a–missing (absent) all dropped. Match.

Scope check (`git diff --stat`): only `lib.rs` (+4) and the tier file modified;
`ariadne-core` and every other crate untouched; no `Cargo.toml` change (insta
already a dev-dep). ADR id 0021 confirmed next free (0020 = cyclomatic).

Library-behaviour citations re-grounded:
- code-maat degree `shared-revs / average(revs_a, revs_b)` and defaults (min_revs/min_shared/min_coupling) match D3 [src: https://github.com/adamtornhill/code-maat/blob/master/src/code_maat/analysis/logical_coupling.clj; .../README.md].
- `f64::midpoint` stable since Rust 1.85 = repo MSRV floor (rust-toolchain `stable`, observed 1.95) — available [src: https://doc.rust-lang.org/std/primitive.f64.html#method.midpoint].
</checks_run>

<findings>
| id | category | severity | location | problem | fix |
|----|----------|----------|----------|---------|-----|
| F1 | docs | INFO | tier-13-…md:56-58 | Tier file ends with stray unmatched closing tags `</content></invoke></output>` (no matching opener); cosmetic, no functional impact and outside the implementation deliverable. | Drop the three trailing tags from the plan file. |
</findings>

<verdict>
PASS. Zero FAIL findings; one non-gating INFO.

Every exit criterion independently verified:
1. `file_hotspots`/`symbol_hotspots` rank by product of max-normalized churn×complexity; hot unit first; zero-factor → 0.0 — confirmed in code, tests, and hand-checked snapshots.
2. `co_change_report` applies `min_shared_commits`, `min_degree`, and `min_revs` exclusion with degree = `shared/mean(revs_a,revs_b)` — confirmed; each filter outcome asserted.
3. All result/config types in `ariadne-graph`; `ariadne-core` unchanged — confirmed by `git diff --stat`.
4. Pure/deterministic; re-run byte-identical; insta goldens pin ranking — `assert_eq!(call(), call())` present in both tests; free fns, no clock/RNG; total order with key tie-break.
5. ADR-0021 records scoring, coupling formula + thresholds, rejected alternatives (weighted sum, single-metric, Jaccard, directional confidence); status Accepted.
6. nextest + architecture + clippy + fmt + doc all green (re-run this session).

Notes (non-defects): `co_change.rs:89` uses `f64::midpoint(ra, rb)` instead of the
plan step-4 literal `(a+b)/2.0`; numerically identical for u32→f64 inputs (well
within the 2^53 exact range) and documented inline — an equivalent, overflow-safe
form, not a deviation that matters. `degree ∈ [0,1]` rests on the input invariant
`count ≤ min(revs)` from tier-11; trusted persisted input, correctly out of
tier-13's scope.
</verdict>

<next_steps>
Optional cleanup only (does not gate): remove the stray trailing XML tags (F1)
from the tier file. No code changes required; tier-13 is ready to commit.
</next_steps>

<sources>
- [code-maat logical_coupling.clj](https://github.com/adamtornhill/code-maat/blob/master/src/code_maat/analysis/logical_coupling.clj)
- [code-maat README (defaults)](https://github.com/adamtornhill/code-maat/blob/master/README.md)
- [CodeScene hotspots (change-freq × complexity)](https://docs.enterprise.codescene.io/versions/4.0.16/guides/technical/hotspots.html)
- [f64::midpoint — std](https://doc.rust-lang.org/std/primitive.f64.html#method.midpoint)
- [OWASP Top 10](https://owasp.org/www-project-top-ten/) (security pass: no input-trust, injection, or secret surface — pure functions over owned domain types)
</sources>
