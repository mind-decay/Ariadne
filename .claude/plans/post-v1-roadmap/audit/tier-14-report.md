---
tier_id: tier-14
audited: 2026-06-02
verdict: PASS
commit: 5bc5136ccd90032e64d8a7fb7f90d62ba2c48f73
---

<scope>
Tier-14 "Diff-aware blast radius". Reviewed the working-tree diff scoped to the
tier's `<files>` plus build-created files. Changed/new under review:
- `crates/ariadne-core/src/domain/records.rs` (+`DiffSpec`), `…/lib.rs` (re-export).
- `crates/ariadne-git/Cargo.toml` (gix `status` feature), `…/adapters/gix/diff.rs`
  (new), `…/adapters/gix/mod.rs` (declare/expose `diff`), `…/line_hunks.rs`
  (3 fns → `pub(super)`), `…/errors.rs` (+`Revspec`), `…/lib.rs` (re-export `diff`).
- `crates/ariadne-graph/src/span_lines.rs` (new shared resolver), `…/symbol_churn.rs`
  (consume it), `…/diff_blast.rs` (new use case), `…/lib.rs` (declare/re-export).
- `crates/ariadne-graph/tests/diff_blast.rs` + snapshot, `crates/ariadne-git/tests/diff.rs`.
- `docs/adr/0022-diff-aware-blast-radius.md`, `plan.md` tech_inventory row, Cargo.lock.
Treated as third-party work; every changed file read end-to-end; all
`<verification>` commands re-run fresh (cache-busted where a stale hit was possible).
</scope>

<checks_run>
- `cargo nextest run -p ariadne-graph -p ariadne-git` → **54 passed, 0 failed**.
  Includes the three-kind fixture-repo diff (Commit/RefRange/WorkingTree +
  clean-repo) and the diff-blast union-equality/unresolved/determinism/snapshot;
  tier-11b symbol-churn goldens still green after the `span_lines` extraction.
- `cargo test --test architecture` → **1 passed** (ariadne-git deps ⊆ {core}; the
  symbol join stays in ariadne-graph; no new crate/dep edge).
- `cargo clippy -p ariadne-graph -p ariadne-git --all-targets --all-features -- -D warnings`
  → **clean** (fresh build in throwaway target dir, not a cache hit).
- `cargo fmt --all --check` → **clean**.
- `RUSTDOCFLAGS="-D warnings" cargo doc -p ariadne-graph -p ariadne-git --no-deps`
  → **clean** (fresh build; gix-status v0.31.0 / gix-dir v0.26.0 compiled).
- Snapshot `diff_blast__diff_blast_two_chains.snap` re-derived by hand against the
  test graph's dominator structure — matches exactly.
- Cargo.lock additions from the `status` feature: only `gix-status` + `gix-dir`,
  both pure-Rust; grep for openssl/curl/reqwest/libgit2/git2/native-tls/hyper → NONE.
  Pure-Rust critical-path claim (D3/plan D5) holds.
- Probed clippy with the `#[allow(clippy::too_many_arguments)]` removed → fails
  "too many arguments (6/5)" (threshold 5 in `clippy.toml`, self counted), so the
  allow is *required*, not dead. File restored byte-identical.

<exit_criteria_check>
1. Pure `diff_blast` maps a changeset's hunks+spans → seeds, runs v1 `blast_radius`
   per seed, returns deduped must/may union, insta-pinned — `diff_blast.rs:81-138`,
   snapshot present. ✓
2. must∪may == union over seeds of v1 `blast_radius`(must∪may), both from one
   `GraphIndex` — asserted `diff_blast.rs` test:107-133 (lhs==rhs). Algebraically
   final must∪may = must_union∪may_union holds for any seed overlap. ✓
3. ariadne-git emits per-path LineHunks + changed-path list for WorkingTree/Commit/
   RefRange, symbol-agnostic — `gix/diff.rs`, all three tested; arch test green. ✓
4. A changed file with no resolved symbol → `unresolved`, never dropped —
   `diff_blast.rs:119-130`, test:146-150. ✓
5. ADR-0022 records the boundary, union semantics, `status` feature + pure-Rust
   justification — present, `<decision>`/`<rationale>` cover all three. ✓
6. nextest + architecture + clippy + fmt all green — re-run, all green. ✓
</exit_criteria_check>
</checks_run>

<findings>
| id | category | severity | location | problem | fix |
|----|----------|----------|----------|---------|-----|
| F1 | plan_adherence | INFO | `crates/ariadne-git/src/{errors.rs,lib.rs,adapters/gix/line_hunks.rs}` | Three git-crate files were modified but are not in the tier's `<files>` manifest (the `Revspec` variant, the `diff` re-export, and the `pub(super)` bump of `collect_change_hunks`/`blob_bytes`/`push_new_side_hunks`). | None needed — each is mandated by steps 4/5/7 and is minimal; the edits are correct, only the `<files>` list under-enumerated them. |
| F2 | tests | INFO | `crates/ariadne-git/src/adapters/gix/diff.rs:122-123` | The `WorkingTree` head-vs-index leg (`head_tree(...)`, staged-only changes) is wired but only the index-vs-worktree leg (unstaged edit) and the clean case are exercised; a staged-but-not-worktree change has no direct test. | Optionally add a `git add`-then-assert case; plan step 6 only required the uncommitted worktree edit, so this is non-blocking. |
</findings>

<verdict>
**PASS.** Zero FAIL findings. All six exit criteria independently verified; all
five `<verification>` commands re-run green (clippy/doc confirmed on fresh
builds, not stale cache). The hexagonal boundary holds: `DiffSpec` carries only
`String` revspecs (no gix type in `ariadne-core`), the git adapter returns
`(Vec<LineHunk>, Vec<String>)` with no gix leak, and the symbol join + blast
union live in `ariadne-graph` — architecture test green, deps ⊆ {core} for both.
The `span_lines` extraction is behaviour-preserving (tier-11b goldens green). The
`status` feature adds only pure-Rust crates. Union semantics, determinism (every
output sorted), and the no-silent-drop `unresolved` contract are all correct and
hand-verified. The two INFO items are non-blocking and do not gate the verdict.
</verdict>

<next_steps>
None required for PASS. The two INFO items are optional polish: (F1) widen the
tier's `<files>` manifest to list the three git-crate edits next time the plan is
revised; (F2) add a staged-change case to `tests/diff.rs` if the head-vs-index
leg warrants explicit coverage. MCP/daemon exposure of `diff_blast` remains
deferred to tier-15 per ADR-0022, as planned.
</next_steps>

<sources>
- [src: .claude/plans/post-v1-roadmap/tier-14-diff-aware-blast-radius.md] — tier spec
- [src: .claude/plans/post-v1-roadmap/plan.md RD7] — gix adapter + diff-blast block
- [src: crates/ariadne-graph/src/blast.rs:64-96] — v1 blast_radius reused per seed
- [src: docs/adr/0022-diff-aware-blast-radius.md] — boundary / union / status feature
- [src: clippy.toml] — `too-many-arguments-threshold = 5` (probe-confirmed)
- [src: https://docs.rs/gix/0.84.0/gix/status/index.html] — WorkingTree status API
- [src: https://docs.rs/gix/0.84.0/gix/struct.Repository.html] — diff_tree_to_tree
- [src: https://google.github.io/eng-practices/review/reviewer/standard.html] — INFO bar
</sources>
