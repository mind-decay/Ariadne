---
tier_id: tier-04
title: Advisory escalation + discoverability + deterministic token-delta re-measure
deps: [tier-02, tier-03]
exit_criteria:
  - "The `ariadne-grep-advisor.sh` advisory names `read_outline` for a whole-file source `Read` (skeleton first), keeps naming `read_symbol` for symbol-targeted reads, and still only ever returns `allow` — never `deny`/`ask`."
  - "The MCP server `with_instructions` and the CLAUDE.md \"Search / Read\" entry both list `read_outline` (and mention `ariadne outline`); server instructions stay ≤2KB; regenerated instructions snapshot accepted."
  - "A deterministic token-delta harness (`bytes/4` proxy) compares whole-file `Read` vs `read_outline` over a fixed multi-symbol fixture set and records the median reduction; target ≥50%, reported not gated."
  - "Advisor classification tests, clippy `-D warnings`, fmt, and `cargo test --test architecture` are green."
status: completed
completed: 2026-06-07
---

<context>
Tiers 02–03 shipped the capability; this tier connects it to the adoption
machinery built by `ariadne-mcp-adoption`. The `PreToolUse` advisory already
routes symbol-targeted source `Read`s to `read_symbol`; it should additionally
route a *whole-file* source `Read` (the residual gap this plan targets) to
`read_outline` — staying advisory (`allow` + `additionalContext`), never blocking
[src: .claude/plans/ariadne-mcp-adoption/tier-09-search-read-advisory-eval.md;
plan.md D6]. The server `with_instructions` and the CLAUDE.md tool list must
advertise the new tool (a scope-allowed CLAUDE.md edit: listing a tool, not
rewriting prose — the exact precedent tier-09 used) [src: tier-09 context]. The
spike's premise (skeleton ≫ cheaper than whole-file read) is then confirmed with a
deterministic re-measure [src: plan.md D8].
</context>

<files>
- `crates/ariadne-cli/src/commands/setup.rs` — advisor script template: add the
  whole-file-source-`Read` → `read_outline` suggestion [src: tier-09 files].
