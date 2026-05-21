# ADR-0013: SCIP SFC bridge — Volar-based Vue semantic indexing

<status>
Accepted
Date: 2026-05-21
Decider: user / claude
</status>

<context>
The `js-framework-support` plan needs semantic (SCIP) indexing for `.vue`
single-file components, but no off-the-shelf SCIP indexer covers `.vue`
(plan.md D10). `scip-typescript` only sees `.ts`/`.tsx`/`.js`/`.jsx`; a `.vue`
file is HTML skeleton wrapping an embedded `<script>`, invisible to
`ts.createProgram`.

Forces:
- **Reliability** — a `.vue` SCIP index is worthless if occurrence ranges land
  in virtual-TypeScript coordinates instead of the original SFC text; the
  graph would point `find_references` at nonexistent source (risk R-Map).
- **Maintainability** — hand-rolling Vue→TS transpilation and source maps would
  duplicate `vue-tsc`; that machinery must be reused, not re-implemented.
- **Efficiency / process isolation** — D5 forbids a Node runtime inside the
  `ariadne` binary; any Node tooling must stay a subprocess.

This was the plan's highest-risk tier (R-Bridge), so it was spike-gated: a
feasibility spike had to pass before any driver code landed.

Spike outcome — **pass.** A ~60-line Node script wrapped `ts.createProgram`
with `@volar/typescript`'s `proxyCreateProgram` and the `@vue/language-core`
Vue language plugin over a 3-file Vue project, walked the resulting program,
and remapped identifier positions through Volar's `Language.maps`. All
`buttonName` occurrences (1 definition + 4 cross-`.vue` references) remapped to
exact `.vue` `<script>` source ranges; the type checker resolved the alias
across `.vue` files. No occurrence landed in virtual-TS coordinates. The pass
condition — occurrences keyed to `.vue` paths with ranges inside the original
SFC text — held [src: tier-07 step 1; @volar/typescript proxyCreateProgram].
</context>

<decision>
Index `.vue` semantics with a vendored Node CLI, `tools/ariadne-sfc-scip`, that
wraps `ts.createProgram` via `@volar/typescript`'s `proxyCreateProgram` plus the
`@vue/language-core` language plugin, walks every `.vue` source file through the
TypeScript type checker, remaps each occurrence from virtual-TS positions back
to the SFC source via Volar's `Language.maps`, and emits a SCIP index.
`ariadne-scip` gains a `ScipVueIndexer` driver that invokes this CLI as a
subprocess on PATH — exactly the `ScipIndexer` contract `scip-typescript` already
follows. A missing bridge binary degrades to `IndexerMissing`, never a crash.
</decision>

<rationale>
- **Reliability** — `proxyCreateProgram` is the same mechanism `vue-tsc` builds
  on, so `.vue` files become first-class program inputs and the TS checker
  resolves cross-`.vue` symbols natively. Volar's `Language.maps` owns the
  virtual-TS↔SFC source map; the bridge reuses it rather than guessing offsets,
  which is what makes occurrence ranges trustworthy
  [src: deepwiki.com/vuejs/language-tools — proxyCreateProgram].
- **Maintainability** — Volar is the canonical Vue tooling layer; pinning to it
  means Vue grammar/transpile churn is absorbed upstream, not in this repo.
- **Efficiency / isolation** — shipping the bridge as a Node CLI on PATH (built
  and vendored separately from the Cargo workspace) keeps Node out of the
  `ariadne` binary, holding D5. It reuses the existing subprocess+protobuf
  `ScipIndexer` adapter shape, so no new architectural surface
  [src: crates/ariadne-scip/src/indexer/mod.rs, scip_typescript.rs].

R-Bridge mitigation: the spike gated the tier and passed, so the driver was
built on a proven path rather than a hope. R-Map mitigation: the committed
`sample-vue` fixture's `index.scip` was decoded and all 18 occurrence ranges
verified in-bounds against the SFC text, three of them spot-checked by byte
offset, before the snapshot was accepted.

scip-typescript vendoring decision: `scip-typescript` exposes no programmatic
indexer API — it builds its own `ts.Program` from a tsconfig and cannot run
over a Volar-wrapped program. Per tier-07 step 3's stated fallback ("vendor the
minimal emit path identified in the spike"), the bridge implements a
self-contained minimal SCIP emit instead: a ~70-line hand-written protobuf
writer (`src/scip.ts`) over the field numbers in `proto/scip.proto`, plus the
checker walk. No `scip-typescript` source and no `google-protobuf` runtime are
vendored — the minimal writer is smaller and dependency-free, which the Rust
`proto::Index::decode` consumer validates end-to-end via `ingest_vue.rs`.
</rationale>

<alternatives>
- **`scip-typescript` + a tsconfig plugin** — rejected: the plugin hook is
  undocumented and fragile, and `scip-typescript`'s AST walk would still emit
  virtual-TS positions with no remap. `[src: github.com/sourcegraph/scip-typescript]`
- **Reusing `scip-typescript`'s `Indexer` as a library** — rejected: it is a
  CLI, not a library; the `Indexer` constructs its own program from a tsconfig
  and offers no seam for a pre-built Volar program. `[src: github.com/sourcegraph/scip-typescript]`
- **Vendoring `scip-typescript`'s generated `scip.ts` + `google-protobuf`** —
  rejected: a ~2k-line generated file plus a protobuf runtime, for an emit the
  bridge only needs Index/Document/Occurrence/SymbolInformation; the hand-written
  writer is the smaller, pure surface. `[src: tier-07 step 3]`
- **Syntactic-only `.vue`** — rejected by the user; loses cross-component
  definition/reference edges. `[src: plan.md D10]`
</alternatives>

<consequences>
- A new out-of-workspace build artifact, `tools/ariadne-sfc-scip/`, with its own
  pinned npm dependency set (`@volar/typescript`, `@vue/language-core`,
  `typescript`); it is built with `npm ci && npm run build` and placed on PATH,
  documented in the tool's `README.md`.
- `ariadne-scip`'s default driver set grows from seven to eight indexers; the
  `IngestPlan` registration test tracks the count.
- The bridge's SCIP symbol scheme is `scip-vue-bridge`, distinct from
  `scip-typescript`'s `scip-typescript` scheme — cross-tool symbol equality
  between a `.vue` and a `.ts` consumer is therefore not expected and is out of
  scope here.
- Pin bumps of `@volar/typescript` or `@vue/language-core` are a follow-up with
  a fixture re-generation, since Volar's mapping internals are version-coupled.
- Svelte semantic indexing (tier-08) is expected to reuse this same bridge shape
  (`svelte2tsx` in place of the Vue plugin); `.astro` semantic indexing remains
  deferred (plan.md D11, R-Astro).
</consequences>

<sources>
- `[src: .claude/plans/js-framework-support/tier-07-scip-bridge-vue.md]`
- `[src: .claude/plans/js-framework-support/plan.md D5, D10]`
- `[src: https://deepwiki.com/vuejs/language-tools/7.1-vue-tsc]`
- `[src: https://github.com/sourcegraph/scip-typescript]`
- `[src: crates/ariadne-scip/src/indexer/scip_vue.rs]`
- `[src: tools/ariadne-sfc-scip/src/index.ts]`
</sources>
