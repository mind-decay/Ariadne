---
tier_id: tier-13
title: Diff-aware blast radius — impact of a working-tree diff or commit range
deps: [tier-10]
exit_criteria:
  - A use case maps a git diff (working tree, a commit, or a ref range) to its changed symbols.
  - Blast radius over the changed-symbol set equals the union of per-symbol v1 `blast_radius`.
  - Uncommitted working-tree changes are supported, not only committed ranges.
  - `cargo nextest run -p ariadne-graph -p ariadne-git` + architecture + clippy + fmt all green.
status: pending
---

<context>
v1 `blast_radius` answers "what depends on symbol X". A reviewer's real question is "what does *this change* affect" — a diff, a branch, a PR. This tier composes the `gix` diff reader (tier-10) with the v1 dominator-based `blast_radius` to answer impact for a changeset (plan RD plan `<context>` Block C). Full context: plan.md.
</context>

<files>
- crates/ariadne-git/src/adapters/gix.rs — modify: expose changed paths + byte ranges for a working-tree diff and for a ref range.
- crates/ariadne-graph/src/diff_blast.rs — new: changed files → changed symbols → unioned blast radius.
- crates/ariadne-core/src/domain/ — modify: `DiffSpec` (working-tree | commit | ref-range) input type.
- crates/ariadne-graph/tests/ — new: diff-aware blast-radius goldens.
- crates/ariadne-graph/fixtures/ — modify/ensure a fixture repo with a known diff.
</files>

<steps>
1. Failing test first (`ariadne-graph` tests): over a fixture repo with a known two-file diff, assert diff-aware blast radius equals the union of `blast_radius` for every symbol intersecting the diff. Red — `diff_blast.rs` does not exist.
2. Extend `ariadne-git` `gix` adapter: given a `DiffSpec`, return changed file paths with changed byte ranges — for the working tree, diff the index/worktree against `HEAD`; for a ref range, diff the two trees [src: https://github.com/GitoxideLabs/gitoxide].
3. Define `DiffSpec` in `ariadne-core` (`WorkingTree`, `Commit(oid)`, `RefRange(from,to)`).
4. Implement `diff_blast.rs`: intersect changed byte ranges with symbol spans to get the changed-symbol set; run the v1 `blast_radius` on each; return the deduplicated union with per-seed attribution [src: .claude/plans/ariadne-core/tier-07-graph-analytics.md].
5. Handle a changed file with no resolved symbols (new/binary/deleted file) — report it as an unresolved-impact entry rather than dropping it silently.
6. Goldens for all three `DiffSpec` kinds.
</steps>

<verification>
- `cargo nextest run -p ariadne-graph -p ariadne-git` — diff-blast goldens for working-tree / commit / ref-range green.
- Manual: on a real feature branch of ariadne_v2, run diff-aware blast radius; confirm it equals the union of per-changed-file `blast_radius`.
- `cargo test --test architecture`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check` — green.
</verification>

<rollback>
`git checkout -- crates/ariadne-graph crates/ariadne-git crates/ariadne-core`. v1 `blast_radius` is untouched.
</rollback>
