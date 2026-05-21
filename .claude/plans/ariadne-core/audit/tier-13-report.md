---
tier_id: tier-13
audited: 2026-05-21
verdict: PASS
commit: 2de7c0ba7c1f084b53ef8f5c1285a9ed42db6e80
---

<scope>
Audited tier-13 (`tier-13-cold-index-slo.md`, status `completed`) against its
sibling `plan.md`. tier-13 closes the v1 SLO release gate with a streaming
parse→committer cold-index pipeline + tree-sitter `Query`/`QueryCursor` reuse.

In-scope `<files>`, all verified end-to-end:
- `docs/adr/0010-streaming-cold-index.md` — NEW.
- `crates/ariadne-parser/src/adapters/treesitter/facts.rs` — `FactExtractor`.
- `crates/ariadne-parser/src/lib.rs` — `FactExtractor` re-export.
- `crates/ariadne-cli/src/domain/mod.rs` — streaming pipeline rewrite.
- `crates/ariadne-cli/src/commands/index.rs` — parse sub-phase stderr line.
- `crates/ariadne-storage/src/adapters/redb/*` — CONDITIONAL (step 8).
- `plan.md` + `tier-10` + `tier-12` status/risks edits (step 10).

The working tree carries tiers 10–13 uncommitted over HEAD (tier-09); the
C/C++ deltas in `registry.rs` / `lang.rs` are tier-11 and were excluded.
</scope>

