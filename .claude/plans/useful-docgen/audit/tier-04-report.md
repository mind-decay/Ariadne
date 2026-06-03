---
tier_id: tier-04
audited: 2026-06-04
verdict: PASS
commit: fb76303efe7936d8415c9c2b110f9815f9dbbceb
---

<scope>
Tier-04 "Module doc redesign (for_module)". Reviewed the uncommitted working-tree
diff on top of HEAD `fb76303` scoped to the tier's `<files>`:
- `crates/ariadne-graph/src/docgen.rs` — `for_module` re-shaped (Role / Neighbourhood /
  Coupling / Cycles / Dead code / Risk); new `pub fn module_svg`; helpers
  `neighbour_histograms`, `scope_filter`, `svg_ref`, `sym_node`.
- `crates/ariadne-graph/src/docgen_insights.rs` — added `module_role`, `risk_line`.
- `crates/ariadne-graph/src/lib.rs` — re-export `module_svg` (façade only).
- `crates/ariadne-daemon/src/domain/queries/docs.rs` — threads `&cat.churn`.
- `crates/ariadne-mcp/src/tools/doc_module.rs` — threads `&cat.churn`.
- `crates/ariadne-graph/tests/docgen_module.rs` — NEW (5 tests).
- `crates/ariadne-graph/tests/docgen_fixture.rs` + golden snapshot — updated.
- Out-of-list (compile-forced ripples): `crates/ariadne-daemon/tests/support.rs`,
  `crates/ariadne-mcp/tests/tools_doc.rs` (see INFO-2).
Index `project_status` revision 1115, fresh (not stale).
</scope>

<checks_run>
- `cargo nextest run -p ariadne-graph` → 59/59 pass, incl. `docgen_module::*` and
  `docgen_fixture::golden_module_doc_core`.
- `cargo nextest run -p ariadne-daemon -p ariadne-mcp` → 103/103 pass, incl.
  `tools_doc::doc_for_module_renders_markdown` (real cold render over the fixture index).
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` → clean.
- `cargo fmt --all --check` → clean (exit 0).
- `cargo test --test architecture` → 1 passed (hexagon invariants hold).
- `cargo deny check` → advisories/bans/licenses/sources ok (only benign
  unmatched-license-allowance warnings).
- Read every changed file end-to-end; verified `module_role`/`risk_line`/`crate_key`/
  `purpose` definitions and `DocScope::include`/`LayerHint::of`/`crate_of` semantics.
- `find_references for_module` → all 8 call sites accounted for; no missed ripple; CLI
  has no caller yet (correctly deferred to tier-06).
</checks_run>

<findings>
| id | category | severity | location | problem | fix |
| --- | --- | --- | --- | --- | --- |
| INFO-1 | correctness | INFO | crates/ariadne-graph/src/docgen.rs `module_svg` | The neighbourhood SVG path-scopes only the centre module (`scope.include(module.name)`); its caller/callee nodes are taken from un-`scope_filter`-ed histograms, so the SVG may draw a fixture/test neighbour the scope-filtered coupling table omits — step 3 says "top-N *scoped* callers/callees". | Non-blocking: the plan's own `<files>` mandates the snapshot-free signature `module_svg(graph, module, scope)`, which lacks the `SymbolTable` needed to resolve neighbour paths; the limitation is documented in the fn doc comment, neighbour nodes are opaque `#<id>` labels, and exit criterion #3 scopes only the "coupling tables". Thread a resolver in a later tier or amend step-3 wording if SVG-side scoping is wanted. |
| INFO-2 | plan_adherence | INFO | crates/ariadne-daemon/tests/support.rs; crates/ariadne-mcp/tests/tools_doc.rs | Two files modified that the tier's `<files>` does not enumerate. | Justified: both are compile-forced test ripples — `support.rs::cold_doc_module` and the mcp doc test would not build/pass after the mandated `for_module` signature change and the removed "Public API" section. No production code outside `<files>`. List them next time. |
</findings>

<verdict>
PASS. Zero FAIL findings. All five `exit_criteria` independently verified:
1. `for_module` emits Role (crate-aware), Neighbourhood SVG ref, scope-filtered named
   inbound/outbound coupling, cycle participation, dead code, and a churn×complexity risk
   line — confirmed in the golden snapshot and `module_doc_emits_insight_headers`.
2. `module_svg` references the module centre + top callers/callees and is byte-identical
   across two calls (`module_svg_is_deterministic_and_well_formed`).
3. Non-source neighbours drop from the coupling tables via `DocScope`
   (`coupling_excludes_out_of_scope_neighbours`: `api.rs` exclude removes `api::serve`,
   keeps `db::connect`).
4. Empty `churn` degrades the risk line to an explicit "_Git history unavailable …_"
   (`empty_history_degrades_risk_to_explicit_line`).
5. Golden Markdown green; double-render identical; clippy/fmt/deny/architecture green.
Hexagon preserved (D6/D13): `docgen` consumes only graph-pure `file_hotspots` and core
types; no daemon/mcp analytics dependency; `module_svg` re-export is façade-only.
</verdict>

<next_steps>
None blocking. Tier-04 may be committed. Optional follow-ups for later tiers:
- Decide whether the neighbourhood SVG should path-scope its neighbours (INFO-1); if so,
  revisit the `module_svg` signature or amend the plan step-3 wording.
- Enumerate the two compile-forced test files in `<files>` for future tiers (INFO-2).
- The `svg_ref` ↔ `module_svg` filename/centre-label contract is consumed by the tier-06
  CLI write path; verify it there.
</next_steps>

<sources>
- crates/ariadne-graph/src/{docgen.rs,docgen_insights.rs,doc_model.rs,lib.rs}; tests/docgen_module.rs; snapshot docgen_fixture__module_core.snap
- crates/ariadne-daemon/src/domain/queries/docs.rs; crates/ariadne-mcp/src/tools/doc_module.rs
- .claude/plans/useful-docgen/{plan.md,tier-04-module-content.md} (D3/D4/D6, exit_criteria)
- CLAUDE.md `<architecture>` / D11 / D13; .claude/hooks/audit-gate.sh
- [Google eng-practices — reviewer standard](https://google.github.io/eng-practices/review/reviewer/standard.html)
</sources>
</content>
</invoke>
