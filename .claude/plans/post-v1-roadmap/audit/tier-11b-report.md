---
tier_id: tier-11b
audited: 2026-06-01
verdict: PASS
commit: a6a131f9a3e85b9464c68e76a6434f5193ffd38c
---

<scope>
Tier-11b "Symbol-level churn — gix line-hunks attributed to symbol spans via an
`ariadne-graph` use-case". Diff scoped to the tier `<files>` plus build-created
files (the new `gix/line_hunks.rs` submodule, three test files, ADR-0019). HEAD
`a6a131f`; tier-11b changes are an uncommitted working-tree diff on top (the
audit-gate hook gates the subsequent commit). Sibling `plan.md` (RD7, R-C3, R-C4)
read for decision/risk context.
</scope>

<checks_run>
- Read every changed file end-to-end: `symbol_churn.rs` (use-case), `line_hunks.rs`
  (git adapter), `records.rs` (`LineHunk`/`SymbolChurn`), `ports.rs` (Storage port
  +2 methods), `redb/mod.rs` + `tables.rs` + `migration.rs` (SYMBOL_CHURN + v5→v6),
  `cli/commands/index.rs` (composition-root wiring), `cli/config.rs`
  (`symbol_churn_depth`), all three test files, ADR-0019.
- `cargo nextest run -p ariadne-git -p ariadne-graph -p ariadne-storage` →
  89 passed, 2 skipped. New tests present and green: graph `symbol_churn` (4),
  git `line_hunks` (3), storage `symbol_churn` (3), migration `v5→v6` round-trip.
- `cargo test --test architecture` → ok (1 passed). Confirms `ariadne-git` keeps
  deps ⊆ {core}; the symbol join lives in `ariadne-graph` (use-case crate).
- `cargo clippy --workspace --all-targets -- -D warnings` → clean.
- `cargo fmt --all --check` → clean (exit 0).
- Manual dogfood: `ariadne init` + `ariadne index` on a fresh clone →
  `[index] symbol churn: 2964 symbols across 325 changed files`; re-run yields the
  identical line (determinism exit-criterion verified end-to-end).
- Hand-traced the byte→line conversion (`line_of`/`byte_span_to_lines`) and the
  overlap join against the unit-test fixtures: lines 1-3 / 5-7, gap line 4,
  multi-hunk dedup — all arithmetic correct (half-open byte span → 1-based
  inclusive lines).
- Verified `Storage` has exactly one implementor (`RedbStorage`) — no stub/no-op
  implementor silently satisfying the new trait methods.
- Verified `Span.byte_start/byte_end` (u32) and `ReadSnapshot::iter_files`/
  `iter_symbols` chunk signatures match the CLI's usage.
</checks_run>

<findings>
| id | category | severity | location | problem | fix |
|---|---|---|---|---|---|
| F1 | plan_adherence | INFO | crates/ariadne-cli/src/config.rs:64-97 | `config.rs` modified (adds `symbol_churn_depth`) but is absent from the tier `<files>` list; justified by step 6 (bounded attribution window) but the list under-specified the touched set. | None required — note for `<files>` accuracy in future tiers. |
| F2 | plan_adherence | INFO | crates/ariadne-git/src/adapters/gix/line_hunks.rs:1 | Line-hunk logic landed in a new `gix/line_hunks.rs` submodule rather than literally "modify `gix.rs`"; consistent with the existing `incremental.rs` submodule precedent and the ≤200-line rule, so justified. | None required. |
</findings>

<verdict>
PASS. Zero FAIL findings; two INFO notes, both justified plan-vs-`<files>`
deltas that do not affect correctness, architecture, or any exit criterion.

All five exit criteria independently verified:
1. `ariadne-git::walk_line_hunks` emits per-commit, per-file new-side line-hunk
   ranges via `gix` `blob-diff` (imara-diff Histogram); architecture test confirms
   the adapter stays symbol-agnostic (deps ⊆ {core}). ✓
2. Pure `ariadne-graph::attribute_symbol_churn` joins hunks to symbol HEAD line
   ranges → `SymbolChurn`, persisted to the new `SYMBOL_CHURN` table behind one
   additive `v5→v6` migration step (migration round-trip test green). ✓
3. Determinism: pure function (BTreeMap/BTreeSet, no clock, no RNG); unit test
   asserts byte-identical re-run, and the dogfood index reproduces an identical
   `2964 symbols / 325 files` line across two runs. ✓
4. Git adapter holds no symbol/parser dep; the join lives only in `ariadne-graph`;
   ADR-0019 records the boundary. ✓
5. nextest (git/graph/storage) + architecture + clippy + fmt all green. ✓

Correctness spot-checks passed: half-open byte span → 1-based inclusive line
conversion is correct; multi-hunk-per-commit dedup counts a commit once per
symbol; deletions/non-blob entries contribute no new-side hunk; the blake3 guard
in `build_symbol_lines` ensures the on-disk line index matches the indexed byte
offsets before attribution. The full-table redb scans in `build_symbol_lines`
are forced by the port API (`symbols_in_file` drops the `SymbolId` key) and run
once per cold index — within the cold-index budget, not a hot/incremental path.
The R1 memory probe does not apply (no Salsa / in-RAM petgraph touched).
</verdict>

<next_steps>
None blocking. Optional, non-gating: align the tier `<files>` list with the
actual touched set (add `cli/config.rs` and the `gix/line_hunks.rs` submodule) so
future audits scope the diff from an accurate manifest. Ready to commit.
</next_steps>

<sources>
- [src: .claude/plans/post-v1-roadmap/tier-11b-symbol-churn-attribution.md — exit_criteria, steps, files]
- [src: .claude/plans/post-v1-roadmap/plan.md — RD7, R-C3, R-C4]
- [src: docs/adr/0019-symbol-churn-attribution.md — use-case-layer join boundary]
- [src: tests/architecture.rs — adapter-isolation + use-case-deps invariant]
- [src: crates/ariadne-core/src/domain/types/span.rs:12-18 — half-open byte span]
- [src: https://docs.rs/gix/0.84.0/gix/diff/blob/struct.Diff.html — blob-diff hunks]
- [src: https://understandlegacycode.com/blog/key-points-of-software-design-x-rays/ — HEAD-layout approximation]
</sources>
