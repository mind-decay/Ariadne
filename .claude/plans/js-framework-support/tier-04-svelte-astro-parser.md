---
tier_id: tier-04
title: Svelte and Astro SFC parsing on the injection engine
deps: [tier-01, tier-03]
exit_criteria:
  - "`Lang::Svelte` registered against `tree_sitter_svelte_ng::LANGUAGE`; `Lang::Astro` against `tree_sitter_astro_next::LANGUAGE`."
  - "Parsing a `.svelte` fixture yields an HTML-ish host layer + an injected JS/TS `<script>` layer; merged facts carry script decls + template `RenderSite`s."
  - "Parsing a `.astro` fixture yields a host layer + an injected TS frontmatter layer; merged facts carry frontmatter decls + body `RenderSite`s."
  - "`extract_syntactic_facts` over both fixtures produces `Component`/`RenderSite`/`HookSite` facts via `svelte.scm` / `astro.scm`."
  - "Incremental proptest: 100 random edit sequences on a `.svelte` fixture — incremental `ParsedFile` ≡ full reparse."
  - "`cargo nextest run -p ariadne-parser`, `cargo clippy ... -D warnings`, `cargo test --test architecture` all green."
status: pending
---

<context>
Svelte and Astro are SFCs handled by the tier-03 `ParsedFile` injection engine.
This tier adds two host grammars and their injection-range derivation + facts
queries — no engine change. Svelte's grammar ships an `INJECTIONS_QUERY`
constant; Astro's frontmatter is a TS block fenced by `---`. plan.md D4/D5/D6.
</context>

<files>
- `crates/ariadne-parser/Cargo.toml` — add `tree-sitter-svelte-ng = "=1.0.2"`, `tree-sitter-astro-next = "=0.1.1"`.
- `crates/ariadne-parser/src/adapters/treesitter/registry.rs` — register `Lang::Svelte` and `Lang::Astro`.
- `crates/ariadne-parser/src/adapters/treesitter/injection.rs` — injection-range derivation for Svelte (`<script>` block) and Astro (frontmatter).
- `crates/ariadne-parser/src/adapters/treesitter/facts.rs` — `query_source` arms for `Lang::Svelte`/`Lang::Astro`.
- `crates/ariadne-parser/src/adapters/treesitter/queries/svelte.scm` — NEW. Component renders, `{#each}`/`{#if}` blocks, directives.
- `crates/ariadne-parser/src/adapters/treesitter/queries/astro.scm` — NEW. Component renders in the body.
- `crates/ariadne-parser/fixtures/svelte/*.svelte`, `fixtures/astro/*.astro`.
- `crates/ariadne-parser/tests/facts_svelte.rs`, `tests/facts_astro.rs`, `tests/incremental_svelte.rs` (proptest).
</files>

<steps>
1. **Failing test first** (`tests/facts_svelte.rs`, `tests/facts_astro.rs`): assert `ParserRegistry` supports `Lang::Svelte`/`Lang::Astro` and that facts over each fixture carry an injected-layer decl and a template/body `RenderSite`. Red.
2. `Cargo.toml`: add `tree-sitter-svelte-ng = "=1.0.2"` (exports `LANGUAGE` + `INJECTIONS_QUERY`) [src: docs.rs/tree-sitter-svelte-ng] and `tree-sitter-astro-next = "=0.1.1"` (runtime `tree-sitter-language ^0.1.7`) [src: crates.io/api/v1/crates/tree-sitter-astro-next/0.1.1/dependencies].
3. `registry.rs`: register `Lang::Svelte` → `tree_sitter_svelte_ng::LANGUAGE.into()`, `Lang::Astro` → `tree_sitter_astro_next::LANGUAGE.into()`; both join `V1_LANGS`.
4. `injection.rs` — Svelte: derive the `<script>` content range from the host tree. Prefer the grammar's `INJECTIONS_QUERY` constant (`tree_sitter_svelte_ng::INJECTIONS_QUERY`) — it already maps `<script>`/`<style>` to languages; run it and keep only the JS/TS injection. If using the constant proves brittle, fall back to the same `<script>`-element walk as Vue (tier-03 injection.rs); document the choice [src: docs.rs/tree-sitter-svelte-ng — INJECTIONS_QUERY].
5. `injection.rs` — Astro: the frontmatter is the leading `---`-fenced block; the astro-next grammar exposes it as a dedicated `frontmatter` node. Take that node's content range and inject it as `Lang::TypeScript`. If the grammar has no `frontmatter` node, derive the range between the first two `---` tokens; document which.
6. `facts.rs`: `query_source` → `Lang::Svelte => QUERY_SVELTE`, `Lang::Astro => QUERY_ASTRO` (`include_str!`).
7. Author `svelte.scm` against the svelte-ng node types: `@render.component` on capitalised element tag names; `{#each}`/`{#if}` block markers and directives only if cheaply expressible. Author `astro.scm` against astro-next node types: `@render.component` on capitalised tags in the body. Reuse the tier-02 capture names so `facts.rs` needs no new capture handling.
8. Author the `insta` snapshot tests + the Svelte incremental proptest (mirror `tests/incremental_vue.rs` from tier-03). 100 random edits; incremental ≡ full reparse on every layer; fail loud on divergence.
9. Pin note: `tree-sitter-astro-next` is 0.1.x — record R-Astro-ts in the test file's header comment; a future pin bump is a follow-up + ADR amendment.
</steps>

<verification>
- `cargo nextest run -p ariadne-parser` — green: `facts_svelte`/`facts_astro` snapshots, Svelte incremental proptest, registry coverage; tier-03 Vue tests still green.
- Manual: parse a real `.svelte` component — facts list `<script>` functions/`$:` reactive decls *and* template child components; parse a real `.astro` page — facts list frontmatter imports/consts *and* body components.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo fmt --all --check`, `cargo test --test architecture` — clean.
- `cargo deny check` — passes; both grammar crates are license-clean (MIT).
</verification>

<rollback>
Revert the `Cargo.toml`/`registry.rs`/`injection.rs`/`facts.rs` arms; delete
`svelte.scm`, `astro.scm`, the fixtures, the new tests. `Lang::Svelte`/`Astro`
stay defined (tier-01) but unregistered. No on-disk index migration.
</rollback>
