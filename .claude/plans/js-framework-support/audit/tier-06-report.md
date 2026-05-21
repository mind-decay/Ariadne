---
tier_id: tier-06
audited: 2026-05-21
verdict: PASS
commit: 71dcd5f6ba9ad65df16b3ef945cfd3a6cf344b4e
---

<scope>
Audit of tier-06 (`SCIP semantic ingest for React/Solid — .jsx and .tsx via
scip-typescript`). Diff scoped to the tier's `<files>`:
- `crates/ariadne-scip/src/indexer/scip_typescript.rs` — `detect` widened to
  accept `jsconfig.json`; new `mod tests` with four `detect` unit tests.
- `crates/ariadne-scip/tests/ingest_react.rs` — NEW. Four tests over a real
  committed `scip-typescript` index.
- `crates/ariadne-scip/tests/fixtures/sample-react/` — NEW. Minimal
  license-clean React TSX/JSX fixture: `package.json`, `tsconfig.json`,
  `src/App.tsx`, `src/Button.tsx`, `src/legacy.jsx`, `index.scip`, `README.md`.
- `crates/ariadne-scip/tests/snapshots/ingest_react__ingest_react_summary.snap`
  — NEW. Accepted golden.
`normalize/mod.rs` and `normalize/grammar.rs` were listed in `<files>` but not
modified — see finding I1; the build correctly determined no change is needed.
</scope>

<checks_run>
- `cargo nextest run -p ariadne-scip` — 27 tests, 27 passed (4 new in
  `ingest_react`, 4 new `detect` unit tests).
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` —
  clean.
- `cargo fmt --all --check` — clean.
- `cargo test --test architecture` — `architecture_invariants_hold` passed
  (hexagonal boundary holds; `ariadne-scip` does not depend on `ariadne-cli`).
- Manual fixture decode: `strings index.scip` confirms 3 documents
  (`src/App.tsx`, `src/Button.tsx`, `src/legacy.jsx`), `Button` defined in
  `Button.tsx` and used in `App.tsx` — the cross-file def→ref edge the tier
  requires; `cross_file_tsx_definition_reference_resolves` exercises it green.
- `index.scip` is not gitignored (`git check-ignore` rc=1) — committable like
  the sibling `sample.scip`.
- Citation check: `ingest_react.rs:27` cites `scip.proto` line 526
  `Definition = 0x1` — verified accurate against `proto/scip.proto:526`.
- Dependency check: `tempfile` is a real `ariadne-scip` dependency, so the new
  `#[cfg(test)]` module compiles; `insta` is a dev-dependency.
</checks_run>

<findings>
| id | category | severity | file:line | problem | fix |
|----|----------|----------|-----------|---------|-----|
| I1 | tests | INFO | crates/ariadne-scip/tests/ingest_react.rs:42-49,106-121 | `lang_for_relative_path` is a test-local copy of `ariadne_cli::lang_for_path`; the `assert_eq!(lang_for_relative_path(...), Some(Lang::Tsx))` checks the copy, not production, so a future `lang_for_path` remap (e.g. `.jsx`→a new variant) would not be caught here. The copy also omits `mts/cts/mjs/cjs` arms the production function has. | Acceptable given the hexagonal rule bars depending on `ariadne-cli`; consider noting in the comment that the mirror is partial, or move the canonical extension→`Lang` map into `ariadne-core` so both sites share it. Non-blocking. |
</findings>

<verdict>
PASS. Zero FAIL findings. All five `exit_criteria` independently verified:

1. `detect` fires on React/Solid — `detect` now accepts `package.json` + (`tsconfig.json` OR `jsconfig.json`); `detect_fires_on_package_and_tsconfig`, `detect_fires_on_package_and_jsconfig`, and the two negative `detect_skips_*` tests pass. A TSX React app and a Solid app both ship `tsconfig.json`; a JS-only app with `jsconfig.json` is now covered.
2. `.tsx`/`.jsx` SCIP index ingests without error, occurrences attributed not dropped — `tsx_and_jsx_documents_are_attributed_not_dropped` parses the real `index.scip`, finds ≥1 `.tsx` and ≥1 `.jsx` document, and asserts non-empty occurrences on each.
3. `.tsx`→`Lang::Tsx`, `.jsx`→`Lang::JavaScript` — verified. The build session correctly found `ariadne-scip` has no per-file `Lang` layer (`ScipDoc` carries one index-level `lang`) and that `scip-typescript` emits no per-document `language` string to map (confirmed by fixture decode). Per-file attribution is `ariadne_cli::lang_for_path` (`crates/ariadne-cli/src/domain/mod.rs:74`), which production-correctly maps `tsx`→`Lang::Tsx` and `jsx`→`Lang::JavaScript`. The tier's "through the normalize layer" wording rests on a premise that does not match the architecture; the build's choice to not edit `normalize/mod.rs` and verify via inspection + a mirrored helper is correct, not corner-cutting (see I1).
4. Golden `insta` snapshot present and green — `ingest_react__ingest_react_summary.snap` committed shape, `ingest_react_summary` passes; no stray `.snap.new`.
5. `cargo nextest run -p ariadne-scip`, `cargo clippy ... -D warnings`, `cargo test --test architecture` — all re-run green this audit.

The fixture is license-clean (no third-party deps; `jsx: "preserve"` avoids needing a React runtime), the `index.scip` is a real `scip-typescript` artifact with a documented regeneration command, and the existing per-language ingest goldens are unregressed.
</verdict>

<next_steps>
None blocking. Tier-06 may commit. I1 is an optional follow-up: if the
extension→`Lang` map is ever centralised in `ariadne-core`, retire the
test-local `lang_for_relative_path` mirror.
</next_steps>

<sources>
- SCIP `SymbolRole.Definition = 0x1`: `crates/ariadne-scip/proto/scip.proto:526`.
- `scip-typescript` indexes `.tsx`/`.jsx` first-class: plan.md D9;
  https://github.com/sourcegraph/scip-typescript
- Hexagonal boundary (adapter crates depend only on `ariadne-core`):
  CLAUDE.md `<rules>`; tier-00 `tests/architecture.rs`.
- Production lang attribution: `crates/ariadne-cli/src/domain/mod.rs:74`
  (`lang_for_path`).
</sources>
