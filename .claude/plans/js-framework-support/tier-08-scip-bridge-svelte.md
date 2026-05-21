---
tier_id: tier-08
title: SCIP SFC bridge — Svelte semantic indexer
deps: [tier-04, tier-07]
exit_criteria:
  - "`tools/ariadne-sfc-scip/` gains a `--framework svelte` mode that emits an `index.scip` keyed to `.svelte` source files."
  - "`ariadne-scip` exposes `ScipSvelteIndexer` implementing `ScipIndexer`; `detect` fires on a Svelte project; `index` invokes the bridge CLI."
  - "Ingesting the bridge output over a Svelte fixture resolves a cross-component definition→reference edge."
  - "`crates/ariadne-scip/tests/ingest_svelte.rs` golden snapshot committed and green."
  - "ADR-0013 amended with the Svelte path; `.astro` semantic is explicitly recorded as deferred (R-Astro)."
  - "`cargo nextest run -p ariadne-scip`, `cargo clippy ... -D warnings`, `cargo test --test architecture` all green."
status: pending
---

<context>
Svelte reuses the tier-07 bridge. `svelte2tsx` transpiles `.svelte` to TSX and
the Svelte language tooling integrates with the TypeScript program the same way
Vue's does [src: github.com/sveltejs/language-tools — svelte2tsx]. This tier
adds a `--framework svelte` mode to `ariadne-sfc-scip` and a `ScipSvelteIndexer`
driver. Astro semantic is **out of scope** — no Volar→SCIP path was verified
(plan.md D11, R-Astro); `.astro` stays syntactic-only (tier-04). Full context:
plan.md D10/D11; tier-07.
</context>

<files>
- `tools/ariadne-sfc-scip/src/index.ts` — add the `svelte` framework branch (Svelte language plugin / `svelte2tsx` over the wrapped program).
- `tools/ariadne-sfc-scip/package.json` — add the Svelte tooling deps, pinned exactly.
- `crates/ariadne-scip/src/indexer/scip_svelte.rs` — NEW. `ScipSvelteIndexer` driver.
- `crates/ariadne-scip/src/indexer/mod.rs` — `mod scip_svelte;` + `pub use`.
- `crates/ariadne-scip/src/indexer/plan.rs` — register `ScipSvelteIndexer`.
- `crates/ariadne-scip/tests/ingest_svelte.rs`, `tests/fixtures/sample-svelte/`, `tests/snapshots/ingest_svelte__*.snap` — NEW.
- `docs/adr/0013-scip-sfc-bridge.md` — amend: Svelte path + Astro deferral.
</files>

<steps>
1. **Failing test first** (`tests/ingest_svelte.rs`): ingest a committed
   `sample-svelte` bridge-produced SCIP fixture; assert occurrences on
   `.svelte` documents and one resolved cross-component definition→reference
   pair. Red.
2. Extend `tools/ariadne-sfc-scip/src/index.ts`: add `--framework svelte`.
   Reuse the tier-07 `proxyCreateProgram` host; swap the Vue language plugin
   for the Svelte one (`svelte2tsx`-based). The SCIP-emit walk and the
   Volar position-mapping are unchanged from tier-07 — only the language
   plugin differs [src: github.com/sveltejs/language-tools]. If the Svelte
   tooling does not expose a Volar `LanguagePlugin` and instead only offers
   the raw `svelte2tsx` transform, fall back to: transform `.svelte`→`.tsx`
   on disk with sourcemaps, run the SCIP emit on the generated TSX, remap
   occurrence ranges through the sourcemap. The build session picks whichever
   the Svelte tooling actually supports and records it in ADR-0013.
3. Pin the new npm deps exactly in `package.json`; update the tool README.
4. `scip_svelte.rs`: `ScipSvelteIndexer` modelled on `ScipVueIndexer`
   (tier-07). `lang()` → `Lang::Svelte`. `detect(root)` → `package.json` deps
   name `svelte` plus a `.svelte` file. `index(root)` → `run_indexer` invoking
   `ariadne-sfc-scip --framework svelte`. Missing binary → `IndexerMissing`.
5. Register `ScipSvelteIndexer` in `IngestPlan`.
6. Generate and commit `tests/fixtures/sample-svelte/index.scip` with the
   built bridge; document the command.
7. Accept the `insta` snapshot after manual range inspection; green.
8. Amend ADR-0013: the Svelte branch, the chosen transform path, and an
   explicit "Astro semantic deferred — see R-Astro" line.
</steps>

<verification>
- `cargo nextest run -p ariadne-scip` — green: `ingest_svelte` snapshot + all prior ingest tests unregressed.
- Manual: build the updated `ariadne-sfc-scip`, run `--framework svelte` over a
  real small Svelte repo, decode the SCIP — a component used across files
  resolves; spot-check 3 occurrence ranges land on real `.svelte` text.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`,
  `cargo fmt --all --check`, `cargo test --test architecture` — clean.
</verification>

<rollback>
Delete `scip_svelte.rs`, its `mod.rs`/`plan.rs` lines, the `ingest_svelte`
test + fixture + snapshot, and the `svelte` branch in `ariadne-sfc-scip`;
revert the ADR-0013 amendment. Svelte reverts to syntactic-only; the tier-07
Vue bridge is untouched. No on-disk index migration.
</rollback>
