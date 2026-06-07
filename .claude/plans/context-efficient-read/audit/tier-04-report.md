---
tier_id: tier-04
audited: 2026-06-07
verdict: PASS
commit: 0af641eb20fe515e34782d60fa539ff1169b7c58
---

<scope>
Tier-04 "Advisory escalation + discoverability + deterministic token-delta
re-measure" of plan `context-efficient-read`. The tier work is uncommitted in the
working tree on top of HEAD `0af641e`. Audit scoped to the tier's `<files>`:
- `.claude/hooks/ariadne-grep-advisor.sh` (installed copy) + the
  `ADVISOR_HOOK` template in `crates/ariadne-cli/src/commands/setup.rs`.
- `crates/ariadne-mcp/src/server.rs` `with_instructions` Search/Read line +
  `crates/ariadne-mcp/tests/snapshots/handshake__server_instructions.snap`.
- `CLAUDE.md` "Search / Read" bullet + `render_block()` mirror in `setup.rs`.
- `crates/ariadne-e2e/tests/outline_token_delta.rs` deterministic harness +
  its generated `outline-token-delta.md` report.

Observation (not a tier-04 defect): the working tree commingles this tier with
sibling tiers 01–03 (the `read_outline` capability + `#[tool]`) and an unrelated
parallel workstream — a `fitness_report` `#[tool]` + `FitnessReportInput` in the
same `server.rs` (the `ariadne-fitness.toml` block-a plan, per `git status`).
Those additions are not tier-04's change; the audit isolates the tier-04 deltas
and does not judge them. The tier-04 `server.rs` contribution is the
`with_instructions` line only, which is correct and within the 2KB cap.
</scope>

<checks_run>
- `cargo fmt --all --check` → clean (exit 0).
- `cargo nextest run -p ariadne-cli -E 'binary(advisory)'` → 12/12 passed (1
  benign nextest LEAK flag on a subprocess fd; test passed). Covers whole-file
  Read → `read_outline`, ranged Read (offset/limit) → `read_symbol` and NOT
  `read_outline`, non-source/free-text → defer, and never-deny/ask.
- `cargo nextest run -p ariadne-mcp -E 'test(handshake)'` → 5/5 passed.
  `handshake_snapshots_server_instructions` asserts `instructions.len() <= 2048`
  (handshake.rs:160-164) against the live server + the regenerated snapshot.
  Measured snapshot instructions = 1150 bytes.
- `cargo test --test architecture` → 1 passed (no driving→driving edge; the
  e2e harness links only core/graph/storage, never `ariadne-mcp`).
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` →
  clean (exit 0).
- `cargo test -p ariadne-e2e --test outline_token_delta -- --ignored` → passed.
  Re-run produced a byte-identical `outline-token-delta.md` (`git diff --stat`
  empty) over the persisted index rev 107 → determinism confirmed. Median
  reduction 70.9% ≥ 50% target; recorded, not gated. Median math verified:
  sorted per-mille [585,638,671,702,717,742,768,861], (702+717)/2 = 709.
- Live confirmation: two whole-file source `Read`s during this audit each fired
  the installed advisor's `read_outline` (and, on a ranged read, `read_symbol`)
  `additionalContext` — the end-to-end advisory works in a real session.
</checks_run>

<findings>
| id | category | severity | location | problem | fix |
|----|----------|----------|----------|---------|-----|
| I1 | plan_adherence | INFO | tier-04 `<files>` (lines 28-41) | `crates/ariadne-cli/tests/advisory.rs` is modified (the new `advisor_nudges_read_outline_for_whole_file_read` + `advisor_nudges_read_symbol_for_ranged_read` cases that step 1 mandates) but is not listed in `<files>`. | Add `crates/ariadne-cli/tests/advisory.rs` to the tier `<files>` for an accurate change manifest. Non-blocking. |
</findings>

<verdict>
PASS. All four `exit_criteria` are independently verified:
1. Advisor names `read_outline` for a whole-file source `Read`, keeps
   `read_symbol` for a ranged (offset/limit) `Read`, and only ever emits
   `allow`/`defer` — never `deny`/`ask` (advisory.rs 12 tests green; live nudge
   observed). Matches plan.md D6.
2. `with_instructions` and the CLAUDE.md "Search / Read" entry both list
   `read_outline`; CLAUDE.md also notes the `ariadne outline` CLI; instructions
   1150 ≤ 2048 bytes; regenerated snapshot accepted (handshake green).
3. The `#[ignore]` `outline_token_delta.rs` harness (bytes/4 proxy, 8 fixed
   multi-symbol fixtures) records median 70.9% ≥ 50%, deterministic and
   reported-not-gated.
4. Advisor classification tests, clippy `-D warnings`, fmt, and
   `cargo test --test architecture` all green.
No FAIL findings; the single INFO is a documentation-completeness nit that does
not gate.
</verdict>

<next_steps>
None required to ship tier-04. Optional: fold I1 into the tier `<files>` list.
Process note for the committer: the working tree commingles tier-04 with sibling
tiers and the unrelated `fitness_report` workstream — stage the tier-04 `<files>`
deltas separately so the commit matches this tier's scope.
</next_steps>

<sources>
- Re-run command output captured in this session (fmt, nextest cli/mcp, cargo
  test architecture, clippy, ignored e2e harness).
- `crates/ariadne-mcp/tests/handshake.rs:160-164` (2KB cap assertion).
- `.claude/plans/context-efficient-read/plan.md` D6, D8; tier-04 `<exit_criteria>`/`<steps>`.
- [Claude Code hooks reference — PreToolUse schema](https://code.claude.com/docs/en/hooks)
- [Reviewer standard — Google eng-practices](https://google.github.io/eng-practices/review/reviewer/standard.html)
</sources>
