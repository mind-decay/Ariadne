---
tier_id: tier-11
title: Cyclomatic complexity — McCabe metric per function from the tree-sitter CST
deps: [tier-02]
exit_criteria:
  - The parser computes McCabe complexity (`decision-points + 1`) for every function-like symbol.
  - Complexity is stored as symbol metadata behind a tier-02 migration step.
  - Per-language decision-node sets are golden-tested on the existing parser fixtures.
  - `cargo nextest run -p ariadne-parser -p ariadne-storage` + architecture + clippy + fmt all green.
status: pending
---

<context>
v1 holds no per-function complexity signal — `weak_spots` reports god modules but not which functions are dense. This tier computes McCabe cyclomatic complexity from the tree-sitter CSTs Ariadne already builds, with no new dependency (plan RD8). It feeds the hotspot metric in tier-12. Full context: plan.md.
</context>

<files>
- crates/ariadne-parser/src/domain/complexity.rs — new: decision-node counter + McCabe formula.
- crates/ariadne-parser/src/ — modify: the syntactic-fact extraction attaches complexity to each function-like symbol.
- crates/ariadne-parser/src/adapters/ — modify: per-`Lang` decision-node kind sets.
- crates/ariadne-core/src/domain/ — modify: add a `complexity: u32` field to the symbol record.
- crates/ariadne-storage/src/ — modify: persist `complexity`; register a tier-02 migration step.
- crates/ariadne-parser/tests/ — modify: per-language complexity goldens.
</files>

<steps>
1. Failing test first (`ariadne-parser` tests): over a fixture function with a known branch count, assert the computed complexity equals `decision-points + 1`. Red — no complexity code exists.
2. Implement `complexity.rs`: walk a function symbol's CST subtree, count decision nodes, return `count + 1` — McCabe's `M = D + 1` for single-entry/single-exit programs [src: https://en.wikipedia.org/wiki/Cyclomatic_complexity ; McCabe, "A Complexity Measure", IEEE TSE, 1976].
3. Define the per-`Lang` decision-node set against each grammar's node kinds: `if`/`else if`, `for`/`while`/`loop`, `match`/`switch` arms (one per arm), `&&`/`||` short-circuits, `?`/ternary, `catch`/`except`. Each entry cites the grammar node kind it matches; kinds with no clean mapping are recorded in the audit, not guessed.
4. Attach the value during syntactic-fact extraction so it costs one extra subtree walk per function — no second parse.
5. Add `complexity` to the `ariadne-core` symbol record; persist it via `ariadne-storage` behind a tier-02 migration step.
6. Per-language complexity goldens over the existing parser fixtures (Rust/Go/Python/TS/JS/Java/C#/C/C++).
</steps>

<verification>
- `cargo nextest run -p ariadne-parser -p ariadne-storage` — complexity goldens + migration green.
- Manual: index the self-index; confirm a known-branchy function (e.g. an indexer-selection match) reports a complexity matching a hand count.
- `cargo test --test architecture`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check` — green.
</verification>

<rollback>
`git checkout -- crates/ariadne-parser crates/ariadne-core crates/ariadne-storage`. The migration step is additive (drop the column/value).
</rollback>
