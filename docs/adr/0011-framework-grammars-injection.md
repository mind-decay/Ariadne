# ADR-0011: Framework grammars and the multi-region injection engine

<status>
Accepted
Date: 2026-05-21
Decider: user / claude
</status>

<context>
The `js-framework-support` plan must index Vue single-file components. A
`.vue` SFC is not a single-grammar file: one file is an HTML skeleton
(`<template>`/`<script>`/`<style>`) wrapping an embedded JS/TS `<script>`
block [src: ../../.claude/plans/js-framework-support/plan.md `<context>`].
The pre-existing parser model bound exactly one `tree_sitter::Language` per
file, so it could represent neither the HTML host nor the embedded script.

Forces:
- **Maintainability** — adding a bespoke per-framework grammar (Vue, later
  Svelte/Astro) multiplies grammar-crate maintenance and `build.rs` burden.
- **Reliability** — the incremental-parse equivalence guarantee (incremental
  tree ≡ full reparse) must survive the move to a multi-grammar file.
- **Efficiency** — re-parsing embedded regions must not force a full-file
  remap of every byte offset.

Upstream constraints: the workspace pins `tree-sitter = "=0.26.8"`; every
grammar crate must reach `Into<Language>` through `tree-sitter-language ^0.1`
[src: ../../crates/ariadne-parser/Cargo.toml].
</context>

<decision>
A `.vue` SFC is parsed as a **multi-layer `ParsedFile`**: a host layer plus
zero or more injected layers. The host grammar for `.vue` is
**`tree-sitter-html` 0.23.2** (plan D1); the embedded `<script>` block is
re-parsed as an injected JS/TS layer via tree-sitter language injection —
`Parser::set_included_ranges` over the *full* file bytes (plan D4). A
single-grammar file (`.ts`/`.tsx`/`.js`/`.rs`/…) is the degenerate case:
host layer only, no injected layers.
</decision>

<rationale>
- **`tree-sitter-html` as the Vue host (maintainability).** A `.vue` SFC's
  top level is valid HTML: `<template>`/`<script>`/`<style>` are elements,
  child components are custom-element tags, directives are attributes.
  `tree-sitter-html` is an official, maintained crate that reaches
  `Into<Language>` through `tree-sitter-language ^0.1` — the established
  workspace pattern — so no C-source vendoring or `build.rs` is added
  [src: https://crates.io/api/v1/crates/tree-sitter-html/0.23.2/dependencies].
- **Injection over slicing (reliability + efficiency).** `set_included_ranges`
  restricts a parse to chosen byte ranges, but the parse still runs over the
  full file buffer, so every node offset in the injected sub-tree is already
  file-absolute — host facts and `<script>` facts share one coordinate space
  with **no manual remap**
  [src: https://tree-sitter.github.io/tree-sitter/3-syntax-highlighting.html].
- **Incremental host, fresh injected (reliability).** `parse_file` reparses
  the host tree incrementally (reusing the prior tree + `InputEdit` delta)
  and re-derives + fully re-parses the small injected layers each call. A
  100-case proptest gates this: an incremental `ParsedFile` must equal a full
  reparse, every layer's root S-expression equal
  [src: ../../crates/ariadne-parser/tests/incremental_vue.rs].
- **One merged injected layer (maintainability).** A Vue SFC may carry both
  `<script>` and `<script setup>`. Both are collected into a single injected
  layer — one `set_included_ranges` set, one JS/TS grammar — keeping the
  layer model flat. The layer takes the most JSX-/type-capable grammar any
  `<script>` declares — `Tsx` for `lang="tsx"`, `TypeScript` for `lang="ts"`,
  else `JavaScript` [src: tier-03 step 5; plan.md D2].
</rationale>

<alternatives>
- **Dedicated `tree-sitter-vue` grammar** — rejected: the crate is dead
  (latest 0.0.3, published 2022-09-24, pre-`tree-sitter-language` ABI)
  [src: https://crates.io/api/v1/crates/tree-sitter-vue].
- **Vendoring + building a Vue grammar from C source** — rejected: a
  `build.rs` maintenance burden for every framework, against plan D5's
  pure-Rust-critical-path rule [src: plan.md D1].
- **Slice the `<script>` block only** — rejected: discards the `<template>`,
  losing every component-render edge [src: plan.md D4].
- **Host grammar only** — rejected: discards every symbol inside `<script>`
  [src: plan.md D4].
</alternatives>

<consequences>
- The parser adapter exposes `ParsedFile { host: (Lang, Tree), injected:
  Vec<(Lang, Tree)> }` and a free `parse_file` returning it. The
  single-grammar `TreeSitterParser::parse_file` method is retained as the
  host-layer primitive `parse_file` builds on.
- `extract_syntactic_facts` runs the per-layer query over every layer and
  merges facts; spans are file-absolute, so a byte-offset sort + dedup is the
  whole merge.
- The parse cache key stays `(host_lang, content)`; injected layers are never
  serialized — `rehydrate` re-derives the whole `ParsedFile`.
- **Accepted limitation (R-VueDir):** the HTML host grammar cannot express
  Vue directive *semantics* (`v-for` scope, `v-if` branches). Directives stay
  visible as plain HTML attributes; component-render edges (custom-element
  tags) are unaffected. Typed directive facts are deferred to a tier-04
  follow-up [src: plan.md R-VueDir].
- Svelte and Astro (tier-04) reuse this injection engine with their own host
  grammars; a new host grammar must keep the incremental-equivalence proptest
  green or the tier is blocked.
- New invariant for CI: the `incremental_vue` proptest is the gate for any
  future change to the injection engine.
</consequences>

<sources>
- `[src: https://tree-sitter.github.io/tree-sitter/3-syntax-highlighting.html]` — language injection / `included_ranges`
- `[src: https://crates.io/api/v1/crates/tree-sitter-html/0.23.2/dependencies]` — `tree-sitter-html` ABI fit
- `[src: https://crates.io/api/v1/crates/tree-sitter-vue]` — dead Vue grammar crate
- `[src: ../../.claude/plans/js-framework-support/plan.md]` — decisions D1, D4; risk R-VueDir
- `[src: ../../.claude/plans/js-framework-support/tier-03-injection-engine.md]` — tier scope
- `[src: ../../crates/ariadne-parser/tests/incremental_vue.rs]` — incremental-equivalence gate
</sources>
