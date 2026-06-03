---
tier_id: tier-04
title: Module doc redesign (for_module)
deps: [tier-01, tier-02]
exit_criteria:
  - "for_module emits: role (within its crate), a neighbourhood SVG, named inbound/outbound (scope-filtered), cycle participation, dead code, and a churn x complexity risk line"
  - "the module neighbourhood SVG is deterministic and references the module + its top callers/callees only"
  - "non-source neighbours (fixtures/tests) are excluded from the coupling tables via DocScope"
  - "empty git-history degrades the risk line to an explicit 'history unavailable', deterministically"
  - "golden Markdown test green; render twice -> identical bytes; cargo clippy/fmt/deny/architecture green"
status: completed
completed: 2026-06-04
---

<context>
Bring the same insight discipline to the per-file `doc_for_module` surface: explain the module's
role, draw its dependency neighbourhood as a small SVG (tier-02), and report named coupling /
cycles / dead code / risk instead of a bare member table. Consumes tier-01 scope + tier-02
emitter; the risk line needs the git-history churn vector threaded in (D6) [src: plan.md D6;
crates/ariadne-graph/src/docgen.rs:47-184]. Full context: plan.md.
</context>

<files>
- crates/ariadne-graph/src/docgen.rs — MODIFY `for_module`: extend signature to
  `for_module(graph, snap, module, churn: &[FileChurn], scope: &DocScope)`; richer sections + neighbourhood SVG.
- crates/ariadne-graph/src/docgen.rs — ADD
  `pub fn module_svg(graph, snap, module, scope) -> Result<String, GraphError>`
  (module node + top-N scope-filtered callers/callees → `diagram::render_svg`; takes `snap` to
  resolve neighbour paths for scoping, INFO-1).
- crates/ariadne-graph/src/docgen_insights.rs — REUSE/extend helpers (role, `risk_line` from tier-03).
- crates/ariadne-graph/src/lib.rs — re-export `module_svg` (façade only).
- crates/ariadne-daemon/src/domain/queries/docs.rs — MODIFY `doc_for_module`: pass `&cat.churn`,
  `&DocScope::default()` [src: docs.rs:53-62; catalog.rs:147-152].
- crates/ariadne-mcp/src/tools/doc_module.rs — MODIFY: pass `&cat.churn`, `&DocScope::default()`.
- crates/ariadne-graph/tests/docgen_fixture.rs — MODIFY module-doc golden.
- crates/ariadne-graph/tests/docgen_module.rs — NEW. asserts sections + SVG determinism +
  SVG neighbour scoping + empty-history path.
- crates/ariadne-daemon/tests/support.rs — MODIFY (compile-forced): `cold_doc_module` threads the
  new `for_module` signature (INFO-2).
- crates/ariadne-mcp/tests/tools_doc.rs — MODIFY (compile-forced): doc test matches the new
  signature + the removed "Public API" section (INFO-2).
</files>

<steps>
1. Write failing `tests/docgen_module.rs`: for a fixture module assert the Markdown contains a
   `## Role`/`## Neighbourhood`/`## Coupling`/`## Cycles`/`## Dead code`/`## Risk` set, references a
   neighbourhood SVG, and that `module_svg` is byte-identical across two calls; assert empty `churn`
   yields the "history unavailable" risk line.
2. **Role**: replace the one-liner `purpose()` with crate-aware role — module name, owning crate
   (`crate_of`), layer (`LayerHint`), and coupling shape (stable/volatile/intermediate)
   [src: crates/ariadne-graph/src/docgen.rs:242-252].
3. **Neighbourhood SVG**: build a node set of the module + its top-N scoped callers (Incoming) and
   callees (Outgoing) from the existing edge walk [src: crates/ariadne-graph/src/docgen.rs:66-84];
   feed to `diagram::render_svg`; embed `![neighbourhood](…svg)`. `module_svg` takes `snap` and
   resolves each neighbour's defining path via the `SymbolTable`, scope-filtering fixture/test
   neighbours out of the diagram exactly like the coupling tables — so the SVG never draws a
   neighbour the table omits (INFO-1). Neighbour nodes are labelled by `SymbolId` (`#<id>`). The
   MCP/daemon path returns Markdown that references the SVG by relative path; the SVG file itself is
   written only by the tier-06 CLI (the read-only tool does no IO).
4. **Coupling**: keep named caller/callee tables but filter both through `DocScope.include` so
   non-source neighbours (fixtures/tests) drop out.
5. **Cycles / Dead code**: retain existing logic [src: docgen.rs:149-182]; if the module participates
   in a large SCC, cross-reference the project-level cluster by SCC index.
6. **Risk**: add a churn×complexity line for this file via the shared `risk_line` helper (tier-03),
   reading `churn` + the file's folded `SymbolRecord.complexity`.
7. Keep the pre-SCIP visibility caveat only if symbol metadata is still unavailable in-session; do
   not depend on unbuilt post-v1 metadata tiers [src: crates/ariadne-graph/src/docgen.rs:118-120].
8. Update the module-doc golden; verify determinism by double render.
</steps>

<verification>
- `cargo nextest run -p ariadne-graph` → docgen_module + docgen_fixture green.
- `cargo nextest run -p ariadne-daemon -p ariadne-mcp` → doc_for_module returns Markdown.
- Determinism double-render assertion in test.
- `cargo clippy … -D warnings`; `cargo fmt --all --check`; `cargo deny check`; `cargo test --test architecture`.
</verification>

<rollback>
`git checkout -- crates/ariadne-graph/src/docgen.rs crates/ariadne-graph/tests crates/ariadne-daemon
crates/ariadne-mcp`; delete `tests/docgen_module.rs`; revert the `module_svg` re-export. tier-01/02/03 untouched.
</rollback>
