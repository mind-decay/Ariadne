---
tier_id: tier-01
title: Native Go SCIP indexer — replace the lsif-go fallback with scip-go
deps: []
exit_criteria:
  - A `ScipGoIndexer` implements the SCIP driver trait and detect-fires on `go.mod`.
  - `ariadne ingest-scip` over a Go fixture produces a non-empty `IngestReport` via `scip-go`.
  - `lsif_go.rs` and `LsifGoIndexer` are removed; `grep -rn "lsif" crates/` returns nothing.
  - `cargo nextest run -p ariadne-scip` + architecture invariant + clippy + fmt all green.
status: completed
completed: 2026-05-22
---

<context>
v1 ships `LsifGoIndexer` (`lsif-go` then `scip convert --from=lsif`) as the Go path because no first-party Go SCIP indexer existed — plan.md risk R3. Sourcegraph's native `scip-go` now exists. This tier swaps the two-step LSIF path for the native indexer (plan RD1). Minimal slice: one driver crate, no other tier touched. Full context: plan.md.
</context>

<files>
- crates/ariadne-scip/src/indexer/scip_go.rs — new: `ScipGoIndexer` SCIP driver.
- crates/ariadne-scip/src/indexer/mod.rs — modify: add `mod scip_go;` + `pub use`; drop `mod lsif_go;` + `LsifGoIndexer` export.
- crates/ariadne-scip/src/indexer/lsif_go.rs — delete.
- crates/ariadne-scip/fixtures/go/ — ensure a minimal Go module fixture (`go.mod` + one `.go`).
- crates/ariadne-scip/tests/ — modify the Go ingest test to drive `ScipGoIndexer`.
- the driver-selection site mapping `Lang::Go`/`go.mod` to a driver — modify to pick `ScipGoIndexer`.
- .claude/plans/ariadne-core/plan.md — modify: mark R3 resolved, citing RD1.
</files>

<steps>
1. Failing test first (`ariadne-scip` tests): assert `ScipGoIndexer` detect-fires on a dir containing `go.mod` and that `run` over the Go fixture yields ≥1 symbol. Red — the type does not exist.
2. Read `scip_python.rs` and `scip_typescript.rs` as the driver template: detect predicate, `run`, subprocess invocation, install-hint string [src: crates/ariadne-scip/src/indexer/scip_python.rs].
3. Implement `ScipGoIndexer`: detect on `go.mod`; invoke `scip-go` from repo root via the `subprocess.rs` helper; decode the emitted SCIP protobuf back through the existing `parse` path. Install hint: `go install github.com/scip-code/scip-go/cmd/scip-go@latest` — the indexer's Go module path is `github.com/scip-code/scip-go` (the `sourcegraph/...` path fails the module-path check) [src: https://github.com/scip-code/scip-go ; `go version -m` on scip-go v0.2.6].
4. `scip-go` shells out to `go` for module metadata; when `go` is absent pass `--module-path`/`--module-version` if known, else return a descriptive `ScipError` so the run degrades to syntactic-only and never crashes (tier-05 driver contract) [src: `scip-go index --help`, scip-go v0.2.6 ; crates/ariadne-scip/src/indexer/mod.rs].
5. `mod.rs`: register `scip_go`, remove `lsif_go`; delete `lsif_go.rs`.
6. Update the driver-selection mapping for `Lang::Go` to `ScipGoIndexer`.
7. Edit plan.md R3 to "resolved by post-v1-roadmap RD1".
</steps>

<verification>
- `cargo nextest run -p ariadne-scip` — all green, Go test exercises `scip-go`.
- Manual: `ariadne ingest-scip` on `golang/example`; assert symbol + relationship counts ≥ the prior lsif-go baseline (record both in the audit report).
- `grep -rn "lsif" crates/` — empty.
- `cargo test --test architecture`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check` — green.
</verification>

<rollback>
`git checkout -- crates/ariadne-scip .claude/plans/ariadne-core/plan.md`. `lsif_go.rs` is recoverable from git history if the swap is reverted.
</rollback>
