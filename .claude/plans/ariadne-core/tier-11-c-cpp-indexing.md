---
tier_id: tier-11
title: C and C++ syntactic indexing — tree-sitter-c + tree-sitter-cpp grammars
deps: [tier-10]
exit_criteria:
  - "`ariadne-core` `Lang` gains `C` + `Cpp` variants; tags `\"c\"` / `\"cpp\"` round-trip through `tag`/`from_tag`."
  - "`ariadne-parser` `ParserRegistry` registers C and C++ grammars; `cargo test -p ariadne-parser` covers parse + fact extraction for a C fixture and a C++ fixture."
  - "`lang_for_path` (ariadne-cli) maps C/C++ extensions to the new variants; `ariadne index` on a C/C++ tree reports `\"c\"`/`\"cpp\"` in `langs` with non-zero symbols."
  - "`docs/adr/0008-c-cpp-syntactic-indexing.md` written, status Accepted, cited from this tier + plan.md `<tech_inventory>`."
  - "`cargo build --workspace`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo test --test architecture` all green."
status: completed
completed: 2026-05-20
---

<context>
The tier-10 SLO corpus assembles only ~55,527 indexed files against a 100K
floor because `lang_for_path` recognises no C/C++ extension — `dotnet/runtime`'s
native tree is skipped entirely [src: tier-10-cli-e2e.md `<blockers>`]. plan.md
already scopes v1 as "syntactic indexing of any tree-sitter language"
[src: plan.md `<context>`]; this tier wires the two highest-impact missing
grammars so tier-12 can assemble a genuine >=100K-indexed-file workload.
Syntactic only — no `scip-clang` wiring (SCIP is opt-in per tier-12 and its
graph bridge is unbuilt, tier-10 D-A). Full context: plan.md.
</context>

<files>
- docs/adr/0008-c-cpp-syntactic-indexing.md — NEW. Decision, rejected alternatives, the `.h` C-vs-C++ ambiguity.
- crates/ariadne-core/src/domain/types/lang.rs — add `C` + `Cpp` variants; `tag`/`from_tag` arms.
- crates/ariadne-parser/Cargo.toml — add `tree-sitter-c`, `tree-sitter-cpp`.
- crates/ariadne-parser/src/adapters/treesitter/registry.rs — `V1_LANGS` + `language_for` arms.
- crates/ariadne-parser/src/adapters/treesitter/queries/c.scm — NEW. Decl + call captures.
- crates/ariadne-parser/src/adapters/treesitter/queries/cpp.scm — NEW. Decl + call captures.
- crates/ariadne-parser/src/adapters/treesitter/facts.rs — dispatch c.scm/cpp.scm in `extract_syntactic_facts`.
- crates/ariadne-parser/fixtures/ — small license-clean C + C++ sample files for the parser tests.
- crates/ariadne-cli/src/domain/mod.rs — `lang_for_path` C/C++ extension arms.
- crates/ariadne-cli/src/config.rs — `enabled_langs` autodetect recognises C/C++ repo signals.
</files>

<steps>
1. **Failing test first** (`ariadne-parser` tests): assert `ParserRegistry::new().supports(Lang::C)` and `Lang::Cpp`, and that `extract_syntactic_facts` over a C fixture and a C++ fixture each yields >=1 decl and >=1 call. Red — the variants do not yet compile.
2. ariadne-core `lang.rs`: add `C` + `Cpp` to the `#[non_exhaustive] enum Lang`; `tag` → `"c"`/`"cpp"`; `from_tag` inverse arms [src: crates/ariadne-core/src/domain/types/lang.rs:42-75]. The compiler flags every exhaustive `Lang` match — add arms there.
3. ariadne-parser `Cargo.toml`: add `tree-sitter-c = "0.24"` (0.24.2) and `tree-sitter-cpp = "0.23"` (0.23.4). Both expose `LANGUAGE: LanguageFn` via `tree-sitter-language ^0.1`, which is `Into<Language>` for the workspace tree-sitter 0.26.8 — the exact pattern the eight existing grammars use [src: https://crates.io/crates/tree-sitter-c, https://docs.rs/tree-sitter-cpp/latest/tree_sitter_cpp/, registry.rs:70-86].
4. `registry.rs`: add `Lang::C` + `Lang::Cpp` to `V1_LANGS`; `language_for` arms `tree_sitter_c::LANGUAGE.into()` / `tree_sitter_cpp::LANGUAGE.into()` [src: registry.rs:16-25,74-86].
5. Author `queries/c.scm` and `queries/cpp.scm` — tree-sitter query captures for declarations (C: functions, structs, enums, typedefs; C++ adds classes, namespaces, methods) and call sites, reusing the capture names the existing `.scm` files + `facts.rs` already consume [src: crates/ariadne-parser/src/adapters/treesitter/queries/rust.scm as the capture-name reference].
6. `facts.rs`: register `c.scm`/`cpp.scm` for the new langs so `extract_syntactic_facts` dispatches them.
7. cli `lang_for_path` (domain/mod.rs:38-51): add `"c" => C`; `"cpp"|"cc"|"cxx"|"c++"|"hpp"|"hh"|"hxx" => Cpp`; `"h" => C`. The `.h` C-vs-C++ ambiguity resolves to C by default — no content sniffing in v1; ADR-0008 records it as a known limitation.
8. cli `config.rs`: `enabled_langs` autodetect treats C/C++ source presence as an enable signal; both on in a fresh `config.toml`.
9. Write ADR-0008 per the `docs/adr/` template (status field format per ADR-0001): decision = adopt tree-sitter-c 0.24 + tree-sitter-cpp 0.23 for syntactic-only C/C++ indexing; rejected = libclang/clang-based parsing (drags cgo/libclang, violates plan.md D5 "no cgo, pure-Rust on the critical path") and deferring to `scip-clang` alone (no syntactic graph for headers, and SCIP is opt-in). Record the `.h`→C default.
</steps>

<verification>
- `cargo build --workspace` + `cargo clippy --workspace --all-targets --all-features -- -D warnings` — clean.
- `cargo test -p ariadne-parser` — green, including the new C + C++ fact-extraction tests.
- `cargo test --test architecture` — green: the grammar crates are dependencies of `ariadne-parser` only, no new cross-crate edge.
- `cargo fmt --all --check` and `RUSTDOCFLAGS=-D warnings cargo doc --workspace --no-deps --document-private-items` — clean.
- `cargo deny check` — passes; tree-sitter-c + tree-sitter-cpp are MIT (license-clean).
- Real run: `ariadne index` on a small C tree and a small C++ tree → JSON summary `langs` contains `"c"` and `"cpp"`, `symbols` > 0.
</verification>

<rollback>
Revert the `ariadne-parser/Cargo.toml` deps and the `registry.rs` / `lang.rs` /
`facts.rs` / `lang_for_path` / `config.rs` arms; delete `c.scm`, `cpp.scm`, the
C/C++ fixtures, and ADR-0008. Removing `Lang::C`/`Cpp` is source-local — no
on-disk index migration is needed; a `"c"`/`"cpp"` tag simply stops appearing.
</rollback>
