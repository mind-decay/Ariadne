---
tier_id: tier-03
title: Multi-region injection parse engine + Vue SFC support
deps: [tier-01]
exit_criteria:
  - "`ParsedFile` exists — a host `(Lang, Tree)` plus zero or more injected `(Lang, Tree)` layers; single-grammar files are the host-only degenerate case."
  - "`Lang::Vue` is registered against `tree_sitter_html::LANGUAGE`; parsing a `.vue` fixture yields a host HTML tree and an injected JS/TS tree for the `<script>` block."
  - "`extract_syntactic_facts` runs over every layer of a `ParsedFile` and merges facts with file-absolute byte spans."
  - "Proptest: 100 random `InputEdit` sequences on a `.vue` fixture — the incremental `ParsedFile` matches a full reparse (root S-expression equality on every layer)."
  - "`docs/adr/0011-framework-grammars-injection.md` written, status Accepted, cited from this tier + plan.md."
  - "`cargo nextest run -p ariadne-parser`, `cargo clippy ... -D warnings`, `cargo test --test architecture` all green."
status: completed
completed: 2026-05-21
---

<context>
The single-grammar-per-file model cannot represent a Vue SFC: one `.vue` file
is HTML skeleton + an embedded JS/TS `<script>`. This tier generalises the
parse model to `ParsedFile` (a host layer + injected layers) and proves it on
Vue, using `tree-sitter-html` as the `.vue` host grammar (plan.md D1, D4).
tree-sitter `set_included_ranges` parses only chosen byte ranges of the full
file, so an injected tree's node offsets are already file-absolute — no manual
remap [src: tree-sitter.github.io/tree-sitter/3-syntax-highlighting.html].
Svelte/Astro reuse this engine in tier-04. Full context: plan.md.
</context>

<files>
- `docs/adr/0011-framework-grammars-injection.md` — NEW. Grammar choices (D1/D4/D5/D6), the injection model, the `tree-sitter-html`-as-Vue-host trade-off (R-VueDir).
- `crates/ariadne-parser/Cargo.toml` — add `tree-sitter-html = "=0.23.2"`.
- `crates/ariadne-parser/src/adapters/treesitter/registry.rs` — register `Lang::Vue` → `tree_sitter_html::LANGUAGE.into()`.
- `crates/ariadne-parser/src/adapters/treesitter/mod.rs` — `ParsedFile` type + re-export.
- `crates/ariadne-parser/src/adapters/treesitter/injection.rs` — NEW. Derive injected `(Lang, ranges)` from a host tree; drive `Parser::set_included_ranges`.
- `crates/ariadne-parser/src/adapters/treesitter/incremental.rs` — `parse_file` returns `ParsedFile`; host tree incremental, injected layers re-derived.
- `crates/ariadne-parser/src/adapters/treesitter/facts.rs` — `extract_syntactic_facts` iterates `ParsedFile` layers, merges, dedups.
- `crates/ariadne-parser/src/adapters/treesitter/cache.rs` — cache key stays `(host_lang, content)`; layers re-derive on rehydrate.
- `crates/ariadne-parser/src/adapters/treesitter/queries/vue.scm` — NEW. Vue host (HTML) captures: component renders, directives.
- `crates/ariadne-parser/src/lib.rs` — re-export `ParsedFile`.
- `crates/ariadne-parser/src/errors.rs` — `IncludedRanges` error variant for the injected-layer parse (step 5).
- `crates/ariadne-parser/fixtures/vue/*.vue`, `tests/facts_vue.rs`, `tests/incremental_vue.rs` (proptest).
- `crates/ariadne-parser/tests/common/mod.rs`, `tests/real_world.rs` — `extract_syntactic_facts`/`parse_file` signature ripple into shared test helpers.
</files>

