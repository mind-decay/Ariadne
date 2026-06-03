---
tier_id: tier-05
title: Symbol doc enrichment (doc_for / DocForReport)
deps: [tier-01]
exit_criteria:
  - "DocForReport carries additive enrichment fields (role, file_risk = churn x complexity, blast_must, blast_may) and all existing fields are unchanged in name and order"
  - "daemon doc_for, mcp doc_for renderer, and cli doc_for renderer all populate/display the new fields"
  - "public_refs are scope-filtered through DocScope (no fixture/test neighbours)"
  - "cold (cli/mcp) and warm (daemon) DocForReport are identical for a sample symbol (parity)"
  - "structured-output test on DocForReport green; cargo clippy/fmt/deny/architecture green"
status: pending
---

<context>
The symbol surface returns a structured `DocForReport` rendered client-side, not Markdown
[src: crates/ariadne-daemon/src/domain/queries/docs.rs:18-50;
crates/ariadne-core/src/domain/daemon/response.rs:71-82]. Enrich it with deterministic, system-only
context: a role one-liner, the file's churn×complexity risk, and blast-radius summary counts —
additive only, so no consumer breaks. Both catalogs already carry `churn` + per-symbol
`complexity`, so `file_risk` is computable on the cold and warm paths alike, preserving parity
[src: crates/ariadne-mcp/src/catalog.rs:50,84-92; crates/ariadne-daemon/src/domain/catalog.rs:147-152].
Consumes tier-01 scope. Full context: plan.md.
</context>

<files>
- crates/ariadne-core/src/domain/daemon/response.rs — MODIFY `DocForReport` (lines 71-82): append
  `role: String`, `file_risk: Option<f32>`, `blast_must: u32`, `blast_may: u32`. Keep the existing
  `signature`/`kind`/`file`/`brief`/`public_refs` fields and order stable [src: CLAUDE.md D13 — core owns the type].
- crates/ariadne-daemon/src/adapters/codec.rs — MODIFY the wire mirror of `DocForReport` if it is
  encoded field-by-field; keep decoding backward-tolerant.
- crates/ariadne-daemon/src/domain/queries/docs.rs — MODIFY `doc_for` (18-50): capture
  `must_touch`/`may_touch` **lengths** before they are consumed into `public_refs` [src: docs.rs:30-41];
  derive `role` from `meta.kind` + owning-module coupling shape; set `file_risk` from the shared
  churn×complexity helper over `cat.churn`; filter `public_refs` via `DocScope.include`.
- crates/ariadne-mcp/src/tools/doc_for.rs + server.rs — MODIFY: compute the same fields from the cold
  `Catalog` (which carries `churn`) and render them; warm/cold output must match.
- crates/ariadne-cli/src/commands/query.rs — MODIFY the `doc_for` display of the new fields.
- crates/ariadne-mcp/tests/tools_doc_for.rs — MODIFY/EXTEND structured assertions.
</files>

<steps>
1. Write a failing test in `tests/tools_doc_for.rs`: assert `DocForReport` now exposes `role`,
   `file_risk`, `blast_must`, `blast_may`; that a fixture neighbour is absent from `public_refs`;
   that pre-existing fields are unchanged; and that cold and warm reports are equal for one symbol.
2. Extend the `DocForReport` core type with the additive fields (and the `codec.rs` DTO mirror) —
   keep serialization backward-tolerant [src: crates/ariadne-daemon/src/adapters/codec.rs].
3. In `doc_for`: derive `role` (deterministic from `meta.kind` + owning-module coupling shape);
   set `file_risk` from the shared helper (`None` only when `churn` is empty); pass the existing
   `must_touch`/`may_touch` lengths as `blast_must`/`blast_may` [src: docs.rs:30-41].
4. Filter `public_refs` (currently the blast-radius `must_touch ∪ may_touch`, `Vec<SymbolSummary>`
   from core rows.rs:9) via `DocScope.include` so fixture/test neighbours drop.
5. Update the mcp `doc_for` renderer (tools/doc_for.rs + server.rs) and the cli `query doc_for`
   renderer to display the new fields.
6. Confirm cold (cli/mcp) and warm (daemon) routes produce equal structured output (parity).
</steps>

<verification>
- `cargo nextest run -p ariadne-mcp -p ariadne-daemon -p ariadne-cli` → doc_for tests green.
- `cargo test --test architecture` (core still owns the type; adapters only render) [src: CLAUDE.md D13].
- `cargo clippy … -D warnings`; `cargo fmt --all --check`; `cargo deny check`.
- Parity: assert cold and warm `DocForReport` are identical for a sample symbol.
</verification>

<rollback>
`git checkout -- crates/ariadne-core crates/ariadne-daemon crates/ariadne-mcp crates/ariadne-cli`.
Additive fields mean reverting is a clean type rollback; no stored-data migration involved.
</rollback>
