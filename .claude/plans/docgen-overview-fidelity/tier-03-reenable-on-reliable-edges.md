---
tier_id: tier-03
title: Re-enable boundary/Role/cycle sections on reliable edges; SymbolId dedup
deps: [tier-01, tier-02]
exit_criteria:
  - "Boundary violations section un-suppressed; dedup keyed on (src SymbolId, dst SymbolId, reason) not rendered strings; rows render qualified names (`crate::name`), not bare `new`"
  - "A fixture asserts an upper bound on cross-crate boundary violations matching the real (post-R1, near-zero) count — not merely `non-empty`"
  - "Architecture Role column restored; `ariadne-cli` and `ariadne-e2e` render a volatile-leaf role (instability > 0.7), never `Stable foundational … many dependents`"
  - "Cross-crate cycle clusters un-suppressed and listed with qualified member names"
  - "`ariadne doc` twice → byte-identical; clippy/fmt/architecture/warm==cold green"
status: pending
---

<context>
With R1 fixed (tier-02), the edge set is trustworthy, so the sections tier-01
withheld can return — now showing real signal. This tier reverts the tier-01
suppression and hardens rendering so a future R1 regression is caught loudly
rather than silently reflooding the doc [src: plan.md D1; tier-01 steps 4-5].
The previous dedup keyed on rendered name strings, collapsing distinct edges and
making the "… and N more" count meaningless [src: crates/ariadne-graph/src/
docgen_insights.rs:196,210-211]. `purpose` maps instability < 0.3 to
"Stable foundational … many dependents"; once phantom afferent is gone, leaf
crates (cli, e2e) land in the volatile branch (instability > 0.7)
[src: crates/ariadne-graph/src/docgen.rs:392; coupling.rs:115-121].

If tier-02 was narrowed to spike-only, this tier is BLOCKED on the dedicated R1
implementation plan and must not start until R1 ships [src: plan.md `<risks>`;
tier-02 `<rollback>`].
</context>

<files>
- crates/ariadne-graph/src/docgen_insights.rs — in `boundary_violations`, restore
  the real body; change the dedup set to `BTreeSet<(SymbolId, SymbolId, &'static
  str)>` and render qualified names via `crate_key(path)` + `table.name`; in
  `architecture_section`, restore the `purpose(row)` Role cell + drop the withheld
  note; in `cycle_clusters`, restore cross-crate clusters and render members
  qualified [src: docgen_insights.rs:191-230,163-172,259-301].
- crates/ariadne-graph/tests/ (docgen_project.rs + snapshots) — re-accept goldens;
  add the cross-crate-violation upper-bound assertion and the cli/e2e leaf-role
  assertion.
- docs/codebase-overview.{md,svg} — regenerate via `ariadne doc`.
</files>

<steps>
1. Confirm tier-02 landed (R1 fix merged, repro test green). If tier-02 was
   spike-only, STOP — this tier blocks on the R1 implementation plan.
2. Write failing assertions in `docgen_project.rs`: (a) on a fixture with a known,
   bounded number of real cross-crate violations, `boundary_violations` lists
   exactly that count (no flood); (b) the `ariadne-cli` Architecture row Role
   matches the volatile-leaf string; (c) rendered boundary rows contain a `::`
   qualifier, never a bare `new`. Run → red (sections still withheld).
3. In `boundary_violations`, revert tier-01's withheld line; key the dedup on
   `(src_id, dst_id, reason)` so distinct edges count distinctly; render each row
   as `` `{crate}::{name}` `` for src and dst using `crate_key(table.path(id))`
   and `table.name(id)` [src: docgen_insights.rs:40-42,196,210-211].
4. In `architecture_section`, restore `purpose(row)` as the Role cell and remove
   the tier-01 withheld note [src: docgen_insights.rs:163-172].
5. In `cycle_clusters`, remove the cross-crate withhold from tier-01 and render
   members qualified; keep the tier-01 source-scope filter [src: tier-01 step 3].
6. `cargo nextest run -p ariadne-graph`; review (not blind-accept) goldens —
   confirm boundary rows are few and name real members, and cli/e2e read as
   volatile leaves; `cargo insta accept`.
7. Regenerate `cargo run -p ariadne-cli -- doc`; read all sections; run twice →
   diff empty.
</steps>

<verification>
- `cargo nextest run -p ariadne-graph` → upper-bound, leaf-role, and qualified-
  render tests green; goldens re-accepted after review.
- `cargo nextest run -p ariadne-daemon -p ariadne-mcp` → refactor + warm==cold
  unchanged-green.
- `cargo run -p ariadne-cli -- doc` twice → `docs/codebase-overview.{md,svg}`
  byte-identical; Boundary violations names qualified members and is short;
  Architecture Role shows `ariadne-cli`/`ariadne-e2e` as volatile leaves.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`;
  `cargo fmt --all --check`; `cargo test --test architecture`.
</verification>

<rollback>
`git checkout -- crates/ariadne-graph/src/docgen_insights.rs
crates/ariadne-graph/tests/ docs/codebase-overview.md docs/codebase-overview.svg`.
Reverts to the tier-01 suppressed-but-honest state; no other crate touched.
</rollback>
