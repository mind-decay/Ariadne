---
tier_id: tier-09
audited: 2026-05-22
verdict: PASS
commit: 1acd8382509db8c9c5cb3546474c2e946f928096
---

<scope>
Tier-09 — component-graph MCP surface, analytics regression guard, and the
JS-framework E2E + SLO gate. Diff reviewed against `<files>`:

- `crates/ariadne-graph/tests/component_graph.rs` (new) — blast-radius /
  coupling regression guard. No `ariadne-graph/src/**` change: the analytics
  walk edges generically, so `Renders`/`UsesHook` flow through unchanged
  (tier step 2). No `component_graph` use case added — tier step 3 permits
  omission when no real gap exists; the MCP layer reads the neighbourhood
  straight off `ReadSnapshot::outgoing_edges`.
- `crates/ariadne-mcp/src/tools/file_summary.rs`, `src/types.rs` — the MCP
  surface. The tier `<files>` named `server.rs`; the real handler lives in
  the per-tool module `tools/file_summary.rs`. Landing the change there +
  the `ComponentRow` output type in `types.rs` is the correct realization of
  "extend `file_summary`" — not a deviation defect.
- `crates/ariadne-mcp/tests/tools_component_graph.rs` + 2 `insta` snapshots,
  `tests/support.rs` (`seed_component_project`).
- `crates/ariadne-e2e/**` — `verify_framework_fixture`, 4 repo suites,
  `Cargo.toml` `[[test]]` entries, `fixtures/repos.toml` (4 pinned repos),
  `slo.rs` corpus wired with the 4 framework repos.

Out of scope: ariadne-scip / tools/ariadne-sfc-scip working-tree changes
belong to tiers 06–08 and their own audits.
</scope>

<checks_run>
Every `<verification>` command re-run this session on commit `1acd838`
(tier-09 diff uncommitted in the working tree):

- `cargo fmt --all --check` — exit 0.
- `cargo nextest run --workspace` — 195 passed, 13 skipped, 0 failed.
  Confirmed executed: `component_graph` ×3 (blast-radius leaf/hook,
  coupling), `tools_component_graph` ×2 (renders+hooks, render-only golden).
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` —
  exit 0, no diagnostics (re-run with explicit exit capture).
- `cargo test --test architecture` — `architecture_invariants_hold` ok.
- `cargo nextest run -p ariadne-e2e --run-ignored all -E
  'test(fixture_has_component_graph)'` — 4/4 passed: svelte 1.2s, vue 3.8s,
  react 10.5s, astro 44.0s. Each clones the real OSS repo, indexes, and
  asserts non-zero `Component` symbols + ≥1 `Renders` edge via the MCP
  surface (exit criterion #3).
- SLO release gate — `cargo nextest run -p ariadne-e2e --release
  --run-ignored all -E 'test(slo_release_gate)'` — PASS in 506.8s on the
  8-repo corpus (kubernetes, vscode, dotnet/runtime, linux + 4 framework
  repos): 121,469 files, 1.92M symbols, 3.52M edges, 9 langs. Cold index
  **40.228s** (<60s), incremental apply p95 **242µs** (<500ms), query p95
  **168µs** (<100ms), peak RSS **3523 MiB** (<4 GiB, R1). Exit criterion #4.

Code-level checks: edge-kind match in `file_summary` keys on the core
`EdgeKind` (`Renders`/`UsesHook`, `_ => {}` arm safe under `#[non_exhaustive]`);
`dep_counts` logic is behaviour-equivalent to the pre-tier code (no
`top_dependencies` regression); `dst_name` falls back to `<unknown>` matching
the `summarize` placeholder; graph-test citations `build.rs` `from_core`,
`coupling.rs edges_directed`, `blast.rs` reverse-BFS verified accurate.
</checks_run>

<findings>
| id | category | severity | location | problem | fix |
|----|----------|----------|----------|---------|-----|
| I1 | docs | INFO | `tier-09-component-graph-e2e.md:83-96` | The `<blockers>` section is still present and states exit criterion #4 is "unverified", contradicting `status: completed` and the now-green SLO gate. | Drop the stale `<blockers>` section (its step-8 memory-probe rationale can move to a comment near step 8 if worth keeping). |
</findings>

<verdict>
PASS. All five exit criteria independently verified this session:

1. `blast_radius` / `coupling_report` traverse `Renders`/`UsesHook` — green
   via `component_graph.rs` (leaf blast radius reaches both rendering
   parents; coupling counts afferent=2 / efferent=1).
2. MCP `file_summary` surfaces `Component` symbols with rendered children +
   used hooks — green via `tools_component_graph.rs` + 2 golden snapshots.
3. `ariadne-e2e` indexes real React/Vue/Svelte/Astro repos, each asserting
   non-zero `Component` symbols + `Renders` edges — 4/4 suites green.
4. SLO gate re-run on the framework corpus — cold 40.2s, incremental p95
   242µs, query p95 168µs, all inside budget; R-SLO did not materialise.
5. `nextest --workspace`, `clippy -D warnings`, `cargo test --test
   architecture` — all green.

No FAIL findings. The single INFO is a tier-file housekeeping nit, not a
code defect, and does not gate the verdict. Step-8 memory probe: tier-09
adds no Salsa or in-RAM graph table — the graph change is test-only, the MCP
change reads existing snapshot state — so there is no per-table delta; the
SLO gate's 4 GiB `PEAK_RSS_BUDGET` (observed 3523 MiB) covers R1.
</verdict>

<next_steps>
- Tier-09 is sound and ships. Optionally clear I1 by removing the stale
  `<blockers>` section from the tier file.
- The js-framework-support plan has no tier beyond 09 — the framework-support
  feature is complete pending commit of the tiers-06–09 working tree.
</next_steps>

<sources>
- OWASP Top 10 — https://owasp.org/www-project-top-ten/ (no input-surface or
  injection finding: the MCP change is read-only over an indexed snapshot;
  the e2e harness clones pinned SHAs over HTTPS).
- Plan + tier file: `.claude/plans/js-framework-support/plan.md`,
  `tier-09-component-graph-e2e.md`.
- Cited code verified: `crates/ariadne-graph/src/build.rs` (`from_core`),
  `src/coupling.rs`, `src/blast.rs`; `crates/ariadne-mcp/src/catalog.rs`,
  `src/tools/file_summary.rs`, `src/tools/list_symbols.rs`.
</sources>
