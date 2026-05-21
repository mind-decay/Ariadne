---
tier_id: tier-02
audited: 2026-05-21
verdict: PASS
commit: 8d91f705a14e97743da00022c13c560021d3f915
---

<scope>
Audit of tier-02 (JSX/TSX parsing) against
`.claude/plans/js-framework-support/tier-02-jsx-tsx-parser.md` and sibling
`plan.md`. Diff scoped to the tier's `<files>`:
- `crates/ariadne-parser/src/adapters/treesitter/registry.rs` — `Lang::Tsx` in `V1_LANGS` + `language_for` arm.
- `crates/ariadne-parser/src/adapters/treesitter/facts.rs` — `DeclKind::Component`, `RenderSite`/`HookSite`, `renders`/`hooks` fields, capture handling, component post-filter.
- `crates/ariadne-parser/src/adapters/treesitter/queries/tsx.scm` — NEW.
- `crates/ariadne-parser/src/adapters/treesitter/queries/javascript.scm` — JSX render/hook captures appended.
- `crates/ariadne-parser/src/adapters/treesitter/queries/typescript.scm` — hook capture appended.
- `crates/ariadne-parser/fixtures/{react,solid}/*.{tsx,jsx}` — 3 fixtures.
- `crates/ariadne-parser/tests/facts_tsx.rs`, `tests/facts_jsx.rs` — new tests + 3 snapshots.
- `crates/ariadne-parser/src/lib.rs` — façade re-export of `RenderSite`/`HookSite` (not in `<files>`; justified — see plan_adherence).
- 8 existing-language `.snap` files — regenerated for the two new `SyntacticFacts` fields.
`Cargo.toml` unchanged (no new dep, as the tier required).
</scope>

<checks_run>
- plan_adherence — every `<files>` entry touched as intended; out-of-list edits inspected.
- correctness — `<steps>` 1-7 traced against the diff; capture/post-filter logic walked.
- architecture — hexagonal boundary, `ariadne-core`-only dep, no tree-sitter type leak.
- tests — TDD order, behavioral asserts, loud failure messages, golden snapshots.
- exit_criteria — all five verified independently.
- `<verification>` re-run in full at HEAD `8d91f70`:
  - `cargo nextest run -p ariadne-parser` — 25 tests, 25 passed.
  - `cargo clippy --workspace --all-targets --all-features -- -D warnings` — clean.
  - `cargo fmt --all --check` — clean (exit 0).
  - `cargo test --test architecture` — 1 passed.
  - `cargo bench -p ariadne-parser --no-run` — compiles.
- Manual: snapshots cross-checked against fixture source. React `.tsx` →
  `Display`/`Counter`/`App` as `Component`, `Display`/`Counter` `RenderSite`s,
  `useState` `HookSite`; host tags (`div`/`span`/`main`/`button`) correctly
  excluded from `renders`. Solid `.tsx` → `createSignal`+`createEffect`
  `HookSite`s. `.jsx` → `Greeting`/`Panel` `Component`, `useState` `HookSite`.
  Non-JSX fixtures (js/ts/rust/etc.) gained only `renders: []`/`hooks: []` —
  no spurious facts despite the appended `@hook.callee` patterns.
</checks_run>

<findings>
| id | category | severity | location | problem | fix |
|---|---|---|---|---|---|
| F1 | correctness | INFO | `crates/ariadne-parser/src/adapters/treesitter/facts.rs:306` | The component post-filter retags only `DeclKind::Function`; an arrow-function component (`const Foo = () => <jsx/>`), captured as `DeclKind::Variable`, is never reclassified as `Component`. Idiomatic React/Solid components written that way are missed. Plan step 5 granted latitude ("pick whichever the build session proves works against the fixture") and all 3 fixtures use `function` declarations, so exit criteria are met and the choice is recorded in `tsx.scm`; this is a known coverage gap, not a plan violation. | When the component graph (tier-09) needs real-repo coverage, extend the post-filter to also retag a `Variable` decl whose `def_byte_range` encloses a JSX span. |
</findings>

<verdict>
PASS. Zero FAIL findings.

All five `exit_criteria` are independently verified:
1. `ParserRegistry::new().supports(Lang::Tsx)` is true; `language_for` maps
   `Lang::Tsx → tree_sitter_typescript::LANGUAGE_TSX` (`registry.rs:80`,
   test `registry_supports_tsx`).
2. `SyntacticFacts` carries `renders: Vec<RenderSite>` / `hooks: Vec<HookSite>`
   and `DeclKind` carries `Component`; `Default`/`Hash`/`Eq` derives preserved
   (`facts.rs:54,116,126,143-145`).
3. React `.tsx` and Solid `.tsx` snapshots each show ≥1 `Component`, ≥1
   `RenderSite`, ≥1 `HookSite`; the Solid case asserts a `createSignal` hook.
4. `.jsx` via the JavaScript grammar yields `Component`/`RenderSite`/`HookSite`
   from the JSX patterns appended to `javascript.scm`.
5. `cargo nextest run -p ariadne-parser`, `cargo clippy … -D warnings`,
   `cargo test --test architecture` all re-run green; `fmt --check` and
   `bench --no-run` also clean.

Plan adherence: all `<files>` entries touched as specified. Two edits sit
outside the literal `<files>` list, both justified — `src/lib.rs` re-exports
the new public `RenderSite`/`HookSite` types (required by the façade rule:
public types reachable from a re-exported struct's fields must be nameable),
and the 8 existing-language `.snap` files are mechanical regenerations of the
two added `SyntacticFacts` fields (verified: only `renders: []`/`hooks: []`
appended, no deletions, no behavioral drift). No new dependency, grammar, or
pattern was smuggled in; `Cargo.toml` is unchanged. Architecture boundary
holds: `RenderSite`/`HookSite` are plain domain structs, no tree-sitter type
crosses the adapter edge, and `tests/architecture.rs` passes.

The `#match?` predicate on `@hook.callee` is confirmed effective at the
pinned `tree-sitter = 0.26.8`: the snapshots list only convention-matching
callees (`useState`, `createSignal`, `createEffect`) while the full `calls`
vector still holds every call (`setCount`, `log`, `seconds`) — proof the
binding evaluates the text predicate during match iteration, as the
`tsx.scm` comment claims.
</verdict>

<next_steps>
No tier steps to redo — tier-02 is accepted. F1 is INFO and does not gate.
Carry F1 forward: when tier-09 wires the component graph against real OSS
repos, broaden the `facts.rs` post-filter to arrow/function-expression
`Variable` declarations or arrow-function components will be absent from
`Renders`/`UsesHook` edges.
</next_steps>

<sources>
- tree-sitter-typescript `LANGUAGE_TSX` (0.23.2): https://docs.rs/tree-sitter-typescript/0.23.2/tree_sitter_typescript/
- tree-sitter `QueryCursor` / text predicates (0.26.8): https://docs.rs/tree-sitter/0.26.8/tree_sitter/struct.QueryCursor.html
- Reviewer standard (code health over perfection): https://google.github.io/eng-practices/review/reviewer/standard.html
- Plan: `.claude/plans/js-framework-support/plan.md` (D2, D3, D7, D8); tier file: `tier-02-jsx-tsx-parser.md`.
</sources>
