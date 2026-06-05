---
tier_id: tier-01
audited: 2026-06-05
verdict: PASS
commit: 6011ea2df1b69d99de2a2db9c806dc98a9d440f1
---

<scope>
Re-audit of `tier-01-occurrence-ingest-reference-edges` against sibling `plan.md`,
after the remediation recorded in the tier file resolved the prior FAIL
(`tier-01-report.md` @ `6011ea2`, two RED parity goldens). Scoped diff = uncommitted
working tree on `main` @ `6011ea2`:
- New: `crates/ariadne-core/src/domain/scip.rs` (pure `ScipFacts`/`ScipOccurrence`),
  `crates/ariadne-scip/src/facts.rs` (`extract_facts`),
  `crates/ariadne-scip/tests/extract_facts.rs`, `crates/ariadne-salsa/tests/scip_edges.rs`.
- Modified (production, byte-identical to the prior audit): salsa `inputs.rs`
  (`ScipDocInput`→`ScipFactsInput`), `derived.rs`
  (`scip_symbols`→`scip_facts_for_file`; `symbols_for_file` drops the scip param),
  `derive.rs` (`file_facts` + `resolve_scip_edges`), `db.rs` (`set_scip_facts`,
  coverage gate in `commit_revision`), `memory.rs` (`scip_facts_bytes`), `lib.rs`
  re-exports; core `lib.rs`/`domain/mod.rs`; scip `lib.rs`; cli `domain/mod.rs`
  (`run_scip_ingest` out-of-band behind `--scip`). API-rename fallout in
  `benches/edit.rs`, salsa `tests/{durability,equivalence}.rs`, watcher `tests/events.rs`.
- Remediation delta vs the prior audit: only the two re-baselined goldens
  `crates/ariadne-cli/tests/goldens/parity_{java,csharp}.txt` (−1 edge each) and the
  tier-doc `<remediation>` block. No production code changed.
- `crates/ariadne-daemon/**` listed in `<files>` but not touched — consistent with
  D6 ("daemon has zero SCIP wiring today"; the daemon pass is T4). Not a defect.
</scope>

<checks_run>
- Read every changed/new file end-to-end; re-verified the salsa↔scip boundary and
  the `commit_revision` coverage gate.
- `cargo nextest run --workspace` → **460 passed, 19 skipped, exit 0** (prior 2/460
  parity failures now green).
- Tier-01 behavioral tests directly: `report_lists_every_tracked_table`,
  `over_budget_filters_correctly`, `deep_size_counts_owned_buffers` (R7 memory
  guard, `scip_facts` table listed) + the two headline `scip_edges` cases → 5/5 PASS;
  all 4 `scip_edges` + 3 `extract_facts` + 5 `facts` unit tests green within the
  workspace run.
