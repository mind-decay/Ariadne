---
tier_id: tier-08
audited: 2026-05-21
verdict: PASS
commit: 1acd8382509db8c9c5cb3546474c2e946f928096
---

<scope>
Tier-08 — SCIP SFC bridge, Svelte semantic indexer. Reviewed the diff scoped to
the tier `<files>`: `tools/ariadne-sfc-scip/src/index.ts`, `.../package.json`,
`crates/ariadne-scip/src/indexer/scip_svelte.rs` (new), `.../indexer/mod.rs`,
`.../indexer/plan.rs`, `crates/ariadne-scip/tests/ingest_svelte.rs` (new),
`tests/fixtures/sample-svelte/` (new), the `ingest_svelte` snapshot (new),
`docs/adr/0013-scip-sfc-bridge.md`. Out-of-`<files>` edits also reviewed:
`crates/ariadne-scip/src/lib.rs`, `tests/ingest_plan.rs`,
`tools/ariadne-sfc-scip/README.md`, `package-lock.json` — see plan_adherence.
</scope>

<checks_run>
- `cargo nextest run -p ariadne-scip` — 43/43 pass, incl. 5 `scip_svelte` unit
  tests, `ingest_svelte_summary`, `svelte_documents_are_attributed_not_dropped`,
  `cross_file_svelte_definition_reference_resolves`, and the renamed
  `default_driver_set_registers_nine_drivers`. No prior ingest test regressed.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` — clean.
- `cargo fmt --all --check` — clean.
- `cargo test --test architecture` — `architecture_invariants_hold` ok.
- Bridge build: `npm run build` (`tsc -p tsconfig.json`) — clean compile.
- Reproducibility: ran the bridge `--framework svelte` over `sample-svelte`;
  output is **byte-identical** to the committed `index.scip`.
- Manual range spot-check: decoded all 3 documents / 16 occurrences from the
  committed SCIP. Every range is single-line, in-bounds, and the source slice
  exactly equals the identifier text. `buttonName` is a Definition in
  `Button.svelte [1:15-1:25]` and a reference in both `App.svelte` and
  `Card.svelte` under one global symbol — the cross-component edge resolves.
- Read every changed file end-to-end; compared to `<decisions>` D5/D10/D11 and
  all six `exit_criteria`.
</checks_run>

<findings>
| id | category | severity | location | problem | fix |
|----|----------|----------|----------|---------|-----|
| — | — | — | — | No FAIL or INFO findings. | — |

Plan-adherence note (not a finding): `lib.rs`, `tests/ingest_plan.rs`,
`README.md`, and `package-lock.json` are touched but absent from the tier
`<files>` list. Each is a necessary consequence of a `<steps>` item — `lib.rs`
completes the façade re-export that `mod.rs`'s `pub use` (step 4) implies;
`ingest_plan.rs`'s driver-count test must change because step 5 registers a 9th
driver, and the exit criterion requires the suite green; the README update is
step 3 verbatim; `package-lock.json` follows the `package.json` dep add. No
smuggled dependency, tech, or pattern. All justified.

Behavioral observation (not a finding): Svelte component-default imports
(`Card`, `Button`) resolve to document-`local` symbols, where the Vue path
resolves them to cross-file globals. This is an accepted consequence of D10's
documented `svelte2tsx` fallback (no Volar `LanguagePlugin`); the plan requires
only one cross-component def→ref edge, satisfied by `buttonName`. The remap's
failure mode is always drop-the-occurrence, never emit a wrong range — confirmed
by the 16/16 in-bounds decode.
</findings>

<verdict>
PASS. All six `exit_criteria` independently verified:
1. `--framework svelte` mode emits `index.scip` keyed to `.svelte` sources —
   verified by byte-exact regeneration and the 16-occurrence range decode.
2. `ScipSvelteIndexer` implements `ScipIndexer`; `detect` fires on a Svelte
   project (`package.json` `"svelte"` token + a `.svelte` file); `run` invokes
   the bridge with `--framework svelte` — `scip_svelte.rs:101-135`, 5 unit tests.
3. Cross-component definition→reference edge resolves — `buttonName` defined in
   `Button.svelte`, referenced in `App.svelte` and `Card.svelte`.
4. `ingest_svelte.rs` golden + `ingest_svelte__ingest_svelte_summary.snap`
   committed and green.
5. ADR-0013 amended with the Svelte transform path and an explicit Astro-
   deferred (R-Astro) line; 166 lines (≤200).
6. `cargo nextest run -p ariadne-scip`, `cargo clippy -D warnings`,
   `cargo test --test architecture` — all re-run green; `cargo fmt` clean.
Architecture intact: `ariadne-scip` driven-adapter pattern preserved, no new
Rust deps, bridge stays a vendored Node CLI outside the Cargo workspace and the
`ariadne` binary (D5/D10). Source-map decode is hand-written, no new npm dep
beyond the pinned `svelte 5.55.9` / `svelte2tsx 0.7.55` pair.
</verdict>

<next_steps>
None. Tier-08 is accepted. Proceed to tier-09 (component-graph E2E).
</next_steps>

<sources>
- [OWASP Top 10](https://owasp.org/www-project-top-ten/) — input handling: the
  bridge consumes only project-local SFC sources via a subprocess; no
  injection/deserialization surface introduced.
- ECMA-426 Source Map (Base64-VLQ / `mappings`): the hand-written decoder
  matches the spec; verified empirically by the byte-exact 16/16 range decode.
- `crates/ariadne-scip/src/indexer/scip_vue.rs` — sibling driver the Svelte
  driver mirrors.
- `docs/adr/0013-scip-sfc-bridge.md` — amended decision record.
</sources>
