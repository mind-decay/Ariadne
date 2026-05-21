---
tier_id: tier-06
title: SCIP semantic ingest for React/Solid — .jsx and .tsx via scip-typescript
deps: [tier-01]
exit_criteria:
  - "`ScipTypescriptIndexer.detect` fires on React/Solid projects (`package.json` + `tsconfig.json`, or `jsconfig.json`)."
  - "A SCIP index produced over a `.tsx`/`.jsx` fixture project ingests without error; occurrences on `.tsx`/`.jsx` documents are attributed, not dropped."
  - "`.tsx`-originated symbols carry `Lang::Tsx` and `.jsx`-originated symbols carry `Lang::JavaScript` through the normalize layer."
  - "`crates/ariadne-scip/tests/ingest_react.rs` golden `insta` snapshot is committed and green."
  - "`cargo nextest run -p ariadne-scip`, `cargo clippy ... -D warnings`, `cargo test --test architecture` all green."
status: pending
---

<context>
React and Solid are TypeScript/JavaScript — the existing `scip-typescript`
driver indexes `.tsx`/`.jsx` files first-class because the TS compiler
(`ts.createProgram()`) treats them as part of the program (plan.md D9). This
tier confirms that path end-to-end and closes any per-file attribution gap so
`.tsx` occurrences are not silently dropped or mis-tagged. No new indexer.
Full context: plan.md D9; crates/ariadne-scip/src/indexer/scip_typescript.rs.
</context>

<files>
- `crates/ariadne-scip/src/indexer/scip_typescript.rs` — verify `detect`; confirm `.tsx`/`.jsx` documents are not filtered out.
- `crates/ariadne-scip/src/normalize/mod.rs` — per-file lang attribution: ensure a `.tsx` relative path normalizes to `Lang::Tsx`, `.jsx` to `Lang::JavaScript`.
- `crates/ariadne-scip/src/normalize/grammar.rs` — only if the SCIP symbol grammar needs a TSX-specific arm (it should not — SCIP symbols are language-agnostic; verify).
- `crates/ariadne-scip/tests/ingest_react.rs` — NEW. Golden snapshot of the ingest summary over a JSX/TSX fixture.
- `crates/ariadne-scip/tests/fixtures/sample-react/` — minimal license-clean React+TSX project with a committed `index.scip` (generated once by `scip-typescript`, like the existing `sample.scip`).
- `crates/ariadne-scip/tests/snapshots/ingest_react__*.snap` — accepted snapshot.
</files>

<steps>
1. **Failing test first** (`tests/ingest_react.rs`): ingest the committed `sample-react` SCIP fixture; assert the summary carries occurrences on both a `.tsx` and a `.jsx` document and that a cross-file definition→reference pair resolves. Red — fixture + assertions do not exist yet.
2. Inspect `scip_typescript.rs::detect`: it fires on `package.json` + `tsconfig.json` [src: crates/ariadne-scip/src/indexer/scip_typescript.rs — detect]. Confirm a TSX-only React app and a Solid app both satisfy that (they do — both ship `tsconfig.json`). If a JS-only React app with only `jsconfig.json` should also be covered, add that signal; otherwise document the limitation.
3. Trace the ingest path from `ScipDoc` through `normalize/mod.rs`: find where a document's `Lang` is decided. If it is derived from the relative path extension, ensure `.tsx`→`Lang::Tsx` and `.jsx`→`Lang::JavaScript`; if it is taken from SCIP `Document.language`, map `scip-typescript`'s value (likely `"TypeScript"`/`"TSX"`) onto the right `Lang`. Cite the exact line the build session finds.
4. Confirm `normalize/grammar.rs` needs no change — SCIP symbol descriptors are language-agnostic; add a test assertion that a TSX symbol descriptor round-trips through `normalize` unchanged rather than a new code arm.
5. Generate `tests/fixtures/sample-react/index.scip` once with a real `scip-typescript` run over the minimal fixture project; commit it (same approach as the existing `sample.scip`). Document the generating command in a fixture README.
6. Accept the `insta` snapshot after manual inspection; green.
</steps>

<verification>
- `cargo nextest run -p ariadne-scip` — green: `ingest_react` snapshot plus the existing per-lang ingest tests unregressed.
- Manual: decode the `sample-react` SCIP fixture; a `.tsx` component imported and used in another `.tsx` file produces a resolved definition→reference edge after ingest.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo fmt --all --check`, `cargo test --test architecture` — clean.
</verification>

<rollback>
Delete `tests/ingest_react.rs`, the `sample-react` fixture, and the snapshot;
revert any `normalize` attribution arm. The existing TS/JS ingest path is
untouched, so rollback is test-local.
</rollback>
