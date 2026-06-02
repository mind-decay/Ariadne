---
tier_id: tier-15b
audited: 2026-06-02
verdict: PASS
commit: 86b59a5bb8b1b0ffe15fee1f9472487a41d80302
---

<scope>
Audited tier-15b "Analytics MCP tools — hotspots, complexity, co_change (daemon-routed, cold fallback)" against its sibling `plan.md` (Block C, RD7/RD8) and the working-tree diff (uncommitted; HEAD is the tier-15a commit, which the audit gate precedes).

Diff in scope:
- `ariadne-core` daemon protocol: `query.rs` (+`Grain`, 3 query variants), `response.rs`/`rows.rs` (3 reports + 6 row DTOs + 3 `DaemonResponse` arms), `mod.rs`/`lib.rs` (re-export plumbing), `daemon/mod.rs` (`Eq` dropped from `DaemonRequest`).
- `ariadne-daemon`: new `queries/analytics.rs`; `queries/mod.rs` + `dispatch.rs` routing.
- `ariadne-mcp`: `types.rs` (`Grain`, `GrainScopeInput`, `CoChangeInput`, 3 row + 3 output types), new `tools/{hotspots,complexity,co_change}.rs`, `tools/mod.rs`, `server.rs` (3 `#[tool]` methods + 3 `project_daemon` arms + `to_core_grain` + 3 parity unit tests), 3 new spawned-server golden tests + 5 snapshots, `handshake.rs` (16) + 2 re-accepted snapshots.
- Out of declared `<files>`: `ariadne-cli/src/commands/query.rs` (modified); `docs/codebase-overview.md` (not modified). See findings.
</scope>

<checks_run>
- plan_adherence: every declared `<files>` entry inspected; two deviations recorded (F1, F2).
- correctness: read all new handlers end-to-end; re-derived all golden values by hand (see verdict math). Cold (`tools/*.rs`) and daemon (`queries/analytics.rs`) handlers are line-for-line equivalent; both `summarize` projections produce identical `SymbolSummary` (id/name/kind/file/byte_start/byte_end + `<unknown>` fallback).
- architecture: no new cross-crate dep edge — `ariadne-mcp`, `ariadne-daemon`, `ariadne-cli` already depend on `ariadne-graph`; `cli→mcp` is the ADR-0007 composition root. `cargo test --test architecture` → ok. Hexagonal port boundary intact: wire DTOs live in `ariadne-core`, graph types never leak through the daemon protocol.
- tests: 3 parity unit tests (core DTO JSON == cold output JSON) + 3 in-module daemon handler tests + 5 spawned-binary insta goldens (real MCP cold path) + re-accepted handshake. All assert behaviour, fail loudly.
- `Eq` removal: grep confirms no `Set`/`Map`/`Hash`/key use of `DaemonQuery`/`DaemonRequest`; codec is postcard, tests use `assert_eq!` (PartialEq suffices). Whole workspace compiles.
- Re-ran every `<verification>` command (results below).
</checks_run>

<verification_rerun>
- `cargo nextest run -p ariadne-core -p ariadne-mcp -p ariadne-daemon` → 89 passed, 0 failed. Includes `tools_hotspots`/`tools_complexity`/`tools_co_change` goldens, the 3 `*_arm_matches_cold_output` parity tests, `handshake_lists_expected_tools` (16), `handshake_descriptions_carry_when_and_triggers`, and all pre-existing v1 tool goldens (unchanged).
- `cargo fmt --all --check` → clean.
- `cargo test --test architecture` → 1 passed.
- `cargo clippy -p ariadne-core -p ariadne-mcp -p ariadne-daemon -p ariadne-cli --all-targets --all-features -- -D warnings` → clean.
- `RUSTDOCFLAGS="-D warnings" cargo doc -p ariadne-core -p ariadne-mcp -p ariadne-daemon --no-deps` → generated, no warnings.
- Manual real-daemon / Claude-tool-selection step: not reproducible in-session (the live `.mcp.json` server runs the committed v1 binary, which predates these tools). Substantially covered by the spawned-binary cold goldens, which drive the actual `ariadne-mcp` process over the MCP protocol; warm path covered by the in-module daemon handler tests + parity tests. Noted, not a blocker — exit criteria require unit-parity + spawned golden, both present.
</verification_rerun>

