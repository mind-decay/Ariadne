---
tier_id: tier-05
audited: 2026-06-03
verdict: PASS
commit: 8c7b759f24424d8d555a3662425c605511007ccb
---

<scope>
Tier-05 — adoption eval. A deterministic wiring gate asserts that `ariadne setup`
composes the four installs (tiers 01–04), plus a behavioral `#[ignore]` harness
that measures Ariadne-vs-grep tool-use on a fixed question set and records
baseline-vs-treated ratios. No product code. Reviewed the working-tree diff on
top of HEAD `8c7b759` scoped to the tier's `<files>` (deliverable uncommitted):

- `crates/ariadne-e2e/tests/adoption_wiring.rs` — new; runs `setup` on a temp
  project pre-seeded with a foreign Bash `PreToolUse` entry, then asserts
  `.mcp.json` `alwaysLoad:true`, the `SessionStart` digest hook, the
  `Grep|Glob|Read` advisory `PreToolUse` entry, preservation of the foreign
  entry, both hook scripts present + `+x`, and `ariadne digest` non-empty,
  composed (`## Ariadne project digest`), under the 10k cap.
- `crates/ariadne-e2e/tests/adoption_harness.rs` — new; `#[ignore]`d behavioral
  runner driving headless `claude -p --output-format stream-json`, tallying
  `tool_use` block names (`mcp__ariadne__*` vs `Grep`/`Read`/`Glob`) + token
  usage across baseline (no wiring) and treated (`setup`+`index`) variants.
- `crates/ariadne-e2e/fixtures/adoption_questions.txt` — new; 5 codebase
  questions over the harness's three-file Rust call chain.
- `crates/ariadne-e2e/src/domain/mod.rs` — added `run_setup(root)` helper
  (anyhow, allowed in `ariadne-e2e`); within the `crates/ariadne-e2e/` scope and
  required by both new test targets.

No `Cargo.toml`/`Cargo.lock` change — no new dependency (`serde_json`/`tempfile`
already in tree). `git status` confirms no file outside the e2e crate + the tier
markdown was touched; no product code introduced, as the tier asserts.
</scope>

<checks_run>
- `cargo nextest run -p ariadne-e2e` → **4 passed, 14 skipped**, incl. the gating
  `adoption_wiring::setup_composes_the_full_adoption_wiring`; the behavioral
  harness is among the skipped (`#[ignore]`). Matches the `<results>` claim
  exactly.
- `cargo test --test architecture` → `architecture_invariants_hold` pass (anyhow
  confined to cli + e2e; e2e is a permitted boundary).
- `cargo clippy -p ariadne-e2e --all-targets --all-features -- -D warnings` →
  clean (also compiles the `#[ignore]` harness target — no warnings).
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` → clean.
- `cargo fmt --all --check` → clean.
- Constant cross-check: `adoption_wiring.rs` `SESSION_START_COMMAND`,
  `ADVISOR_COMMAND`, `ADVISOR_MATCHER` byte-match
  `crates/ariadne-cli/src/commands/setup.rs:29,87,93`; the asserted digest header
  `## Ariadne project digest` matches `commands/digest.rs:96`. The gate asserts
  against the real registered surface, not a hand-copied literal that could drift.
- `<results>` arithmetic re-derived: baseline 0/(4+8)=0.00, 0% share; treated
  11/(0+1)=11.00, 11/12=92% share; delta +92 pts vs the >50% majority-path
  target. Internally consistent; 5 questions = 5 fixture lines.
- Behavioral harness itself: re-execution deferred — it shells a real `claude`
  binary over the network with non-deterministic tool choice, so per D7 / R2 it
  is reported-not-gated and correctly `#[ignore]`d. The recorded run is dated,
  command-cited, and caveat-disclosed; not reproduced here by design, not
  fabricated. The tier's `<verification>` deterministic commands (the only gating
  ones) all pass on re-run.
</checks_run>

<findings>
| id | category | severity | location | problem | fix |
|----|----------|----------|----------|---------|-----|
| — | — | — | — | No FAIL or INFO defects found. | — |
</findings>

<verdict>
PASS. Every gating `<verification>` command re-runs green; all four
`exit_criteria` are independently verified:

1. *Deterministic wiring assert* — `adoption_wiring.rs` exercises real `setup` +
   `index` + `digest` subprocesses and asserts every named artifact
   (`alwaysLoad`, SessionStart, PreToolUse advisory, both `+x` scripts, digest
   non-empty < 10k). It additionally proves merge-preservation by pre-seeding a
   foreign `PreToolUse` entry. End-to-end, not a stub.
2. *Headless adoption harness* — `adoption_harness.rs` drives the fixed 5-question
   set and reports the `mcp__ariadne__*`:`Grep`+`Read` ratio per D7 (Glob
   reported, excluded from the ratio — matches the exit criterion's
   "`Grep`/`Read`" denominator).
3. *Baseline vs treated recorded* — both ratios + token totals captured in the
   tier `<results>` (0% → 92% share).
4. *Reported never gated* — the harness is `#[ignore]`; only the wiring asserts
   sit on the default `cargo nextest` pass. Confirmed: it was among the 14
   skipped.

Architecture holds (no driving→driving dep, anyhow within the e2e boundary, no
new tech). The byte-length digest cap is stricter than the char cap it stands in
for (bytes ≥ chars), so it cannot under-detect truncation. No smuggled
dependency. The honest "behavioral measurement deferred / caveat-disclosed"
handling satisfies the fail-loudly rule.
</verdict>

<next_steps>
None required for tier-05. Per the plan's tier order, tier-06 (search-read
spike) is the next session and is spike-gated (D11): tiers 07–09 proceed only if
the measured median token reduction ≥40%. The tier-05 escalation trigger (step 5)
was correctly not fired — 92% treated adoption clears the majority-path target,
so the tier-04 advisory stays `allow`.
</next_steps>

<sources>
- Tier + plan under review: `.claude/plans/ariadne-mcp-adoption/tier-05-adoption-eval.md`,
  `plan.md` (D7 measurement, R2 anti-flake).
- Setup surface cross-checked: `crates/ariadne-cli/src/commands/setup.rs:29,87,93,241`;
  `crates/ariadne-cli/src/commands/digest.rs:96`.
- [Writing effective tools for AI agents — Anthropic](https://www.anthropic.com/engineering/writing-tools-for-agents) (eval-driven measurement).
- [Code review standard — Google eng-practices](https://google.github.io/eng-practices/review/reviewer/standard.html) (ship-if-satisfies, no perfection gate).
</sources>
