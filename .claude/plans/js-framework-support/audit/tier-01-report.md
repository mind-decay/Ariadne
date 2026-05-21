---
tier_id: tier-01
audited: 2026-05-21
verdict: PASS
commit: 8d91f705a14e97743da00022c13c560021d3f915
---

<scope>
Audit of `tier-01-domain` of the `js-framework-support` plan: pure `ariadne-core`
type additions for JS-framework support. Diff is uncommitted working-tree state
against HEAD `8d91f70` (spec-audit runs before commit; the commit gate consumes
`audit-state.json`).

Scoped files reviewed end-to-end:
- `crates/ariadne-core/src/domain/types/lang.rs` — M
- `crates/ariadne-core/src/domain/records.rs` — M (holds `EdgeKind`; the tier
  `<files>` guessed `types/*` but instructed "grep `enum EdgeKind`" / "locate")
- `crates/ariadne-core/src/lib.rs` — unchanged; façade re-exports verified
- `crates/ariadne-core/tests/tags.rs` — new
- `docs/adr/0012-component-graph-model.md` — new
- `.claude/plans/js-framework-support/{plan.md,tier-01-domain.md}` — M
  (spec-lifecycle bookkeeping: status flip + ADR citation)
Nothing outside the tier `<files>` set was touched.
</scope>

<checks_run>
- `cargo build --workspace` — green.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` — clean.
- `cargo test --test architecture` — `architecture_invariants_hold` ok; no new
  cross-crate edges.
- `cargo nextest run -p ariadne-core` — 9/9 pass, incl. the 3 new `tags` tests
  (`lang_framework_tags_round_trip`, `edge_kind_component_variants_round_trip`,
  `edge_key_carries_component_edge_kinds`).
- `cargo fmt --all --check` — clean.
- `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --document-private-items`
  — clean (all new variants carry doc comments; `#![deny(missing_docs)]` holds).
- Manual: round-trip test exercises `Lang::Vue.tag() == "vue"` and
  `Lang::from_tag("vue") == Some(Lang::Vue)` for all four new variants.
- Cross-crate claim verified: `ariadne-graph` `EdgeKind::from_core`
  (`crates/ariadne-graph/src/build.rs:70`) has a `_ => Self::Calls` wildcard —
  the new `Renders`/`UsesHook` core variants collapse onto `Calls` as ADR-0012
  states; build stays green with no graph-crate change.
- `SymbolRecord.kind` confirmed free-form `String` (`records.rs:32`); no closed
  symbol-kind enum exists, so EC3's "no change, recorded" branch is correct.

Exit criteria — all independently verified:
1. `Lang` gains `Tsx`/`Vue`/`Svelte`/`Astro`; `tag`/`from_tag` arms added; tags
   round-trip (test green). PASS
2. `EdgeKind` gains `Renders = 3`/`UsesHook = 4`; `from_byte` arms added; both
   round-trip through the byte tag and the composite `EdgeKey` 17-byte key —
   the genuine on-wire/storage-key form (`records.rs:78-79`). PASS
3. No closed symbol-kind enum; `kind` is `String`; recorded in ADR-0012
   `<decision>`. PASS
4. `docs/adr/0012-component-graph-model.md` exists, status `Accepted`, follows
   `_template.md` section set, cited from the tier `<context>` and plan.md D8. PASS
5. build / clippy / architecture test all green. PASS
</checks_run>

<findings>
| id | category | severity | location | problem | fix |
|---|---|---|---|---|---|
| 1 | docs | INFO | `.claude/plans/js-framework-support/tier-01-domain.md:79` | Stray `</output>` close tag with no matching `<output>` opener (pre-existing plan-authoring artifact, not introduced by this implementation diff; renders harmlessly). | Drop the orphan tag next time the tier file is edited. |
</findings>

<verdict>
PASS. Zero FAIL findings. Every exit criterion and every `<verification>` command
re-ran green. The diff is minimal and plan-faithful: four `Lang` variants plus
`tag`/`from_tag` arms, two `EdgeKind` variants plus `from_byte` arms, no
symbol-kind change (correctly — `kind` is a free string), an ADR matching the
template, and TDD-style round-trip tests that assert behaviour with loud
messages and exercise the real `EdgeKey` persistence path. `#[non_exhaustive]` +
`Lang::Other` mean no placeholder match arms were needed in adapter crates, so
none were added — correctly scoped per the "no work beyond the tier" rule. The
single INFO is a cosmetic pre-existing typo in the plan file and does not gate.
</verdict>

<next_steps>
No tier steps to redo. Tier-01 is sound; downstream tiers (02 parser, 05 CLI
edge resolver) may proceed against these `ariadne-core` types. Optional cleanup:
remove the orphan `</output>` tag in `tier-01-domain.md` (finding 1). Note for
plan tracking: ADR-0011 is reserved by plan `<risks>` R-VueDir for a later tier,
so the `0010 → 0012` gap on disk is intentional, not a numbering defect.
</next_steps>

<sources>
- [OWASP Top 10](https://owasp.org/www-project-top-ten/) — reviewed; no input
  validation / injection / deserialization surface in this pure-type tier.
- `docs/adr/_template.md` — ADR section structure conformance.
- `crates/ariadne-graph/src/build.rs:70` — `EdgeKind::from_core` wildcard,
  confirming ADR-0012's forward-compatibility claim.
- `crates/ariadne-core/src/domain/records.rs:32,78-79` — `SymbolRecord.kind`
  free-form `String`; `EdgeKey` byte form is the storage primary key.
</sources>
