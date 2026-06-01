# ADR-0019: Symbol-Churn Attribution in the Use-Case Layer

<status>
Accepted
Date: 2026-06-01
Decider: claude
</status>

<context>
tier-11 records *which files* changed and how often; tier-13 hotspots want
finer grain — *which functions* churn [src: .claude/plans/post-v1-roadmap/plan.md
RD7]. Attributing a commit's changes to symbols needs two facts joined: the
changed line ranges (history) and the symbol defining spans (the indexed graph).
The hexagonal invariant forbids the driven `ariadne-git` adapter from depending
on the parser, the symbol table, or any other adapter — it depends only on
`ariadne-core` [src: tests/architecture.rs; CLAUDE.md hexagonal boundary rule].
So the history × symbols join cannot live in the git adapter.
</context>

<decision>
Split the work across the boundary. `ariadne-git` stays symbol-agnostic: for each
modified blob in a commit it runs `gix` `blob-diff` and emits the *new-side*
changed line ranges as `ariadne_core::LineHunk { path, start_line, end_line }`,
keyed per commit (`walk_line_hunks`). A pure `ariadne-graph` use-case
(`attribute_symbol_churn`) holds the symbol join: it converts each symbol's
`defining_span` (bytes) to a HEAD line range against the file's line index and
counts a commit for a symbol when any of that commit's changed lines on the file
fall in the symbol's line range, yielding `SymbolChurn { symbol, commits }`. The
CLI composition root wires the two together and persists to a new `SYMBOL_CHURN`
redb table behind one additive `v5 → v6` migration step.
</decision>

<rationale>
- **Maintainability (hexagonal):** the cross-cutting join lives in the use-case
  layer (`ariadne-graph`, deps ⊆ {core}), which legitimately reasons over symbol
  spans; the driven git adapter keeps deps ⊆ {core} and emits paths + line ranges
  only. Attributing inside `ariadne-git` would force a parser/symbol dependency
  into a driven adapter, breaking adapter isolation [src: tests/architecture.rs].
- **Reliability (determinism):** the use-case is a pure function of its inputs —
  no clock, no RNG — so the same index yields the same per-symbol counts. Reads
  return sorted by `SymbolId`; the imara-diff Histogram algorithm is deterministic.
- **Efficiency:** attribution only reads, hashes, and line-indexes files that
  actually changed in the window; symbols in untouched files are skipped (zero
  churn). The git walk holds no redb handle, matching the tier-11a
  single-open-per-process discipline.
- **Known approximation (R-C3):** historical line hunks are interpreted against
  the HEAD line layout, so attribution is exact for the latest revision and
  degrades for commits predating later line shifts. A bounded
  `[history] symbol_churn_depth` window (default 500 commits) keeps the drift
  small while the signal stays meaningful — the same limitation CodeScene's
  X-Ray accepts [src:
  https://understandlegacycode.com/blog/key-points-of-software-design-x-rays/].
  File-level churn (tier-11) is exact and unaffected.
</rationale>

<alternatives>
- **Attribute inside `ariadne-git`** — rejected: pulls the symbol table / parser
  into a driven adapter, an architecture hard-fail. `[src: tests/architecture.rs]`
- **A new `Symbol`/`History` port joining both in `ariadne-core`** — rejected:
  only the CLI consumes symbol churn and the join is pure data, so a
  composition-root call into the pure use-case suffices (precedent: ADR-0007,
  ADR-0018). `[src: docs/adr/0007-cli-composition-root.md]`
- **Reconstruct each commit's historical line layout (git blame-style)** —
  rejected for this tier: exact per-revision line tracking is far heavier; the
  bounded-window HEAD approximation is the accepted X-Ray trade-off (R-C3).
- **Persist `LineHunk`s** — rejected: they are a transient join input, not a
  query result; only the attributed `SymbolChurn` is persisted.
</alternatives>

<consequences>
- The redb schema bumps `v5 → v6` with one additive `MigrationStep` creating the
  `SYMBOL_CHURN` table (`SymbolId` bytes → postcard `SymbolChurn`); pre-existing
  databases upgrade in place, no rebuild [src: docs/adr/0002-tech-stack.md;
  plan.md RD2].
- `tests/architecture.rs` stays green: `ariadne-git` gains no parser/symbol dep;
  the join lives in `ariadne-graph`.
- `Storage` gains `replace_symbol_churn` (wholesale replace, mirroring
  `replace_history`) + `all_symbol_churn`; symbols with no attributed commit are
  absent from the table (read as zero).
- tier-13 hotspot/co-change metrics consume `SYMBOL_CHURN` for function-level
  signal.
</consequences>

<sources>
- `[src: .claude/plans/post-v1-roadmap/plan.md RD7, R-C3]`
- `[src: .claude/plans/post-v1-roadmap/tier-11b-symbol-churn-attribution.md]`
- `[src: tests/architecture.rs — adapter-isolation invariant]`
- `[src: docs/adr/0018-git-history-adapter.md]`
- `[src: https://docs.rs/gix/0.84.0/gix/diff/blob/struct.Diff.html]`
- `[src: https://understandlegacycode.com/blog/key-points-of-software-design-x-rays/]`
</sources>
