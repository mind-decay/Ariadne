# ADR-0010: Streaming cold-index pipeline

<status>
Accepted
Date: 2026-05-21
Decider: claude
</status>

<context>
The tier-12 SLO release gate still failed — risk R8 unresolved. On the
121,100-file / 9-language corpus the cold index ran 84.3s against the < 60s
SLO and peaked at 4833 MiB against the 4096 MiB ceiling (R1)
[src: ../../.claude/plans/ariadne-core/tier-12-parallel-cold-index.md
`<blockers>`]. Phase attribution on `torvalds/linux` showed parse already
saturating the cores (~11x) at ~83% of cold time, while the single redb
commit and the peak RSS both scaled with the 1.9M-symbol / 3.5M-edge volume.

tier-12 parallelised the parse and dropped each file's bytes inside the parse
closure, but kept a `collect()` → `assemble()` → single-transaction-commit
shape: every `ParsedFile` (with its `SyntacticFacts`) was held in RAM at once,
the whole `Changeset` was built before any write, and commit trailed parse
serially. tier-12 also recompiled the per-language tree-sitter `Query` on
every one of the 121,100 files
[src: ../../crates/ariadne-parser/src/adapters/treesitter/facts.rs].

The release gate is judged on the M4 Pro dev machine; the no-quality-loss
constraint rules out any lossy lever (file-size caps, sampling). The index
must keep every file, symbol, and edge it produces today.
</context>

<decision>
Cold-index the repository as a streaming pipeline: the parallel parse hands
per-file facts down a bounded `std::sync::mpsc::sync_channel` to a single
committer thread that writes redb in bounded file/symbol/edge batches, each
its own write transaction. Each parse worker caches a compiled-`Query`
`FactExtractor` (plus its `tree_sitter::Parser`) per language and reuses one
`QueryCursor` across files, so neither the grammar nor the fact query is
rebuilt per file.
</decision>

<rationale>
- **Efficiency** — reusing a compiled `Query` + `QueryCursor` removes 121,100
  redundant query compilations; a `QueryCursor` is explicitly designed for
  reuse across queries
  [src: https://docs.rs/tree-sitter/0.26.8/tree_sitter/struct.QueryCursor.html].
- **Efficiency** — commit overlaps parse instead of trailing it: the committer
  drains the channel and flushes file/symbol batches while workers still
  parse, so the serial commit tail shrinks to the post-parse edge batches.
- **Reliability / efficiency (R1)** — each batch is a separate redb write
  transaction, so dirty pages flush per batch rather than accumulating the
  whole corpus in one transaction
  [src: https://docs.rs/redb/4.1.0/redb/struct.WriteTransaction.html]. The
  raw file bytes, parse trees, and the in-flight `Changeset` are bounded by
  the batch size, not the corpus size — `ParsedFile`s are processed and
  dropped as they arrive, never all held. The `name_to_symbols`,
  `facts_by_file`, and resolved `edge_list` symbol metadata stays corpus-
  sized and dominates the residual peak, but at a fraction of tier-12's
  raw-byte peak.
- **Reliability** — determinism is preserved without depending on parse-
  completion order. `FileId` stays the sorted-path index + 1; the committer
  sorts each name's symbol-candidate list by `(defining FileId, def byte
  start)` and the per-file edge inputs by `FileId` before edge resolution,
  reproducing tier-12's `FileId`-ordered `candidates.first()` selection. Two
  `--fresh` runs produce identical file/symbol/edge counts and an identical
  JSON summary (modulo the wall-clock field).
- **Maintainability** — the worker fan-out reuses the existing `rayon`
  data-parallel iterator (`for_each_init`)
  [src: https://docs.rs/rayon/1.12.0/rayon/iter/trait.ParallelIterator.html#method.for_each_init];
  no new dependency, technology, or cross-crate edge is introduced.
</rationale>

<alternatives>
- **Single-transaction commit (tier-12 shape)** — rejected: holds the whole
  `Changeset` plus every `ParsedFile` in RAM at once, the peak that breached
  the 4 GiB ceiling, and serialises commit behind parse.
- **File-size cap / sampling** — rejected: lossy. The no-quality-loss
  constraint forbids dropping any file, symbol, or edge
  [src: ../../.claude/plans/ariadne-core/tier-13-cold-index-slo.md `<context>`].
- **`Durability::None` on batch commits** — not adopted; kept as a noted
  fallback. Per-batch transactions with default durability already bound the
  RAM peak; trading the crash-durability of a partially-built index for
  marginal commit speed is unjustified unless a future measurement shows
  commit `fsync` dominating.
</alternatives>

<consequences>
- `run_index` is a parse → channel → committer pipeline; the committer owns
  the redb handle and is the single writer. Edge resolution stays a post-
  parse pass because global name resolution needs every symbol.
- The persisted `revision` after a cold index is now the batch count, not 1;
  it stays deterministic for a fixed corpus (a function of file and edge
  counts). No consumer pins `revision == 1`.
- `ariadne-parser` exposes `FactExtractor`; `extract_syntactic_facts` becomes
  a thin one-shot wrapper, so the parser test suites are unchanged.
- redb schema and on-disk format are untouched — the change is behavioural.
  Reverting the pipeline leaves a correct, slower index (see tier-13
  `<rollback>`).
</consequences>

<sources>
- `[src: ../../.claude/plans/ariadne-core/tier-13-cold-index-slo.md]`
- `[src: ../../.claude/plans/ariadne-core/tier-12-parallel-cold-index.md]`
- `[src: https://docs.rs/tree-sitter/0.26.8/tree_sitter/struct.QueryCursor.html]`
- `[src: https://docs.rs/redb/4.1.0/redb/struct.WriteTransaction.html]`
- `[src: https://docs.rs/rayon/1.12.0/rayon/iter/trait.ParallelIterator.html#method.for_each_init]`
</sources>
