---
tier_id: tier-04
title: A3 — architecture fitness engine (ariadne-fitness.toml → fitness check)
deps: []
exit_criteria:
  - "`cargo nextest run --workspace` green; new failing-first tests now pass"
  - "graph unit test: a synthetic graph with a forbidden layer edge yields exactly one violation; a clean graph yields none"
  - "e2e: `ariadne fitness check` against a committed ariadne-fitness.toml encoding this repo's hexagonal layers exits 0 on the clean self-index"
  - "e2e: a seeded forbidden dependency makes `ariadne fitness check` exit non-zero and list the violation; re-run byte-identical"
  - "MCP `fitness_report` returns the same violations for the same rules; `cargo test --test architecture`, `cargo clippy ... -D warnings`, `cargo fmt --all --check` green; ADR-0028 committed"
status: completed
completed: 2026-06-07
---

<context>
Adds A3: a declarative `ariadne-fitness.toml` (layers as path globs, forbidden dependency directions, cycle/coupling thresholds) checked against the graph — productising the project's own `tests/architecture.rs` as config-driven fitness functions [src: ArchUnit `layeredArchitecture` https://www.baeldung.com/java-archunit-intro; tests/architecture.rs]. The engine is a pure `ariadne-graph` function reusing existing coupling/cycle analytics; rules parse + glob resolution happen at the composition root (plan D5). Full context: ./plan.md.
</context>

<files>
- `crates/ariadne-graph/src/fitness.rs` (new) + `lib.rs` re-export — `struct FitnessRules { layer_of: BTreeMap<FileId, String>, forbidden: Vec<(String, String)>, max_cycles: u32, max_instability: Option<f32> }`, `enum Violation`, `struct FitnessReport { violations, ok }`, `GraphIndex::fitness_check(&self, &FitnessRules) -> FitnessReport`.
- `crates/ariadne-cli/Cargo.toml` (modify) — add `glob = "0.3.3"` (toml 0.9 already present).
- `crates/ariadne-cli/src/{commands/fitness.rs (new), config or new fitness_rules.rs, commands/mod.rs}` + `main.rs` — parse `ariadne-fitness.toml`, resolve globs → `layer_of`, run the engine, print violations, exit non-zero on any.
- `crates/ariadne-mcp/Cargo.toml` (modify) — add `toml = "0.9"` (glob already present).
- `crates/ariadne-mcp/src/{tools/fitness_report.rs (new), tools/mod.rs, types.rs, server.rs}` — read+resolve rules, run the engine in-process (cold), return violations.
- `docs/adr/0028-fitness-rules-format.md` (new) — the `ariadne-fitness.toml` schema as a public contract.
- `ariadne-fitness.toml` (new, repo root) — this repo's hexagonal layers, for dogfooding + the e2e clean-pass test.
- Tests: inline `#[cfg(test)]` in `fitness.rs`; e2e in `crates/ariadne-cli/tests` (clean-pass + seeded-violation exit codes).
</files>

<steps>
1. Write failing tests first (TDD): a graph unit test builds a synthetic `GraphIndex` with a `core`-layer file depending on an `adapter`-layer file and a rule forbidding `core → adapter`, asserting exactly one `Violation`; an e2e test asserts `fitness check` exits non-zero on a seeded forbidden edge [src: CLAUDE.md TDD rule].
2. Implement `fitness_check` (pure): (a) dependency-direction — for each inter-file edge `(src_file → dst_file)`, look up `layer_of`; a `(src_layer, dst_layer)` in `forbidden` is a `Violation`; (b) cycles — reuse existing cycle detection, `> max_cycles` cycles → a `Violation` each; (c) coupling — reuse `coupling_report`, a module instability above `max_instability` → a `Violation`. Sort violations; `ok = violations.is_empty()` [src: crates/ariadne-graph/src/lib.rs:33-35 (`CouplingReport`, `CycleReport`)].
3. Define the `ariadne-fitness.toml` schema + ADR-0028: `[[layer]]` `name` + `paths` (globs); `[[rule]]` `forbid = { from, to }`; `[thresholds]` `max_cycles`, optional `max_instability`. Cite the ArchUnit layered-architecture model [src: https://www.baeldung.com/java-archunit-intro].
4. CLI `fitness check`: deserialize the TOML (toml 0.9), resolve each layer's globs (glob 0.3.3) against the indexed file paths → `layer_of`, build `FitnessRules`, run `fitness_check` over the cold catalog/storage graph, print violations as JSON, and `std::process::exit(1)` when any violation exists (CI gate) else exit 0 [src: crates/ariadne-cli/src/commands/query.rs:36-70].
5. MCP `fitness_report`: read+resolve `ariadne-fitness.toml` from the project root (toml + glob, both now in mcp), run the engine in-process over the catalog, return `{ violations, ok }`. Wire `types.rs`/`server.rs`/`tools/mod.rs`. The warm `DaemonQuery::FitnessReport` leg is deferred (a future tier; cold suffices for the CI gate and agent queries) — note it explicitly, no silent omission.
6. Author `ariadne-fitness.toml` encoding this repo's real layers (core / use-case / driven adapters / driving adapters / composition root), mirroring `tests/architecture.rs`; the e2e clean-pass test asserts `fitness check` exits 0 on the self-index.
</steps>

<verification>
- `cargo nextest run --workspace` green (graph unit + e2e; red before step 2).
- Unit: forbidden edge → one violation; clean graph → none; thresholds trip correctly.
- E2e: `ariadne fitness check` exits 0 on the clean self-index; a seeded forbidden dependency exits non-zero and lists the violation; re-run byte-identical (determinism).
- Real run: `ariadne fitness check` on this repo exits 0; MCP `fitness_report` returns `ok = true` for the same rules (parity).
- `cargo test --test architecture` green (no daemon git edge; `mcp`/`cli` gained only external crates in the tech inventory); `cargo clippy ... -D warnings`, `cargo fmt --all --check` green.
Fail loudly: a missed/spurious violation, a wrong exit code, or non-determinism is a hard fail — root-cause, never weaken the assert [src: CLAUDE.md `<rules>`].
</verification>

<rollback>
Revert the commit. Non-additive changes are the two Cargo edges (`cli += glob`, `mcp += toml`, both external crates) and the new `ariadne-fitness.toml`; removing them returns the build to the tier-02/03 baseline. The engine, command, tool, and ADR files are additive.
</rollback>
