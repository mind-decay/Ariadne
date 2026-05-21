---
tier_id: tier-07
title: SCIP SFC bridge — feasibility spike + Vue semantic indexer
deps: [tier-01, tier-03]
exit_criteria:
  - "`docs/adr/0013-scip-sfc-bridge.md` records the spike outcome, the bridge architecture, and status Accepted (or, on spike failure, the escalation)."
  - "`tools/ariadne-sfc-scip/` builds a Node CLI that, given a Vue project root, emits an `index.scip` whose occurrences key to `.vue` source files with positions inside the original SFC."
  - "`ariadne-scip` exposes `ScipVueIndexer` implementing `ScipIndexer`; `detect` fires on a Vue project; `index` invokes the bridge CLI and returns a decoded `ScipDoc`."
  - "Ingesting the bridge output over a Vue fixture resolves a cross-component definition→reference edge (`<script setup>` import used in another `.vue`)."
  - "`crates/ariadne-scip/tests/ingest_vue.rs` golden snapshot committed and green."
  - "`cargo nextest run -p ariadne-scip`, `cargo clippy ... -D warnings`, `cargo test --test architecture` all green."
status: pending
---

<context>
No off-the-shelf SCIP indexer covers `.vue` (plan.md D10). `@volar/typescript`'s
`proxyCreateProgram()` wraps `ts.createProgram()` to make `.vue` files
program-visible — the exact mechanism `vue-tsc` uses — and Volar's `Language`
layer maps virtual-TS positions back to SFC source [src: deepwiki.com/vuejs/
language-tools — proxyCreateProgram]. `scip-typescript` already builds on
`ts.createProgram()` [src: github.com/sourcegraph/scip-typescript]. The bridge
runs `scip-typescript`'s SCIP emit over a Volar-wrapped program. It is a Node
CLI on PATH — like `scip-typescript` itself — never linked into the `ariadne`
binary (plan.md D5 holds). Highest-risk tier: spike-gated.
</context>

<files>
- `docs/adr/0013-scip-sfc-bridge.md` — NEW. Spike outcome + bridge architecture (D10).
- `tools/ariadne-sfc-scip/` — NEW. Node CLI: `package.json`, `src/index.ts`, `README.md`. Outside the Cargo workspace; built/vendored separately.
- `crates/ariadne-scip/src/indexer/scip_vue.rs` — NEW. `ScipVueIndexer` driver.
- `crates/ariadne-scip/src/indexer/mod.rs` — `mod scip_vue;` + `pub use scip_vue::ScipVueIndexer;`.
- `crates/ariadne-scip/tests/ingest_vue.rs`, `tests/fixtures/sample-vue/`, `tests/snapshots/ingest_vue__*.snap` — NEW.
</files>

<steps>
1. **Spike first** (gate the rest of the tier). In a scratch dir, build a
   3-file Vue project. Write a ~60-line Node script: `proxyCreateProgram` from
   `@volar/typescript` + the Vue language plugin from `@vue/language-core`,
   wrapped around `ts.createProgram`; walk the program emitting a minimal SCIP
   `Index` (one `Document` per `.vue`, occurrences for definitions). Decode the
   output. **Pass condition:** occurrences key to `.vue` paths with ranges that
   fall inside the original `<script>`/`<template>` text. **Fail condition:**
   ranges land in virtual-TS coordinates or `.vue` files are absent. On fail —
   stop, record the failure in ADR-0013, escalate to the user; do not silently
   downscope to syntactic-only [src: @volar/typescript proxyCreateProgram;
   github.com/sourcegraph/scip-typescript].
2. **Failing test first** (`tests/ingest_vue.rs`): ingest a committed
   `sample-vue` bridge-produced SCIP fixture; assert occurrences on `.vue`
   documents and one resolved cross-`.vue` definition→reference pair. Red.
3. Build `tools/ariadne-sfc-scip/`: a TypeScript Node CLI
   `ariadne-sfc-scip --framework vue --cwd <root> --output <out.scip>`. It
   depends on `@volar/typescript`, `@vue/language-core`, and `scip-typescript`
   (vendored or as a dependency for its SCIP-emit/`Indexer` logic — confirm
   `scip-typescript` exposes the indexer programmatically; if not, vendor the
   minimal emit path identified in the spike). It builds the Volar-wrapped
   program and writes a SCIP `Index`. Pin every npm dependency exactly;
   document the build (`npm ci && npm run build`) and the vendoring story in
   the tool's `README.md`.
4. `scip_vue.rs`: `ScipVueIndexer` implementing `ScipIndexer` — model it on
   `ScipTypescriptIndexer` [src: crates/ariadne-scip/src/indexer/scip_typescript.rs].
   `lang()` → `Lang::Vue`. `detect(root)` → `package.json` whose deps name
   `vue` plus at least one `.vue` file. `index(root)` → `run_indexer` invoking
   `ariadne-sfc-scip --framework vue` via the shared subprocess helper
   [src: crates/ariadne-scip/src/indexer/subprocess.rs]. `install_hint` →
   the bridge's install command. A missing bridge binary degrades to
   `IndexerMissing`, never a crash (the existing driver contract).
5. Register `ScipVueIndexer` in the `IngestPlan` driver set so a Vue project
   selects it [src: crates/ariadne-scip/src/indexer/plan.rs].
6. Generate `tests/fixtures/sample-vue/index.scip` once with the built bridge;
   commit it; document the command in a fixture README.
7. Accept the `insta` snapshot after manual range inspection; green.
8. Write ADR-0013: spike result, the `proxyCreateProgram` architecture, the
   Node-CLI-on-PATH placement (D5 rationale), the `scip-typescript` vendoring
   decision, and R-Bridge/R-Map mitigations.
</steps>

<verification>
- The spike pass condition is met and recorded in ADR-0013 before any other step lands.
- `cargo nextest run -p ariadne-scip` — green: `ingest_vue` snapshot + existing ingest tests unregressed.
- Manual: build `tools/ariadne-sfc-scip`, run it over a real small Vue repo, decode the SCIP — a component imported across two `.vue` files yields a resolved reference; occurrence ranges land on the real SFC text (spot-check 3 ranges by byte offset).
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo fmt --all --check`, `cargo test --test architecture` — clean (the Node tool is outside the workspace; no new Rust cross-crate edge).
</verification>

<rollback>
Delete `tools/ariadne-sfc-scip/`, `scip_vue.rs`, its `mod.rs` lines, the
`ingest_vue` test + fixture + snapshot, and the `IngestPlan` registration;
delete ADR-0013. Vue reverts to syntactic-only. No on-disk index migration.
</rollback>