- `cargo test --test architecture` (ariadne-workspace) → `architecture_invariants_hold`
  PASS: USE_CASE_CRATES (`ariadne-salsa`,`ariadne-graph`) may dep only on
  `ariadne-core`+`ariadne-storage`; salsa ⊥ `ariadne-scip` (D2).
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` → clean
  (exit 0). `cargo fmt --all --check` → clean. `cargo deny check` →
  advisories/bans/licenses/sources ok; only a pre-existing benign
  `Zlib license-not-encountered` warning; **no new dep** (no Cargo.toml/lock change;
  `blake3`/`serde` already workspace deps of scip/core).
- Golden re-baseline correctness: the java/csharp fixtures
  (`Caller.run()`→`Callee.helper()`, `Run()`→`Helper()`) are cross-file,
  Path/Method-qualified, no import, no same-file def — the exact ADR-0025 abstention.
  Each golden dropped exactly that one `References` edge; all symbols/files retained;
  the bare-identifier rust golden keeps its free-call edge (R4: −1 edge/fixture,
  files/symbols unchanged). Parity indexes without `--scip`, so the syntactic-only
  goldens now match the precise resolver — confirmed by the green suite.
- Did NOT re-run the live `cargo run -p ariadne-cli -- index --scip` dogfood: it
  depends on external SCIP indexer binaries and would clobber the active MCP index
  (redb open @ revision 1003). Its three assertions are each covered by green,
  deterministic in-process tests — recall recovery
  (`cross_crate_method_call_resolves_with_scip_facts`), the std-`new` no-edge case
  (`std_callee_occurrence_yields_no_edge`), and index-twice determinism
  (`incremental_sequence_equals_fresh_rebuild` + cold==warm equivalence). Production
  code is byte-identical to the prior build that performed the live dogfood.
</checks_run>

<findings>
| id | category | severity | location | problem | fix |
|---|---|---|---|---|---|
| R-F1 | tests / exit_criteria | RESOLVED | `crates/ariadne-cli/tests/goldens/parity_{java,csharp}.txt` | Prior FAIL: `cargo nextest run --workspace` RED (2/460); goldens still held a cross-file Method `run→helper`/`Run→Helper` edge the resolver abstains on. Remediation re-baselined both via `UPDATE_GOLDENS`; suite re-run green (460/460). Verified the dropped edges are the intended ADR-0025 abstention (fixtures are cross-file qualified calls with no import/no same-file def) and no legitimate edge or symbol was lost. | None — fixed. |
| R-F2 | docs / validation | RESOLVED | tier frontmatter `status: completed`; `<verification>` | Prior FAIL: `completed`/green attestation unbacked. Now `cargo nextest run --workspace` is green at this working tree; the `<remediation>` block records the goldens re-baseline and edge delta per R4. Attestation truthful. | None — fixed. |
| R-I1 | plan_adherence | INFO | tier `<remediation>`; `crates/ariadne-core/src/domain/scip.rs`; `crates/ariadne-scip/src/facts.rs` | The pure core type is `ScipFacts` (the salsa mirror is `ScipFactsRaw`), reconciling the exit-criterion wording per the `SyntacticFacts`/`SyntacticFactsRaw` precedent. Contract met (pure, no prost/redb, threaded via a salsa input). Non-blocking. | None needed. |
</findings>

<verdict>
**PASS.** Zero FAIL findings. Every `<verification>` command re-run is green —
`cargo nextest run --workspace` (460/460), `cargo test --test architecture`
(salsa ⊥ scip), clippy `-D warnings`, `cargo fmt --check`, `cargo deny check`
(no new dep), and the R7 memory-table guard. All four `<exit_criteria>` are
independently satisfied by committed tests: the RED→GREEN cross-crate Method
resolution plus its no-SCIP abstention control (crit 1); `extract_facts` as a pure
core type threaded via `ScipFactsInput`/`ScipFactsRaw` with the architecture
invariant green (crit 2); the std-callee no-edge case and the hash-drift
resolver-fallback (crit 3); and recall recovery + determinism + cold==warm /
incremental==fresh parity (crit 4). The single prior blocker — two stale parity
goldens inherited from r1-resolver-completion (`985116d`/`97f122a`), not a tier-01
regression — is correctly remediated: the re-baseline removes only the genuine
ADR-0025 cross-file Method abstentions, with files/symbols intact and the
free-identifier edge retained. `resolve_scip_edges` is deterministic by
construction (file_id + byte_range sorts; the `EdgeKey` HashSet is dedup-membership
only, never iterated for output); the coverage gate keeps the SCIP and tree-sitter
edge sets disjoint by `src`; byte-range conversion handles UTF-8/16/32 and malformed
ranges; the `symbol_roles` cast is bit-preserving. The implementation satisfies the
plan and is safe to ship.
</verdict>

<next_steps>
None. Tier-01 is complete and verified. Proceed to `tier-02-access-role-read-write-edges`.
Optional (non-blocking): when external SCIP indexer binaries are available in a
disposable checkout, run the live `ariadne index --scip` dogfood to observe recall
recovery on the real graph; the behavior is already covered by the in-process suite.
</next_steps>

<sources>
- Tier `<verification>`/`<exit_criteria>`/`<remediation>`: .claude/plans/scip-driven-edges/tier-01-occurrence-ingest-reference-edges.md:5-9,66-106
- Prior FAIL report: .claude/plans/scip-driven-edges/audit/tier-01-report.md (@ 6011ea2)
- Golden re-baseline: parity_{java,csharp}.txt −1 edge; fixtures crates/ariadne-cli/fixtures/{java,csharp}/{Caller,Callee}.* (cross-file qualified, no import/no same-file def)
- ADR-0025 cross-file Method/Path abstention: docs/adr/0025; plan.md:26-33,98-104
- Boundary D2 (salsa ⊥ ariadne-scip): tests/architecture.rs:32-45; plan.md:56-58,81-88
- SCIP roles + edge resolution (D3): crates/ariadne-salsa/src/derive.rs resolve_scip_edges; scip.proto:521-543,645-680
- Coverage gate (D4): crates/ariadne-salsa/src/db.rs commit_revision; crates/ariadne-salsa/tests/scip_edges.rs
- Validate-by-execution / reviewer standard: CLAUDE.md `<rules>`; https://google.github.io/eng-practices/review/reviewer/standard.html
</sources>
