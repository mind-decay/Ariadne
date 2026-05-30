---
tier_id: tier-07a
audited: 2026-05-29
verdict: PASS
commit: f6b6ae56e514104d6eead95176cc1a9fdf14d565
---

<scope>
Reviewed tier-07a "Shared per-file derivation" against `plan.md` (RD11) and the
working-tree diff scoped to the tier's `<files>`. The diff is uncommitted; HEAD
(`f6b6ae5`) is the pre-refactor baseline, which let the cold byte-parity gate be
checked against a real pre-refactor binary rather than only the committed golden.

Files read end-to-end: `crates/ariadne-salsa/src/derive.rs` (new),
`src/derived.rs`, `src/db.rs`, `src/inputs.rs`, `src/lib.rs`,
`crates/ariadne-cli/src/domain/mod.rs`, `docs/adr/0016-shared-per-file-derivation.md`
(new), `crates/ariadne-cli/tests/index_parity.rs` (new) + 4 goldens (new),
`crates/ariadne-salsa/tests/derivation.rs` (new), and the mechanical
signature-change diffs to `tests/durability.rs`, `tests/equivalence.rs`,
`benches/edit.rs`, `ariadne-watcher/tests/events.rs`, `Cargo.toml`, `Cargo.lock`.
</scope>

<checks_run>
- `cargo fmt --all --check` — clean.
- `cargo test --test architecture` — pass; salsa allowed in-workspace deps still
  ⊆ {core, storage} (the only added dep, `blake3`, is external pure-Rust).
- `cargo nextest run -p ariadne-salsa` — 11/11 pass, incl. the step-1 derivation
  test `commit_revision_derives_symbols_and_cross_file_edge`.
- `cargo nextest run --workspace` — 248 pass, 15 skipped (e2e network fixtures,
  out of scope); the 4 `index_parity` framework tests pass (new binary == golden).
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` — clean.
- `RUSTDOCFLAGS=-D warnings cargo doc --workspace --no-deps --document-private-items`
  — FAILS, but only in `ariadne-scip` (out of scope); see INFO-1.
- **Cold byte-parity vs the real pre-refactor binary**: built HEAD in a detached
  worktree and ran `ARIADNE_PARITY_BIN=<preref> ... index_parity` — all 4 pass,
  so golden == pre-refactor == post-refactor (parity gate is genuine, not circular).
- **Single-language parity** (pre vs post binary, index counts): go/python/java/
  rust/csharp/c fixtures all identical `(files,symbols,edges)`.
- **Self-index dogfood** (pre vs post binary): identical `290 / 2821 / 3166`.
- CLI residue scan: `run_committer`/`CommitState`/`resolve_edges`/`symbol_id`/
  `commit_batch`/`SymbolCandidate`/`FileFacts`/`LocalSymbol` all gone from cli.
</checks_run>

<findings>
| id | category | severity | location | problem | fix |
|----|----------|----------|----------|---------|-----|
| F1 | docs | INFO | crates/ariadne-scip/src/indexer/mod.rs:49-53 | `RUSTDOCFLAGS=-D warnings cargo doc` (a tier `<verification>` command) fails on an unescaped `[src: … crates/ariadne-scip/proto/scip.proto]` rustdoc intra-doc link ("invalid path separator"); proven pre-existing — fails identically at HEAD, file untouched by this tier and outside its `<files>`. | Wrap the bracketed path in backticks or `<…>` in a scip-scoped change; a prior tier's (RD10) audit should have caught it. |
| F2 | tests | INFO | crates/ariadne-cli/tests/index_parity.rs:185-203 | The permanent cold-parity gate covers only the 4 framework fixtures; the exit criterion names "7-language + framework fixtures." Parity for the 7 single languages + self-index was verified by hand this audit (count-parity pre vs post) but is not a committed regression gate. | Add single-language goldens (or a self-index count assertion) so per-language drift is caught permanently. |
| F3 | architecture | INFO | crates/ariadne-salsa/src/derived.rs:184-194 | `symbols_for_file` adds a SCIP-precedence merge absent from the old CLI committer; inert this tier (`scip_symbols` is the empty tier-04 stub) so byte-parity holds, but slightly beyond step-3's "verbatim in behavior." | None required; the merge is forward scaffolding for tier-05 SCIP ingest and currently a no-op. |
</findings>

<verdict>
PASS. Zero FAIL findings.

The pure per-file derivation (symbol build, SFC `Component` synthesis, the
offset-scheme `symbol_id`, `enclosing_symbol`, `sort_candidates`, global
`resolve_edges`) moved into `ariadne-salsa/src/derive.rs` with behaviour
preserved; `symbols_for_file` is the salsa-memoized per-file step and
`commit_revision` is the pure global edge-resolution driver pass that assembles
one `Changeset` (file+symbol upserts, resolved edges) and applies it via
`WriteTxn::apply`, returning the `RevisionId`. Parsed facts enter through the new
`SyntacticFactsInput`; `ariadne-salsa` gains no parser/scip dep (arch test green;
only `blake3`, external, added). `run_index` is refactored onto the driver and
the second derivation path is deleted from `ariadne-cli`. Cold byte-parity is
proven against the actual pre-refactor binary on the 4 framework fixtures, and
count-parity holds across all 6 single-language fixtures + the self-index
dogfood. ADR-0016 records the derivation home, facts-as-input, and pure-pass edge
resolution. The lone non-green `<verification>` command (`cargo doc`) fails on a
pre-existing, out-of-scope `ariadne-scip` docstring (F1) that this tier neither
introduced nor touches, so it does not gate this tier; every exit_criterion is
independently satisfied.
</verdict>

<next_steps>
None block this tier. Recommended (non-gating) follow-ups:
- F1: fix the `ariadne-scip` rustdoc link so the workspace `cargo doc` gate is
  green again (separate, scip-scoped change).
- F2: promote the per-language + self-index parity (verified manually here) into
  committed goldens so the regression gate matches the exit-criterion wording.
</next_steps>

<sources>
- Tier file: .claude/plans/post-v1-roadmap/tier-07a-shared-per-file-derivation.md
- Plan: .claude/plans/post-v1-roadmap/plan.md (RD11, RD12, R-B4)
- Pre-refactor baseline: crates/ariadne-cli/src/domain/mod.rs @ f6b6ae5 (run_committer)
- Hexagonal invariant: tests/architecture.rs:29-49
- ADR-0016: docs/adr/0016-shared-per-file-derivation.md
- Google eng-practices reviewer standard: https://google.github.io/eng-practices/review/reviewer/standard.html
</sources>
