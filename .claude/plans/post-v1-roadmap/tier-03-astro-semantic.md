---
tier_id: tier-03
title: Astro semantic indexing — extend the SCIP SFC bridge to .astro frontmatter
deps: []
exit_criteria:
  - The SFC bridge extracts the `.astro` frontmatter TypeScript region and runs it through `scip-typescript`.
  - SCIP occurrences from the frontmatter remap back to original `.astro` byte offsets.
  - An `.astro` golden fixture yields ≥1 semantic edge (definition/reference) in an insta snapshot.
  - `cargo nextest run -p ariadne-scip` + architecture + clippy + fmt all green.
status: pending
---

<context>
js-framework v1 indexes `.astro` syntactically only — D11/R-Astro deferred semantic support because no Volar→SCIP path was verified. But the `.astro` frontmatter fence (`---`) is plain TypeScript, and the v1 SFC bridge already extracts `<script>` TS regions for Vue and Svelte, runs `scip-typescript`, and remaps offsets (ADR-0013). This tier reuses that exact path for the Astro frontmatter region (plan RD3). Full context: plan.md + docs/adr/0013-scip-sfc-bridge.md.
</context>

<files>
- crates/ariadne-scip/src/indexer/scip_astro.rs — new: Astro frontmatter SFC-bridge driver (mirrors `scip_svelte.rs`).
- crates/ariadne-scip/src/indexer/mod.rs — modify: register `scip_astro`.
- the SFC region-extraction module shared by `scip_vue.rs`/`scip_svelte.rs` — modify: add a frontmatter (`---` fence) extractor for `.astro`.
- crates/ariadne-scip/fixtures/astro/ — new: a minimal `.astro` file with a typed frontmatter import + reference.
- crates/ariadne-scip/tests/ — new/modify: `.astro` semantic golden (insta).
- docs/adr/0013-scip-sfc-bridge.md — modify: amend with the Astro frontmatter path; remove the R-Astro deferral note.
</files>

<steps>
1. Failing test first (`ariadne-scip` tests): an insta golden asserting the `.astro` fixture produces ≥1 SCIP occurrence whose offset lands inside the original frontmatter span. Red — no Astro driver exists.
2. Read `scip_svelte.rs` + the shared region-extraction module: how the `<script>` region is sliced, fed to `scip-typescript`, and how occurrence offsets are remapped [src: crates/ariadne-scip/src/indexer/scip_svelte.rs ; docs/adr/0013-scip-sfc-bridge.md].
3. Add a frontmatter extractor: the region between the leading `---` and the matching closing `---` is the TypeScript unit; the template body below is not (it stays syntactic-only). Record the region's byte offset for remapping.
4. Implement `ScipAstroIndexer` mirroring `ScipSvelteIndexer`: extract frontmatter → write a virtual `.ts` → run `scip-typescript` → decode SCIP → shift every occurrence by the frontmatter base offset.
5. Register `scip_astro` in `mod.rs`; wire the driver selection for `.astro` files to the bridge.
6. Amend ADR-0013: add the Astro frontmatter row; delete the "Astro semantic deferred — see R-Astro" line.
</steps>

<verification>
- `cargo nextest run -p ariadne-scip` — `.astro` semantic golden green; remapped offsets fall inside the frontmatter span.
- Manual: index a small real Astro project; confirm a frontmatter import resolves to its definition via `find_definition`.
- `cargo test --test architecture`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check` — green.
</verification>

<rollback>
`git checkout -- crates/ariadne-scip docs/adr/0013-scip-sfc-bridge.md`. Astro reverts to syntactic-only — the v1 behaviour.
</rollback>
