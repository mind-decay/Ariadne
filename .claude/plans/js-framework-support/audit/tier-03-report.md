---
tier_id: tier-03
audited: 2026-05-21
verdict: PASS
commit: 627776868f355abdcd110c23749294cc94638f87
---

<scope>
Tier-03 — multi-region injection parse engine + Vue SFC support. Reviewed the
diff scoped to the tier `<files>`: `Cargo.toml`, `registry.rs`, `mod.rs`,
`injection.rs` (new), `incremental.rs`, `facts.rs`, `cache.rs`,
`queries/vue.scm` (new), `lib.rs`, `errors.rs`, `tests/common/mod.rs`,
`tests/real_world.rs`, `fixtures/vue/{sample,script-tsx}.vue`,
`tests/facts_vue.rs`, `tests/incremental_vue.rs`, the `facts_vue` insta
snapshot, and `docs/adr/0011`. `Cargo.lock` updated as a forced consequence of
the `tree-sitter-html` add. Tier work is uncommitted on top of HEAD `6277768`.
A prior `tier-03-report.md` existed; this audit replaces it fresh — that report
was written against an earlier build state (it claims 29 tests and
`lang="tsx"`→TypeScript, both since superseded).
</scope>

<checks_run>
- `cargo nextest run -p ariadne-parser` — 30/30 pass, incl. `facts_vue`
  (3 tests + snapshot), `incremental_vue` (100-case proptest),
  `script_lang_tsx_injects_a_tsx_layer`, `registry_supports_vue`,
  `rehydrate_returns_well_formed_parsed_file`, and the tier-03(core)
  `incremental_matches_full_reparse` JS proptest (host-only path).
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` —
  clean; proves the whole workspace (incl. `ariadne-cli`) compiles against the
  changed `extract_syntactic_facts` / `rehydrate` signatures and the new free
  `parse_file`.
- `cargo fmt --all --check` — clean.
- `cargo test --test architecture` — `architecture_invariants_hold` ok; the
  `injection` module is `pub(crate)`, no grammar type leaked.
- `cargo bench -p ariadne-parser --no-run` — builds. `benches/parse.rs` drives
  `TreeSitterParser::parse_file` directly; that method body is byte-for-byte
  unchanged by this tier, so the JS-payload cold/incremental budgets cannot
  regress (verified by code inspection — the full SLO run is the tier-09
  R-SLO gate per plan `<risks>`).
- Exit criteria 1-6 each independently verified (see `<verdict>`).
- Byte-offset spot-check on `sample.vue` (recomputed by hand from the fixture):
  `<Child>` tag-name at file-absolute (62,67); `<script>` import `'vue'` at
  (172,177). Snapshot matches — injected-layer spans are file-absolute,
  confirming the `set_included_ranges` no-remap claim.
- Manual `<verification>` scenario (`<script setup lang="ts">` merged facts)
  is covered by `merged_facts_cover_script_decls_and_template_render` +
  snapshot: `defineProps` call, `onSelect`/`props`/`count` decls, `Child`
  render — all asserted with file-absolute spans.
</checks_run>

<findings>
| id | category | severity | file:line | problem | fix |
|---|---|---|---|---|---|
| INFO-1 | docs | INFO | plan.md (203 lines) | The tier-03 build's D2 + `<architecture>` edits to `plan.md` took it from 200 to 203 lines, over the CLAUDE.md `<rules>` ≤200-line cap for plan files. Edits themselves are correct (they fix the stale `offset` parse model). | Trim ≥3 lines of prose elsewhere in `plan.md`; non-blocking, plan-file hygiene only. |
| INFO-2 | tests | INFO | injection.rs:64-86 | The multi-`<script>` collapse path — the grammar-escalation arms `(Lang::Tsx,_)`/`(Lang::TypeScript,_)` and the multi-range `set_included_ranges` call — has no fixture: both Vue fixtures carry exactly one `<script>`. Plan step 5 only required the behavior be documented (ADR-0011 does), not tested, so this is not a plan violation. | Add a two-`<script>` (`<script>` + `<script setup>`) fixture if tier-04 exercises the collapse; otherwise note as a known coverage gap. |
</findings>

<verdict>
PASS. Zero FAIL findings; two INFO findings, neither gating.

Exit criteria — all verified:
1. `ParsedFile { host: (Lang, Tree), injected: Vec<(Lang, Tree)> }` exists
   (`mod.rs:39-46`); single-grammar files degenerate to empty `injected`,
   asserted by `rehydrate_returns_well_formed_parsed_file` (Rust → empty).
2. `Lang::Vue` → `tree_sitter_html::LANGUAGE` (`registry.rs:96`), joined
   `V1_LANGS`; `parsed_file_has_html_host_and_one_injected_layer` proves a
   `.vue` fixture yields an HTML host + one injected `Lang::TypeScript` layer,
   and `script_lang_tsx_injects_a_tsx_layer` proves `lang="tsx"`→`Lang::Tsx`.
3. `extract_syntactic_facts` iterates `host` + `injected`, merges, sorts,
   dedups (`facts.rs:358-386`); the `facts_vue` snapshot shows file-absolute
   spans across both layers (template `Child` render + `<script>` decls).
4. `incremental_vue` runs `ProptestConfig::with_cases(100)`, asserting host
   S-expr equality, injected-layer count, and per-injected-layer S-expr +
   lang equality between incremental and full reparse — passes.
5. ADR-0011 written, status `Accepted`, conforms to `docs/adr/_template.md`
   (110 lines); cited from tier-03 (`<files>`, steps 5/11) and from plan.md
   (R-VueDir row, "ADR-0011 records it").
6. `nextest -p ariadne-parser`, `clippy -D warnings`, `test --test
   architecture` — all green on re-run.

Correctness checks held:
- Injection offsets are file-absolute: `parse_injected` runs the JS/TS grammar
  over the full source buffer under `set_included_ranges`; the snapshot's
  in-`<script>` spans (e.g. `'vue'` at 172) confirm no remap is needed.
- The injected parse is guarded by the same throttled wall-clock
  `PARSE_TIMEOUT` deadline as the host parse (`injection.rs:112-122`,
  `DEADLINE_SAMPLE_EVERY` shared from `incremental.rs`) — no unbounded parse.
- `vue.scm` captures every `start_tag`/`self_closing_tag` `tag_name`; the
  `facts.rs:295` capitalisation post-filter correctly drops host tags
  (`div`, `h1`, `template`) and the `script`/`style` injection-host elements,
  emitting only `Child` as a `RenderSite`.
- The merge dedup is sound: host (HTML) and injected (TS) queries operate on
  disjoint byte regions, so no cross-layer duplicate arises; `dedup` after a
  stable byte-offset sort is a harmless safety net.
- `errors.rs` `IncludedRanges` embeds `tree_sitter::IncludedRangesError` as
  `#[source]` — consistent with the pre-existing `QueryCompile`/`LanguageAssign`
  variants; the architecture invariant test stays green.