- `<root>/.claude/hooks/ariadne-grep-advisor.sh` — this repo's installed copy.
- `crates/ariadne-cli/tests/advisory.rs` — advisor classification tests: add the
  whole-file-`Read` → `read_outline` and ranged-`Read` → `read_symbol` cases
  (step 1's failing tests) [src: audit tier-04 I1].
- `crates/ariadne-mcp/src/server.rs` — extend `with_instructions` Search/Read
  line with `read_outline`, within the 2KB cap [src: tier-09 step 4; server.rs
  with_instructions].
- `crates/ariadne-mcp/tests/snapshots/handshake__server_instructions.snap` —
  accept the regenerated instructions snapshot.
- `CLAUDE.md` — add `read_outline` to the "Search / Read" bullet; note the
  `ariadne outline` CLI.
- `crates/ariadne-e2e/tests/outline_token_delta.rs` — `#[ignore]` deterministic
  harness over a fixed file set [src: tier-09 notes method].
</files>

<steps>
1. **Failing test first.** Feed the advisor: (a) a whole-file `Read` of a `.rs`/
   `.ts` source file with no symbol target → expect `additionalContext` naming
   `read_outline`; (b) a `Read` clearly targeting one known symbol → still names
   `read_symbol`; (c) a `.md`/non-source `Read` → pass-through, empty context.
   Run — fails (advisor names only the old tools).
2. **Update the advisor.** Extend the heuristic: a source-extension whole-file
   `Read` → suggest `read_outline` ("a token-cheap skeleton first; then
   `read_symbol`/`Read` for specific bodies"); keep `read_symbol` for the
   symbol-targeted shape. `permissionDecision:"allow"` on match, defer otherwise;
   never `deny` (plan.md D6) [src: tier-09 step 2].
3. **Install via setup.** Update the template string in `setup.rs`; keep the
   install idempotent and the existing hook entries intact [src: tier-09 step 3].
4. **Server instructions.** Add `read_outline` to the Search/Read line in
   `with_instructions`; assert total ≤2KB in the existing test; accept the
   regenerated `handshake__server_instructions.snap` [src: plan.md constraints;
   tier-09 step 4].
5. **CLAUDE.md list.** Extend "Search / Read — `search_code`, `read_symbol`" to
   add "`read_outline` (whole-file skeleton; expand with `read_symbol`)"; note the
   `ariadne outline` CLI. Listing only — no prose rewrite [src: tier-09 step 5].
6. **Re-measure.** In `outline_token_delta.rs`, for a fixed set of multi-symbol
   files in this repo: baseline = `bytes(file)` (whole-file `Read`); prototype =
   `bytes(read_outline output)`; token proxy `bytes/4`. Record per-file +
   median reduction in this tier's notes; compare to the ≥50% target. `#[ignore]`,
   deterministic (no wall-clock, no model) [src: tier-09 notes; plan.md D8].
7. **Signal.** If real adoption stays low, note escalating the advisory from
   `allow` toward `ask` as a follow-up plan, not this tier [src: tier-09 step 7].
</steps>

<verification>
- `cargo nextest run -p ariadne-cli` — advisor classification cases pass; re-run
  `ariadne setup` is idempotent and preserves existing hooks.
- `cargo nextest run -p ariadne-mcp -E 'test(handshake)'` — instructions ≤2KB;
  regenerated snapshot accepted.
- `cargo test --test architecture`; clippy `-D warnings`; `cargo fmt --all
  --check`.
- Manual: run `outline_token_delta` and record the median reduction (target ≥50%,
  reported). In a fresh session, `Read` a whole source file → confirm the advisory
  now names `read_outline`; report it. If not runnable in-session, say so and
  report only the deterministic harness numbers — never fabricate a ratio [src:
  CLAUDE.md validate-by-execution].
</verification>

<rollback>
Revert the advisor template + installed script, the `with_instructions` line, the
CLAUDE.md bullet, and the harness. Config + doc + test only; tiers 01–03 (the
capability) stay intact.
</rollback>

<notes>
Deterministic token-delta re-measure (`outline_token_delta.rs`, index revision
107, bytes/4 proxy; full report in
`.claude/plans/context-efficient-read/outline-token-delta.md`):

| File | Baseline tok | Outline tok | Reduction |
|------|-------------:|------------:|----------:|
| `crates/ariadne-cli/src/commands/setup.rs` | 6734 | 930 | 86.1% |
| `crates/ariadne-cli/src/config.rs` | 2982 | 981 | 67.1% |
| `crates/ariadne-core/src/domain/records.rs` | 2785 | 1154 | 58.5% |
| `crates/ariadne-core/src/domain/types/lang.rs` | 1340 | 345 | 74.2% |
| `crates/ariadne-graph/src/doc_model.rs` | 1712 | 618 | 63.8% |
| `crates/ariadne-graph/src/docgen.rs` | 4220 | 978 | 76.8% |
| `crates/ariadne-graph/src/hotspot.rs` | 1791 | 506 | 71.7% |
| `crates/ariadne-mcp/src/server.rs` | 14004 | 4173 | 70.2% |

**Median reduction 70.9% ≥ 50% target (D8); reported, not gated.** Server
instructions 1150 / 2048 bytes after adding `read_outline`. End-to-end: the
installed `ariadne-grep-advisor.sh` returns `read_outline` for a whole-file
source `Read`, `read_symbol` for an `offset`/`limit` ranged `Read`, and `defer`
for a `.md`/non-source `Read`.

Step 7 signal — adoption is not yet instrumented this session, so the advisory
stays `allow`. Escalating it toward `ask` (if real adoption stays low) is a
follow-up plan, not this tier [src: <steps> 7; plan.md D6].
</notes>
