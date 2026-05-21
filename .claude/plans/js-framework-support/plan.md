---
slug: js-framework-support
title: JS-framework support — React/Solid JSX-TSX, Vue, Svelte, Astro across parse, semantic, and graph
created: 2026-05-21
owners: [user, claude]
review: [user, codex?]
single_tier: false
tiers:
  - tier-01-domain
  - tier-02-jsx-tsx-parser
  - tier-03-injection-engine
  - tier-04-svelte-astro-parser
  - tier-05-cli-detection
  - tier-06-scip-jsx-tsx
  - tier-07-scip-bridge-vue
  - tier-08-scip-bridge-svelte
  - tier-09-component-graph-e2e
---

<context>
Problem: Ariadne is blind to JS-framework structure. `.tsx` is mis-mapped to the
non-TSX TypeScript grammar (the `<T>x` cast vs `<Foo/>` element ambiguity makes
that grammar wrong for TSX [src: docs.rs/tree-sitter-typescript/0.23.2]); `.jsx`
parses but `javascript.scm` has zero JSX/component captures; `.vue`/`.svelte`/
`.astro` are unrecognised by `lang_for_path` and skipped entirely
[src: crates/ariadne-cli/src/domain/mod.rs:70-83].
Solution: index five framework families — React + SolidJS (JSX/TSX), Vue,
Svelte, Astro — syntactically (tree-sitter) and, where an indexer exists,
semantically (SCIP); surface a component graph (which component renders which,
which hooks each uses).
Success: `ariadne index` over a real repo of each family reports the family's
lang tag with non-zero symbols and `Renders`/`UsesHook` edges; the v1 SLOs
(cold <60s, incremental p95 <500ms, query p95 <100ms) still hold.
In scope: parse + facts for all five; SCIP for React/Solid (scip-typescript)
and Vue/Svelte (custom bridge); component graph; MCP surface; E2E.
Out of scope: Astro semantic indexing (D11); Angular; cross-repo resolution;
template type-checking; CSS/`<style>` analysis.
</context>

<constraints>
- Inherits plan `ariadne-core` constraints: cold full index <60s, incremental
  p95 <500ms, query p95 <100ms on 100K files; <4GB RAM; pure-Rust deps on the
  critical path; no cgo/Node/JVM in the `ariadne` binary [src: .claude/plans/ariadne-core/plan.md `<constraints>`, D5].
- External SCIP indexers (incl. the new bridge) run as subprocesses on PATH,
  vendored — never linked into the binary; consistent with the existing
  `ScipIndexer` model [src: crates/ariadne-scip/src/indexer/mod.rs:1-13].
- Workspace pins `tree-sitter = "=0.26.8"`; every new grammar crate must reach
  `Into<Language>` via `tree-sitter-language ^0.1` [src: crates/ariadne-parser/Cargo.toml:30-47].
- TDD per tier: failing test first; realistic fixtures from real OSS repos;
  no mocks at module boundaries [src: CLAUDE.md `<rules>`].
- Each new architectural decision gets an ADR under `docs/adr/NNNN-*.md`.
</constraints>

<decisions>
**D1 — Vue host grammar = `tree-sitter-html` 0.23.2, not a Vue grammar.** The
dedicated `tree-sitter-vue` crate is dead (latest 0.0.3, published 2022-09-24,
pre-`tree-sitter-language` ABI) [src: crates.io/api/v1/crates/tree-sitter-vue].
A `.vue` SFC's top level is valid HTML — `<template>`/`<script>`/`<style>` are
elements, directives are attributes, child components are custom elements;
`tree-sitter-html` is official and ABI-compatible (`tree-sitter-language ^0.1`)
[src: crates.io/api/v1/crates/tree-sitter-html/0.23.2/dependencies].
*Rejected:* vendoring+building a Vue grammar from C source (build.rs burden,
maintenance); `tree-sitter-vue` 0.0.3 (incompatible, unmaintained).

**D2 — TSX grammar = `tree_sitter_typescript::LANGUAGE_TSX`; new `Lang::Tsx`.**
The crate (already a workspace dep at =0.23.2) exports `LANGUAGE_TSX` distinct
from `LANGUAGE_TYPESCRIPT` [src: docs.rs/tree-sitter-typescript/0.23.2]. `.tsx`
re-maps off the wrong grammar onto `Lang::Tsx`. A `<script lang="tsx">` block
in a Vue SFC host injects a `Lang::Tsx` layer for the same `<T>x`-ambiguity
reason — JSX inside it must not be parsed by the non-TSX grammar (tier-03 step 5).

**D3 — `.jsx` stays `Lang::JavaScript`.** `tree-sitter-javascript` parses JSX
natively (emits `jsx_element` nodes) [src: github.com/tree-sitter/tree-sitter-javascript].
JSX captures are added to `javascript.scm`; plain `.js` has no JSX nodes so the
extra patterns are inert there. No new variant for `.jsx`.

