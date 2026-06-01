---
tier_id: tier-14
title: Diff-aware blast radius — impact of a working-tree diff, a commit, or a ref range
deps: [tier-11b]
exit_criteria:
  - A pure `ariadne-graph` `diff_blast` use case maps a `DiffSpec` changeset's `LineHunk`s + symbol spans to its changed-symbol seed set, runs v1 `blast_radius` per seed, and returns the deduped must/may union — insta-golden-pinned.
  - The returned must∪may impact set equals the union over seeds of each seed's v1 `blast_radius` (must∪may) — asserted directly in a test, both sides computed from the same `GraphIndex`.
  - "`ariadne-git` emits per-path `LineHunk`s + the changed-path list for all three `DiffSpec` kinds — `WorkingTree` (uncommitted, via gix `status`), `Commit`, `RefRange` — staying symbol-agnostic (deps ⊆ {core})."
  - A changed file with no resolved symbol (new / binary / deleted) is returned as an unresolved-impact entry, never silently dropped.
  - ADR-0022 records the diff-source boundary, the union semantics, and the gix `status` feature addition + its pure-Rust justification.
  - "`cargo nextest run -p ariadne-graph -p ariadne-git` + `cargo test --test architecture` + clippy + fmt all green."
status: completed
completed: 2026-06-02
---

<context>
v1 `blast_radius` answers "what depends on symbol X" [src: crates/ariadne-graph/src/blast.rs:64-96]. A reviewer's real question is "what does *this change* affect" — a working-tree diff, a commit, a PR range. This tier composes the gix diff reader with the v1 dominator-based `blast_radius`: `ariadne-git` emits a changeset's changed line-hunks + paths; a pure `ariadne-graph` use case joins them against symbol spans to get the changed-symbol seed set, then unions per-seed blast radius. MCP/daemon exposure is tier-15 (plan Block C). Full context: plan.md.
</context>

<decisions>
- **D1 — depend on tier-11b and reuse its line-hunk + span-attribution machinery; do not re-derive.** The "changed lines → changed symbols" join is exactly what `attribute_symbol_churn` already does: byte-span→line→overlap over `FileSymbolSpans` + `LineHunk`s [src: crates/ariadne-graph/src/symbol_churn.rs:57-106, 113-131]. The stub's `deps:[tier-11]` is under-specified — `LineHunk` [src: crates/ariadne-core/src/domain/records.rs — tier-11b], the `blob-diff` line-hunk emitter [src: crates/ariadne-git/src/adapters/gix/line_hunks.rs], and the shared-derivation symbol spans (tier-07a) all arrive at tier-11b. Retargeted to `deps:[tier-11b]`. The span↔line↔overlap primitives (`byte_span_to_lines`/`line_of`/`overlaps`) + a `changed_symbols(spans, hunks)->BTreeSet<SymbolId>` resolver are extracted into a shared `ariadne-graph` module reused by both `symbol_churn` and `diff_blast` (DRY; tier-11b's existing goldens guard the refactor). *Rejected:* reimplementing the overlap math in `diff_blast.rs` (duplicates the line-intersection logic two ways).
- **D2 — diff-source boundary: the git adapter emits paths + `LineHunk`s for all three `DiffSpec` kinds; the symbol join + blast union live in `ariadne-graph`.** All three kinds reduce to (old, new) blob pairs → existing `blob-diff` line-hunks: `WorkingTree` diffs the index/worktree against `HEAD`, `Commit` diffs a commit's tree vs its first-parent tree, `RefRange` diffs the two resolved trees [src: https://docs.rs/gix/0.84.0/gix/struct.Repository.html ; tier-11 `diff_tree_to_tree` + tier-11b `blob-diff`]. The adapter never sees symbol ranges — same boundary as ADR-0019 (recorded in ADR-0022). *Rejected:* attributing inside `ariadne-git` (forces a symbol/parser dep into a driven adapter, breaking adapter isolation [src: tests/architecture.rs adapter-isolation invariant; CLAUDE.md hexagonal boundary rule]).
- **D3 — `WorkingTree` support adds gix's `status` feature; it stays pure-Rust.** Uncommitted diff needs `Repository::status()` → `Platform` → `Iter`/`Item` (categories: index-vs-worktree, head-vs-index) [src: https://docs.rs/gix/0.84.0/gix/status/index.html]. The `status` feature pulls `dirwalk`/`gix-status`/`index`; none reference curl/reqwest/transport — network lives only in the opt-in `*-http-transport-*`/`async-network-client` features, so the critical path stays pure-Rust (plan D5) [src: https://docs.rs/crate/gix/0.84.0/features]. Pin becomes `features = ["blob-diff", "revision", "sha1", "status"]`. *Rejected:* shelling to `git diff` (breaks "no external runtime"); a hand-rolled worktree walk (re-implements `.gitignore`/index semantics).
- **D4 — report = per-seed attribution + must/may union + unresolved-impact list; pure and deterministic.** `DiffBlastReport { seeds: Vec<DiffSeed>, must_touch, may_touch, unresolved: Vec<String> }`; `DiffSeed { symbol, must_touch, may_touch, depth_used }` mirrors v1 `BlastRadius` [src: crates/ariadne-graph/src/blast.rs:24-32]. Output types live in `ariadne-graph` per the analytics-output convention [src: tier-13 D1]. The union dedups across seeds (a symbol that is `must` for any seed lands in `must_touch`, else `may_touch`); a seed already in the seed set is still listed as a seed. No clock, no RNG; every collection sorted by `SymbolId`/path so re-runs are byte-identical [src: crates/ariadne-graph/src/symbol_churn.rs:102-105; tier-13 D4]. `DiffSpec` (`WorkingTree | Commit(String) | RefRange{from,to}`; revspec strings resolved by the gix adapter, keeping `ariadne-core` gix-free) lives beside `LineHunk` in `ariadne-core` domain.
</decisions>

