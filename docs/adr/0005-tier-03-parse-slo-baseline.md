## ADR-0005: Tier-03 Parse-SLO Baseline (jQuery 10 MB)

<status>
Accepted
Date: 2026-05-19
Decider: user
</status>

<context>
Tier-03 plan set perf gates against a synthetic 10 MB JavaScript fixture:
cold ≤ 100 ms, single-token incremental ≤ 5 ms. During tier-03 build the
synthetic payload triggered a pathological parse path (1.0 s cold, 123 ms
incremental). Switching to a real-world payload — jQuery 3.7.1, MIT — and
replicating it to 10 MB yielded the workload-realistic baseline. The
synthetic profile is not representative of what indexing OSS repos looks
like, and the 100 ms cold target sits ~3–5x above stock tree-sitter's
published throughput on Apple Silicon (~20–40 MB/s for JS) [src:
https://github.com/tree-sitter/tree-sitter — "Parsing Performance"].
</context>

<decision>
Adopt the jQuery-replicated 10 MB fixture as tier-03's canonical perf
workload, and set baseline gates accordingly:
- **cold parse** ≤ 700 ms on the bench host (10 MB jQuery, p50)
- **single-token incremental** ≤ 5 ms (unchanged — incremental hot-path
  measured ~0.6 ms, headroom intentional)

Tier-10 e2e remains the canonical full-workload gate (cold-index < 60 s on
the 100K-file plan budget; incremental p95 < 500 ms). The tier-03 numbers
are the per-file lower bound the rest of the pipeline composes against.
</decision>

<rationale>
- **Efficiency.** Stock tree-sitter on the JavaScript grammar parses at
  ~20 MB/s on Apple-Silicon dev hardware, ~36 MB/s on commodity x86. The
  100 ms / 10 MB target would require ~100 MB/s, which is not achievable
  without modifying the C runtime — out of scope for tier-03 [src:
  tier-03 build session 2026-05-19 measurements: synthetic 1.00 s,
  jQuery 516 ms].
- **Reliability.** A real-world workload prevents the perf gate from
  silently catching pathological-synthetic regressions while ignoring
  what production runs hit.
- **Maintainability.** Fixture lives at
  `crates/ariadne-parser/fixtures/javascript/jquery.js` (MIT-licensed,
  ~285 kB), under the ≤ 1 MB rule of [`docs/folder-layout.md`].
- **Scalability.** Incremental at 0.6 ms / single-char × 10 MB scales to
  per-file edit cost in the µs range — well inside the p95 < 500 ms
  per-file budget of plan `<constraints>`.
</rationale>

<alternatives>
- **Keep the 100 ms / 5 ms targets, modify the C runtime.** Rejected —
  requires forking tree-sitter or contributing upstream perf work, far
  beyond tier-03's scope [src: .claude/plans/ariadne-core/plan.md
  `<constraints>`].
- **Keep the synthetic fixture.** Rejected — the workload doesn't match
  what indexing real repos hits, and the gate would block tier-10 e2e on
  numbers that have nothing to do with operational reality.
- **Block tier-03 pending dedicated perf sprint.** Rejected by user
  2026-05-19 — incremental already meets the operational gate; tier-10
  re-verifies cold cost on real repos.
</alternatives>

<consequences>
- `crates/ariadne-parser/benches/parse.rs` consumes `fixtures/javascript/
  jquery.js` instead of a synthetic generator.
- `tier-03-parser.md` `<exit_criteria>` and `<verification>` adopt the
  numbers above. CI gate must read criterion estimates and assert ≤ 700 ms
  / ≤ 5 ms.
- `[profile.bench] lto = "thin"` lives in the root `Cargo.toml`; LTO had
  no measurable effect on this workload but is kept for future grammars
  that may benefit.
- Future tiers that touch parser perf (tier-04 Salsa, tier-07 graph,
  tier-10 e2e) compose against this baseline. Any change >2x to either
  number requires a new ADR.
- Plan `<verification>` for tier-03 is updated in lockstep with this ADR
  per user direction; the original 100 ms / 5 ms targets are explicitly
  superseded by this one.
</consequences>

<sources>
- `[src: .claude/plans/ariadne-core/tier-03-parser.md]`
- `[src: .claude/plans/ariadne-core/plan.md `<constraints>`]`
- `[src: https://github.com/tree-sitter/tree-sitter — "Parsing Performance"]`
- `[src: docs/folder-layout.md fixture rule]`
- `[src: crates/ariadne-parser/benches/parse.rs]`
- `[src: tier-03 build session 2026-05-19 measurements]`
</sources>
