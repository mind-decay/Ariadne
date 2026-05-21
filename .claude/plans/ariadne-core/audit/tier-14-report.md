---
tier_id: tier-14
audited: 2026-05-21
verdict: PASS
commit: 2b1a0d3f6b16eeecbe42318c845d49ad7c0bfa1d
---

<scope>
Tier-14 — analytics-quality fixes: F1 `blast_radius` empty/absent
disambiguation (`Option<BlastRadius>`), F2 `weak_spots` god-module
denoise (`is_library_target` filter + `GOD_THRESHOLD` raise).

Scoped diff (`git diff` vs HEAD `2b1a0d3`, 12 files / +278 −21):
- In `<files>`: `ariadne-graph/src/blast.rs`,
  `ariadne-graph/tests/golden_repo.rs`,
  `ariadne-graph/benches/blast.rs`, `ariadne-mcp/src/types.rs`,
  `ariadne-mcp/src/tools/blast_radius.rs`,
  `ariadne-mcp/src/tools/weak_spots.rs`,
  `ariadne-mcp/tests/support.rs`,
  `ariadne-mcp/tests/tools_weak_spots.rs`,
  `ariadne-mcp/tests/tools_blast_radius.rs`.
- Outside `<files>` but in-scope: `ariadne-graph/tests/synthetic.rs`
  and `ariadne-mcp/src/tools/doc_for.rs` — both are in-workspace
  `blast_radius` callers; step 3 mandates updating *every* caller of
  the signature change, with `cargo build --workspace` as the backstop.
  The `<files>` list under-enumerated the caller set; the changes
  themselves are minimal `.expect()` / `.unwrap_or_default()` adapters
  and are correctly justified. `plan.md` (+6 −0, tier-list addition)
  is the plan itself, not implementation. No code file outside the
  justified set was touched.
</scope>

<checks_run>
Verification gate (all re-run against the scoped diff):
- `cargo fmt --all --check` — clean (exit 0).
- `cargo build --workspace` — clean; no missed `blast_radius` caller.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
  — clean (exit 0).
- `cargo test --test architecture` — `architecture_invariants_hold`
  ok; no new cross-crate edge, no new dependency.
- `cargo nextest run --workspace` — 132 passed, 0 failed, 9 skipped.
  The three step-1/4/6 tests pass:
  `blast_radius_distinguishes_absent_from_resolved_empty`,
  `blast_radius_resolved_symbol_with_no_callers_echoes_target`,
  `weak_spots_excludes_non_library_god_modules`. Pre-existing
  `golden_blast_radius_user_struct`, `tools_blast_radius`,
  `golden_god_modules` still pass.
- `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
  --document-private-items` — clean (exit 0); new `symbol` field +
  revised `blast_radius` / `is_library_target` / `dead_symbols`
  rustdoc satisfy `#![deny(missing_docs)]`.
- `cargo bench --workspace --no-run` — clean; `benches/blast.rs`
  compiles against the `Option<BlastRadius>` signature.
- Snapshots: no `.snap` / `.snap.new` file in the diff or working
  tree. `golden_blast_radius_user_struct` and the `handshake` suite
  pass green ⇒ `golden_repo__golden_blast_radius_user_struct.snap`
  and `handshake__tools_list.snap` are content-unchanged — the output
  type is never advertised by rmcp `#[tool]`.

Dogfood (live `ariadne` MCP server, `--watch`, revision 2 — 202
files / 2032 symbols / 1889 edges):
- F1: `blast_radius` on `FactExtractor` (a zero-inbound symbol)
  returns `{"symbol":{"name":"FactExtractor","kind":"struct",
  "file":"crates/ariadne-parser/src/adapters/treesitter/facts.rs",...},
  "must_touch":[],"may_touch":[],"depth_used":0}`. The populated
  `symbol` field proves the symbol resolved; the empty touch sets
  now read as "resolved, no dependents", not "not found".
- F2: `weak_spots.god_modules` = 4 — `ariadne-cli/src/domain/mod.rs`
  (efferent 30), `ariadne-graph/src/docgen.rs` (23),
  `ariadne-graph/src/refactor.rs` (18),
  `ariadne-e2e/src/domain/mod.rs` (17). All four are library `src/`
  files; no `tests/`/`benches/` entry. Counted independently from
  `coupling_report`: before exclusion (efferent > 8, no library
  filter) = 41 modules; after `is_library_target` exclusion
  (efferent > 8) = 26 modules; after `GOD_THRESHOLD` raised to 15 = 4
  modules. The threshold raise is justified — 26 / ~155 library
  files (~17%) is noise; the post-raise tail of 4 is an actionable
  signal. The `GOD_THRESHOLD` rustdoc records 25 for the
  after-exclusion count; the live re-measurement is 26 (one-module
  drift, consistent with `--watch` re-indexing during the tier's own
  edits — immaterial to the justification, which holds at 25 or 26).
