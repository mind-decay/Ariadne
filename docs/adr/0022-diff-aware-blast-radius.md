# ADR-0022: Diff-Aware Blast Radius

<status>
Accepted
Date: 2026-06-02
Decider: claude
</status>

<context>
v1 `blast_radius` answers "what depends on symbol X" [src:
crates/ariadne-graph/src/blast.rs:64-96]. A reviewer's real question is "what
does *this change* affect" — an uncommitted working-tree diff, a commit, or a PR
range. Answering it joins two facts: a changeset's changed line ranges (history)
and the symbol defining spans (the indexed graph), then unions per-changed-symbol
blast radius. The hexagonal invariant forbids the driven `ariadne-git` adapter
from depending on the parser or symbol table — it depends only on `ariadne-core`
[src: tests/architecture.rs; CLAUDE.md hexagonal boundary rule]. So neither the
symbol join nor the blast union can live in the git adapter. This is the same
boundary ADR-0019 drew for symbol-churn attribution.
</context>

<decision>
Split the work across the boundary, three parts.

1. **Diff source — `ariadne-git`.** A new `DiffSpec` (`WorkingTree | Commit(rev) |
   RefRange { from, to }`, pure, in `ariadne-core`) selects the changeset. The
   adapter's `diff(repo_root, spec)` resolves it to `(Vec<LineHunk>, changed
   paths)`, staying symbol-agnostic. All three kinds reduce to (old, new) blob
   pairs → the tier-11b `blob-diff` line-hunk emitter: `WorkingTree` enumerates
   index-vs-worktree + head-vs-index paths via `Repository::status` and diffs each
   path's `HEAD` blob against its current worktree bytes; `Commit` diffs a commit
   tree against its first-parent tree; `RefRange` diffs the two resolved trees.
   The revspec strings are resolved inside the adapter, so `ariadne-core` stays
   `gix`-free [src: docs.rs/gix/0.84.0/gix/status/index.html;
   docs.rs/gix/0.84.0/gix/struct.Repository.html].

2. **Symbol join + blast union — `ariadne-graph`.** A pure `GraphIndex::diff_blast`
   resolves the line hunks to the changed-symbol seed set (the shared
   `span_lines` resolver, reused with `symbol_churn` — DRY, D1), runs v1
   `blast_radius` per seed, and folds the results into a deduped `DiffBlastReport
   { seeds, must_touch, may_touch, unresolved }`. A symbol that is `must` for any
   seed lands in `must_touch`, every other reached symbol in `may_touch` (must
   wins on conflict); a changed path owning no seed symbol is an `unresolved`
   entry.

3. **`gix` `status` feature.** `WorkingTree` adds `status` to the pin (now
   `["blob-diff", "revision", "sha1", "status"]`); see rationale.
</decision>

<rationale>
- **Maintainability (hexagonal):** the cross-cutting join lives in the use-case
  layer (`ariadne-graph`, deps ⊆ {core}); the driven git adapter keeps deps ⊆
  {core} and emits paths + line ranges only. Attributing inside `ariadne-git`
  would force a parser/symbol dependency into a driven adapter, breaking adapter
  isolation [src: tests/architecture.rs].
- **Reliability (determinism):** `diff_blast` and the adapter's `diff` are pure
  functions of their inputs — no clock, no RNG. Every output collection is
  sorted (`seeds`/unions by `SymbolId`, paths lexicographically), so re-runs are
  byte-identical. The union equals the union over seeds of v1 `blast_radius`
  (must∪may), asserted directly from one `GraphIndex`.
- **Reliability (no silent drops):** new / binary / deleted files contribute a
  changed path but no new-side hunk, so they resolve to no seed and surface in
  `unresolved` rather than being dropped.
- **Efficiency:** the seed resolver only line-indexes files that actually
  changed; untouched files are skipped. v1 `blast_radius` is reused unchanged.
- **`status` stays pure-Rust:** the feature pulls `dirwalk`/`gix-status`/`index`;
  none reference curl/reqwest/transport — network lives only in the opt-in
  `*-http-transport-*`/`async-network-client` features — so the critical path
  stays pure-Rust (plan D5) [src: docs.rs/crate/gix/0.84.0/features].
</rationale>

<alternatives>
- **Attribute inside `ariadne-git`** — rejected: pulls the symbol table / parser
  into a driven adapter, an architecture hard-fail [src: tests/architecture.rs].
- **Shell out to `git diff`** — rejected: breaks "no external runtime" (plan D5),
  and parsing diff text is fragile.
- **Hand-rolled worktree walk for `WorkingTree`** — rejected: re-implements
  `.gitignore`/index semantics that `Repository::status` already encodes.
- **Reimplement the line-overlap math in `diff_blast`** — rejected: duplicates
  the tier-11b line-intersection logic; the shared `span_lines` resolver is the
  DRY home (D1), guarded by tier-11b's existing goldens.
</alternatives>

<consequences>
- `ariadne-core` gains `DiffSpec` (a query input, never persisted); `ariadne-git`
  gains `GitError::Revspec` for an unresolvable revspec / missing HEAD.
- The gix pin gains `status`; `tests/architecture.rs` stays green (no new
  crate/dep edge — the join lives in `ariadne-graph`, the diff in `ariadne-git`,
  both deps ⊆ {core}).
- `symbol_churn`'s span↔line↔overlap primitives move into a shared `span_lines`
  module; behaviour is unchanged (tier-11b goldens guard the refactor).
- MCP/daemon exposure of `diff_blast` is deferred to tier-15 (plan Block C); a
  live self-index run on a real ariadne_v2 branch lands there (tier-13 deferral
  precedent).
</consequences>

<sources>
- `[src: .claude/plans/post-v1-roadmap/plan.md RD7]`
- `[src: .claude/plans/post-v1-roadmap/tier-14-diff-aware-blast-radius.md]`
- `[src: tests/architecture.rs — adapter-isolation invariant]`
- `[src: docs/adr/0018-git-history-adapter.md ; docs/adr/0019-symbol-churn-attribution.md]`
- `[src: https://docs.rs/gix/0.84.0/gix/status/index.html]`
- `[src: https://docs.rs/gix/0.84.0/gix/struct.Repository.html]`
- `[src: https://docs.rs/crate/gix/0.84.0/features]`
- `[src: crates/ariadne-graph/src/blast.rs:64-96]`
</sources>
