---
tier_id: tier-05
title: Adoption eval — measure Ariadne-vs-grep tool-use, assert wiring deterministically
deps: [tier-01, tier-02, tier-03, tier-04]
exit_criteria:
  - "A deterministic e2e test asserts the full wiring after `ariadne setup`: .mcp.json alwaysLoad, SessionStart + PreToolUse entries, both scripts executable, digest runs non-empty < 10k."
  - "A headless adoption harness runs a fixed codebase-question set and reports the ratio of `mcp__ariadne__*` calls to `Grep`/`Read` calls."
  - "Baseline (setup reverted) vs treated (setup applied) ratios are recorded in this tier's verification notes."
  - "Behavioral ratio is reported, never a hard CI gate (model non-determinism); only the wiring asserts gate."
status: completed
completed: 2026-06-03
---

<context>
Anthropic's tool guidance is eval-driven: measure accuracy, tokens, and tool
errors on real multi-call tasks [src:
https://www.anthropic.com/engineering/writing-tools-for-agents]. The project rule
demands validate-by-execution but forbids flaky wall-clock/behavioral gates [src:
CLAUDE.md; feedback_validation_required]. So split: deterministic wiring asserts
gate CI; the behavioral adoption ratio is measured and reported [src: plan.md D7].
</context>

<files>
- `crates/ariadne-e2e/tests/adoption_wiring.rs` — new; `setup` on a temp project,
  assert all artifacts present + digest output.
- `crates/ariadne-e2e/` (harness) — a headless runner that drives `claude -p` over
  a question set against a fixture repo and tallies tool calls from the transcript.
- A question fixture file (codebase questions: "who calls X", "what breaks if I
  change Y", "explain the architecture").
</files>

<steps>
1. **Failing wiring test.** In `adoption_wiring.rs`, run `setup` on a temp copy of a
   fixture repo and assert: `.mcp.json` ariadne `alwaysLoad:true`;
   `.claude/settings.json` has SessionStart + a Grep/Glob/Read PreToolUse entry
   (plus any pre-existing); both hook scripts exist and are `+x`; `ariadne digest`
   exits 0 with non-empty stdout `< 10_000` bytes. Run — fails until tiers 01–04 are
   present.
2. **Wire it green.** With tiers 01–04 built, the asserts pass with no new product
   code — this is the integration gate that the four installs compose correctly.
3. **Adoption harness.** Add a runner that, for each fixture question, invokes
   `claude -p <question>` (headless) in the fixture repo and parses the session
   transcript for tool-call names, tallying `mcp__ariadne__*` vs `Grep`/`Read`
   [src: ariadne-cli main.rs headless run model; transcript inspection]. Compute the
   adoption ratio. Mark this test `#[ignore]` (opt-in) so model non-determinism
   never flakes CI (R2, anti-flake rule).
4. **Baseline vs treated.** Run the harness twice — once with `setup` reverted
   (baseline) and once applied (treated) — and record both ratios + token counts in
   the verification notes below. State the delta against the success target in
   plan.md `<context>` (Ariadne becomes the majority path).
5. **Signal escalation.** If treated adoption is still low, note it explicitly as a
   trigger to escalate the tier-04 advisory from `allow` toward `ask` (a follow-up
   plan, not this tier).
</steps>

<verification>
- `cargo nextest run -p ariadne-e2e` — `adoption_wiring` passes; the ignored
  harness is excluded from the gate.
- Manual: `cargo nextest run -p ariadne-e2e --run-ignored all` (or the harness
  binary) produces baseline + treated ratios; paste the numbers here. Real
  execution required — do not claim improvement without the recorded run.
- `cargo test --test architecture`; clippy `-D warnings`; fmt check.
- Fail loudly: if the harness cannot run headless in-session, state that the
  behavioral measurement is deferred and report only the deterministic wiring
  result — never fabricate a ratio.
</verification>

<results>
Recorded 2026-06-03. No product code introduced — only the e2e test, harness,
and question fixture.

**Deterministic wiring gate** — `adoption_wiring.rs::setup_composes_the_full_adoption_wiring`
passes on the default `cargo nextest run -p ariadne-e2e` pass (4 passed, ignored
harness excluded). It runs `ariadne setup` on a temp project pre-seeded with a
foreign Bash `PreToolUse` entry, then asserts: `.mcp.json` ariadne
`alwaysLoad:true`; `.claude/settings.json` carries the SessionStart digest hook
and the `Grep|Glob|Read` advisory entry *and* preserves the foreign Bash entry;
both hook scripts exist and are `+x`; after `ariadne index`, `ariadne digest`
exits 0 with a composed (non-fallback) document under the 10k cap.

**Behavioral harness** — `adoption_harness.rs` (`#[ignore]`), real run
2026-06-03, 5-question set, nested headless `claude -p` (model `claude-opus-4-8`,
`--output-format stream-json`), `mcp__ariadne__*` vs `Grep`/`Read` per D7:

| Variant | ariadne | grep | read | glob | ratio (ariadne:grep+read) | ariadne share | tokens in / out |
|---------|---------|------|------|------|---------------------------|---------------|-----------------|
| Baseline (setup reverted) | 0 | 4 | 8 | 1 | 0.00 | 0% | 24224 / 3193 |
| Treated (setup applied)   | 11 | 0 | 1 | 0 | 11.00 | 92% | 28372 / 1980 |

Delta: ariadne share **0% → 92% (+92 pts)** against the plan `<context>` success
target (Ariadne the majority path, >50%) — **met**. Output tokens fell
(3193 → 1980); input tokens rose modestly (24224 → 28372) from the SessionStart
digest injection plus the always-loaded tool descriptions — a net shift strongly
toward the graph path.

Caveats: counts the `Grep`/`Read`/`Glob` native tools (D7); `grep`/`cat` issued
via `Bash` are not tallied; model tool-choice is non-deterministic (R2), so the
harness stays `#[ignore]` and never gates CI. Nested sessions inherit the
operator's global Claude config; the baseline-vs-treated delta isolates the
Ariadne wiring within that fixed environment.

**Escalation (step 5)** — not triggered. Treated adoption (92%) already clears
the majority-path target, so the tier-04 advisory stays `allow`; revisit only if
a future consumer measures low adoption.

Commands: `cargo nextest run -p ariadne-e2e` (4 passed, 14 skipped);
`cargo nextest run -p ariadne-e2e --run-ignored all -E 'test(adoption_ratio_baseline_vs_treated)' --no-capture`
(1 passed, 147s, numbers above); `cargo test --test architecture` (ok);
`cargo clippy --workspace --all-targets --all-features -- -D warnings` (clean);
`cargo fmt --all --check` (clean).
</results>

<rollback>
Remove the e2e test + harness + fixture; no product code is introduced in this
tier, so nothing else reverts.
</rollback>
