---
tier_id: tier-09
audited: 2026-06-03
verdict: PASS
commit: 7aaf9f8742e9b8fe969d8e574ab158dbacc0437f
---

<scope>
Tier-09 — wire `search_code`/`read_symbol` into the advisory, server instructions,
and CLAUDE.md; re-measure the real-tool token delta. Diff is the uncommitted working
tree on top of HEAD `7aaf9f8` (tier-07/08 shipped the two tools). Ten files changed,
all within the tier `<files>` set:
- `.claude/hooks/ariadne-grep-advisor.sh` + `crates/ariadne-cli/src/commands/setup.rs`
  (`ADVISOR_HOOK` template + `render_block` CLAUDE.md bullet) — kept byte-identical.
- `crates/ariadne-mcp/src/server.rs` (`with_instructions`) + regenerated
  `handshake__server_instructions.snap` + `handshake.rs` (2KB assert, step 4).
- `CLAUDE.md` Search/Read bullet.
- `crates/ariadne-cli/tests/advisory.rs` + `tests/setup.rs` (classification cases).
- `crates/ariadne-e2e/tests/adoption_harness.rs` (`Tally` extension + remeasure test).
- The tier file's own `status: completed` + `<notes>` record (expected).
No file outside scope touched. `handshake.rs` is the test home of the snap step 4
mandates, so its 2KB assertion is in-scope-justified.
</scope>

<checks_run>
- `cargo fmt --all --check` → exit 0.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` → exit 0.
- `cargo test --test architecture` → `architecture_invariants_hold` ok.
- `cargo nextest run -p ariadne-cli` → 40/40 pass, incl. the four new advisor cases
  (`advisor_names_search_code_for_symbol_shaped_grep`,
  `advisor_nudges_read_symbol_for_source_file_read`,
  `advisor_defers_quoted_grep_and_doc_read_without_new_tools`,
  `advisor_never_denies_or_asks`) and `setup_writes_all_three_artifacts`.
- `cargo nextest run -p ariadne-mcp -E 'test(handshake)'` → 5/5 pass (2KB assert +
  regenerated snapshot accepted).
- `cargo nextest run -p ariadne-e2e --no-run` → harness wiring compiles
  (`McpClient`, `collect_source_files`, `tool_text` resolve).
- `real_tool_token_delta_vs_grep` (`#[ignore]`) re-run over the live index
  (revision 563) → 8/8 tasks resolved, reproduces the recorded `<notes>` table
  byte-for-byte, median **87.5%** vs tier-06 spike 87.3% (≥40% D11). PASS.
- Server instructions measured independently: **1087 bytes** ≤ 2048.
- Live in-session real-run: a `Read` of `crates/ariadne-core/src/domain/types/lang.rs`
  triggered the installed advisor, which injected the `read_symbol` context —
  proves the hook is installed and active this session.
- `Lang::from_extension` (`crates/ariadne-core/src/domain/types/lang.rs:113`) cross-
  checked against the advisor's Read extension `case`: the two sets are identical
  (`rs ts mts cts tsx js jsx mjs cjs vue svelte astro py pyi go java kt kts cs c h
  cpp cc cxx c++ hpp hh hxx`). The cited claim is accurate.
- Security: the shell advisor pipes the extracted `QUERY`/`FILE` as data to
  `grep -Eq`/`case`, never into a regex or shell word, and never echoes them into
  output JSON (only fixed quote-free CTX strings interpolate) — no injection vector
  under a hostile payload [src: OWASP A03 Injection]. `set -u`; every unhandled
  branch fails open to `defer`.
</checks_run>

<findings>
| id | category | severity | location | problem | fix |
|----|----------|----------|----------|---------|-----|
| — | — | — | — | No defects found. | — |
</findings>

<verdict>
PASS. All four exit criteria independently verified:
1. The advisor names `search_code` for symbol-pattern Grep/Glob (`SEARCH_DEF_CTX`/
   `SEARCH_REF_CTX`) and `read_symbol` for source-file Reads (`READ_CTX`); it emits
   only `allow`/`defer`, never `deny`/`ask` (`advisor_never_denies_or_asks` green).
2. `with_instructions` and the CLAUDE.md list both gain the Search/Read group;
   instructions are 1087 bytes ≤ 2KB.
3. `Tally` counts `mcp__ariadne__search_code`/`read_symbol` as a counted subset of
   `ariadne` (no ratio double-count); the recorded re-run reports the real-tool
   delta and reproduces exactly on re-execution.
4. Advisor tests, harness wiring, clippy, fmt, and architecture are green; the
   behavioural ratio is reported via the `#[ignore]` harness, not gated.
The implementation is config/doc/test only, satisfies D5/D8/D9/D11, holds the
hexagonal and advisory invariants, and is safe.
</verdict>

<next_steps>
None required. The work is ready to commit. The signal step (7) correctly defers
the `allow`→`ask` escalation to a follow-up plan, gated on a recorded behavioural
run of `adoption_ratio_baseline_vs_treated`.
</next_steps>

<sources>
- `.claude/plans/ariadne-mcp-adoption/tier-09-search-read-advisory-eval.md` (tier under audit).
- `.claude/plans/ariadne-mcp-adoption/plan.md` D5, D8, D9, D11; `<constraints>` 2KB cap.
- `crates/ariadne-core/src/domain/types/lang.rs:113` (`from_extension` table).
- [OWASP Top 10 — A03 Injection](https://owasp.org/www-project-top-ten/).
</sources>
