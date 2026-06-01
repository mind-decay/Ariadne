---
tier_id: tier-11b
title: Symbol-level churn — gix line-hunks attributed to symbol spans via an ariadne-graph use-case
deps: [tier-11, tier-07a]
exit_criteria:
  - `ariadne-git` emits per-commit, per-file changed line-hunk ranges (new-side) via `gix` `blob-diff`, staying symbol-agnostic (deps ⊆ {core}).
  - A pure `ariadne-graph` use-case attributes each commit's changed lines to the symbols whose span covers them, yielding per-symbol churn; persisted to a new `SYMBOL_CHURN` redb table behind one migration step.
  - Attribution is deterministic — the same index yields the same per-symbol counts (no clock, no RNG).
  - The git adapter holds no symbol/parser dependency; the symbol join lives only in `ariadne-graph` (recorded in ADR-0019).
  - `cargo nextest run -p ariadne-git -p ariadne-graph -p ariadne-storage` + architecture + clippy + fmt all green.
status: completed
completed: 2026-06-01
---

<context>
tier-11 records which files changed and how often; tier-13 hotspots want finer grain — which functions churn. This tier attributes commit changes to symbols: `ariadne-git` emits changed line ranges, and an `ariadne-graph` use-case joins them against the symbol spans from the shared derivation (tier-07a) to produce per-symbol churn. The cross-cutting join (history × symbols) lives in the use-case layer, not the driven git adapter, which depends only on `ariadne-core` and cannot know symbol ranges — this boundary is the ADR-0019 decision. Full context: plan.md RD7.
</context>

<files>
- crates/ariadne-git/src/adapters/gix/line_hunks.rs — new: per modified blob in a commit, emit new-side changed line-hunk ranges via `blob-diff` (the feature already enabled in tier-11). Lives in a submodule beside the existing `incremental.rs`, honouring the ≤200-line rule and the one-file-per-concern precedent.
- crates/ariadne-git/src/adapters/gix/mod.rs — modify: declare + re-export the `line_hunks` submodule (`gix.rs` became the `gix/` directory module in tier-11a).
- crates/ariadne-git/src/lib.rs — modify: expose the per-commit line-hunk output (pure type in `ariadne-core`).
- crates/ariadne-core/src/domain/records.rs — modify: add `LineHunk { path, start_line, end_line }` (transient join input) + `SymbolChurn { symbol, commits }` (persisted) [src: crates/ariadne-core/src/domain/records.rs:11-49].
- crates/ariadne-graph/src/symbol_churn.rs — new: the pure attribution use-case.
- crates/ariadne-core/src/domain/ports.rs — modify: `Storage` gains `replace_symbol_churn()` + a reader.
- crates/ariadne-storage/src/adapters/redb/tables.rs — modify: add `SYMBOL_CHURN` (`&[u8]` `SymbolId` → postcard `SymbolChurn`); bump `SCHEMA_VERSION` by 1 [src: crates/ariadne-storage/src/adapters/redb/tables.rs:12-17].
- crates/ariadne-storage/src/domain/migration.rs — modify: register the next step opening `SYMBOL_CHURN`.
- crates/ariadne-cli/src/commands/index.rs — modify: feed git line-hunks + the symbol table + per-file line index into the graph use-case; persist.
- crates/ariadne-cli/src/config.rs — modify: add `[history] symbol_churn_depth` (default 500) bounding attribution to a recent commit window so HEAD-layout line drift stays small (step 6, R-C3).
- docs/adr/0019-symbol-churn-attribution.md — new (authored at build).
</files>

<steps>
1. Failing test first (`ariadne-graph` tests): a fixture file with two functions and a commit editing only the first; assert the use-case credits the first symbol and not the second. Red — `symbol_churn` does not exist.
2. `ariadne-git`: for each modified blob, run `gix` `blob-diff` and collect the new-side changed line ranges as `LineHunk`s per `(commit, path)` [src: https://lib.rs/crates/gix — `blob-diff` feature; https://docs.rs/gix/0.84.0/gix/struct.Repository.html].
3. `ariadne-graph::attribute_symbol_churn(symbol_lines, commit_hunks) -> Vec<SymbolChurn>`: convert each symbol's `defining_span` (bytes) to a line range using the file's HEAD line index, then count a commit for a symbol when any of that commit's changed lines on the file fall in the symbol's line range. Pure and deterministic [src: crates/ariadne-core/src/domain/records.rs:38-41 — `SymbolRecord::defining_span`].
4. Boundary (ADR-0019): the git adapter stays symbol-agnostic — it emits paths + line ranges only; the symbol join lives in `ariadne-graph` (a use-case crate that legitimately holds the symbol table, deps ⊆ {core, storage}). *Rejected:* attributing inside `ariadne-git` — it would force a parser/symbol dependency into a driven adapter, breaking adapter isolation [src: tests/architecture.rs adapter-isolation invariant; CLAUDE.md hexagonal boundary rule].
5. Persist `SYMBOL_CHURN` behind the next migration step (additive; in-place upgrade, no rebuild). The CLI composition root builds the per-file HEAD line index from contents it already read at parse time and supplies the symbol spans from the snapshot.
6. Document the approximation: line ranges are interpreted against the HEAD layout, so attribution is exact for the latest revision and degrades for commits predating later line shifts (CodeScene X-Ray's known limitation); a `[history] symbol_churn_depth` bounds attribution to a recent window so the signal stays meaningful (R-C3) [src: https://understandlegacycode.com/blog/key-points-of-software-design-x-rays/].
</steps>

<verification>
- `cargo nextest run -p ariadne-git -p ariadne-graph -p ariadne-storage` — line-hunk extraction, attribution golden, migration round-trip green; a re-run yields identical counts (determinism).
- Manual: `ariadne index` this repo; confirm a known churn-heavy function (e.g. an indexer-selection `match`) reports a symbol-churn count consistent with `git log -L`.
- `cargo test --test architecture` (`ariadne-git` has no parser/symbol dep; the join is in `ariadne-graph`), `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check` — green.
</verification>

<rollback>
`git checkout -- crates docs/adr/0019-symbol-churn-attribution.md`. Drop `SYMBOL_CHURN` and revert `SCHEMA_VERSION`; file-level churn (tier-11) is untouched.
</rollback>
</content>
