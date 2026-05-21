---
tier_id: tier-13
title: Streaming cold-index pipeline + parse-phase tuning — close the SLO release gate
deps: [tier-12]
exit_criteria:
  - "`run_index` streams per-file facts from the parallel parse to a committer thread; redb is committed in bounded file/symbol/edge batches, not one transaction."
  - "`ariadne-parser` exposes a `FactExtractor` that compiles each language's tree-sitter `Query` once and reuses one `QueryCursor`; `extract_syntactic_facts` no longer recompiles the query per file."
  - "Two `ariadne index --fresh` runs on a mixed-language tree produce a JSON summary identical modulo `elapsed_ms` (deterministic FileId + edge-dst selection preserved)."
  - "`cargo nextest run -p ariadne-e2e --run-ignored all` is green on the 121,100-file corpus: cold < 60s, peak RSS < 4 GiB, incremental apply p95 < 500ms, query p95 < 100ms."
  - "`docs/adr/0010-streaming-cold-index.md` written, status Accepted, cited from this tier + plan.md `<risks>`."
  - "`cargo build --workspace`, `clippy -D warnings`, `fmt --check`, `cargo test --test architecture`, `cargo nextest run --workspace` all green."
status: completed
completed: 2026-05-21
---

<context>
tier-12's SLO release gate FAILED [src: tier-12-parallel-cold-index.md
`<blockers>`]: on the 121,100-file / 9-language corpus the cold index runs
84.343s against the < 60s SLO and peaks at 4833 MiB against the 4096 MiB
ceiling (R1). The test panics on the cold assertion before the incremental
and query stages run, so those p95 SLOs are unverified at 100K scale (R9).

Phase attribution (tier-12 step 1, `torvalds/linux` isolated): parse is ~83%
of cold time and already saturates the cores (~11x); the single redb commit
and the peak RSS both scale with symbol/edge volume (1.9M symbols, 3.5M
edges). This tier closes the gap with two non-lossy levers — the index keeps
every file, symbol, and edge it produces today:
1. A streaming pipeline — parse workers hand per-file facts to a committer
   thread that writes redb in bounded batches, so commit overlaps parse
   instead of trailing it and the in-RAM working set is bounded by batch
   size, not corpus size.
