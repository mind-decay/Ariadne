---
tier_id: tier-07
audited: 2026-05-21
verdict: PASS
commit: 71dcd5f6ba9ad65df16b3ef945cfd3a6cf344b4e
---

<scope>
Audit of `tier-07-scip-bridge-vue` — the SCIP SFC bridge: a feasibility spike,
the `tools/ariadne-sfc-scip` Volar-based Node CLI, the `ScipVueIndexer` driver,
and ADR-0013. Diff scoped to the tier `<files>` plus the build's new files.

Scoped diff (tier-07): NEW `docs/adr/0013-scip-sfc-bridge.md`;
NEW `tools/ariadne-sfc-scip/` (`package.json`, `package-lock.json`,
`tsconfig.json`, `.npmrc`, `.gitignore`, `src/index.ts`, `src/scip.ts`,
`README.md`); NEW `crates/ariadne-scip/src/indexer/scip_vue.rs`;
NEW `crates/ariadne-scip/tests/ingest_vue.rs` + `tests/fixtures/sample-vue/` +
`tests/snapshots/ingest_vue__ingest_vue_summary.snap`;
MOD `crates/ariadne-scip/src/indexer/mod.rs`, `src/indexer/plan.rs`,
`src/lib.rs`, `tests/ingest_plan.rs`. The working tree also carries
uncommitted tier-06 changes (`scip_typescript.rs`, `lang.rs`, `ingest_react*`,
CLI/core files); those were excluded — they belong to the tier-06 audit.
</scope>

<checks_run>
- Read end-to-end: `scip_vue.rs`, `index.ts`, `scip.ts`, ADR-0013, the three
  `sample-vue` SFCs, fixture/tool `package.json`/`tsconfig`/`README`, and the
  diffs of `mod.rs`, `plan.rs`, `lib.rs`, `ingest_plan.rs`. Cross-read
  `scip_typescript.rs` + `subprocess.rs` as the modelled-on baseline.
- `cargo nextest run -p ariadne-scip` — 35/35 PASS, including
  `ingest_vue::{ingest_vue_summary, vue_documents_are_attributed_not_dropped,
  cross_file_vue_definition_reference_resolves}` and
  `ingest_plan::default_driver_set_registers_eight_drivers`; all pre-existing
  ingest goldens unregressed.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` — clean.
- `cargo fmt --all --check` — clean.
- `cargo test --test architecture` — `architecture_invariants_hold` PASS.
- Manual bridge run: `npm run build` then
  `node dist/index.js --framework vue --cwd .../sample-vue --output /tmp/...`
  exited 0 and produced bytes **byte-identical** (`cmp`) to the committed
  `index.scip` — the fixture is reproducible.
- Decoded `/tmp/vue-regen.scip` (independent protobuf reader): 3 docs, 18
  occurrences. Spot-checked 5 occurrence ranges by byte offset against the SFC
  text — all exact: `buttonName` def `Button.vue:[1,13-23]`; refs
  `App.vue:[2,9-19]`, `App.vue:[4,16-26]`; `Button` import `Card.vue:[1,7-13]`;
  `buttonName` import `Card.vue:[1,17-27]`. No range lands in virtual-TS space.
- Proto field numbers in `scip.ts`/`index.ts` verified against
  `crates/ariadne-scip/proto/scip.proto` (Index 1/2, Metadata 1-4, ToolInfo
  1-3, Document relative_path=1/occurrences=2/symbols=3/language=4, Occurrence
  range=1/symbol=2/symbol_roles=3) — all correct; proto3 default-omission honored.
- ADR-0013: status `Accepted`, spike outcome `pass` recorded, bridge
  architecture + Node-CLI-on-PATH (D5) + `scip-typescript` non-vendoring
  decision + R-Bridge/R-Map mitigations all present.
</checks_run>

<findings>
| id | category | severity | location | problem | fix |
|---|---|---|---|---|---|
| F1 | correctness | INFO | `crates/ariadne-scip/src/indexer/scip_vue.rs:57-63` | The doc comment claims `"vue"` (quoted) "only occurs as the dependency key in a well-formed manifest"; a `keywords: ["vue"]` array — or any string value `"vue"` — also matches the substring scan, so the "only" is inaccurate. Behaviour is still safe: `detect` also requires a real `.vue` file and a false positive degrades gracefully. | Soften the comment to state the scan is a heuristic that can match `"vue"` anywhere in the manifest, not solely the dependency key. |
| F2 | plan_adherence | INFO | `.claude/plans/js-framework-support/tier-07-scip-bridge-vue.md:28-34` | The tier `<files>` list omits `src/indexer/plan.rs`, `src/lib.rs`, and `tests/ingest_plan.rs`, all of which the build correctly touched. Each is justified — step 5 explicitly names `plan.rs`; `lib.rs` is the mandated façade re-export; `ingest_plan.rs`'s seven→eight count change is the consequence ADR-0013 itself documents. | None required for the code; note the `<files>` list as under-specified for future tiers. |
</findings>

<verdict>
PASS. Zero FAIL findings. Every exit criterion is independently verified: the
spike outcome is recorded in ADR-0013 (Accepted); `tools/ariadne-sfc-scip`
builds a Node CLI whose emitted `index.scip` keys occurrences to `.vue` paths
with positions inside the original SFC text (5 ranges spot-checked exact, regen
byte-identical to the committed fixture); `ScipVueIndexer` implements
`ScipIndexer`, `detect` fires on a Vue project, `run` invokes the bridge and
`parse` decodes a `ScipDoc`; a cross-`.vue` definition→reference edge
(`buttonName` defined in `Button.vue`, referenced in `App.vue` and `Card.vue`)
resolves; the `ingest_vue` golden snapshot is committed and green; and
`cargo nextest run -p ariadne-scip`, `cargo clippy -D warnings`,
`cargo fmt --check`, and `cargo test --test architecture` all re-run green.
The two INFO findings do not gate. The bridge stays a subprocess on PATH, so
D5 (no Node in the `ariadne` binary) holds; `node_modules/`+`dist/` are
gitignored while `package-lock.json` (exact pins, `save-exact=true`) is tracked.
</verdict>

<next_steps>
None blocking. Optional cleanup before commit: reword the `package_declares_vue`
comment (F1) and, for plan hygiene, add `plan.rs`/`lib.rs`/`ingest_plan.rs` to
the tier `<files>` record (F2). Tier-07 is ready to commit and unblocks tier-08
(Svelte bridge), which ADR-0013 anticipates reusing this bridge shape.
</next_steps>

<sources>
- [OWASP Top 10](https://owasp.org/www-project-top-ten/) — checked subprocess
  invocation (`Command` args, no shell), JSON/`package.json` parsing, and the
  synthetic virtual entry file; no injection or deserialization exposure.
- SCIP schema field numbers: `crates/ariadne-scip/proto/scip.proto`
  (SCIP_COMMIT `99236e35450ccd8b87fe58c38d31fd499d0ffdfa`).
- `@volar/typescript` `proxyCreateProgram`: https://deepwiki.com/vuejs/language-tools/7.1-vue-tsc
- `scip-typescript`: https://github.com/sourcegraph/scip-typescript
- Reviewer standard: https://google.github.io/eng-practices/review/reviewer/standard.html
</sources>