<steps>
1. **Failing test first** (`tests/facts_vue.rs`): parse `fixtures/vue/sample.vue`; assert the result is a `ParsedFile` with an HTML host layer and one JS/TS injected layer, and that merged facts contain a `<script>`-block decl and a `RenderSite` from a `<Child/>` in `<template>`. Red.
2. `Cargo.toml`: add `tree-sitter-html = "=0.23.2"` (`tree-sitter-language ^0.1`, `Into<Language>` — the established workspace pattern) [src: crates.io/api/v1/crates/tree-sitter-html/0.23.2/dependencies].
3. `registry.rs`: register `Lang::Vue` → `tree_sitter_html::LANGUAGE.into()`. `Vue` joins `V1_LANGS`.
4. `mod.rs`: define `ParsedFile { host: (Lang, Tree), injected: Vec<(Lang, Tree)> }`. A plain `.ts`/`.tsx`/`.js` file is `ParsedFile { host, injected: vec![] }`.
5. `injection.rs`: for `Lang::Vue`, walk the host HTML tree for the `<script>` element; read its `lang`/`setup` attributes to choose `Lang::Tsx` (`lang="tsx"`), `Lang::TypeScript` (`lang="ts"`), or `Lang::JavaScript` (default); collect the script's content byte range. Build the injected layer by `Parser::set_included_ranges(&[range])` then `parse` over the *full file bytes* — offsets stay file-absolute. Multiple `<script>` blocks (`<script>` + `<script setup>`) → one included-ranges set for the same JS/TS lang, or two layers; pick one and document it [src: tree-sitter `set_included_ranges` docs].
6. `incremental.rs`: `parse_file(content, prev: Option<&ParsedFile>, edits) -> Result<ParsedFile>`. Apply `edits` + incremental reparse to the host tree (existing logic). Re-derive injection ranges from the new host tree; re-parse each injected layer (fresh parse is acceptable — the proptest gate guarantees correctness; the incremental win is the host). Document this in a doc-comment.
7. `facts.rs`: `extract_syntactic_facts` takes `&ParsedFile`; run each layer's query (`vue.scm` for the HTML host, the JS/TS query for an injected layer) via a per-layer `FactExtractor`; concatenate the `SyntacticFacts` vectors; sort by byte offset; dedup exact duplicates. Spans are already file-absolute (step 5).
8. `vue.scm` (HTML grammar): `@render.component` on a `tag_name` that is a custom element (contains `-`, or is capitalised — filter in `facts.rs` if the query cannot express it); capture `v-`/`@`/`:` `attribute` names as directive facts only if cheaply expressible, else defer to tier-04 follow-up. `<script>`/`<style>` elements are *not* facts — they are injection hosts.
9. `cache.rs`: key remains `(host_lang, content)`; on rehydrate, re-parse rebuilds the whole `ParsedFile` incl. layers. Document that injected trees are never serialised.
10. Proptest `tests/incremental_vue.rs`: 100 random `InputEdit` sequences on the `.vue` fixture; assert incremental `ParsedFile` ≡ full reparse — every layer's root S-expression equal. If any divergence: fail loud, do not weaken.
11. Write ADR-0011 per `docs/adr/_template.md`.
</steps>

<verification>
- `cargo nextest run -p ariadne-parser` — green: `facts_vue` snapshot + injection proptest + registry coverage; the tier-03(core) JS proptest still green (`ParsedFile` host-only path unchanged for `.js`).
- Manual: parse a real `.vue` file with `<script setup lang="ts">`; merged facts list the script's `defineProps`/functions *and* the template's child components.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo fmt --all --check`, `cargo test --test architecture` — clean.
- `cargo bench -p ariadne-parser` — cold/incremental budgets on the existing JS payload unregressed.
</verification>

<rollback>
Revert `Cargo.toml`/`registry.rs`/`injection.rs`/`mod.rs` and the `facts.rs`/
`incremental.rs`/`cache.rs` signature changes; delete `vue.scm`, the Vue
fixtures, the new tests, ADR-0011. If `ParsedFile` rollback is impractical
mid-tier, the `rollback` is `git revert` of the whole tier commit — no on-disk
index format changed.
</rollback>