**D4 — SFC parsing = tree-sitter language injection (`included_ranges`).** The
host grammar parses the file skeleton; the `<script>`/frontmatter byte ranges
are re-parsed by the JS/TS grammar as a second layer
[src: tree-sitter.github.io/tree-sitter/3-syntax-highlighting.html — injections;
github.com/tree-sitter/tree-sitter/discussions/793].
*Rejected:* slice-the-`<script>`-only (loses the template → loses render
edges); host-grammar-only (loses every symbol inside `<script>`).

**D5 — Svelte grammar = `tree-sitter-svelte-ng` 1.0.2.** Maintained; exports
`LANGUAGE` + `INJECTIONS_QUERY` for embedded script/style
[src: docs.rs/tree-sitter-svelte-ng]. *Rejected:* legacy `tree-sitter-svelte`
(stale, no `-ng` ABI guarantees).

**D6 — Astro grammar = `tree-sitter-astro-next` 0.1.1.** Dev-deps `tree-sitter
^0.26.5`, runtime `tree-sitter-language ^0.1.7` — clean fit for the 0.26.8 pin
[src: crates.io/api/v1/crates/tree-sitter-astro-next/0.1.1/dependencies].

**D7 — SolidJS reuses the JSX/TSX path; no new grammar.** Solid components are
functions returning JSX; its reactive primitives (`createSignal`,
`createEffect`) are detected as hook-like calls by the same JSX query family
[src: solidjs.com — components are JSX functions].

**D8 — Component graph = `DeclKind::Component` (ariadne-parser) +
`EdgeKind::Renders` / `EdgeKind::UsesHook` (ariadne-core).** Components become
first-class symbols; render/hook usage become first-class edges so
`blast_radius`/`coupling` already traverse them [src: user scope decision;
existing `EdgeKind` pattern in ariadne-core; docs/adr/0012-component-graph-model.md].

**D9 — React/Solid semantic = existing `scip-typescript`.** The TS compiler
treats `.tsx`/`.jsx` as first-class; `scip-typescript` builds on
`ts.createProgram()` and indexes them with no new indexer
[src: github.com/sourcegraph/scip-typescript].

**D10 — Vue/Svelte semantic = a custom Volar-based SCIP bridge subprocess.**
`@volar/typescript`'s `proxyCreateProgram()` wraps `ts.createProgram()` to make
`.vue`/`.svelte` program-visible (the exact mechanism of `vue-tsc`); the bridge
runs `scip-typescript`'s SCIP emit over that wrapped program. Volar's `Language`
layer maps virtual-TS positions back to SFC source — no hand-rolled sourcemaps
[src: deepwiki.com/vuejs/language-tools — proxyCreateProgram; svelte language-
tools svelte2tsx]. The bridge is a Node CLI on PATH, like `scip-typescript`
itself — not in the `ariadne` binary, so D5's no-Node-in-binary rule holds.
*Rejected:* `scip-typescript` + tsconfig plugin (fragile, undocumented);
syntactic-only (user-rejected).

**D11 — Astro semantic deferred.** No Volar→SCIP path for `.astro` was verified
this session; `.astro` is syntactic-only this plan. Recorded as R-Astro;
revisit as a follow-up plan.
</decisions>

<architecture>
Parse layering (ariadne-parser) — tier-03 generalises to `ParsedFile`:
```
ParsedFile
  host:     (Lang, Tree)          whole-file skeleton grammar
  injected: Vec<(Lang, Tree)>     JS/TS sub-trees from <script>/frontmatter
```
`extract_syntactic_facts` runs each layer's query and merges; injected layers
parse over the full file bytes under `set_included_ranges`, so spans are file-
absolute — no remap. Single-grammar files are host-only (`injected` empty).

Component graph: parser emits `RenderSite` (child-component tag) and `HookSite`
(hook/primitive call) alongside `Decl`/`Import`/`CallSite`; the CLI edge
resolver maps them to `EdgeRecord{ kind: Renders | UsesHook }`.

SCIP: React/Solid → existing `ScipTypescriptIndexer`. Vue/Svelte → new
`ScipSfcBridgeIndexer` driver invoking the external `ariadne-sfc-scip` Node CLI
(`tools/ariadne-sfc-scip/`, built/vendored separately from the Cargo workspace).
Both feed the same SCIP-protobuf ingest path.

Touched crates: ariadne-core (Lang, EdgeKind), ariadne-parser (registry,
queries, injection engine, facts), ariadne-cli (lang_for_path, autodetect),
ariadne-scip (jsx/tsx ingest, bridge driver), ariadne-graph + ariadne-mcp
(component-graph surface), ariadne-e2e (framework corpus).
</architecture>