<checks_run>
- `cargo build --workspace` — clean.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` — clean.
- `cargo fmt --all --check` — clean.
- `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --document-private-items` — clean.
- `cargo test --test architecture` — 1 passed; no new cross-crate edge.
- `cargo nextest run --workspace` — 129 passed, 9 skipped. `facts_*` suites
  green unchanged: `tests/common/mod.rs` routes through the
  `extract_syntactic_facts` wrapper, whose signature is preserved.
- `cargo nextest run -p ariadne-e2e --run-ignored all --release -j 1` — 11/11
  passed. Every `repos/*` cold index < 60s.
- SLO release gate (`slo_release_gate`, re-run `--no-capture`): PASS.
  121,100 files / 1,917,162 symbols / 3,511,123 edges / 9 langs —
  **cold 39.0s** (<60s), **peak RSS 3577 MiB** (<4096), **incremental apply
  p95 361µs** (<500ms, 160 samples), **query p95 181µs** (<100ms, 100 samples).
- Determinism: two `ariadne index --fresh` runs on the 10-language parser
  fixture tree — `files=11 symbols=1047 edges=243 revision=2` and the `langs`
  order are byte-identical across runs; only `elapsed_ms` differs (see I1).
- Code read end-to-end: streaming pipeline, committer, edge resolution,
  `FactExtractor`, `apply_writes`, channel/`for_each_init` `!Send` handling.
- `FactExtractor` API keeps `tree_sitter::{Language,Query,QueryCursor}` crate-
  private (`compile` is `pub(crate)`); hexagonal boundary intact.
- redb step 8 (`SYMBOLS_BY_FILE`) correctly NOT applied — incremental p95
  361µs ≪ 500ms, so the conditional `apply.rs` scan fix was not triggered.
</checks_run>

<findings>
| id | category | severity | file:line | problem | fix |
|----|----------|----------|-----------|---------|-----|
| I1 | exit_criteria | INFO | tier-13 exit_criterion 3 / `<verification>`; crates/ariadne-cli/src/domain/mod.rs:107 | The criterion says two `--fresh` runs yield a "byte-identical JSON summary", but `IndexSummary.elapsed_ms` is wall-clock and differs run-to-run (observed 71ms vs 54ms); all graph-content fields are identical. | Tighten the criterion to "identical modulo `elapsed_ms`" as ADR-0010 already states; implementation needs no change. |
| I2 | docs | INFO | docs/adr/0010-streaming-cold-index.md:51-54 | `<rationale>` claims the in-RAM working set is "bounded by the batch size, not the corpus size"; `name_to_symbols`, `facts_by_file` and the resolved `edge_list` are in fact corpus-sized and dominate the 3577 MiB peak. | Qualify: raw bytes + parse trees + the in-flight `Changeset` are batch-bounded; symbol/call/edge metadata stays corpus-sized but far below tier-12's raw-byte peak. |
</findings>

<verdict>
**PASS** — 0 FAIL, 2 INFO.

Every exit criterion is met:
1. `run_index` is a parse→`sync_channel`→committer pipeline; `commit_batch`
   commits file/symbol upserts every `COMMIT_BATCH` (4096) files as separate
   redb transactions, edges in `EDGE_COMMIT_BATCH` batches post-parse. ✓
2. `FactExtractor` compiles each lang's `Query` once and reuses one
   `QueryCursor`; per-worker `extractors: HashMap<Lang, FactExtractor>` caches
   it; `extract_syntactic_facts` is a thin one-shot wrapper, so the parser
   test suites are unchanged and green. ✓
3. Two `--fresh` runs produce identical FileId/symbol/edge/lang/revision
   output — the determinism the tier targets (parse-order-independent FileId
   + `(file, def_start)`-sorted edge-dst). The JSON line is identical except
   `elapsed_ms`; see I1 — wording, not a defect. ✓ (intent met)
4. SLO gate green on the 121,100-file corpus, re-verified this audit: cold
   39.0s, peak 3577 MiB, incremental p95 361µs, query p95 181µs — all four
   budgets cleared with headroom. ✓
5. ADR-0010 status `Accepted`, follows `docs/adr/_template.md`, cited from
   tier-13 and from `plan.md` risk R8. ✓
6. build / clippy / fmt / doc / architecture / workspace nextest all green. ✓

I1 is not a FAIL: the exit criterion's parenthetical ("deterministic FileId +
edge-dst selection preserved") states the real intent, that determinism is
empirically verified, and ADR-0010 already documents the wall-clock exception
honestly. `elapsed_ms` predates tier-13. Failing the release gate over a timer
field would be a false positive. I2 is a doc-accuracy nit; the peak-RSS budget
is met empirically. Neither blocks.

The streaming pipeline is correct: the committer is the sole redb writer;
`for_each_init` builds `!Send` parsers/extractors per worker thread, never
shared; `ParsedFile` carries no raw bytes; channel backpressure bounds the
parse-time working set; a committer error/panic is surfaced via `join`; edge
resolution is a deterministic post-parse pass independent of send order.
</verdict>

<next_steps>
None blocking — tier-13 is accepted. Optional, at the user's discretion:
- I1: reword tier-13 exit criterion 3 + `<verification>` to "identical modulo
  `elapsed_ms`" so the criterion matches ADR-0010 and the artifact.
- I2: soften ADR-0010 `<rationale>` lines 51-54 per the fix column.
Process note (outside tier-13 scope): `audit-state.json` previously recorded
tier-11 `FAIL` and no tier-12 audit exists; the commit/push gate state should
be reconciled before landing tiers 10–13.
</next_steps>

<sources>
- tier-13: `.claude/plans/ariadne-core/tier-13-cold-index-slo.md`
- plan: `.claude/plans/ariadne-core/plan.md`; tier-12: `tier-12-parallel-cold-index.md`
- ADR: `docs/adr/0010-streaming-cold-index.md`, `docs/adr/_template.md`
- tree-sitter QueryCursor reuse: https://docs.rs/tree-sitter/0.26.8/tree_sitter/struct.QueryCursor.html
- redb WriteTransaction (per-batch commit): https://docs.rs/redb/4.1.0/redb/struct.WriteTransaction.html
- rayon `for_each_init`: https://docs.rs/rayon/1.12.0/rayon/iter/trait.ParallelIterator.html#method.for_each_init
- Reviewer standard: https://google.github.io/eng-practices/review/reviewer/standard.html
</sources>
