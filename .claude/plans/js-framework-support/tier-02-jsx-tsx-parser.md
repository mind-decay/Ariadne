---
tier_id: tier-02
title: JSX/TSX parsing — TSX grammar, JSX-aware queries, component + hook facts
deps: [tier-01]
exit_criteria:
  - "`ParserRegistry` registers `Lang::Tsx` against `tree_sitter_typescript::LANGUAGE_TSX`; `supports(Lang::Tsx)` is true."
  - "`SyntacticFacts` gains `renders: Vec<RenderSite>` and `hooks: Vec<HookSite>`; `DeclKind` gains `Component`."
  - "`extract_syntactic_facts` over a React `.tsx` fixture and a Solid `.tsx` fixture each yields >=1 `Component` decl, >=1 `RenderSite`, >=1 `HookSite`."
  - "`.jsx` content (parsed with the JavaScript grammar) yields component/render/hook facts from the JSX patterns added to `javascript.scm`."
  - "`cargo nextest run -p ariadne-parser`, `cargo clippy ... -D warnings`, `cargo test --test architecture` all green."
status: completed
completed: 2026-05-21
---

<context>
React and SolidJS are single-grammar-per-file: `.jsx`→JavaScript grammar (JSX
native), `.tsx`→the TSX grammar. This tier is the tier-11 pattern (register a
grammar + author a `.scm`), extended with the component-graph fact vectors.
The injection engine (tier-03) is *not* needed here. Full context: plan.md
D2, D3, D7, D8; `<architecture>`.
</context>

<files>
- `crates/ariadne-parser/src/adapters/treesitter/registry.rs` — add `Lang::Tsx` to `V1_LANGS`; `language_for` arm `tree_sitter_typescript::LANGUAGE_TSX.into()`.
- `crates/ariadne-parser/src/adapters/treesitter/facts.rs` — `DeclKind::Component`; `RenderSite`/`HookSite` structs; `renders`/`hooks` fields on `SyntacticFacts`; capture handling for `@render.component` and `@hook.callee`; `query_source` arm for `Lang::Tsx`.
- `crates/ariadne-parser/src/adapters/treesitter/queries/tsx.scm` — NEW. TSX decls + JSX render/hook captures.
- `crates/ariadne-parser/src/adapters/treesitter/queries/javascript.scm` — append JSX render/hook + component captures.
- `crates/ariadne-parser/src/adapters/treesitter/queries/typescript.scm` — append the same component/hook captures (non-JSX TS may still define hooks/components).
- `crates/ariadne-parser/fixtures/react/*.tsx`, `fixtures/solid/*.tsx`, `fixtures/react/*.jsx` — small license-clean fixtures.
- `crates/ariadne-parser/tests/facts_tsx.rs`, `tests/facts_jsx.rs` — golden `insta` snapshots.
- `crates/ariadne-parser/Cargo.toml` — no new dep (`tree-sitter-typescript` already present).
</files>

<steps>
1. **Failing test first** (`tests/facts_tsx.rs`): assert `ParserRegistry::new().supports(Lang::Tsx)` and that facts over `fixtures/react/sample.tsx` carry a `Component` decl, a `RenderSite`, and a `HookSite`. Red.
2. `registry.rs`: add `Lang::Tsx` to `V1_LANGS`; `language_for` → `tree_sitter_typescript::LANGUAGE_TSX.into()`. `LANGUAGE_TSX` is a distinct `LanguageFn` from `LANGUAGE_TYPESCRIPT` [src: docs.rs/tree-sitter-typescript/0.23.2 — exported constants].
3. `facts.rs`: add `DeclKind::Component` (+ `from_tag` arm `"component"`). Add `RenderSite { component: String, byte_range: (u32,u32) }` and `HookSite { callee: String, byte_range: (u32,u32) }`. Add `renders: Vec<RenderSite>` and `hooks: Vec<HookSite>` to `SyntacticFacts` (keeps `Default`/`Hash` derives). Extend `FactExtractor::extract` to handle `@render.component` and `@hook.callee` captures and to sort the new vectors by byte offset, mirroring the existing `@call.callee` arm [src: crates/ariadne-parser/src/adapters/treesitter/facts.rs:201-249].
4. `query_source`: add `Lang::Tsx => QUERY_TSX` with `const QUERY_TSX = include_str!("queries/tsx.scm")`.
5. Author `tsx.scm`: reuse `typescript.scm`'s decl/import/call captures (TSX node types are the TS node types plus `jsx_element`/`jsx_self_closing_element`) [src: crates/ariadne-parser/src/adapters/treesitter/queries/typescript.scm]. Add:
   - `@def.component` — a `function_declaration`/`lexical_declaration` whose body contains a `jsx_element` or `jsx_self_closing_element`. If a single tree-sitter pattern cannot express "returns JSX", capture all functions as `@def.function` and let a post-filter in `facts.rs` re-tag a function as `Component` when its `def_byte_range` encloses a `RenderSite` — pick whichever the build session proves works against the fixture; record the choice in a query comment.
   - `@render.component` — the tag-name `identifier` of a `jsx_opening_element`/`jsx_self_closing_element` whose name is capitalised (component, not host element). Capitalisation filtering happens in `facts.rs` if the query cannot express it.
   - `@hook.callee` — a `call_expression` callee `identifier` matching the hook convention (`use*` for React, `createSignal`/`createEffect`/`createMemo`/`createResource` for Solid).
6. Append the JSX render/hook/component captures to `javascript.scm` so `.jsx` (JavaScript grammar) produces the same facts; append the component/hook captures (no JSX node) to `typescript.scm`.
7. Author the `insta` snapshot tests `facts_tsx.rs` / `facts_jsx.rs`; accept snapshots after manual inspection.
</steps>

<verification>
- `cargo nextest run -p ariadne-parser` — green: TSX + JSX fact snapshots, registry coverage.
- Manual: `extract_syntactic_facts` over the React fixture lists each `function Foo()` returning JSX as a `Component`, every `<Child/>` as a `RenderSite`, every `useState(...)` as a `HookSite`; the Solid fixture lists `createSignal` as a `HookSite`.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo fmt --all --check`, `cargo test --test architecture` — clean.
- `cargo bench -p ariadne-parser --no-run` — compiles.
</verification>

<rollback>
Revert `registry.rs`/`facts.rs` arms and the query-file edits; delete `tsx.scm`,
the React/Solid/JSX fixtures, and `facts_tsx.rs`/`facts_jsx.rs`. `Lang::Tsx`
stays defined (tier-01) but unregistered — `supports` returns false again.
</rollback>
