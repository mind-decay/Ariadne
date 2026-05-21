---
tier_id: tier-04
audited: 2026-05-21
verdict: PASS
commit: 627776868f355abdcd110c23749294cc94638f87
---

<scope>
Tier-04 — Svelte and Astro SFC parsing on the tier-03 injection engine.
Scoped diff (tier-04 `<files>` only; tier-03's `extract_syntactic_facts`
rewrite, Vue arms, ADR-0011, and shared `injection.rs`/`mod.rs`/`lib.rs`
edits belong to the already-PASSED tier-03 audit and were not re-judged):
- `Cargo.toml` — `tree-sitter-svelte-ng = "=1.0.2"`, `tree-sitter-astro-next = "=0.1.1"`.
- `registry.rs` — `Lang::Svelte`/`Lang::Astro` in `V1_LANGS` + `language_for`.
- `injection.rs` — `frontmatter_injection_plan` (Astro), Svelte sharing `script_injection_plan`.
- `facts.rs` — `query_source` arms `Lang::Svelte`/`Lang::Astro` + `QUERY_*` consts.
- `queries/svelte.scm`, `queries/astro.scm` — NEW host-layer render queries.
- `fixtures/svelte/sample.svelte`, `fixtures/astro/sample.astro` — NEW.
- `tests/facts_svelte.rs`, `tests/facts_astro.rs`, `tests/incremental_svelte.rs` — NEW.
- two NEW `insta` snapshots.
</scope>

<checks_run>
- Read every tier-04 file end-to-end; compared to `<steps>`, `<decisions>` D5/D6,
  and `<exit_criteria>`.
- `cargo nextest run -p ariadne-parser` — 38/38 pass, incl. `facts_svelte`,
  `facts_astro`, `incremental_svelte` (100 proptest cases), tier-03 Vue suite.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` — clean.
- `cargo fmt --all --check` — exit 0.
- `cargo test --test architecture` — 1/1 pass; hexagonal invariants hold.
- `cargo deny check` — advisories/bans/licenses/sources ok (only benign
  unused-license-allowance warnings).
- `Cargo.lock` — `tree-sitter-svelte-ng 1.0.2`, `tree-sitter-astro-next 0.1.1`
  resolved at the exact pins.
- Node-type claims (`script_element`/`raw_text` for svelte-ng,
  `frontmatter`/`frontmatter_js_block` for astro-next) execution-verified: a
  wrong kind would empty the injection plan or fail `Query::new`, failing the
  tests — all pass.
- `INJECTIONS_QUERY` confirmed unused in `src/` (Cargo.toml "unused" comment
  accurate; step-4 fallback choice documented in Cargo.toml + injection.rs).
</checks_run>

<findings>
| id | category | severity | location | problem | fix |
|---|---|---|---|---|---|
| F1 | tests | INFO | `fixtures/svelte/sample.svelte:8`; `tests/snapshots/facts_svelte__*.snap` | The `<verification>` "Manual" bullet says facts should list `$:` reactive decls, but `$: doubled = count * 2` is a TS `labeled_statement` the generic `typescript.scm` does not capture, so `doubled` is absent from the snapshot. | Out of tier-04 scope (`typescript.scm` is not a tier-04 file; `<context>` says "no engine change"); either capture `$:` in a follow-up tier or correct the verification text. Non-blocking — `exit_criteria` only requires "script decls", which `increment`/`count`/`title` satisfy. |
</findings>

<verdict>
PASS. Zero FAIL findings.

Exit criteria, each independently verified:
1. `Lang::Svelte`→`tree_sitter_svelte_ng::LANGUAGE`, `Lang::Astro`→
   `tree_sitter_astro_next::LANGUAGE` — `registry.rs:106-107`; `registry_supports_*`
   tests pass. ✓
2. `.svelte` fixture → Svelte host + one injected TS `<script>` layer; merged
   facts carry `increment`/`count` script decls + `<Child/>` RenderSite —
   `facts_svelte.rs` 3 tests pass, snapshot matches. ✓
3. `.astro` fixture → host + one injected TS frontmatter layer; merged facts
   carry `title`/`heading` frontmatter decls + `Layout`/`Card` RenderSites —
   `facts_astro.rs` 3 tests pass, snapshot matches. ✓
4. `extract_syntactic_facts` produces `RenderSite` facts via `svelte.scm` /
   `astro.scm`; `Component`/`HookSite` are absent only because the fixtures'
   `<script>`/frontmatter contain none — the merge path handles all three. ✓
5. Svelte incremental proptest — 100 cases, host + injected layers sexp-equal
   to full reparse, fails loud on divergence. ✓
6. nextest, clippy, architecture all green. ✓

Plan adherence: every `<files>` entry touched as intended; nothing outside the
list. Grammars pinned exact per R-Astro-ts; R-Astro-ts pin note recorded in
`facts_astro.rs` header (step 9). Step-4 `INJECTIONS_QUERY`-vs-walk choice
documented. Injection spans are file-absolute; no architectural smuggling.
</verdict>

<next_steps>
None blocking — tier-04 may commit. Optional follow-up for F1: a later tier
may add Svelte `$:` reactive-decl capture, or the tier's `<verification>` text
should be corrected to match delivered scope.
</next_steps>

<sources>
- tier-04 file `.claude/plans/js-framework-support/tier-04-svelte-astro-parser.md`
- plan `.claude/plans/js-framework-support/plan.md` D5, D6, R-Astro-ts, R-Inject
- `docs/adr/0011-framework-grammars-injection.md` (Accepted)
- tree-sitter-svelte-ng / tree-sitter-astro-next: https://docs.rs/tree-sitter-svelte-ng ; https://crates.io/crates/tree-sitter-astro-next
- node-type / API claims verified by green `cargo nextest run -p ariadne-parser`
</sources>
