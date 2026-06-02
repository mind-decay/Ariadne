---
tier_id: tier-05
title: Symbol doc enrichment (doc_for / DocForReport)
deps: [tier-01]
exit_criteria:
  - "DocForReport carries additive enrichment fields (role one-liner, file risk = churn×complexity, blast-radius summary counts) and all existing fields unchanged"
  - "daemon doc_for, mcp doc_for renderer, and cli doc_for renderer all populate/display the new fields"
  - "public_refs are noise-filtered through DocScope (no fixture/test neighbours)"
  - "structured-output test on DocForReport green; cargo clippy/fmt/deny/architecture green"
status: pending
---

<context>
The symbol surface returns a structured `DocForReport` (signature/kind/file/brief/public_refs),
rendered client-side — not Markdown [src: crates/ariadne-daemon/src/domain/queries/docs.rs:16-50;
ariadne-core DocForReport]. Enrich it with deterministic, system-only context: a role one-liner,
the file's churn×complexity risk, and blast-radius summary counts — additive only, so no consumer
breaks. Consumes tier-01 scope [src: plan.md D-architecture]. Full context: plan.md.
</context>

<files>
- crates/ariadne-core/src/domain/daemon/* (DocForReport definition) — MODIFY: add optional fields
  `role: String`, `file_risk: Option<f32>`, `blast_must: u32`, `blast_may: u32`. Additive; keep
  existing fields and order stable [src: CLAUDE.md D13 — core owns the type].
- crates/ariadne-daemon/src/domain/queries/docs.rs — MODIFY `doc_for`: compute role from symbol
  kind + owning module coupling shape; compute file risk via the tier-03 risk helper; carry the
  blast-radius `must_touch`/`may_touch` counts already computed [src: docs.rs:30-41].
- crates/ariadne-daemon/src/domain/queries/docs.rs — filter `public_refs` through `DocScope.include`.
- crates/ariadne-mcp/src/tools/doc_for.rs + server.rs — MODIFY render the new fields.
- crates/ariadne-cli/src/commands/query.rs — MODIFY `doc_for` display of the new fields.
- crates/ariadne-mcp/tests/tools_doc_for.rs — MODIFY/EXTEND structured assertions.
</files>

<steps>
1. Write a failing test in `tests/tools_doc_for.rs`: assert `DocForReport` now exposes `role`,
   `file_risk`, `blast_must`, `blast_may`, that a fixture neighbour is absent from `public_refs`,
   and that pre-existing fields are unchanged.
2. Extend the `DocForReport` core type with the additive fields (and any DTO mirror in the wire
   codec) — keep serialization backward-tolerant [src: crates/ariadne-daemon/src/adapters/codec.rs].
3. In `doc_for`: derive `role` (deterministic from `meta.kind` + owning-module coupling shape);
   set `file_risk` from the shared churn×complexity helper; pass through the existing
   `must_touch`/`may_touch` lengths as `blast_must`/`blast_may`.
4. Filter `public_refs` via `DocScope.include` so language-noise neighbours from fixtures/tests drop.
5. Update the mcp `doc_for` renderer and the cli `query doc_for` renderer to display the new fields.
6. Confirm cold (cli) and warm (daemon) routes produce equal structured output (parity).
</steps>

<verification>
- `cargo nextest run -p ariadne-mcp -p ariadne-daemon -p ariadne-cli` → doc_for tests green.
- `cargo test --test architecture` (core still owns the type; adapters only render) [src: CLAUDE.md D13].
- `cargo clippy … -D warnings`; `cargo fmt --all --check`; `cargo deny check`.
- Parity: assert cli-cold and daemon-warm `DocForReport` are identical for a sample symbol.
</verification>

<rollback>
`git checkout -- crates/ariadne-core crates/ariadne-daemon crates/ariadne-mcp crates/ariadne-cli`.
Additive fields mean reverting is a clean type rollback; no stored data migration involved.
</rollback>
