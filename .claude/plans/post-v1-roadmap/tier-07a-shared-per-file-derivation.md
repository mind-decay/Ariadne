---
tier_id: tier-07a
title: Shared per-file derivation — extract CLI logic into ariadne-salsa, real driver + commit_revision, cold-path refactor
deps: [tier-07]
exit_criteria:
  - The per-file derivation (symbol building, SFC component synthesis, global edge resolution, stable `symbol_id`, `enclosing_symbol`) lives in `ariadne-salsa`, not `ariadne-cli`.
  - A new `SyntacticFactsInput` salsa input carries parsed facts; composition roots parse via `ariadne-parser` and feed it — `ariadne-salsa` gains no parser/scip dependency.
  - `symbols_for_file` returns real symbols; a pure driver pass resolves global edges; `AriadneDb::commit_revision` writes a real `Changeset` (upserts) to redb and returns the `RevisionId`.
  - `ariadne-cli run_index` is refactored onto the shared driver — no second derivation path remains.
  - Cold-index output is byte-identical to the pre-refactor CLI output across the 7-language + framework fixtures (symbol/edge/file records and counts unchanged).
  - ADR-0016 records the derivation home, facts-as-input, and pure-pass edge resolution; `tests/architecture.rs` still passes (`ariadne-salsa` deps ⊆ {core, storage}).
  - `cargo nextest run --workspace` + architecture + clippy + fmt + the ariadne_v2 self-index dogfood all green.
status: completed
completed: 2026-05-29
---

<context>
The only real per-file derivation lives in the `ariadne-cli` driving adapter [src: crates/ariadne-cli/src/domain/mod.rs:495-768], so the `ariadne-daemon` adapter cannot reuse it (adapters never depend on each other [src: tests/architecture.rs:38-49]); meanwhile the salsa queries and `commit_revision` are empty stubs [src: crates/ariadne-salsa/src/derived.rs:116-182; crates/ariadne-salsa/src/db.rs:106-110]. This tier moves the pure derivation into `ariadne-salsa` (a use-case crate limited to core + storage [src: tests/architecture.rs:32,35]) behind a driver, refactors the CLI cold-index onto it, and proves byte-parity. tier-07b then makes it edit-stable + diff-aware for the tier-08 watcher. Full context: plan.md RD11.
</context>

<files>
- crates/ariadne-salsa/src/inputs.rs — modify: add `#[salsa::input] SyntacticFactsInput { facts: SyntacticFactsRaw }` so parsed facts enter salsa via the setter chain rather than being parsed inside salsa (which may not depend on `ariadne-parser` [src: crates/ariadne-salsa/src/inputs.rs:6-7]).
- crates/ariadne-salsa/src/derive.rs — new: the pure derivation moved from cli — `build_symbols` (decls→`SymbolFactsRaw` + SFC synthesis), `resolve_edges` (global call/render/hook resolution), `symbol_id` (offset scheme, unchanged this tier), `enclosing_symbol`, `sort_candidates`, `decl_kind_tag` [src: crates/ariadne-cli/src/domain/mod.rs:497-600,606-616,689-768,771-814].
- crates/ariadne-salsa/src/derived.rs — modify: `syntactic_facts` reads `SyntacticFactsInput`; `symbols_for_file` returns real symbols via `derive::build_symbols` [src: crates/ariadne-salsa/src/derived.rs:116-171].
- crates/ariadne-salsa/src/db.rs — modify: `seed_from_disk` creates inputs from a storage snapshot; `commit_revision` derives all files, runs the pure global `resolve_edges`, builds a `Changeset` (upserts), applies via `WriteTxn::apply` [src: crates/ariadne-salsa/src/db.rs:88-110; crates/ariadne-core/src/domain/changeset.rs:16-28].
- crates/ariadne-cli/src/domain/mod.rs — modify: `run_index` parses in parallel (unchanged), converts `SyntacticFacts`→`SyntacticFactsRaw`, seeds the salsa db, and calls `commit_revision`; delete `CommitState`/`run_committer`/`resolve_edges`/`symbol_id` (now in salsa) [src: crates/ariadne-cli/src/domain/mod.rs:205-291,470-768].
- docs/adr/0016-shared-per-file-derivation.md — new: per docs/adr/_template.md.
</files>

<steps>
1. Failing test first (`ariadne-salsa` tests): seed a 2-file Rust fixture (a caller + callee), `commit_revision`, then read the redb snapshot and assert the expected symbols + the cross-file `References` edge exist. Red — `symbols_for_file`/`commit_revision` are stubs [src: crates/ariadne-salsa/src/derived.rs:139-171; crates/ariadne-salsa/src/db.rs:106-110].
2. Add `SyntacticFactsInput`; route `syntactic_facts` to read it. Keep `FileContentInput` for the content hash (change detection in tier-07b/08) [src: crates/ariadne-salsa/src/inputs.rs:22-34].
3. Move the pure derivation into `derive.rs` verbatim in behavior (same `symbol_id`, same SFC synthesis, same edge policy) so parity holds [src: crates/ariadne-cli/src/domain/mod.rs:497-768]. `symbols_for_file` calls `derive::build_symbols`.
4. Implement the driver: `seed_from_disk` enumerates the storage snapshot into inputs; `commit_revision` collects `symbols_for_file` for every file (salsa-memoized), runs the pure `resolve_edges` over the union (global name map), assembles a `Changeset`, and `WriteTxn::apply`s it, returning the `RevisionId` [src: crates/ariadne-salsa/src/db.rs:106-110; crates/ariadne-graph/src/build.rs:214].
5. Refactor `run_index`: keep the rayon parallel parse + progress; per parsed file convert facts to `SyntacticFactsRaw` and seed; after the parse join, one `commit_revision`. Remove the now-duplicate `CommitState`/`run_committer`/`resolve_edges`/`symbol_id` from cli [src: crates/ariadne-cli/src/domain/mod.rs:470-768].
6. Cold byte-parity: snapshot the pre-refactor cold redb (symbols/edges/files records, sorted) for the fixtures; assert the post-refactor output is identical. Re-run the self-index dogfood and assert unchanged `(symbols, edges, files)` counts.
7. Write ADR-0016: derivation home = `ariadne-salsa`; facts enter as a salsa input (salsa stays parser-free); global edge resolution is a pure driver pass (per-file symbol derivation is the memoized step); cold + warm share it. Cite the architecture invariant + the rejected alternatives (parallel daemon derivation; new crate).
</steps>

<verification>
- `cargo nextest run -p ariadne-salsa` — the step-1 derivation test green; cache-hit tests (equivalence/durability) still green [src: crates/ariadne-salsa/tests/equivalence.rs; tests/durability.rs].
- `cargo nextest run --workspace` green; cold byte-parity test green; self-index dogfood counts unchanged.
- `cargo test --test architecture` (salsa deps ⊆ {core, storage}), `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo fmt --all --check`, `RUSTDOCFLAGS=-D warnings cargo doc --workspace --no-deps` — green.
- Memory probe: `memory_report()` after seeding the self-index shows no salsa table > 256MB (plan R1) [src: .claude/plans/ariadne-core/plan.md `<risks>`].
</verification>

<rollback>
`git checkout -- crates docs/adr/0016-shared-per-file-derivation.md`. The CLI cold-index reverts to its self-contained `run_committer` path; the daemon (tier-06/07) is untouched; salsa returns to stubs.
</rollback>