- `dead_symbols` exhibits exactly the documented syntactic-only
  false positives: a `#[test]` fn, `ariadne-scip/build.rs::main`,
  and serde-derived structs (`ScopeInput`, `FileQuery`, `EdgeKind`)
  — matching the new `WeakSpotsOutput.dead_symbols` rustdoc note.
</checks_run>

<findings>
| id | category | severity | location | problem | fix |
|----|----------|----------|----------|---------|-----|
| I1 | tests | INFO | crates/ariadne-mcp/src/tools/weak_spots.rs:29-31 | The `build.rs` arm of `is_library_target` (an exclusion target named explicitly in exit-criterion 3) has no test — `weak_spots_excludes_non_library_god_modules` exercises only the `tests/` component path, and the dogfood's only `build.rs` (`ariadne-scip/build.rs`, efferent 1) sits below threshold. A regression to that branch would pass undetected. | Add a high-efferent `build.rs`-named file to `seed_god_module_project` and assert it is excluded. |
</findings>

<verdict>
PASS. Zero FAIL findings.

All six exit criteria independently verified:
1. `GraphIndex::blast_radius` → `Option<BlastRadius>`; `None` on the
   `index` miss, `Some(_)` (radius possibly empty) for a present
   node. `blast_radius_distinguishes_absent_from_resolved_empty`
   covers both arms; failing-first by construction (`.is_none()` /
   `.expect()` will not compile against the old `BlastRadius`
   return).
2. `BlastRadiusOutput` carries `symbol: SymbolSummary`, populated via
   `summarize(cat, id)`.
   `blast_radius_resolved_symbol_with_no_callers_echoes_target`
   asserts the echoed `crate::main` plus empty `must_touch` /
   `may_touch`; the F1 dogfood confirms it end-to-end.
3. `is_library_target` excludes `tests/`/`benches/`/`examples/`
   components and `build.rs`; `weak_spots_excludes_non_library_god_modules`
   asserts a `tests/` file is dropped while a library file is kept.
   `coupling_report` is untouched (exclusion scoped to `weak_spots`).
   See I1 for the untested `build.rs` arm.
4. Dogfood recorded above; `GOD_THRESHOLD` raised 8→15 only after the
   post-exclusion count (26) proved still noisy, with the measurement
   cited in the const rustdoc. The step-6 fixture seeds efferent 18,
   correctly clearing the raised threshold.
5. `WeakSpotsOutput.dead_symbols` rustdoc records the syntactic-only
   false positives and points at the `--scip` path; `dead_code` is
   called unchanged (`DeadCodeConfig::default()`), no behavioural
   change.
6. Full gate green (see `<checks_run>`).

`blast_radius`'s `None` is handled divergently by the two MCP callers
— `blast_radius.rs` maps it to `McpError::NotFound`, `doc_for.rs` to
`unwrap_or_default()`. Both branches are unreachable
(`build_from_snapshot` adds every symbol as a node) and the
divergence is deliberate and documented in `doc_for.rs` (impact
query surfaces a desync loudly; doc query degrades gracefully). Not
a defect.
</verdict>

<next_steps>
Verdict is PASS — the tier may commit. I1 is optional and
non-blocking: if addressed, extend `seed_god_module_project` in
`crates/ariadne-mcp/tests/support.rs` with a `build.rs`-named
high-efferent file and add the exclusion assertion to
`weak_spots_excludes_non_library_god_modules`. Doing so closes the
only untested arm of exit-criterion 3's exclusion set.
</next_steps>

<sources>
- Cargo project layout (target conventions): https://doc.rust-lang.org/cargo/guide/project-layout.html
- rmcp `#[tool]` attribute (input schema only advertised): https://docs.rs/rmcp/1.7.0/rmcp/attr.tool.html
- petgraph `simple_fast` dominators: https://docs.rs/petgraph/latest/petgraph/algo/dominators/fn.simple_fast.html
- Reviewer standard (code health over perfection): https://google.github.io/eng-practices/review/reviewer/standard.html
- OWASP Top 10 (no input-handling / injection surface in this diff): https://owasp.org/www-project-top-ten/
</sources>