<files>
- crates/ariadne-core/src/domain/records.rs — modify: add `DiffSpec` (`WorkingTree`, `Commit(String)`, `RefRange { from: String, to: String }`) beside `LineHunk` [src: crates/ariadne-core/src/domain/records.rs — tier-11b `LineHunk`].
- crates/ariadne-core/src/lib.rs — modify: re-export `DiffSpec` from the façade.
- crates/ariadne-git/Cargo.toml — modify: gix `features = ["blob-diff", "revision", "sha1", "status"]` (D3).
- crates/ariadne-git/src/adapters/gix/diff.rs — new: `DiffSpec` → (`Vec<LineHunk>`, changed paths). WorkingTree via `status`; Commit/RefRange via `diff_tree_to_tree` + `line_hunks` reuse.
- crates/ariadne-git/src/adapters/gix/mod.rs — modify: declare `diff`; expose the `diff(spec)` method through the façade (no `gix` type leaks).
- crates/ariadne-git/src/errors.rs — modify: add `GitError::Revspec` for an unresolvable revspec / missing HEAD (D4, step 4).
- crates/ariadne-git/src/lib.rs — modify: re-export `diff` from the crate façade (step 7).
- crates/ariadne-git/src/adapters/gix/line_hunks.rs — modify: bump `collect_change_hunks`/`blob_bytes`/`push_new_side_hunks` to `pub(super)` so `diff.rs` reuses the tier-11b blob-diff emitter (step 5).
- crates/ariadne-graph/src/span_lines.rs — new: shared `byte_span_to_lines`/`line_of`/`overlaps` + `changed_symbols(spans, hunks) -> BTreeSet<SymbolId>` (extracted from `symbol_churn.rs`).
- crates/ariadne-graph/src/symbol_churn.rs — modify: consume the shared helpers (behaviour unchanged; tier-11b goldens guard it).
- crates/ariadne-graph/src/diff_blast.rs — new: `GraphIndex::diff_blast(...) -> DiffBlastReport`; `DiffBlastReport`/`DiffSeed`.
- crates/ariadne-graph/src/lib.rs — modify: declare `mod span_lines; mod diff_blast;` and re-export the public types/method [src: crates/ariadne-graph/src/lib.rs:11-37].
- crates/ariadne-graph/tests/diff_blast.rs — new: union-equality + unresolved + determinism asserts; insta snapshot.
- crates/ariadne-git/tests/diff.rs — new: fixture-repo helper exercising all three `DiffSpec` kinds (incl. an uncommitted worktree edit).
- docs/adr/0022-diff-aware-blast-radius.md — new (authored at build; 0022 is the next free id).
</files>

