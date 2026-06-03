---
tier_id: tier-03
audited: 2026-06-03
verdict: PASS
commit: f4339f0a26fd31756430aa2cc59542f35786f667
---

<scope>
Tier-03 "Project overview content redesign (for_project + architecture_svg)". The
implementation is an **uncommitted working-tree diff atop HEAD `f4339f0`** (tier-02). Scoped
diff covers the tier `<files>`:
- `crates/ariadne-graph/src/docgen.rs` — `for_project` rewritten to six insight sections; old
  `ModuleStat`/`push_hotspots`/`push_coupling`/`push_glossary`/`render_layers` deleted; new
  `pub fn architecture_svg`; `purpose` promoted to `pub(crate)`.
- `crates/ariadne-graph/src/docgen_insights.rs` — NEW. `synopsis`, `architecture_section`,
  `boundary_violations`, `cycle_clusters`, `risk_hotspots`, `change_coupling`,
  `file_complexity_map` + helpers. Pure `std::fmt::Write`, `BTree*`-ordered.
- `crates/ariadne-graph/src/lib.rs` — `mod docgen_insights;` + `pub use docgen::architecture_svg`.
- `crates/ariadne-daemon/src/domain/queries/docs.rs`, `crates/ariadne-mcp/src/tools/doc_project.rs`
  — thread `&cat.churn`, `&cat.co_change`, `&DocScope::default()`.
- `crates/ariadne-graph/tests/{docgen_project.rs(NEW),docgen_fixture.rs}` + snapshot.
Forced ripple updates outside the declared `<files>` but required to compile/keep green:
`ariadne-graph/src/heuristics.rs` (2 `pub(crate)` accessors), `ariadne-daemon/tests/support.rs`,
`ariadne-graph/tests/doc_scope.rs`, `ariadne-mcp/tests/tools_doc.rs`. All justified by the
`for_project` signature + content change. Index fresh at revision 692 (`project_status`).
</scope>

<checks_run>
- `cargo nextest run -p ariadne-graph` → **54 passed, 0 skipped**. New `docgen_project` (6 tests)
  + `docgen_fixture` golden + `doc_scope` all green.
- `cargo nextest run -p ariadne-daemon -p ariadne-mcp` → **103 passed, 0 skipped**
  (`doc_for_project` still returns Markdown; `tools_doc` asserts the new sidecar ref + Risk hot-spots).
- `cargo test --test architecture` → **ok** (no graph→daemon dep introduced; D6 guard holds).
- `cargo nextest run -p ariadne-cli` → **40 passed** (digest still emits bounded markdown).
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` → clean.
- `cargo fmt --all --check` → clean (exit 0).
- `cargo deny check` → **exit 0** (advisories/bans/licenses/sources ok — proves no new dependency).
- Determinism re-checked: `for_project_is_deterministic`, `architecture_svg_is_deterministic…`,
  and the `project_doc_insertion_order_independent` proptest (shuffled graph → identical bytes) pass.
- Read every changed file end-to-end; traced the fixture (`tests/support.rs`) against the golden
  snapshot — synopsis counts (5 crates · 1 layer · 8 syms · 7 edges), the {core::run, db::query,
  db::connect} SCC, and the `core::run → db::query` lowest-(src,dst) cut edge all reconcile.
- Verified the `GOD_THRESHOLD = 15.0` citation: `daemon/queries/health.rs:15` is
  `const GOD_THRESHOLD: u32 = 15` — citation accurate.
</checks_run>

<findings>
| id | category | severity | location | problem | fix |
| --- | --- | --- | --- | --- | --- |
| F1 | correctness | INFO | crates/ariadne-cli/src/commands/digest.rs:246 (+ stale doc comments :136-137, :243) | `overview_slice` slices the project doc on `\n## Layers`, a section tier-03 deleted; `split_once` now always returns `None`, so the function silently falls through to the whole markdown capped at `OVERVIEW_BUDGET` (600 chars) instead of cleanly dropping the diagram, and the `![architecture](…svg)` ref can leak into the digest overview. Output stays safe/bounded; no test covers the slice. | Retarget the split to `\n## Architecture` (or the first post-synopsis header) and refresh the comments; natural fold into the tier-06 CLI doc work. |
</findings>

<verdict>
**PASS.** Zero FAIL findings. Every `<verification>` command re-ran green, all five
`exit_criteria` are independently satisfied:
1. `for_project` emits Synopsis / Architecture / Boundary violations / Cycle clusters / Risk
   hot-spots / Refactor & change-coupling; the per-file Martin dump, language glossary, and
   Mermaid `flowchart TD` are deleted and negatively asserted in `docgen_project`/`tools_doc`.
2. `architecture_svg` is a deterministic crate-level SVG (`render_svg`, capped at 24 nodes);
   Markdown embeds `![architecture](codebase-overview.svg)`.
3. Cycle clusters name the largest SCC by member count with a deterministic lowest-(src,dst) cut
   edge; Risk hot-spots filter to source-scoped files (out-of-scope `x/fixtures/bar.rs` dropped).
4. Git-history vectors threaded into `for_project` + both catalogs; empty `churn`/`co_change`
   degrade to explicit "history unavailable" lines.
5. Golden Markdown green; render-twice byte-identical; clippy/fmt/deny/architecture green.
The hexagonal boundary holds — `docgen_insights` consumes only `ariadne_core` types, crate-local
graph-pure use cases, and `petgraph`; no daemon/mcp analytics dependency (D6). The single INFO is
a downstream-consumer staleness that neither breaks the build, fails a test, nor violates an exit
criterion, so it does not gate the verdict.
</verdict>

<next_steps>
- None required for tier-03 to ship. F1 is non-blocking; address it when tier-06 wires the CLI
  `doc` command (the same surface that owns digest/overview slicing).
</next_steps>

<sources>
- Re-run command output (this session): nextest graph/daemon/mcp/cli, `cargo test --test
  architecture`, clippy, fmt, deny.
- crates/ariadne-graph/src/docgen_insights.rs; crates/ariadne-graph/src/docgen.rs;
  crates/ariadne-graph/tests/{docgen_project.rs, support.rs, snapshots/docgen_fixture__project.snap}.
- crates/ariadne-daemon/src/domain/queries/health.rs:15 (GOD_THRESHOLD value).
- crates/ariadne-cli/src/commands/digest.rs:244-250 (F1).
- .claude/plans/useful-docgen/{plan.md (D2/D4/D5/D6), tier-03-project-content.md}.
- [Google eng-practices — reviewer standard](https://google.github.io/eng-practices/review/reviewer/standard.html).
</sources>
