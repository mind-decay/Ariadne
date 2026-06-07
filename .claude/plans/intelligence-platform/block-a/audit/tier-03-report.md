---
tier_id: tier-03
audited: 2026-06-07
verdict: PASS
commit: d6daae8fb3bf94ba24ecdcd903910d2514ee815f
---

<scope>
A2 verdict tier — the public-surface semver classifier plus its MCP
`api_surface_diff` `#[tool]` and CLI `api-diff` surfaces. Audited the working
tree (tier-03 is built but uncommitted; HEAD is the tier-02/affected-tests
commit d6daae8). This is a RE-AUDIT: the prior report (same HEAD) returned FAIL
on F1 (a flaky e2e perf gate); the working tree has since been amended and both
prior findings are independently re-verified as resolved below. Scoped `<files>`:
- `crates/ariadne-graph/src/api_surface.rs` (new) + graph `lib.rs` re-export — `SemverBump`, `SignatureChange`, `ApiDiffReport`, pure `api_surface_diff`.
- `crates/ariadne-mcp/Cargo.toml` (+`ariadne-parser`), root `Cargo.toml`/`Cargo.lock` (workspace dep), `src/tools/api_surface_diff.rs` (new), `tools/mod.rs`, `types.rs`, `server.rs` (`#[tool]`).
- `crates/ariadne-cli/src/commands/api_diff.rs` (new) + `commands/mod.rs` + `main.rs` (`ApiDiff` subcommand).
- `docs/adr/0027-mcp-parser-dependency.md` (new).
- `crates/ariadne-e2e/tests/api_diff.rs` (new) + handshake test/snapshots (tool count 20→21).

The working tree also carries already-PASS tier-01/tier-02 changes and no other
tier-03 files; nothing out of scope changed (diffstat: 14 files, +152/-12, all
within `<files>` or the required tool-registration consequence). No stray
`*.snap.new` artifacts.
</scope>

