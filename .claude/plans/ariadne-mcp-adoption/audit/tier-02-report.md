---
tier_id: tier-02
audited: 2026-06-03
verdict: PASS
commit: 0909c2e532078bbed3cbe06338cb6f36cd260679
---

<scope>
Tier-02 "ariadne digest" — a compact, deterministic project digest for
SessionStart bootstrap. Audited the working-tree diff scoped to the tier
`<files>`:
- `crates/ariadne-cli/src/commands/digest.rs` (new, 249 lines)
- `crates/ariadne-cli/tests/digest.rs` (new, golden-shape + length test)
- `crates/ariadne-cli/src/commands/query.rs` (refactor: extract `run_tool`)
- `crates/ariadne-cli/src/commands/mod.rs` (register `digest`)
- `crates/ariadne-cli/src/main.rs` (add `Cmd::Digest` + dispatch)

Out of scope (belong to tier-01 / other plans, not reviewed here):
`.mcp.json`, `crates/ariadne-mcp/src/server.rs`, `handshake.rs`,
`handshake__server_instructions.snap`, `crates/ariadne-cli/src/commands/setup.rs`,
`crates/ariadne-cli/tests/setup.rs`.
</scope>

<checks_run>
- Index freshness: `project_status` → revision 345, 361 files, 3359 symbols,
  4702 edges, root = repo. Graph current; edges trusted.
- `cargo build -p ariadne-cli` → clean.
- `cargo nextest run -p ariadne-cli` → 27/27 PASS, incl.
  `digest::digest_emits_bounded_agent_shaped_markdown`.
- `cargo fmt --all --check` → clean (exit 0).
- `cargo clippy -p ariadne-cli --all-targets --all-features -- -D warnings` →
  clean.
- `cargo test --test architecture` → 1/1 PASS (CLI stays the only multi-adapter
  crate).
- `cargo nextest run -p ariadne-e2e --test cli_daemon_parity` → 1/1 PASS
  (warm/cold query routes still agree post-refactor).
- Real run `ariadne digest` (this repo): 1591 bytes (< 10k cap), revision 345
  matches `project_status`, 8 non-empty top modules, all four sections present.
- Real run `ariadne query project_status '{}'` to confirm output key ordering
  (see F1).
- Read every changed file end-to-end; compared to `<steps>`,
  `<exit_criteria>`, and plan `<decisions>` D3/D4.
</checks_run>

<findings>
| id | category | severity | location | problem | fix |
|----|----------|----------|----------|---------|-----|
| F1 | correctness | INFO | query.rs:321-323 (`json`), 36-43 (`run`) | The refactor switched `json()` from `to_string_pretty(struct)` to `to_value(struct)`, and `run` now re-serializes the `Value`; with `serde_json` built without `preserve_order`, `Value::Object` is a `BTreeMap`, so `ariadne query` output keys now print alphabetically (`edge_count,file_count,revision,root,symbol_count`) instead of struct-declaration order (`revision` first) — an observable output change vs step 2's "No behavior change to query." Inert for JSON parsers; warm/cold parity still holds; no test breaks. | If declaration order matters to a consumer, enable `serde_json/preserve_order` (pulls `indexmap`, already in the tree) or thread the original `String` path through; otherwise tighten step 2's wording to "no functional change." |
</findings>

<verdict>
PASS. Zero FAIL findings.

Every exit criterion is independently verified:
1. `ariadne digest [root]` prints agent-shaped Markdown — header with revision +
   counts, "Top modules" (top-8 by Ca+Ce), "Project overview", and a fixed
   question→tool cheat-sheet. Confirmed by real run + `digest.rs` shape asserts.
2. Bounded well under 10k (1591 bytes observed; test asserts `< 10_000`) and the
   `fallback()` returns a non-empty `## Ariadne` document; `is_empty()`
   (symbol_count == 0) and the `gather_bounded` `None` arm both route to it.
3. Resolves through the same daemon/cold path as `query`: `gather` calls the
   shared `query::run_tool`, which is the exact warm-then-cold plumbing `query`
   uses; `cli_daemon_parity` confirms the two routes agree.
4. Golden-shape test asserts the four sections, a `revision` line, non-empty
   output, and the length bound on a real indexed fixture (no mocks).

D3 honored: header, cheat-sheet, and fallback are phrased as factual statements,
not out-of-band imperatives. D4 honored: pure projection over `project_status` +
`coupling_report` + `doc_for_project`; no new domain logic, no inference, no new
dependency (only in-tree `std::thread`/`mpsc`, `serde_json`, `anyhow`). Step 5
timeout honored: a detached worker bounded by `DIGEST_TIMEOUT` (5s) degrades to
the minimal fallback so a cold daemon cannot stall session start.
F1 is a cosmetic JSON-key reordering on a sibling command, immaterial to JSON
consumers and not an exit criterion — INFO, does not gate.
</verdict>

<next_steps>
None blocking. Tier-02 may commit. Optional: decide whether `ariadne query`
output key order is part of any consumer contract (F1); if not, no action and
step 2's prose can be softened to "no functional change to query."
</next_steps>

<sources>
- Tier under review: `.claude/plans/ariadne-mcp-adoption/tier-02-digest-command.md`
- Plan: `.claude/plans/ariadne-mcp-adoption/plan.md` (D3, D4, R1, R4)
- serde_json `Value`/`Map` key ordering (BTreeMap without `preserve_order`):
  https://docs.rs/serde_json/1.0.149/serde_json/map/struct.Map.html
- Hooks `additionalContext` 10k cap: https://code.claude.com/docs/en/hooks
- Reviewer standard (code health over perfection):
  https://google.github.io/eng-practices/review/reviewer/standard.html
</sources>
