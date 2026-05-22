---
tier_id: tier-01
audited: 2026-05-22
verdict: PASS
commit: 5cbb699b59fd99d836e398acaca744aeb2d05405
---

<scope>
Tier-01 — "Native Go SCIP indexer — replace the lsif-go fallback with scip-go"
(post-v1-roadmap). Reviewed the diff scoped to the tier `<files>`:

- `crates/ariadne-scip/src/indexer/scip_go.rs` — new `ScipGoIndexer`.
- `crates/ariadne-scip/src/indexer/lsif_go.rs` — deleted.
- `crates/ariadne-scip/src/indexer/mod.rs` — module/`pub use` swap.
- `crates/ariadne-scip/src/indexer/plan.rs` — `IngestPlan` default-set swap.
- `crates/ariadne-scip/src/lib.rs` — façade re-export swap.
- `crates/ariadne-scip/fixtures/go/{go.mod,demo.go}` — new fixture module.
- `crates/ariadne-scip/tests/ingest_go.rs` — rewritten Go ingest test.
- `crates/ariadne-scip/tests/snapshots/ingest_go__ingest_go_summary.snap` — deleted.
- `crates/ariadne-scip/tests/common/mod.rs` — doc comment only.
- `crates/ariadne-scip/README.md` — Go-row table update.
- `crates/ariadne-cli/src/config.rs` — `INDEXER_BINARIES` Go entry.
- `.claude/plans/ariadne-core/plan.md` — R3 marked resolved.

`config.rs`, `plan.rs`, `lib.rs`, `README.md` are not named verbatim in
`<files>` but are the mandatory footprint of removing the `LsifGoIndexer`
symbol and the `lsif` string (exit criterion 3 requires `grep lsif` empty).
All in-scope; no smuggled changes.
</scope>

<checks_run>
- `cargo nextest run -p ariadne-scip` — 44 passed, 0 failed.
- `cargo nextest run --workspace` — 196 passed (1 leaky, pre-existing), 13 skipped.
- `cargo test --test architecture` — `architecture_invariants_hold` ok.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` — clean.
- `cargo fmt --all --check` — clean.
- `grep -rn "lsif" crates/` and `grep -rn "Lsif" crates/` — both empty.
- Installed `scip-go` v0.2.6 via the implementation's hint and re-ran
  `ingest_go` — `run_over_fixture_yields_symbols` exercised the real
  subprocess (0.07s) and passed; `detect` test passed.
- End-to-end: `ariadne index --scip` on a copy of `fixtures/go` →
  `{"symbols":2,"scip_successes":["go"],"scip_missing":[]}` — a non-empty
  `IngestReport` produced via native `scip-go` (exit criterion 2).
- Verified the `scip-go` CLI surface against the installed v0.2.6
  (`scip-go index --help`): `index` subcommand exists and is the default;
  `-o/--output` default `index.scip`; `--module-path` / `--module-version`
  are shared flags [src: `scip-go index --help`, scip-go v0.2.6].
</checks_run>

<findings>
| id | category | severity | location | problem | fix |
|----|----------|----------|----------|---------|-----|
| F1 | docs | INFO | scip_go.rs:13-15,56-58,65-67 | The `--module-path` flag and its doc cite `tier-01 step 4`, but step 4 names `--module-name`; the code is correct against scip-go v0.2.6 (`--module-path`), only the citation points at a step with a different flag name. | Re-point the citation, or note the plan flag name is superseded. |
| F2 | tests | INFO | ingest_go.rs:54-60 | `run_over_fixture_yields_symbols` early-returns when `scip-go` is absent; nextest then reports it `PASS` with `0 skipped`, so a build environment without `scip-go` silently has zero coverage of the `run`/`parse` path. | Acceptable per the plan's degrade contract; consider retaining a synthesized-proto golden for offline `parse` coverage, or surface the skip. |
</findings>

<verdict>
PASS. Zero FAIL findings.

The implementation matches tier-01 `<steps>` and `<decisions>` RD1. The
`ScipGoIndexer` mirrors the `scip_python.rs` driver template (detect
predicate, `with_binary` injection point, `run` via the shared
`run_indexer` helper, `parse` via `proto::Index::decode`). All four
exit criteria are independently verified:

1. `ScipGoIndexer` implements `ScipIndexer` and detect-fires on `go.mod`
   (`detect_fires_on_go_mod`, positive + negative case).
2. `ariadne index --scip` over the Go fixture yields a non-empty
   `IngestReport` via `scip-go` (`scip_successes:["go"]`, 2 symbols) —
   verified after installing scip-go v0.2.6.
3. `lsif_go.rs` / `LsifGoIndexer` removed; `grep -rn "lsif" crates/`
   returns nothing.
4. `nextest -p ariadne-scip` + architecture invariant + workspace clippy
   + fmt all green.

Subprocess invocation is injection-safe (args passed as discrete
`OsStr`, no shell). No new dependency, no architecture violation
(arch test green). The crate's "missing indexer degrades, never
crashes" contract is preserved via the shared `run_indexer` helper.

Notable: the implementation's install hint
`go install github.com/scip-code/scip-go/cmd/scip-go@latest` diverges
from the plan (RD1/step 3 say `github.com/sourcegraph/...`). Verified
empirically — the `scip-code` path installs; the `sourcegraph` path
fails with `module declares its path as: github.com/scip-code/scip-go`.
The implementation is correct and the plan citation is stale; this is
not a finding against the diff.
</verdict>

<next_steps>
None blocking. Optional follow-ups (out of this tier's editable scope):
- Correct `post-v1-roadmap/plan.md` RD1 + `tier-01` step 3/4 citations:
  the indexer module path is `github.com/scip-code/scip-go` and the
  metadata flag is `--module-path`, not `--module-name`.
- F1/F2 are INFO — address opportunistically; neither gates commit.
</next_steps>

<sources>
- scip-go v0.2.6 CLI surface: `scip-go index --help` (installed binary).
- scip-go install path / module path: `go install` resolution error —
  `module declares its path as: github.com/scip-code/scip-go`.
- scip-go README: https://raw.githubusercontent.com/sourcegraph/scip-go/main/README.md
- Index a Go repository — Sourcegraph docs:
  https://sourcegraph.com/docs/code-navigation/how-to/index-a-go-repository
- Driver template: crates/ariadne-scip/src/indexer/scip_python.rs
- Shared spawn helper: crates/ariadne-scip/src/indexer/subprocess.rs
- Tier spec: .claude/plans/post-v1-roadmap/tier-01-go-native-scip.md
- Plan: .claude/plans/post-v1-roadmap/plan.md (RD1)
</sources>
