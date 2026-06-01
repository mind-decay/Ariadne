---
tier_id: tier-12
title: Cyclomatic complexity — McCabe metric per function-like symbol from the tree-sitter CST
deps: [tier-02, tier-04]
exit_criteria:
  - "`SymbolRecord` gains a `complexity: u32` field; function-like symbols carry McCabe `decisions + 1` (>=1), non-function symbols carry 0."
  - The parser computes complexity in one CST walk; `&&`/`||` count as decisions (strict McCabe); a decision inside a nested captured decl is attributed to that inner decl, not the enclosing one.
  - redb `SCHEMA_VERSION` is `7`; a registered `MigrationStep` re-encodes `SYMBOLS` v6->v7 in place (complexity defaulted 0), no rebuild; a committed v6 file migrates with its first six fields byte-identical.
  - Per-language complexity goldens (Rust/Go/Python/TS/JS/Java/C#/Kotlin/C/C++) assert hand-counted values, including a branchy, a nested-function, and a boolean-operator case.
  - ADR-0020 records the metric, the strict-McCabe choice, the decl-span boundary rule, and the arrow-as-variable limitation.
  - "`cargo nextest run -p ariadne-parser -p ariadne-storage -p ariadne-core -p ariadne-salsa -p ariadne-cli` + architecture + clippy + fmt all green."
status: pending
---

<context>
v1 holds no per-function complexity signal — `weak_spots` reports god modules but not which functions are dense. This tier computes McCabe cyclomatic complexity from the tree-sitter CSTs Ariadne already builds (plan RD8, no new dependency) and stores it as a `SymbolRecord` field, threaded core->storage->parser->cli->salsa exactly as tier-04 threaded `visibility`/`attributes`. tier-13 consumes it at file grain (aggregate of a file's symbols) and symbol grain (per-symbol). Full context: plan.md.
</context>

<decisions>
- **D1 — `complexity: u32`, 0 = N/A (not `Option`).** Function-like symbols carry McCabe `>=1`; non-function symbols (struct/module/variable) carry 0. Matches tier-04's all-`u32`/all-defaulted metadata + the postcard prefix-extension migration; tier-13 already treats a zero factor as non-hotspot [src: post-v1-roadmap tier-13 step 2]. *Rejected:* `Option<u32>` — Option handling in every consumer + migration default + a one-way door (later change = another migration).
- **D2 — strict McCabe: `M = decisions + 1`, counting `&&`/`||`.** The standard definition equals the number of decision points + 1; a compound predicate `cond1 AND cond2` counts two decisions because it decomposes to sequential branches at machine level [src: https://en.wikipedia.org/wiki/Cyclomatic_complexity ; McCabe, "A Complexity Measure", IEEE TSE 1976]. Consistent with plan RD8 (`if`/`for`/`while`/`case`/`&&`/`||`/`?`). *Rejected:* control-flow-only (CodeScene-style) — simpler walker but diverges from RD8 and the cited definition.
- **D3 — nested-function boundary is grammar-agnostic via decl spans.** A captured decision is attributed to the innermost `Decl` whose `def_byte_range` contains it (reusing the `innermost_containing_decl` logic already in `facts.rs`), so a nested captured `fn`/method owns its own decisions and the parent does not double-count. No per-grammar "function-boundary" node set is needed — only the decision predicate is per-grammar [src: crates/ariadne-parser/src/adapters/treesitter/facts.rs:549-562].
- **D4 — function-like = `Function | Method | Component`.** Only these get `decisions + 1`; all other `DeclKind`s get 0. A non-component arrow assigned to a `Variable` (`const f = () => {…}`) is *not* attributed and reads 0 — a known limitation recorded in ADR-0020 + the tier audit, not silently dropped [src: crates/ariadne-parser/src/adapters/treesitter/facts.rs:28-63].
</decisions>

<files>
- crates/ariadne-parser/src/adapters/treesitter/complexity.rs — new: per-`Lang` `is_decision_node` predicate + a single-walk counter that attributes decisions to decls by span.
- crates/ariadne-parser/src/adapters/treesitter/facts.rs — modify: add `complexity: u32` to `Decl` (constructed 0); call `attach_complexity` post-pass in `extract`.
- crates/ariadne-parser/src/adapters/treesitter/mod.rs — modify: `mod complexity;`.
- crates/ariadne-core/src/domain/records.rs — modify: add `complexity: u32` to `SymbolRecord` after `attributes`.
- crates/ariadne-storage/src/domain/migration.rs — modify: frozen `SymbolRecordV6`, `migrate_v6_to_v7`, register `MigrationStep { from: 6, to: 7 }`.
- crates/ariadne-storage/src/adapters/redb/tables.rs — modify: `SCHEMA_VERSION` `6` -> `7`.
- crates/ariadne-storage/tests/migration.rs — modify: v6->v7 round-trip + registry contiguity (v6->v7, v1->v7).
- crates/ariadne-storage/fixtures/ — add a v6 redb fixture (committed `#[test]` helper, tier-04 pattern).
- crates/ariadne-cli/src/domain/mod.rs — modify: `convert_facts` `DeclRaw` map adds `complexity: d.complexity` (~L479-486).
- crates/ariadne-salsa/src/derived.rs — modify: add `complexity: u32` to `DeclRaw` (L44) and `SymbolFactsRaw` (L103).
- crates/ariadne-salsa/src/derive.rs — modify: `build_symbols` carries it — SFC component 0 (L93), decl `decl.complexity` (L103).
- crates/ariadne-salsa/src/db.rs — modify: `commit_revision` `SymbolRecord` adds `complexity: s.complexity` (L274).
- crates/ariadne-salsa/src/memory.rs — modify: test `DeclRaw`/`SymbolFactsRaw` literals add `complexity` (L182, L199).
- crates/ariadne-parser/tests/complexity.rs — new: per-language explicit `assert_eq!` complexity goldens.
- crates/ariadne-parser/tests/snapshots/*.snap (+ sample fixtures) — modify: regenerate `facts_*` snapshots; review each `complexity` value by hand.
- docs/adr/0020-cyclomatic-complexity.md — new: per docs/adr/_template.md.
</files>

<steps>
1. Failing test first (`crates/ariadne-parser/tests/complexity.rs`): over a Rust fixture function with a known branch count (e.g. 2 `if` + 1 `&&` => 4), assert the decl's `complexity == decisions + 1`. Red — no field, no code [src: crates/ariadne-parser/tests/facts_rust.rs].
2. ariadne-core: add `pub complexity: u32` to `SymbolRecord` after `attributes`, so the v7 postcard layout extends the v6 byte prefix [src: crates/ariadne-core/src/domain/records.rs:32-49].
3. ariadne-parser: add `pub complexity: u32` to `Decl` (built 0 at facts.rs:347). New `complexity.rs` exposes `attach_complexity(lang, &mut decls, tree, source)`: one `tree.root_node().walk()` `TreeCursor` pass; for each node where `is_decision_node(lang, node, source)`, find the innermost `Decl` containing `node.byte_range()` and increment its count; finally set function-like decls (D4) to `count + 1`, others 0 [src: https://docs.rs/tree-sitter/0.26.8/tree_sitter/struct.Node.html — `kind()->&'static str`, `walk()->TreeCursor`, `byte_range()`, `child_by_field_name`].
4. Define `is_decision_node` per `Lang` from each grammar's node kinds. Verified this session: **Rust** `if_expression`/`while_expression`/`loop_expression`/`for_expression`/`match_arm`/`try_expression` + token nodes `&&`/`||` [src: tree-sitter-rust node-types]; **Python** `if_statement`/`elif_clause`/`while_statement`/`for_statement`/`except_clause`/`conditional_expression`/`case_clause`/comprehension `if_clause` + `boolean_operator` [src: tree-sitter-python node-types]; **JS/TS/TSX** `if_statement`/`while_statement`/`do_statement`/`for_statement`/`for_in_statement`/`switch_case`/`ternary_expression`/`catch_clause` + `binary_expression` with `child_by_field_name("operator")` in {`&&`,`||`} [src: tree-sitter-javascript node-types]; **Go** `if_statement`/`for_statement`/`expression_case`/`type_case`/`communication_case` + `binary_expression` operator in {`&&`,`||`} [src: tree-sitter-go node-types]. Boolean is a token node in Rust but an `operator` field elsewhere — the predicate handles both. **Java/Kotlin/C#/C/C++/TS/TSX** decision sets follow the same categories (control-flow + switch/when arm + ternary + catch + `&&`/`||`); the executor verifies each kind against the bundled grammar's `node-types.json` and records any kind with no clean mapping in the tier audit, not guessed [src: tree-sitter-{java,kotlin,c-sharp,c,cpp,typescript} repos; plan RD8].
5. ariadne-parser: call `attach_complexity` in `FactExtractor::extract` after `attach_visibility`/`attach_attributes`, passing the layer `lang` + `tree`; the existing per-layer absorb keeps spans file-absolute [src: crates/ariadne-parser/src/adapters/treesitter/facts.rs:401-409].
6. Thread the field end-to-end (mirror tier-04): cli `convert_facts` `DeclRaw` (`complexity: d.complexity`) [src: crates/ariadne-cli/src/domain/mod.rs:479-486]; salsa `DeclRaw` + `SymbolFactsRaw` gain `complexity: u32` (`u32` is `salsa::Update`-safe) [src: crates/ariadne-salsa/src/derived.rs:44-62,103-120]; `build_symbols` sets SFC component 0, decl `decl.complexity` [src: crates/ariadne-salsa/src/derive.rs:93-111]; `commit_revision` `SymbolRecord` adds `complexity: s.complexity` [src: crates/ariadne-salsa/src/db.rs:274-282]; fix the `memory.rs` test literals.
7. ariadne-storage migration: define a frozen `SymbolRecordV6 { canonical_name, kind, defining_file, defining_span, visibility, attributes }` (the current six-field layout). `migrate_v6_to_v7` re-encodes every `SYMBOLS` body to a v7 `SymbolRecord` with `complexity: 0` — same collect-then-reinsert shape as `migrate_v2_to_v3`. Register `MigrationStep { from: 6, to: 7, apply: migrate_v6_to_v7 }`; bump `SCHEMA_VERSION` `6` -> `7`. Confirm `SCHEMA_VERSION` is 6 at build start (set by tier-11b in roadmap order); if higher, rebase `from`/`to`. The contiguity check fails loudly on a duplicate `from->to` [src: crates/ariadne-storage/src/domain/migration.rs:47-77,130-174; tables.rs:8; plan R-C4].
8. Storage test: a committed v6 fixture (helper writes known `SymbolRecord`s at `SCHEMA_VERSION=6`); reopen with 7 -> assert migrate runs, first six fields byte-identical, `complexity == 0`; add `builtin` contiguity tests for `v6->v7` and `v1->v7` [src: crates/ariadne-storage/tests/migration.rs; crates/ariadne-storage/src/domain/migration.rs:358-376].
9. Regenerate the `facts_*` insta snapshots (every `Decl` now prints `complexity`); review each diff so the value is hand-confirmed per language — do not blind-`--accept` [src: crates/ariadne-parser/tests/facts_rust.rs:8-10].
10. Write `docs/adr/0020-cyclomatic-complexity.md` (template): metric = McCabe from CST; D2 strict count; D3 decl-span boundary; D4 arrow-as-variable limitation. Status `Accepted`; cite plan RD8. Run full verification; step 1 goes green.
</steps>

<verification>
- `cargo nextest run -p ariadne-parser -p ariadne-storage -p ariadne-core -p ariadne-salsa -p ariadne-cli` — complexity goldens (branchy + nested + boolean per language) + v6->v7 migration round-trip + contiguity tests green.
- Manual: `ariadne index` the self-index; dump the `SYMBOLS` table — a known-branchy function (e.g. `FactExtractor::extract`) reports complexity matching a hand count; a `struct` reports 0; a nested helper `fn` owns its own decisions (parent not inflated).
- `cargo test --test architecture`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check` — green.
</verification>

<rollback>
`git checkout -- crates/` and `rm docs/adr/0020-cyclomatic-complexity.md`; revert `SCHEMA_VERSION` to `6`. The v6->v7 migration commits atomically in one `WriteTransaction`, so a crashed run leaves the file at v6. A file already migrated to v7 cannot be reopened by the reverted binary (`SchemaMismatch`); roll back before any production index is migrated [src: tier-04 rollback].
</rollback>
</content>
</invoke>
