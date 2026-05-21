---
tier_id: tier-11
audited: 2026-05-20
verdict: FAIL
commit: 2de7c0ba7c1f084b53ef8f5c1285a9ed42db6e80
---

<scope>
Audited `tier-11-c-cpp-indexing.md` (status `completed`) against sibling
`plan.md`. Working tree carries uncommitted tier-10, tier-11, and tier-12 work
simultaneously (last commit `2de7c0b` = tier-09); the diff was scoped to
tier-11's `<files>` only.

Tier-11 `<files>` reviewed end-to-end:
- `docs/adr/0008-c-cpp-syntactic-indexing.md` (new)
- `crates/ariadne-core/src/domain/types/lang.rs` (C/Cpp variants + tag arms)
- `crates/ariadne-parser/Cargo.toml` (`tree-sitter-c`, `tree-sitter-cpp`)
- `crates/ariadne-parser/src/adapters/treesitter/registry.rs`
- `crates/ariadne-parser/src/adapters/treesitter/queries/{c,cpp}.scm` (new)
- `crates/ariadne-parser/src/adapters/treesitter/facts.rs`
- `crates/ariadne-parser/fixtures/{c/sample.c,cpp/sample.cpp}` (new)
- `crates/ariadne-parser/tests/facts_{c,cpp}.rs` (new, step-1 tests)
- `crates/ariadne-cli/src/domain/mod.rs` (`lang_for_path` C/C++ arms only)

Reconciliation note: `config.rs` is listed in `<files>` but received no edit.
Step 8's requirement — autodetect treats C/C++ source as an enable signal — is
satisfied transitively: `Config::detect` walks via `lang_for_path`, which now
maps C/C++ extensions. Verified by real run (init reported `enabled langs: c`
and `cpp`). Not a defect; no edit was needed.
</scope>

<checks_run>
- `cargo build --workspace` — green.
- `cargo test -p ariadne-parser` — green; `registry_supports_c/cpp`,
  `facts_c_sample`, `facts_cpp_sample` all pass. Query compilation against the
  pinned grammars succeeds (proves every node type in `c.scm`/`cpp.scm` exists
  in tree-sitter-c 0.24.2 / tree-sitter-cpp 0.23.4).
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` — clean.
- `cargo fmt --all --check` — clean.
- `cargo test --test architecture` — green; no new cross-crate edge.
- `cargo deny check` — `advisories ok, bans ok, licenses ok, sources ok`
  (tree-sitter-c/cpp are MIT).
- `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
  --document-private-items` — **FAILS** (see F1).
- Real run: `ariadne init` + `ariadne index` on a C tree and a C++ tree:
  C → `{"files":1,"symbols":5,"edges":1,"langs":["c"],...}`;
  C++ → `{"files":1,"symbols":6,"edges":1,"langs":["cpp"],...}`. Exit
  criterion 3 (`langs` contains `"c"`/`"cpp"`, `symbols` > 0) met.

Exit criteria 1, 2, 3, 4, and the build/clippy/architecture subset of 5 are
independently verified PASS. Criterion 5 also names `cargo doc` indirectly via
`<verification>`; that command fails.
</checks_run>

<findings>
| id | category | severity | location | problem | fix |
|----|----------|----------|----------|---------|-----|
| F1 | docs / verification | FAIL | `crates/ariadne-cli/src/commands/mod.rs:3`; `crates/ariadne-e2e/src/domain/mod.rs:3,36` | Tier-11 `<verification>` command `RUSTDOCFLAGS="-D warnings" cargo doc --workspace` fails: `[src: …/tier-10-cli-e2e.md <files>]` doc comments are parsed as broken intra-doc links (`has invalid path separator`), so `ariadne-cli` and `ariadne-e2e` fail to document. The tier is marked `completed` and its `<verification>` claims `cargo doc … — clean`; that claim is false. | Resolve tier-10's open FAIL audit — escape or restructure the `[src: …]` doc comments in the offending tier-10 files — so `cargo doc --workspace` passes. No tier-11 file is implicated. |
</findings>

<verdict>
FAIL.

Tier-11's own diff is correct and complete: the C/C++ grammar wiring, `Lang`
variants, `.scm` queries, fixtures, tests, `lang_for_path` arms, and ADR-0008
all match the plan and pass every functional exit criterion, confirmed by a
real `ariadne index` run on both a C and a C++ tree.

The verdict is FAIL solely because a tier-11 `<verification>` command —
`cargo doc --workspace` — does not pass when re-run, which the audit gate
forbids a PASS over. The failure is wholly attributable to tier-10 files
(`commands/mod.rs`, `e2e/src/domain/mod.rs`) — none of them in tier-11's
`<files>` — and tier-10 already carries an open FAIL audit verdict
(`audit-state.json`, tier-10). Tier-11 was built and marked `completed` on top
of an unresolved failing dependency; its `<verification>` "clean" claim was not
truthfully exercised.
</verdict>

<next_steps>
- No tier-11 implementation rework. Steps 1–9 are all verified correct; do not
  re-run `spec-build` against tier-11's `<files>`.
- Clear tier-10's FAIL first: fix the broken intra-doc links in
  `crates/ariadne-cli/src/commands/mod.rs` and `crates/ariadne-e2e/src/domain/mod.rs`
  (tier-10 scope) so `RUSTDOCFLAGS="-D warnings" cargo doc --workspace` is clean.
- Re-run tier-11's full `<verification>` afterward and re-audit tier-11; with
  the doc build green, all eight verification commands pass and the verdict
  flips to PASS with no code change inside tier-11's `<files>`.
</next_steps>

<sources>
- [tree-sitter-c 0.24.2 — crates.io](https://crates.io/crates/tree-sitter-c) — MIT, `LANGUAGE: LanguageFn`.
- [tree-sitter-cpp — docs.rs](https://docs.rs/tree-sitter-cpp/latest/tree_sitter_cpp/) — 0.23.4 API.
- [rustdoc lints — broken_intra_doc_links](https://doc.rust-lang.org/rustdoc/lints.html#broken_intra_doc_links) — `[text]` resolved as intra-doc link; `/` triggers `invalid path separator`.
- `.claude/plans/ariadne-core/tier-11-c-cpp-indexing.md` — tier under review.
- `.claude/plans/ariadne-core/plan.md` — D5, risk R8, `<tech_inventory>`.
- `.claude/plans/ariadne-core/audit-state.json` (pre-audit) — tier-10 verdict FAIL.
</sources>
