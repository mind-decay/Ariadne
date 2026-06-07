---
tier_id: tier-03
audited: 2026-06-07
verdict: PASS
commit: 0af641eb20fe515e34782d60fa539ff1169b7c58
---

<scope>
Tier-03 (`ariadne outline <path>` CLI subcommand) of the
`context-efficient-read` plan. Reviewed the working-tree diff scoped to the
tier's `<files>`:
- `crates/ariadne-cli/src/main.rs` — `Outline` variant + dispatch arm (the
  coexisting `Fitness` subcommand belongs to a separate workstream and is out of
  scope here).
- `crates/ariadne-cli/src/commands/outline.rs` — new runner (NEW).
- `crates/ariadne-cli/src/commands/mod.rs` — `pub mod outline;` registration.
- `crates/ariadne-cli/tests/outline_cli.rs` — integration test (NEW).
Cross-checked against `plan.md` (D3/D7, parity), the shared use case
`crates/ariadne-graph/src/outline.rs`, the parity peer
`crates/ariadne-mcp/src/tools/read_outline.rs`, and `tests/architecture.rs`.
Tier-03 work is uncommitted; the audit reflects the working tree at HEAD
`0af641e`. Index freshness confirmed via `project_status` (revision 90).
</scope>

<checks_run>
- `cargo fmt --all --check` → clean (exit 0).
- `cargo test --test architecture` → `architecture_invariants_hold` passes.
  Read the test end-to-end: it genuinely iterates every workspace crate and
  asserts only the composition root `ariadne-cli` may depend on a driving
  adapter (ADR-0007), so the `use ariadne_mcp::Catalog` in the runner is the
  sanctioned carve-out, not a smuggled driving→driving edge.
- `cargo clippy -p ariadne-cli --all-targets --all-features -- -D warnings` →
  clean on a forced re-lint (Checking ariadne-cli … Finished, no warnings).
- `cargo nextest run -p ariadne-cli` → 55/55 pass, including the five
  `outline_cli` tests (folds-bodies, include-private, json, zero-symbol note,
  missing-path non-zero exit).
- Real run: `cargo run -p ariadne-cli -- outline crates/ariadne-graph/src/outline.rs`
  → folded skeleton, bodies elided to `{ … N lines }`, signatures + doc comments
  kept. Byte delta: 1185 skeleton bytes vs 16665 source bytes (≈92.9% smaller).
  `--include-private` run also exercised.
- Parity (exit criterion 2): read both adapters. CLI `outline.rs` and MCP
  `read_outline.rs` enumerate `cat.symbols` filtered by `file_id`, build
  `OutlineSymbol` field-for-field, sort by `(byte_start, byte_end)`, cap at
  `MAX_OUTLINE_SYMBOLS = 800`, fall back to `Lang::Other("unknown")`, and call
  the identical `ariadne_graph::assemble`. Same options ⇒ byte-identical
  skeleton. Confirmed.
- Graph facade: `ariadne-graph/src/lib.rs:51` re-exports
  `{Outline, OutlineEntry, OutlineOptions, OutlineRequest, OutlineSymbol,
  assemble}` — every symbol the runner imports resolves.
</checks_run>

<findings>
| id | category | severity | location | problem | fix |
|----|----------|----------|----------|---------|-----|
| INFO-1 | correctness | INFO | `crates/ariadne-cli/src/main.rs` Outline (`include_private` defaults false) vs `crates/ariadne-mcp/src/tools/read_outline.rs:91` (`unwrap_or(true)`) | The two surfaces' *defaults* diverge: `ariadne outline X` hides non-public symbols while `read_outline X` shows them, so default-to-default skeletons are not byte-identical despite the parity framing of the verification step. | None required — by design: the tier specs `--include-private` as an opt-in flag, and the agent-facing MCP default of "show all" is reasonable. To reproduce identical output, pass `--include-private`. Recorded for transparency. |
</findings>

<verdict>
PASS. Zero FAIL findings. Every `<verification>` command re-ran green, all four
`exit_criteria` are independently satisfied:
1. The subcommand builds/queries the catalog through the existing
   `RedbStorage::open` + `Catalog::build` plumbing (the pattern query/status/
   doc/fitness/affected_tests already use), enumerates symbols, reads bytes, and
   renders the folded skeleton (text default, JSON with `--json`).
2. The CLI integration test asserts folded bodies + kept signatures + strictly
   byte-smaller output, produced by the same `ariadne_graph::assemble` the MCP
   tool uses (parity verified at source level).
3. Unknown path → typed non-zero exit with "not an indexed file" (no panic);
   zero-symbol file → the line-count note, not a source dump (both tested + run).
4. clippy `-D warnings`, fmt, `cargo test --test architecture` (no
   driving→driving edge added — the cli→mcp dependency predates this tier and is
   the ADR-0007 carve-out), and `cargo nextest run -p ariadne-cli` all green.
The single INFO is a documented, by-design interface choice and does not gate.
</verdict>

<next_steps>
None required for acceptance. Optional follow-up (non-blocking): if exact
default-to-default CLI/MCP skeleton equality is ever desired, align the
`include_private` defaults or note the divergence in user-facing docs.
</next_steps>

<sources>
- `.claude/plans/context-efficient-read/tier-03-outline-cli.md` (tier under review)
- `.claude/plans/context-efficient-read/plan.md` D3/D7 (CLI parity, no driving→driving)
- `docs/adr/0007-cli-composition-root.md` (cli may depend on driving adapters)
- `tests/architecture.rs` (boundary invariant, re-run green)
- `crates/ariadne-graph/src/outline.rs`, `crates/ariadne-mcp/src/tools/read_outline.rs` (shared use case / parity peer)
- [Google eng-practices: reviewer standard](https://google.github.io/eng-practices/review/reviewer/standard.html)
</sources>
