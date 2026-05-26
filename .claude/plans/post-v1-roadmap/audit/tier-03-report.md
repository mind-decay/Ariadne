---
tier_id: tier-03
audited: 2026-05-22
verdict: PASS
commit: d363c2a27cd0b573da8bb871811fc535d0edf5cf
---

<scope>
Audited tier-03 "Astro semantic indexing — extend the SCIP SFC bridge to
`.astro` frontmatter" of the `post-v1-roadmap` plan. Working-tree diff scoped to
the tier's `<files>`: `crates/ariadne-scip/src/indexer/scip_astro.rs` (new),
`crates/ariadne-scip/src/indexer/mod.rs`, the SFC region-extraction module
`tools/ariadne-sfc-scip/src/index.ts`, `crates/ariadne-scip/fixtures/astro/`
(new), `crates/ariadne-scip/tests/ingest_astro.rs` + its snapshot (new),
`docs/adr/0013-scip-sfc-bridge.md`. Also touched and reviewed:
`crates/ariadne-scip/src/indexer/plan.rs` and `crates/ariadne-scip/src/lib.rs`
(driver registration + façade re-export — see INFO-1). Tier-02 working-tree
changes (`ariadne-storage`, `ariadne-core/errors.rs`) are out of scope —
covered by the separate tier-02 audit (PASS).
</scope>

<checks_run>
- Read every changed file end-to-end: `scip_astro.rs`, `indexer/mod.rs`,
  `indexer/plan.rs`, `lib.rs`, `tests/ingest_plan.rs`, `tests/ingest_astro.rs`,
  the `index.ts` diff (+256 lines), `ADR-0013` diff, all `fixtures/astro/`
  sources, the committed snapshot, and `tests/common/mod.rs`.
- `cargo nextest run -p ariadne-scip` — 53/53 PASS, incl. 5 `scip_astro` unit
  tests + 3 `ingest_astro` integration tests (summary, remap-span, def→ref).
- `cargo test --test architecture` — 1/1 PASS (adapter-isolation invariant).
- `cargo clippy --workspace --all-targets -- -D warnings` — clean.
- `cargo fmt --all --check` — clean (exit 0).
- Built the bridge: `npm run build` (`tsc -p tsconfig.json`) — exit 0, the
  `index.ts` Astro path type-checks.
- Reproduced the fixture: ran `node dist/index.js --framework astro --cwd
  fixtures/astro` to a temp file — byte-identical (`cmp`) to the committed
  `fixtures/astro/index.scip` (379 bytes). The fixture is genuine,
  bridge-produced, and reproducible from the README command.
- Verified Astro frontmatter semantics against
  [docs.astro.build/en/basics/astro-components](https://docs.astro.build/en/basics/astro-components/):
  the `---` code fence delimits the component script, and the script is
  TypeScript — confirms the verbatim-slice approach is sound.
- Compared the diff to `<decisions>` RD3 and `<tech_inventory>`: no new crate,
  no new npm dependency, no smuggled tech.
</checks_run>

<findings>
| id | category | severity | location | problem | fix |
|---|---|---|---|---|---|
| INFO-1 | plan_adherence | INFO | tier-03-astro-semantic.md:18-25; `crates/ariadne-scip/src/indexer/plan.rs`, `crates/ariadne-scip/src/lib.rs` | The tier `<files>` list names `mod.rs` for "register `scip_astro`" but the diff also necessarily touches `plan.rs` (default driver-set registration) and `lib.rs` (façade re-export); neither is enumerated. | None required — both edits are minimal, correct, and mandated by step 5 ("wire the driver selection") plus the crate's re-export convention; the `<files>` list was merely under-specified. Non-blocking. |
</findings>

<verdict>
PASS. Zero FAIL findings.

Exit criteria, each independently verified:
1. *Frontmatter extracted + type-checked* — `index.ts:extractFrontmatter`
   slices the region between the leading `---` and the matching closing `---`
   verbatim into a virtual `.ts`; `indexAstro` backs it with `ts.createProgram`
   + `getTypeChecker`. The driver `ScipAstroIndexer` invokes the bridge in
   `--framework astro` mode, mirroring `ScipSvelteIndexer`. Met.
2. *Occurrences remap to original `.astro` coordinates* — `indexAstroDocument`
   shifts each occurrence line by `frontmatterStartLine` (columns unchanged,
   since the slice is byte-identical) and keeps it only when the shifted span
   exactly covers the identifier text. `astro_occurrences_remap_inside_
   frontmatter_span` asserts every occurrence lands strictly between the
   fences. Met. (SCIP uses line/char ranges, not literal byte offsets — the
   criterion's wording; the substance — remap to original source — holds.)
3. *Golden fixture yields ≥1 semantic edge* — committed
   `ingest_astro__ingest_astro_summary.snap`; `astro_frontmatter_yields_
   definition_reference_edge` proves the `heading` symbol carries both a
   definition (line 3) and a reference (line 4) occurrence. Met.
4. *Tests + architecture + clippy + fmt green* — all re-run clean (see
   `<checks_run>`). Met.

Architecture: `scip_astro.rs` is a driven adapter depending only on
`ariadne-core` (`Lang`) + crate-internal modules; `lib.rs` re-exports only;
the `architecture` invariant test passes. ADR-0013 amended with the Astro
path and the R-Astro deferral line removed (step 6). No new dependency on
the critical path — RD3 / D5 honoured. The committed fixture is provably
bridge-produced (byte-identical regeneration). Tests are realistic (ingest a
real SCIP index, no module-boundary mocks) and fail loudly.
</verdict>

<next_steps>
None — tier-03 is accepted. The single INFO is advisory: a future tier-file
edit could list `plan.rs`/`lib.rs` in `<files>` for any new SCIP driver, since
driver registration + façade re-export are always mechanically required.
</next_steps>

<sources>
- [OWASP Top 10](https://owasp.org/www-project-top-ten/) — no input-handling,
  injection, or deserialization risk found in scope.
- [Astro components / component script](https://docs.astro.build/en/basics/astro-components/)
  — `---` code fence delimits the component script; the script is TypeScript.
- `docs/adr/0013-scip-sfc-bridge.md` — SFC bridge precedent + Astro amendment.
- `crates/ariadne-scip/src/indexer/scip_svelte.rs` — the mirrored driver.
- Re-run command output captured this session (see `<checks_run>`).
</sources>