2. Parse-phase tuning — `extract_syntactic_facts` recompiles the tree-sitter
   `Query` on every one of the 121,100 files [src: crates/ariadne-parser/src/adapters/treesitter/facts.rs:160-163].
   A compiled `Query` + `QueryCursor` is reusable [src: https://docs.rs/tree-sitter/0.26.8/tree_sitter/struct.QueryCursor.html];
   caching them per worker removes 121,100 redundant compilations.

The gate is judged on the M4 Pro dev machine; only non-lossy optimisation is
in scope [user]. tier-13 owns all three SLOs. Full context: plan.md +
tier-12-parallel-cold-index.md.
</context>

<files>
- docs/adr/0010-streaming-cold-index.md — NEW. Streaming pipeline, chunked
  redb commit, `Query`/`QueryCursor` reuse, deterministic-ordering guarantee.
- crates/ariadne-parser/src/adapters/treesitter/facts.rs — add `FactExtractor`
  (owns a compiled `Query` + a reusable `QueryCursor`); `extract_syntactic_facts`
  becomes a thin wrapper over it.
- crates/ariadne-parser/src/lib.rs — re-export `FactExtractor`.
- crates/ariadne-cli/src/domain/mod.rs — rewrite `run_index` as a
  parse→channel→committer pipeline with bounded batch commits; per-worker
  `FactExtractor` cache; deterministic candidate ordering; parse sub-timings.
- crates/ariadne-cli/src/commands/index.rs — print the parse sub-phase line.
- crates/ariadne-storage/src/adapters/redb/* — CONDITIONAL (step 8 only): a
  `SYMBOLS_BY_FILE` multimap index + `SCHEMA_VERSION` bump, applied solely if
  the incremental p95 probe implicates the `apply.rs` SYMBOLS full-scan.
</files>

<steps>
1. **Failing measurement first.** Sub-instrument the parse closure to record
   wall time in three buckets — file read, tree-sitter parse, fact extraction
   — accumulated across workers via atomics, and emit them on stderr beside
   the existing `PhaseTimings` line [src: crates/ariadne-cli/src/domain/mod.rs:91-104].
   Run `ariadne index` on the corpus; record the 84s breakdown in the tier-13
   audit. This attributes the parse phase before any fix and gates step 7.
2. **`FactExtractor`.** In `ariadne-parser` add `FactExtractor { query: Query,
   cursor: QueryCursor }`, built once per `Lang` from the lang's `Language`;
   method `extract(&mut self, tree, source) -> Result<SyntacticFacts>`.
   `Query::new` runs in the constructor; the `QueryCursor` is reused across
   `extract` calls [src: https://docs.rs/tree-sitter/0.26.8/tree_sitter/struct.QueryCursor.html].
   Keep `extract_syntactic_facts` as a wrapper that builds a one-shot
   `FactExtractor`, so the existing `facts_*.rs` parser tests are unchanged.
3. **Per-worker state.** Extend the parse workers' init state to hold, beside
   the existing per-`Lang` `TreeSitterParser` cache, a per-`Lang`
   `FactExtractor` cache and a cloned `SyncSender`. Both parsers and extractors
   are `!Send`, built lazily on the worker thread, so no `Query`/`Parser` is
   shared [src: crates/ariadne-cli/src/domain/mod.rs:274-309].
4. **Streaming pipeline.** Replace the `collect()`-then-`assemble()`-then-commit
   sequence with: spawn one committer OS thread; drive the parse with
   `paths.par_iter().enumerate().for_each_init(...)`
   [src: https://docs.rs/rayon/1.12.0/rayon/iter/trait.ParallelIterator.html#method.for_each_init],
   each closure parsing one file and sending its `ParsedFile` down a bounded
   `std::sync::mpsc::sync_channel` (capacity a few thousand — large enough
   that parse is throttled only when commit genuinely lags); drop the senders;
   `join` the committer for the counts. `FileId` stays the sorted-path index
   + 1, so it is unaffected by send order.
5. **Chunked commit.** The committer drains the channel, accumulating a
   `Changeset` of file + symbol upserts plus `name_to_symbols` and per-file
   edge inputs; every N files (N from step-1 data, start at 4096) it runs
   `storage.begin_write()?.apply(&cs)?` and starts a fresh `Changeset`. Each
   `apply` is a separate redb transaction [src: crates/ariadne-storage/src/adapters/redb/mod.rs:138-147],
   so dirty pages flush per batch and never hold the whole corpus
   [src: https://docs.rs/redb/4.1.0/redb/struct.WriteTransaction.html].
6. **Deterministic edges.** After the channel closes, sort every
   `name_to_symbols` candidate list by `(defining FileId, def byte start)` —
   reproducing tier-12's FileId-ordered `candidates.first()` selection
   regardless of parse-completion order [src: crates/ariadne-cli/src/domain/mod.rs:472-519].
   Run the existing `resolve_edges` pass; commit edges in the same N batches.
7. **Re-measure cold + memory.** Re-run `ariadne index` on the corpus. With
   `Query` reuse and commit overlapped behind parse, record the new phase
   breakdown and `/usr/bin/time` peak RSS in the audit. If parse remains the
   gap, apply the further cuts the step-1 data implicates (e.g. one `metadata`
   syscall instead of two; drop the `SyntacticFacts` intermediate clone) —
   never a file-size cap.
8. **Incremental p95.** Run the `slo` gate; it now reaches the incremental
   stage. If apply p95 ≥ 500ms, attribute it: `apply.rs` scans the whole
   `SYMBOLS` table on every file delete [src: crates/ariadne-storage/src/adapters/redb/apply.rs:24-44]
   — at 1.9M symbols that is the prime suspect. Fix = add a `SYMBOLS_BY_FILE`
   multimap (mirrors `EDGES_BY_FILE`), populate it in `apply_writes`, replace
   the scan with `remove_all`, bump `SCHEMA_VERSION`. Apply only if the
   measurement implicates the scan; otherwise record the real cause.
9. **Query p95.** The gate now reaches the query stage. Confirm `blast_radius`
   p95 < 100ms at 3.5M edges; if it breaches, attribute (graph build vs
   traversal) and fix from measurement. Do not pre-optimise.
10. **ADR-0010** + plan.md: write `docs/adr/0010-streaming-cold-index.md` per
    the ADR template (decision = streaming parse→commit pipeline + chunked
    redb transactions + `Query`/`QueryCursor` reuse; rejected = single-txn
    commit, file-size cap [lossy — ruled out by the no-quality-loss
    constraint], `Durability::None` batch commits [kept as a noted fallback]).
    Set tier-13 / tier-12 / tier-10 `status` per the gate result.
</steps>

<verification>
- `cargo build --workspace`, `cargo clippy --workspace --all-targets
  --all-features -- -D warnings`, `cargo fmt --all --check`,
  `RUSTDOCFLAGS=-D warnings cargo doc --workspace --no-deps
  --document-private-items` — clean.
- `cargo test --test architecture` green: no new cross-crate edge — `rayon`,
  `redb`, `tree-sitter` are existing dependencies of the crates touched.
- `cargo nextest run --workspace` green; the parser `facts_*` tests pass
  unchanged via the `extract_syntactic_facts` wrapper.
- Determinism: two `ariadne index --fresh` runs on a mixed-language tree
  produce a JSON summary identical modulo `elapsed_ms` and identical redb
  symbol/edge counts.
- `cargo nextest run -p ariadne-e2e --run-ignored all` — `slo` green: cold
  < 60s, peak RSS < 4 GiB, incremental p95 < 500ms, query p95 < 100ms; every
  `tests/repos/*` cold < 60s. Any breach fails loud — the bench is never
  silenced [src: tier-10-cli-e2e.md `<verification>`].
- Parse sub-phase breakdown + new peak RSS recorded in the tier-13 audit.
- If, after every step 2–9 lever, cold index stays ≥ 60s on the M4 Pro, the
  tier closes `blocked` with the measured residual and escalates per plan R8 —
  exhausting the non-lossy levers is a decision for the user, not a silent
  miss or a weakened assertion.
</verification>

<rollback>
`git revert` the `domain/mod.rs` pipeline rewrite, the `facts.rs`
`FactExtractor` addition, the `index.rs` print line, and (if applied) the
`ariadne-storage` `SYMBOLS_BY_FILE` change; delete ADR-0010. The streaming
pipeline and `Query` reuse are behavioural only — reverting leaves a correct,
slower index. The `SYMBOLS_BY_FILE` revert restores the prior
`SCHEMA_VERSION`; an index built under the new schema is rebuilt from source
on a version mismatch (R4), so no on-disk data migration is owed.
</rollback>
