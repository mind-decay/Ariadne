---
tier_id: tier-01
title: A1 — test-impact reachability (affected_tests)
deps: []
exit_criteria:
  - "`cargo nextest run --workspace` green; new failing-first tests now pass"
  - "graph unit test: `classify_test_symbols` marks a known test symbol in each of the 15 fixture languages"
  - "e2e golden: `affected_tests` on a seeded fixture change returns exactly the hand-verified test-symbol set, deterministically (two runs byte-identical)"
  - "`ariadne query affected_tests '{...}'` (warm + cold) and `ariadne affected-tests <spec>` print that set"
  - "`cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo fmt --all --check`, `cargo test --test architecture` all green"
status: completed
completed: 2026-06-07
---

<context>
Adds A1 from the plan: which tests a change transitively reaches, via reverse call/ref-graph reachability — the standard call-graph test-impact technique [src: https://martinfowler.com/articles/rise-test-impact-analysis.html]. Static only, no coverage (plan D1). Test classification is a pure graph function over `Decl.attributes` + path (plan D2). Reuses the `diff_blast` changeset→seed→reverse-reach machinery; the answer is the test subset of the reached set [src: crates/ariadne-graph/src/diff_blast.rs]. Full context: ./plan.md.
</context>

<files>
- `crates/ariadne-graph/src/test_impact.rs` (new) — `TestRootInput { id, lang, path, kind, name, attributes }`; pure `classify_test_symbols<I>(I) -> BTreeSet<SymbolId>`; `GraphIndex::affected_tests(symbol_lines, hunks, changed_paths, test_roots, depth, kinds) -> AffectedTestsReport { tests, seeds, unresolved }` (all `Vec<SymbolId>`/`Vec<String>` sorted).
- `crates/ariadne-graph/src/lib.rs` (modify) — `mod test_impact;` + `pub use test_impact::{AffectedTestsReport, TestRootInput, classify_test_symbols};`.
- `crates/ariadne-mcp/src/{tools/affected_tests.rs (new), tools/mod.rs, types.rs, server.rs}` — warm+cold `handle`, wire `AffectedTestsInput`/`AffectedTestsOutput`, `#[tool]` method.
- `crates/ariadne-core` daemon protocol + `crates/ariadne-daemon` dispatch (modify) — `DaemonQuery`/`DaemonResponse` variant + warm dispatch arm; precompute `test_roots: BTreeSet<SymbolId>` on catalog build/`apply_changeset` (warm projection).
- `crates/ariadne-cli/src/commands/{affected_tests.rs (new), mod.rs}` + `main.rs` (modify) — thin `affected-tests <spec>` subcommand delegating to the `affected_tests` query route.
- Tests: inline `#[cfg(test)]` in `test_impact.rs` (synthetic graph + classifier table); e2e fixtures golden in `crates/ariadne-e2e` (mirror `crates/ariadne-cli/tests/incremental_history.rs` real-pipeline precedent).
</files>

<steps>
1. Write the failing graph unit tests first (TDD): a synthetic `GraphIndex` where test symbol T calls changed symbol S → `affected_tests` returns {T}; and `classify_test_symbols` over one `TestRootInput` per language asserts the expected classification [src: CLAUDE.md TDD rule].
2. Implement `classify_test_symbols`: a per-`Lang` table — attribute markers (Rust `test`; Java/Kotlin/C# `Test`/`Fact`; TS/JS decorator names) ∪ path conventions (`*_test.go`+`Test*`/`Benchmark*`/`Fuzz*` names; `*.test.*`/`*.spec.*`; `test_*.py`/`*_test.py`/`test*` names; `*Test.java`/`src/test/`). Cite each language's convention inline. `attributes`/`lang`/path are already on every record [src: crates/ariadne-parser/src/adapters/treesitter/facts.rs:99-106; crates/ariadne-daemon/src/domain/catalog.rs:48-53].
3. Implement `affected_tests`: resolve seeds from `hunks` via `changed_symbols` (as `diff_blast` does), reverse-reach each seed with `blast_radius` over the CALLS|REFS `kinds`, take `tests = test_roots ∩ (seeds ∪ reached)`, sort all outputs; carry `unresolved` paths through [src: crates/ariadne-graph/src/diff_blast.rs:61-102; crates/ariadne-graph/src/lib.rs:47].
4. Surface as an MCP tool: grep the existing `DiffBlast`/`diff_blast` tool name and replicate every site it touches (tool `handle` warm+cold, `types.rs`, `server.rs` `#[tool]`, the `DaemonQuery`/`DaemonResponse` variant, the cold dispatch arm) for `affected_tests`; the input mirrors `diff_blast` (`spec`, `depth`, `kinds`); the git diff runs in the handler before the use-case [src: crates/ariadne-mcp/src/tools/diff_blast.rs:60-103].
5. Add the warm `test_roots` projection: classify once at `WarmCatalog`/`Catalog` build over `cat.symbols` (`SymbolMeta` carries attributes+lang) and re-derive affected entries on `apply_changeset`, so warm queries stay <10ms [src: crates/ariadne-daemon/src/domain/catalog.rs:37-59; arc plan.md SLO].
6. Add the thin `ariadne affected-tests <spec>` subcommand wrapping the `affected_tests` query route (warm→cold) [src: crates/ariadne-cli/src/commands/query.rs:36-70].
7. Add the e2e fixtures golden: index a fixture tree, seed a change to a non-test symbol, assert the returned test set equals the hand-computed set and is byte-identical across two runs.
</steps>

<verification>
- `cargo nextest run --workspace` — all green, including the new tests (red before step 2/3).
- Unit: classifier marks a known test symbol per language; `affected_tests` on the synthetic graph returns the expected set and excludes non-test ancestors.
- E2e: `affected_tests` on the seeded fixture change equals the hand-verified set; re-run is byte-identical (determinism).
- `ariadne query affected_tests '{"spec":"working_tree"}'` and `ariadne affected-tests working_tree` print the same set on warm and cold paths.
- `cargo clippy ... -D warnings`, `cargo fmt --all --check`, `cargo test --test architecture` green (no adapter→adapter dep introduced).
Fail loudly: a wrong/missing test in the set, a non-deterministic re-run, or any clippy/fmt/arch failure is a hard fail — root-cause, never weaken the assert [src: CLAUDE.md `<rules>`].
</verification>

<rollback>
Single-commit tier. Revert the commit (or `git restore` the listed files): the new `mod test_impact;` line, the new tool/command files, and the daemon-protocol variant are additive — removing them returns the build to the tier-00 baseline with no migration to undo.
</rollback>