<tech_inventory>
| tech | version pinned | tier | source verified this session |
|---|---|---|---|
| tree-sitter-typescript `LANGUAGE_TSX` | =0.23.2 (existing dep) | 02 | docs.rs/tree-sitter-typescript/0.23.2 |
| tree-sitter-html | =0.23.2 | 03 | crates.io/api/v1/crates/tree-sitter-html/0.23.2/dependencies |
| tree-sitter-svelte-ng | =1.0.2 | 04 | docs.rs/tree-sitter-svelte-ng ; crates.io deps |
| tree-sitter-astro-next | =0.1.1 | 04 | crates.io/api/v1/crates/tree-sitter-astro-next/0.1.1/dependencies |
| tree-sitter injection (`included_ranges`) | tree-sitter 0.26.8 | 03 | tree-sitter.github.io/tree-sitter/3-syntax-highlighting.html |
| scip-typescript (external) | existing PATH binary | 06 | github.com/sourcegraph/scip-typescript |
| @volar/typescript `proxyCreateProgram` | bridge tool dep | 07 | deepwiki.com/vuejs/language-tools |
| @vue/language-core | bridge tool dep | 07 | npmjs.com/package/vue-tsc |
| svelte2tsx / svelte language-tools | bridge tool dep | 08 | github.com/sveltejs/language-tools |
</tech_inventory>

<risks>
| id | risk | likelihood | mitigation |
|---|---|---|---|
| R-Inject | injection-engine refactor breaks incremental-parse equivalence | medium | tier-03 keeps the tier-03(core) proptest green on multi-layer files; failure blocks the tier |
| R-Astro-ts | tree-sitter-astro-next is 0.1.x (pre-1.0 API/grammar churn) | medium | pin `=0.1.1`; tier-04 fixture-tests the exact grammar; pin bump = follow-up + ADR |
| R-Bridge | the SCIP bridge is a new Node tool — highest-risk deliverable | high | tier-07 is spike-gated: a feasibility spike precedes the driver; spike failure escalates to the user, not a silent skip |
| R-Map | Volar virtual-TS→SFC position mapping yields wrong SCIP ranges | medium | tier-07 golden-fixtures occurrence ranges against hand-checked SFC offsets |
| R-VueDir | `tree-sitter-html` host loses Vue directive *semantics* (`v-for` scope) | low | accepted: directives stay visible as attributes; component-render edges (custom-element tags) are unaffected; ADR-0011 records it |
| R-SLO | multi-region SFC parse is slower per file → cold-index SLO regression | medium | tier-09 re-runs the tier-13 SLO gate on the framework corpus; regression blocks the tier |
| R-Astro | no SCIP path for `.astro` (D11) | high | accepted + deferred; `.astro` syntactic-only this plan |
</risks>

<verification>
Feature is done when all hold:
- `ariadne index` on a real React, Solid, Vue, Svelte, and Astro repo each
  reports its lang tag(s) with non-zero symbols; `Component` symbols and
  `Renders`/`UsesHook` edges are present (tier-09 E2E).
- React/Solid SCIP ingest produces cross-file definition/reference edges;
  Vue/Svelte bridge ingest produces the same for `.vue`/`.svelte` (tier-06/07/08).
- `cargo nextest run --workspace` green; per-lang `insta` fact snapshots green.
- Incremental proptest: 100 random edit sequences on a `.vue` and a `.svelte`
  fixture — incremental tree ≡ full reparse (tier-03).
- tier-13 SLO gate re-run on the framework corpus: cold <60s, incremental p95
  <500ms, query p95 <100ms (tier-09).
- MCP `file_summary`/component-graph tool returns components + render edges on
  a fixture (tier-09 golden).
</verification>

<sources>
- tree-sitter injection / included_ranges: https://tree-sitter.github.io/tree-sitter/3-syntax-highlighting.html ; discussion: https://github.com/tree-sitter/tree-sitter/discussions/793
- tree-sitter-typescript LANGUAGE_TSX: https://docs.rs/tree-sitter-typescript/0.23.2/tree_sitter_typescript/ ; tree-sitter-html: https://crates.io/crates/tree-sitter-html ; tree-sitter-vue (dead): https://crates.io/crates/tree-sitter-vue
- tree-sitter-svelte-ng: https://docs.rs/tree-sitter-svelte-ng ; tree-sitter-astro-next: https://crates.io/crates/tree-sitter-astro-next
- scip-typescript: https://github.com/sourcegraph/scip-typescript ; @volar/typescript proxyCreateProgram / vue-tsc: https://deepwiki.com/vuejs/language-tools/7.1-vue-tsc
- svelte language-tools / svelte2tsx: https://github.com/sveltejs/language-tools ; ariadne-core plan: .claude/plans/ariadne-core/plan.md
</sources>
