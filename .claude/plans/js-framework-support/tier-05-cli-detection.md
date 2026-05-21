---
tier_id: tier-05
title: CLI extension mapping, framework autodetect, component-edge resolution
deps: [tier-01, tier-02, tier-03, tier-04]
exit_criteria:
  - "`lang_for_path` maps `.tsx`→`Lang::Tsx`, `.vue`→`Lang::Vue`, `.svelte`→`Lang::Svelte`, `.astro`→`Lang::Astro`; `.jsx` stays `Lang::JavaScript`."
  - "`ariadne index` on a React+TSX tree reports `tsx` in `langs` with non-zero symbols; the file is no longer parsed by the wrong grammar."
  - "`ariadne index` on a Vue, a Svelte, and an Astro tree each reports the matching lang tag with non-zero symbols and >=1 `Renders` edge."
  - "`enabled_langs` autodetect enables the framework langs when the repo carries `.vue`/`.svelte`/`.astro`/`.tsx` files or matching `package.json` deps."
  - "`cargo nextest run -p ariadne-cli`, `cargo clippy ... -D warnings`, `cargo test --test architecture` all green."
status: completed
completed: 2026-05-21
---

<context>
The parser tiers (02/03/04) produce framework facts, but the CLI cold-index
walker still skips `.vue`/`.svelte`/`.astro` (`lang_for_path` returns `None`)
and mis-routes `.tsx` to the non-TSX grammar. This tier wires extension
recognition and framework autodetect so a real `ariadne index` run exercises
the new langs end-to-end. plan.md `<architecture>`; mirrors tier-11's CLI arm.
</context>

<files>
- `crates/ariadne-cli/src/domain/mod.rs` — `lang_for_path`: re-point `.tsx`, add `.vue`/`.svelte`/`.astro`.
- `crates/ariadne-cli/src/config.rs` — `enabled_langs` autodetect recognises framework repo signals.
- `crates/ariadne-cli/tests/*` — CLI test: `ariadne index` over framework fixture trees asserts the JSON `IndexSummary`.
- `crates/ariadne-cli/fixtures/` (or reuse `ariadne-parser` fixtures via a small fixture tree) — minimal React/Vue/Svelte/Astro project trees.
</files>

<steps>
1. **Failing test first** (`crates/ariadne-cli/tests/`): run `ariadne index` over a fixture Vue project; assert the emitted `IndexSummary.langs` contains `"vue"` and `symbols > 0`. Red — `lang_for_path` returns `None` for `.vue`.
2. `lang_for_path` (`domain/mod.rs:70-83`): change the `"ts" | "tsx" | …` arm so `"tsx"` no longer maps to `Lang::TypeScript`; add `"tsx" => Lang::Tsx`. Add `"vue" => Lang::Vue`, `"svelte" => Lang::Svelte`, `"astro" => Lang::Astro`. Leave `"jsx"` on `Lang::JavaScript` (plan.md D3). Update the doc-comment's "ten grammars" count [src: crates/ariadne-cli/src/domain/mod.rs:64-83].
3. Confirm the cold-index parse path (`domain/mod.rs`) builds a `ParsedFile` (tier-03) per file and that the per-`Lang` `FactExtractor` cache still keys correctly — SFC files need the host-lang extractor *and* the injected-lang extractor. Adjust the worker's extractor cache to hold one extractor per `Lang` *encountered across all layers*, not per file lang. Verify against a Vue fixture.
4. Component-edge resolution: the cold-index edge resolver in `domain/mod.rs`
   resolves `CallSite`s to `EdgeRecord`s after the parse channel closes. Extend
   it to resolve the parser's `RenderSite`s to `EdgeRecord { kind: EdgeKind::Renders }`
   (source = the enclosing `Component` symbol, target = the rendered component
   symbol resolved by name) and `HookSite`s to `EdgeKind::UsesHook` (target =
   the hook symbol, or an unresolved-symbol record when the hook is library-
   defined). Reuse the existing name→`SymbolId` resolution; an unresolved
   render/hook target is dropped, not errored — same policy as unresolved calls
   [src: crates/ariadne-cli/src/domain/mod.rs — edge resolution].
5. `config.rs` `enabled_langs` autodetect: treat presence of `.vue`/`.svelte`/`.astro`/`.tsx` files, or a root `package.json` whose `dependencies`/`devDependencies` name `react`/`vue`/`svelte`/`astro`/`solid-js`, as an enable signal for the corresponding langs. Mirror the C/C++ autodetect arm shape [src: tier-11-c-cpp-indexing.md step 8].
6. Author the CLI tests: one fixture project tree per family (React, Vue, Svelte, Astro); each test runs `index` and asserts the `IndexSummary` lang tag, `symbols > 0`, and (for SFC families) `edges > 0` including at least one `Renders` edge.
</steps>

<verification>
- `cargo nextest run -p ariadne-cli` — green: all four framework index tests.
- Real run: `ariadne index` over a checked-out real Vue/Svelte/Astro repo →
  JSON summary lists the lang tag with non-zero `symbols`; `parse_failures` is
  not inflated by SFC files.
- Real run: `ariadne index` over a React TSX repo → `langs` contains `tsx`
  (not just `typescript`), confirming the grammar re-route.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`,
  `cargo fmt --all --check`, `cargo test --test architecture` — clean.
</verification>

<rollback>
Revert the `lang_for_path` and `config.rs` arms and the extractor-cache change;
delete the CLI tests and fixture trees. `.vue`/`.svelte`/`.astro` go back to
being skipped; `.tsx` reverts to its prior (wrong) `Lang::TypeScript` mapping.
No on-disk index migration.
</rollback>
