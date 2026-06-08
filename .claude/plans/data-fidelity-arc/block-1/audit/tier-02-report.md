---
tier_id: tier-02
audited: 2026-06-08
verdict: PASS
commit: f96356f51593038b7817c3e92f54f86d9e6dc44c
---

<scope>
Tier-02 rolls the tier-01 `ariadne_graph::economy` helper (cap + opaque
revision-stamped cursor + concise/detailed verbosity, ADR-0029) out to the four
single-`Vec` tools: `coupling_report`, `co_change`, `hotspots`, `complexity`.
Diff scoped to the tier `<files>`: core `query.rs`/`response.rs` (four variants +
report DTOs), `mcp/types.rs` (four inputs/outputs + `SymbolSummary` cryptic
fields → `Option`), the four `mcp/tools/*.rs` cold handlers, `mcp/server.rs`
(`#[tool]` params + `project_daemon`), daemon `analytics.rs` + `health.rs` warm
handlers (+ `dispatch.rs` wiring), and `cli/query.rs`. Justified out-of-list
edits: `mcp/tools/mod.rs` + daemon `dispatch.rs::summarize` (the shared
`SymbolSummary`→`Option` change), `cli/digest.rs` (step 5 detailed-pin),
`core/lib.rs` + `daemon/mod.rs` façade re-exports, and test/snapshot updates.
The working tree also carries the already-PASSED tier-01 changes (economy.rs,
find_references) and unrelated `.claude/skills/*` edits — both out of scope here.
</scope>

<checks_run>
- Read every tier-02 `<files>` entry end-to-end plus the justified out-of-list
  edits and the four new/updated MCP test files.
- `cargo fmt --all --check` → clean (exit 0).
- `cargo nextest run -p ariadne-mcp -E 'test(coupling)|test(co_change)|test(hotspots)|test(complexity)'`
  → 21/21 passed (cap, cursor round-trip completeness, concise field-set, parity).
- `cargo nextest run -p ariadne-daemon` → 30/30 passed (warm analytics + coupling
  parity, memory probe, incremental rebuild).
- `cargo test --test architecture` → 1/1 passed (hexagonal invariants hold).
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` → clean.
- `cargo nextest run -p ariadne-mcp -p ariadne-cli` (full) → 155/155 passed — no
  regression from the shared `SymbolSummary`→`Option` change (blast_radius /
  file_summary / diff_blast / digest snapshots unchanged, all green).
- Dogfood (cold path, freshly-built binary, copy of the self-index in a
  daemon-less root): `co_change` default caps at 50 edges ≈ 2,518 tok with cursor
  + steer ("Showing 50 of 15821 …"); uncapped (`limit:20000`) = 15,821 edges =
  2,934,309 B ≈ 733,577 tok. ~291× reduction, matching plan.md's ~733k. A real
  5-page cursor walk (250 rows) equals the uncapped stable-order prefix exactly,
  no gap/dup.
- Verbosity-parity reasoning verified: sort runs inside `paginate` before the
  concise `project_row` nulls the embedded `id`, so the symbol-id tie-break reads
  real ids on both cold and warm paths; note nouns match per tool (modules /
  co-change pairs / hotspots / complexity rows).
</checks_run>

<findings>
| id | category | severity | location | problem | fix |
|----|----------|----------|----------|---------|-----|
| — | — | — | — | No defects found. | — |
</findings>

<verdict>
PASS. Zero FAIL findings, zero INFO findings.

Every exit criterion is independently verified:
- EC1 (cap + `next_cursor`/`note` + round-trip per tool): the four
  `*_caps_and_round_trips` tests assert the default page cap, a cursor on
  truncation, and page-union == un-capped sorted set with no gap/dup; the
  `co_change` dogfood confirms it on the 15,821-pair real index.
- EC2 (documented stable sort key): each cold/warm comparator matches the tier's
  declared key — coupling `Ca desc, module asc`; co_change `degree desc, (a,b)
  asc` (`total_cmp`); hotspots `score desc, file/sym-id asc`; complexity
  `complexity desc, key asc`.
- EC3 (concise default; symbol-grain drops id/offsets; metric tools
  concise==detailed; recorded per tool): hotspots/complexity
  `*_concise_omits_symbol_ids` + coupling/co_change `*_concise_equals_detailed`
  tests all pass; the two symbol-grain snapshots now omit `id`/`byte_start`/
  `byte_end`.
- EC4 (cold==warm==CLI; clippy/fmt/architecture/dogfood green): the `server.rs`
  `*_arm_matches_cold_output` parity tests pass over the wrapped reports
  (including a daemon concise-projection-through-postcard test); CLI uses the same
  `From` projections; all toolchain gates and the dogfood are green.

The shared helper is reused unchanged (no smuggled mechanism, no new dep, no redb
change), parity holds by construction across the three serving paths, and the
`digest` detailed-pin correctly preserves its whole-set re-rank contract against
the new default cap (modules ≤ file_count, so the digest page never truncates).
</verdict>

<next_steps>
None — tier-02 is accepted. Tier-03 (multi-list rollout) may proceed in a fresh
`/spec-build` session. Note for the eventual commit: the working tree bundles
tier-01 + tier-02 + unrelated `.claude/skills/*` edits; stage only the
tier-attributable files so the conventional-commit scope stays honest.
</next_steps>

<sources>
- Tier file: .claude/plans/data-fidelity-arc/block-1/tier-02-single-list-rollout.md
- Block plan + decisions: .claude/plans/data-fidelity-arc/block-1/plan.md (D1–D6)
- Mechanism ADR: docs/adr/0029-response-economy-cursor-verbosity.md
- MCP opaque-cursor model (list-ops-only → self-cursor): https://modelcontextprotocol.io/specification/2025-06-18/server/utilities/pagination
- Concise≈⅓ tokens, steer-on-truncate, 25k cap: https://www.anthropic.com/engineering/writing-tools-for-agents
- Reviewer standard (ship code-health over perfection): https://google.github.io/eng-practices/review/reviewer/standard.html
</sources>