<checks_run>
Every `<verification>` command re-run on the working tree:
- `cargo fmt --all --check` — clean, exit 0.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` — clean, no warnings/errors.
- `cargo test --test architecture` — `architecture_invariants_hold ... ok` (1 passed). Re-read the test: clause 3 (driven adapters ⊆ {core}) covers `ariadne-parser` (architecture.rs:108-119,42); clause 4 permits `ariadne-mcp → ariadne-parser` because parser ∉ DRIVING set (121-140,56); clause 5 asserts `ariadne-daemon ↛ ariadne-git` (148-154). Exit criterion 5 met and meaningfully asserted.
- `cargo nextest run --workspace` — **499 passed, 0 failed, 19 skipped** (incl. handshake insta snapshots, which fail loudly on drift). Exit criterion 1 met.
- Graph units targeted — 8/8 pass: removed→Major, added→Minor, signature-changed→Major, identical→None, same-name-different-kind, max-bump-over-mixed, added-only→Minor, sorted-output. Exit criterion 2 met.
- CLI units targeted — 3/3 pass (`parse_spec` two-ref / missing-separator / empty-side).
- E2e golden — **passes from a COLD binary** (`rm target/debug/ariadne` then `nextest -p ariadne-e2e -E 'test(api_diff)'`), and 3 warm repeats stable at ~0.20s (2.5× margin under the 500ms budget). This is the exact scenario the prior F1 failed; it no longer fails. Determinism + exact-list + budget assertions all green.
- Real run — `./target/debug/ariadne api-diff HEAD~1..HEAD --root .` prints `"verdict": "minor"` with the added-symbol list, exit 0. Exit criterion 4 (CLI leg) met.
- MCP/CLI parity (runtime, not just by-construction) — drove the freshly built `ariadne-mcp serve --root .` over stdio (initialize → tools/call `api_surface_diff {base:HEAD~1, head:HEAD}`); the returned payload is **byte-identical** to the CLI JSON (`diff` clean): same verdict `minor`, same added/removed/changed. Exit criterion 4 (parity) met.

Evidence pass:
- Read every changed file end-to-end. Re-verified each consumed tier-02/core API by source: `ariadne_git::diff(&Path,&DiffSpec) -> Result<(Vec<LineHunk>,Vec<String>),GitError>` (matches `(_hunks, changed_paths)`); `read_blobs_at(&Path,&str,&[String]) -> Result<Vec<(String,Vec<u8>)>,GitError>` (absent path skipped, sorted, owned bytes — no `gix` leak); `public_surface(Lang,&[u8]) -> Result<Vec<PublicSymbol>,ParserError>`; `Lang::from_extension(&str) -> Option<Self>`; `PublicSymbol{name,kind,visibility,signature}`; `DiffSpec::RefRange{from,to}`. No `gix`/`tree-sitter` type crosses the MCP/CLI boundary.
- Classifier: identity `(name,kind)`; removed (base∖head)→Major, added (head∖base)→Minor, both-with-differing-signature→Major; `verdict = max` over deltas via derived `Ord` (None<Patch<Minor<Major); all three lists sorted by `(name,kind)`. Inputs hold only `Visibility::Public`, so a visibility narrowing surfaces as a removal — matches plan step 2 and the Cargo SemVer taxonomy [src: https://doc.rust-lang.org/cargo/reference/semver.html].
- Determinism: no clock/RNG; BTreeMap keying + explicit sorts; the duplicate-key collapse (below) is itself deterministic. E2e asserts byte-identical re-runs and the runtime parity diff is clean.
- ADR-0027 present, status Accepted; records the `mcp → parser` driving→driven edge and the no-daemon-leg rationale, consistent with ADR-0023.
- Observation (not a finding): `(name,kind)` identity means two public symbols sharing the same `(name,kind)` in the changed set collapse to one (BTreeMap last-wins, api_surface.rs:102-105). This is the plan's explicitly chosen identity (step 2), bounded to changed files, and deterministic — plan-faithful, so it does not gate.

Prior-finding re-verification:
- **F1 (was FAIL) — RESOLVED.** `crates/ariadne-e2e/tests/api_diff.rs` now spawns once to warm the binary (line 113) and times the *second, warm* spawn (143–145), so the BR3 assertion no longer wall-clocks a cold subprocess start. Reproduced the prior cold-cache failure path and it passes; the chosen fix is one the prior report explicitly endorsed.
- **I1 (was INFO) — RESOLVED.** Both regenerated handshake snapshots now carry only `source:`/`expression:` headers (no volatile `assertion_line:`), matching the sibling-snapshot convention.
</checks_run>

<findings>
| id | category | severity | location | problem | fix |
|----|----------|----------|----------|---------|-----|
| — | — | — | — | No FAIL or actionable INFO findings. | — |
</findings>

<verdict>
PASS. All six exit criteria independently verified by execution: workspace tests
green (499/0), each verdict unit matches the SemVer taxonomy, the e2e golden
returns verdict=Major with the exact lists and is byte-identical and under budget
**from a cold binary**, the real CLI run prints a verdict at exit 0, the MCP tool
returns a byte-identical payload for the same refs (runtime parity), the
architecture invariants hold (`daemon ↛ git`, `mcp → parser` accepted), and
clippy/fmt are clean with ADR-0027 (Accepted) present. The two prior findings
(F1 flaky perf gate, I1 volatile snapshot field) are both fixed and re-verified.
Zero FAIL findings.
</verdict>

<next_steps>
None. The tier is complete and audits PASS. The work may be committed (the
audit-gate now records PASS for tier-03 at this base commit).
</next_steps>

<sources>
- `[src: .claude/plans/intelligence-platform/block-a/tier-03-api-diff.md exit_criteria/steps/verification]`
- `[src: .claude/plans/intelligence-platform/block-a/plan.md D3/D4/D6, BR3, <constraints> determinism]`
- `[src: CLAUDE.md <rules> validation-by-execution + determinism hard-fail]`
- `[src: tests/architecture.rs:40-56,108-154 — driven/driving classification, daemon-no-git clause]`
- `[src: crates/ariadne-graph/src/api_surface.rs ; crates/ariadne-mcp/src/tools/api_surface_diff.rs]`
- `[src: docs/adr/0027-mcp-parser-dependency.md ; docs/adr/0023-mcp-git-diff-dependency.md]`
- `[src: https://doc.rust-lang.org/cargo/reference/semver.html — semver taxonomy]`
- `[src: https://google.github.io/eng-practices/review/reviewer/standard.html — code-health gating bar]`
</sources>