- `cache.rs::rehydrate` now returns `ParsedFile`; its only consumer is the
  not-yet-built tier-04 Salsa loader, and the workspace clippy run confirms no
  current caller breaks.
</verdict>

<next_steps>
No tier steps require redo. Tier-03 is clear to commit; tier-04 (Svelte/Astro)
may proceed. The two INFO findings are optional, non-blocking follow-ups:
- INFO-1 — trim `plan.md` back under the 200-line cap during any later plan
  edit.
- INFO-2 — add a two-`<script>` Vue fixture when tier-04 reuse makes the
  collapse path load-bearing.
</next_steps>

<sources>
- tree-sitter language injection / `set_included_ranges`:
  https://tree-sitter.github.io/tree-sitter/3-syntax-highlighting.html
- tree-sitter-html 0.23.2 ABI fit (`tree-sitter-language ^0.1`):
  https://crates.io/api/v1/crates/tree-sitter-html/0.23.2/dependencies
- Reviewer standard (code-health-over-perfection):
  https://google.github.io/eng-practices/review/reviewer/standard.html
- tier-03-injection-engine.md `<exit_criteria>`/`<steps>`/`<verification>`;
  plan.md D1/D2/D4, R-VueDir; docs/adr/0011-framework-grammars-injection.md;
  CLAUDE.md `<rules>` (≤200-line cap).
</sources>
