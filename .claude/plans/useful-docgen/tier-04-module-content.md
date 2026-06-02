---
tier_id: tier-04
title: Module doc redesign (for_module)
deps: [tier-01, tier-02]
exit_criteria:
  - "for_module emits: real purpose (role within its crate), a neighbourhood SVG, named inbound/outbound (noise-filtered), cycle participation, dead code, and a churn×complexity risk line"
  - "the module neighbourhood SVG is deterministic and references the module + its top callers/callees only"
  - "noise symbols (language built-ins from non-source neighbours) are excluded via DocScope"
  - "golden Markdown test green; render twice → identical bytes; cargo clippy/fmt/deny/architecture green"
status: pending
---

<context>
Bring the same insight discipline to the per-file `doc_for_module` surface: explain the module's
role, draw its dependency neighbourhood as a small SVG (tier-02), and report named coupling /
cycles / dead code / risk instead of a bare member table. Consumes tier-01 scope + tier-02
emitter [src: plan.md; crates/ariadne-graph/src/docgen.rs:47-184]. Full context: plan.md.
</context>

<files>
- crates/ariadne-graph/src/docgen.rs — MODIFY `for_module`: richer sections + neighbourhood SVG.
- crates/ariadne-graph/src/docgen.rs — ADD `pub fn module_svg(graph, module, scope) -> String`
  (module node + top-N scoped callers/callees → `diagram::render_svg`).
- crates/ariadne-graph/src/docgen_insights.rs — REUSE/extend helpers (role, risk line).
- crates/ariadne-graph/src/lib.rs — re-export `module_svg`.
- crates/ariadne-graph/tests/docgen_fixture.rs — MODIFY module-doc golden.
- crates/ariadne-graph/tests/docgen_module.rs — NEW. asserts sections + SVG determinism.
</files>

<steps>
1. Write failing `tests/docgen_module.rs`: for a fixture module assert the Markdown contains a
   `## Role`/`## Neighbourhood`/`## Coupling`/`## Cycles`/`## Dead code`/`## Risk` set, references
   a neighbourhood SVG, and that `module_svg` is byte-identical across two calls.
2. **Role**: replace the one-liner `purpose()` with crate-aware role — module name, owning crate
   (`crate_of`), layer (domain/adapter), and coupling shape (stable/volatile/intermediate)
   [src: crates/ariadne-graph/src/docgen.rs:242-252].
3. **Neighbourhood SVG**: build a node set of the module + its top-N scoped callers (Incoming) and
   callees (Outgoing) from the existing edge walk [src: crates/ariadne-graph/src/docgen.rs:66-84];
   feed to `diagram::render_svg`; embed `![neighbourhood](…svg)` — for the MCP/daemon path the SVG
   bytes are returned alongside Markdown only via the tier-06 CLI; the Markdown references a path.
4. **Coupling**: keep named caller/callee tables but filter both through `DocScope.include` so
   non-source neighbours (fixtures/tests) drop out.
5. **Cycles / Dead code**: retain existing logic; if the module participates in a large SCC, link
   to the project-level cluster naming (cross-reference by SCC index).
6. **Risk**: add a churn×complexity line for this file, reusing the tier-03 risk helper.
7. Keep the pre-SCIP visibility caveat only if symbol metadata is still unavailable in-session;
   do not depend on unbuilt post-v1 metadata tiers [src: crates/ariadne-graph/src/docgen.rs:118-120].
8. Update the module-doc golden; verify determinism by double render.
</steps>

<verification>
- `cargo nextest run -p ariadne-graph` → docgen_module + docgen_fixture green.
- `cargo nextest run -p ariadne-daemon -p ariadne-mcp` → doc_for_module returns Markdown.
- Determinism double-render assertion in test.
- `cargo clippy … -D warnings`; `cargo fmt --all --check`; `cargo deny check`; `cargo test --test architecture`.
</verification>

<rollback>
`git checkout -- crates/ariadne-graph/src/docgen.rs crates/ariadne-graph/tests`; delete
`tests/docgen_module.rs`; revert the `module_svg` re-export. tier-01/02/03 untouched.
</rollback>