<steps>
1. Failing test first (`crates/ariadne-graph/tests/diff_blast.rs`): build a `GraphIndex` + `FileSymbolSpans` + synthetic `LineHunk`s for a known two-file changeset; assert `diff_blast`'s must∪may impact equals the union of `blast_radius` over the changed seeds, and a changed path with no covering symbol appears in `unresolved`. Red — `diff_blast` does not exist [src: crates/ariadne-graph/tests/symbol_churn.rs for the direct-input + determinism-rerun pattern].
2. Extract `span_lines.rs`: move `byte_span_to_lines`/`line_of`/`overlaps` out of `symbol_churn.rs` and add `changed_symbols(spans, hunks) -> BTreeSet<SymbolId>` (per-file span→line range build, then any-hunk-overlap). Refactor `attribute_symbol_churn` onto it; its goldens must stay green (D1) [src: crates/ariadne-graph/src/symbol_churn.rs:61-100].
3. Implement `diff_blast.rs` as a `GraphIndex` method: `seeds = changed_symbols(spans, hunks)`; for each seed `self.blast_radius(seed, depth, kinds)` [src: crates/ariadne-graph/src/blast.rs:64-96]; collect `DiffSeed`s; fold into the deduped must/may union (must wins on conflict); `unresolved` = `changed_paths` with no resolved seed. Sort every output vector (D4).
4. Add `DiffSpec` to `ariadne-core` and re-export it (D4). Define `GitError` variants for an unresolvable revspec / missing HEAD.
5. Implement `gix/diff.rs`: `WorkingTree` → `repo.status()` to enumerate index-vs-worktree + head-vs-index changed paths, then `blob-diff(HEAD blob, worktree/index content)` per modified path for new-side `LineHunk`s [src: https://docs.rs/gix/0.84.0/gix/status/index.html]; `Commit`/`RefRange` → resolve revspecs, `diff_tree_to_tree`, reuse `line_hunks` per changed blob [src: https://docs.rs/gix/0.84.0/gix/struct.Repository.html]. Return (`Vec<LineHunk>`, changed paths). New/binary/deleted files contribute a path but no new-side hunk (→ `unresolved` downstream, step 3).
6. `crates/ariadne-git/tests/diff.rs`: a `#[test]` helper builds a fixture repo (commit sequence, then an uncommitted worktree edit), asserting the changed paths + line-hunks for `WorkingTree`, `Commit`, and `RefRange` (fixture-repo pattern from tier-11) [src: tier-11 step 1].
7. `lib.rs` re-exports; insta-snapshot the full `DiffBlastReport` on the fixture (review by hand, no blind `--accept`) + a re-run equality assert (determinism). Write ADR-0022 (D2/D3/D4, rejected alternatives, status `Accepted`); update plan.md `<tech_inventory>` gix row to append `status` + tier 14. No new crate/dep edge → `tests/architecture.rs` unchanged.
</steps>

<verification>
- `cargo nextest run -p ariadne-graph -p ariadne-git` — diff-blast union-equality + unresolved + determinism, and the three-kind fixture-repo diff (incl. uncommitted worktree), all green; tier-11b symbol-churn goldens still green after the `span_lines` extraction.
- End-to-end (real, not stub): the git test runs the actual gix `status`/`diff_tree_to_tree` over a real on-disk fixture repo; the graph golden runs the actual `diff_blast` and asserts its impact set equals the independently computed per-seed `blast_radius` union. Live self-index run on a real ariadne_v2 branch is deferred to tier-15, where the MCP `diff_blast_radius` tool makes it invokable (tier-13 deferral precedent).
- `cargo test --test architecture` (`ariadne-git` deps ⊆ {core}; the symbol join stays in `ariadne-graph`), `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo fmt --all --check`, `RUSTDOCFLAGS=-D warnings cargo doc -p ariadne-graph -p ariadne-git --no-deps` — green.
</verification>

<rollback>
`git checkout -- crates docs/adr/0022-diff-aware-blast-radius.md .claude/plans/post-v1-roadmap/plan.md` and `rm -f crates/ariadne-graph/src/diff_blast.rs crates/ariadne-graph/src/span_lines.rs crates/ariadne-git/src/adapters/gix/diff.rs docs/adr/0022-diff-aware-blast-radius.md` plus the new snapshots. The gix `status` feature + `DiffSpec` are additive; v1 `blast_radius` and tier-11/11b are untouched (the `span_lines` extraction is behaviour-preserving, guarded by tier-11b goldens).
</rollback>
