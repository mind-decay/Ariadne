---
tier_id: tier-07b
title: Incremental per-file re-derivation — edit-stable SymbolId, stale-record removal, incremental==full-rebuild invariant
deps: [tier-07a]
exit_criteria:
  - `SymbolId` is edit-stable — a symbol's id is independent of byte offsets, so an edit elsewhere in a file leaves unchanged symbols' ids (and the edges to them) intact.
  - Incremental `commit_revision` emits stale removals — symbols/edges/files no longer derived are deleted via the `Changeset` delete vectors, consistent with a full rebuild.
  - A single-file re-derive driver API (set/clear one file's inputs → derive → commit a delta) exists for the watcher (tier-08) to call.
  - A proptest of random edit/create/delete sequences shows the storage after incremental commits is identical to a fresh full cold rebuild (divergence 0).
  - ADR-0017 records the stable-id scheme + its collision handling and the stale-removal semantics; cold goldens are re-baselined to the new id scheme.
  - `cargo nextest run --workspace` + architecture + clippy + fmt + the ariadne_v2 self-index dogfood all green.
status: completed
completed: 2026-05-30
---

<context>
tier-07a unified derivation in `ariadne-salsa` but kept the offset-based `symbol_id` (`blake3("{path}#{name}@{offset}")`) for cold byte-parity [src: crates/ariadne-cli/src/domain/mod.rs:788-792]. That scheme is unsafe for incremental updates: an edit above a symbol shifts its `def_byte_range.0`, changing its id, so a benign edit churns the symbol and severs every edge to it — the warm-graph delta would be maximal, not minimal. This tier makes the id edit-stable, makes incremental `commit_revision` remove stale records, then proves an incremental sequence equals a full rebuild. It is the last prerequisite: tier-08's watcher calls the single-file re-derive API and relies on the divergence-0 invariant. Full context: plan.md RD12.
</context>

<files>
- crates/ariadne-salsa/src/derive.rs — modify: replace the offset-based `symbol_id` with `blake3("{path}#{kind}#{name}#{nth}")`, where `nth` is the 0-based occurrence index among same-`(name,kind)` decls in that file in source order; the synthesized SFC component uses `nth=0`, kind `component` [src: crates/ariadne-cli/src/domain/mod.rs:527-554,788-792].
- crates/ariadne-salsa/src/db.rs — modify: `commit_revision` becomes diff-aware — diff the freshly derived symbol/edge/file set against the prior committed set and fill `Changeset` deletes (`symbol_deletes`, `edges_removed`, `file_deletes`) alongside upserts [src: crates/ariadne-core/src/domain/changeset.rs:20,24,28].
- crates/ariadne-salsa/src/db.rs — modify: add `rederive_file` and `forget_file` — set/clear one file's `FileContentInput`/`SyntacticFactsInput`/`ScipDocInput` via the salsa setter chain, then run the diff-aware `commit_revision` [src: crates/ariadne-salsa/tests/durability.rs:67-69; crates/ariadne-salsa/tests/equivalence.rs:119-121].
- crates/ariadne-salsa/tests/incremental.rs — new: proptest comparing an incremental edit/create/delete sequence to a fresh full build (extends the incremental-vs-fresh harness) [src: crates/ariadne-salsa/tests/equivalence.rs:119-146].
- crates/ariadne-cli + crates/ariadne-graph + crates/ariadne-mcp — modify: re-baseline insta/golden snapshots whose `SymbolId` literals change (mechanical; the id is opaque to consumers).
- docs/adr/0017-incremental-id-stability.md — new: per docs/adr/_template.md.
</files>

<steps>
1. Failing test first (`ariadne-salsa` tests): a 1-file fixture with two functions; record the second function's `SymbolId`; prepend a blank line via `set_content().to(...)` + re-derive; assert the id is unchanged and the edge to it survives. Red — the offset id changes [src: crates/ariadne-salsa/tests/durability.rs:67-69].
2. Replace `symbol_id`: drop `@offset`, add the kind + intra-file occurrence-index disambiguator; compute `nth` during the per-file symbol build with a `(name,kind)→count` map in source order [src: crates/ariadne-cli/src/domain/mod.rs:553-579].
3. Make `commit_revision` diff-aware: read the prior committed symbol/edge/file ids from a snapshot, diff against the derived set, emit deletes for the difference plus upserts for the current set [src: crates/ariadne-core/src/domain/changeset.rs:16-28; crates/ariadne-salsa/src/db.rs:106-110]. A removed file forgets all its symbols and the edges incident to them.
4. Add `rederive_file(path, content, facts, scip)` and `forget_file(path)`: mutate only that file's inputs so salsa recomputes only the affected `symbols_for_file`, then run the diff-aware `commit_revision` to apply the delta [src: crates/ariadne-salsa/tests/equivalence.rs:119-121].
5. Stale-removal correctness: deleting a file removes its symbols and any edge whose `src`/`dst` was defined there; edges from *other* files that referenced a now-deleted symbol also drop (re-resolution leaves them unresolved). Assert via the proptest.
6. Proptest (`incremental.rs`): random sequence of edits/creates/deletes over a small file set; apply incrementally via `rederive_file`/`forget_file`; build a fresh `AriadneDb` from the final set; assert both snapshots' sorted `(SYMBOLS, EDGES, FILES)` record sets are identical (divergence 0) — the invariant tier-08's watcher proptest depends on [src: crates/ariadne-salsa/tests/equivalence.rs:119-146].
7. Re-baseline cold goldens across cli/graph/mcp under the new id scheme; confirm only `SymbolId` literals changed, not symbol/edge counts or shapes. Write ADR-0017: the stable-id scheme, the same-name/overload collision policy (occurrence index; residual churn when a same-named sibling is inserted before — accepted, noted, see plan R-B5), and the stale-removal contract.
</steps>

<verification>
- `cargo nextest run -p ariadne-salsa` — the step-1 stability test + the `incremental.rs` divergence-0 proptest green.
- `cargo nextest run --workspace` green after golden re-baseline; run `ariadne index` twice on the self-index (full, then touch one file + incremental) and assert identical `(symbols, edges, files)` counts.
- `cargo test --test architecture`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo fmt --all --check`, `RUSTDOCFLAGS=-D warnings cargo doc --workspace --no-deps` — green.
- Memory probe: `memory_report()` after the proptest shows no salsa table > 256MB (plan R1) [src: .claude/plans/ariadne-core/plan.md `<risks>`].
</verification>

<rollback>
`git checkout -- crates docs/adr/0017-incremental-id-stability.md`. Reverts to tier-07a's offset-based id + upsert-only `commit_revision`; cold parity from tier-07a is preserved. The daemon (tier-06/07) is untouched.
</rollback>
</content>