<findings>
| id | category | severity | location | problem | fix |
|----|----------|----------|----------|---------|-----|
| F1 | plan_adherence | INFO | crates/ariadne-cli/src/commands/query.rs:11-21,28-50,58-73,82-94 | Modified though absent from the tier `<files>` list. The `project()` arms for the 3 new `DaemonResponse` variants are forced (the enum is exhaustively matched by design — query.rs:54-64); the added `build_query`/`dispatch` arms + `to_core_grain` are a coherent, unforced extension mirroring the existing `to_core_kinds` precedent. | Record `ariadne-cli/query.rs` in `<files>` (or note the forced exhaustive-match consequence) in future analytics tiers. No code change needed. |
| F2 | plan_adherence/docs | INFO | docs/codebase-overview.md | Listed in `<files>` and step 7 ("list the three new tools") but untouched; the 3 new `tools/*.rs` nodes are absent. The file is an auto-generated graph/metrics dump (`187 modules · 1991 symbols …`, mermaid nodes), so the right fix is regeneration, consistent with the tier's own "README + CLAUDE.md catalog finalized in 15c". | Regenerate the overview (or fold the doc deliverable into 15c). Not an exit criterion; non-blocking. |
</findings>

<verdict>
PASS — zero FAIL findings; both findings are non-blocking INFO.

All six exit criteria independently verified:
1. `hotspots`/`complexity`/`co_change` registered on `AriadneServer`, discoverable (handshake = 16), each `try_query_async`→`project_daemon` with `catalog()`+cold-`handle`+`wire` fallback (server.rs:423-482). ✓
2. `ariadne-core` gained `Grain` + 3 `DaemonQuery` variants, 3 reports/6 rows + 3 `DaemonResponse` arms; daemon dispatches via new `queries/analytics.rs` (dispatch.rs:41-58). ✓
3. Ranking semantics correct, math re-derived: file hotspots alpha (9×7)→score 1.0, beta (4/9)(3/7)=0.190476; symbol hotspots alpha 1.0, beta (2/5)(3/7)=0.171428; complexity descending (file Σ 7/3, symbol 7/3); co_change degree 3/mean(9,4)=0.461538, defaults (min_revs/shared=5) correctly exclude the fixture pair, lowered thresholds surface it. ✓
4. One JSON-parity unit test per arm (`*_arm_matches_cold_output`) + one spawned-server golden per tool (2 each for hotspots/complexity grains, 2 for co_change). ✓
5. Handshake re-accepted at 16; each new description carries literal `Use when ` + quoted triggers (verified in `handshake__tools_descriptions.snap`; asserted by `handshake_descriptions_carry_when_and_triggers`). ✓
6. nextest (3 crates) + architecture + clippy + fmt all green. ✓

Cold/daemon byte-parity holds by construction (identical handler logic + identical `summarize`) and is guarded by the parity unit tests; no graph type leaks the hexagonal boundary; `complexity` is the handler-side fold per D1/tier-13 D2; no smuggled dependency or pattern.
</verdict>

<next_steps>
None required for PASS. Optional, non-gating: regenerate `docs/codebase-overview.md` (or defer to 15c per its note); add `ariadne-cli/query.rs` to the `<files>` of future protocol-extending tiers to keep the forced exhaustive-match consequence explicit.
</next_steps>

<sources>
- tier-13 use cases: crates/ariadne-graph/src/hotspot.rs:102-150; co_change.rs:74-95.
- Reviewer standard (code health over perfection; nits don't gate): https://google.github.io/eng-practices/review/reviewer/standard.html
- Conventional/severity discipline: .claude/skills/spec-audit/SKILL.md `<non_negotiables>`.
</sources>
</content>
</invoke>
