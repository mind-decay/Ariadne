---
tier_id: tier-01
title: ariadne-core domain — Lang variants + component-graph EdgeKind
deps: []
exit_criteria:
  - "`Lang` gains `Tsx`, `Vue`, `Svelte`, `Astro`; tags `tsx`/`vue`/`svelte`/`astro` round-trip through `tag`/`from_tag`."
  - "`EdgeKind` gains `Renders` and `UsesHook` variants; both round-trip through whatever on-wire form `EdgeKind` already uses."
  - "Any closed symbol-kind enum in ariadne-core gains a `Component` variant; if symbol kind is a free string, no change and that is recorded."
  - "`docs/adr/0012-component-graph-model.md` written, status Accepted, cited from this tier + plan.md."
  - "`cargo build --workspace`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo test --test architecture` all green."
status: pending
---

<context>
Foundational domain tier. Every later tier depends on these `ariadne-core`
types existing. No parsing, no IO — pure type additions plus the exhaustive-
match arms the compiler then demands across the workspace. Full context:
plan.md `<decisions>` D2, D8; `<architecture>`.
</context>

<files>
- `crates/ariadne-core/src/domain/types/lang.rs` — add `Tsx`, `Vue`, `Svelte`, `Astro` to the `#[non_exhaustive] enum Lang`; `tag`/`from_tag` arms.
- `crates/ariadne-core/src/domain/types/*` — locate the `EdgeKind` enum (grep `enum EdgeKind`); add `Renders` + `UsesHook` variants and their (de)serialization arms.
- `crates/ariadne-core/src/domain/types/*` — locate any symbol-kind enum; add `Component` if it is a closed enum.
- `crates/ariadne-core/src/lib.rs` — confirm new variants are re-exported via the existing façade (no new `pub use` needed if `Lang`/`EdgeKind` are already exported).
- `crates/ariadne-core/tests/*` — extend the existing `Lang`/`EdgeKind` round-trip test (or add one) with the new variants.
- `docs/adr/0012-component-graph-model.md` — NEW. Decision record for the component-graph entity model.
</files>

<steps>
1. **Failing test first**: in `ariadne-core`'s test module assert
   `Lang::from_tag("tsx") == Some(Lang::Tsx)` (and `vue`/`svelte`/`astro`), and
   that an `EdgeKind::Renders` value round-trips through its serde form.
   Red — the variants do not yet exist.
2. `lang.rs`: add `Tsx`, `Vue`, `Svelte`, `Astro` to `enum Lang`. Extend `tag`
   with `Self::Tsx => "tsx"`, `Vue => "vue"`, `Svelte => "svelte"`,
   `Astro => "astro"`; add the inverse arms to `from_tag` before the
   `other:` fallthrough [src: crates/ariadne-core/src/domain/types/lang.rs:42-82].
3. Locate `EdgeKind` (it is re-exported and consumed in
   `crates/ariadne-cli/src/domain/mod.rs`). Add `Renders` (a component renders
   a child component) and `UsesHook` (a component uses a hook / reactive
   primitive). Mirror the existing variants' serde / tag handling exactly —
   match the pattern already in the file, do not invent a new encoding.
4. Locate the symbol-kind representation (`SymbolRecord`'s kind field). If it
   is a closed enum, add `Component`. If it is a free-form string, add nothing
   and note in ADR-0012 that components are tagged via the string `"component"`.
5. The compiler now flags every exhaustive `match` on `Lang`/`EdgeKind`/symbol
   kind across the workspace. For `Lang` matches that are grammar tables
   (`ParserRegistry`, `query_source`, `lang_for_path`) add a `_ => …` or
   explicit arms only where a later tier owns them — for *this* tier, add the
   minimal arm that keeps the build green without inventing behaviour (e.g.
   `Lang::Tsx | Lang::Vue | Lang::Svelte | Lang::Astro => return None/Err`),
   leaving a `// tier-NN wires this` comment. Real behaviour lands in tier-02/03/04.
6. Write `docs/adr/0012-component-graph-model.md` per `docs/adr/_template.md`:
   decision = `DeclKind::Component` (parser, tier-02) + `EdgeKind::Renders` /
   `UsesHook` (core, here); rationale = components/hooks become first-class so
   blast-radius and coupling traverse them unchanged; rejected = modelling
   components as plain functions (UI structure invisible to analytics).
7. Extend the round-trip test to cover all four langs and both edge kinds; green.
</steps>

<verification>
- `cargo nextest run -p ariadne-core` — green, including the new round-trip cases.
- `cargo build --workspace` — green; the placeholder match arms compile.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` — clean.
- `cargo test --test architecture` — green (no new cross-crate edges).
- `cargo fmt --all --check` and `RUSTDOCFLAGS=-D warnings cargo doc --workspace --no-deps --document-private-items` — clean.
- Manual: `Lang::Vue.tag()` is `"vue"`; `Lang::from_tag("vue")` is `Some(Lang::Vue)`.
</verification>

<rollback>
Revert the `lang.rs` / `EdgeKind` / symbol-kind variants and the placeholder
match arms; delete `docs/adr/0012-component-graph-model.md`. No on-disk index
migration — an unused `Lang`/`EdgeKind` tag simply never appears.
</rollback>
