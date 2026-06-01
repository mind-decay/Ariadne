---
tier_id: tier-15a
audited: 2026-06-02
verdict: PASS
commit: c83902ac6758931a33250f0a148d35b7059a5f01
---

<scope>
Tier-15a — analytics catalog projection. Extends the two read-only catalog
projections (cold MCP `Catalog`, warm daemon `WarmCatalog`) to carry per-symbol
cyclomatic `complexity` and to load file churn / co-change / symbol churn from
the `Storage` port at build time. Substrate-only: no MCP tool, no daemon
protocol variant, no new dependency.

Diff under review (working tree atop HEAD `c83902a`, tier-15a uncommitted):
- `crates/ariadne-mcp/src/catalog.rs` — `complexity` on `SymbolMeta`; `churn` /
  `co_change` / `symbol_churn` `Vec`s on `Catalog`, loaded + sorted in `build`.
- `crates/ariadne-daemon/src/domain/catalog.rs` — identical changes to
  `WarmCatalog`; `apply_changeset` leaves analytics vectors untouched; new
  in-module unit test.
- `crates/ariadne-daemon/src/domain/dump.rs` — `CatalogDump`/`MetaRow` extended
  with the four new fields (outside declared `<files>`; see INFO-1).
- `crates/ariadne-mcp/tests/support.rs` — `seed_analytics_project` fixture.
- `crates/ariadne-mcp/tests/catalog_projection.rs` — new cold-projection test.
</scope>

<checks_run>
- plan_adherence: every `<files>` entry touched as intended. One file outside
  the list (`dump.rs`) and one test-placement deviation — both justified, see
  findings.
- correctness: load-time sorts match D3 (`path`; `(a,b)`; `SymbolId`) and are
  identical across both crates; `from_record` threads `rec.complexity`;
  `apply_changeset` re-derives `SymbolMeta::from_record` on every upsert
  (catalog.rs:229-230) so warm edits update `complexity`; analytics vectors
  correctly untouched on code-edit changesets (no git-history delta).
- security: no input parsing, no secrets, no injection/deserialization surface.
  Loads already-persisted records into RAM. OWASP Top 10 — N/A.
- performance: three `all_*` reads + three `O(n log n)` sorts on the cold/rebuild
  path only (never the hot query path); vectors small vs symbol set (D3). Memory
  probe `warm_graph_tables_stay_within_the_per_table_budget` PASS (R1 <256MB/table).
- architecture: no adapter→adapter edge; `cargo test --test architecture` PASS.
  No `DaemonQuery`/`DaemonResponse` variant added; no new dependency
  (`ariadne-storage`/`tempfile` already (dev-)deps of `ariadne-daemon`).
- tests: real redb seeded via `ariadne-storage`, real `Catalog`/`WarmCatalog`
  built — no mocks; assertions on concrete field values with loud `assert_eq!`.
- docs: new fields carry `///` docs (passes `#![deny(missing_docs)]` where it
  applies); `RUSTDOCFLAGS=-D warnings cargo doc` PASS.
- exit_criteria: all four independently verified (see <verdict>).

Re-run `<verification>` (all green):
- `cargo nextest run -p ariadne-mcp -p ariadne-daemon` → 67 passed, 0 skipped.
  Incl. `ariadne-mcp::catalog_projection catalog_loads_analytics_and_complexity`
  and `ariadne-daemon domain::catalog::tests::build_loads_analytics_and_complexity`
  and the divergence-0 `incremental_warm warm_apply_equals_fresh_rebuild`.
- `cargo test --test architecture` → 1 passed (no new dep edge).
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` → exit 0.
- `cargo fmt --all --check` → exit 0.
- `RUSTDOCFLAGS="-D warnings" cargo doc -p ariadne-mcp -p ariadne-daemon --no-deps`
  → exit 0.
</checks_run>

<findings>
| id | category | severity | file:line | problem | fix |
|---|---|---|---|---|---|
| INFO-1 | plan_adherence | INFO | crates/ariadne-daemon/src/domain/dump.rs:39,68-70,51,92 | File modified outside the tier's declared `<files>`. Justified: `CatalogDump` is the divergence-0 projection; extending it with `complexity`+churn vectors is what makes `warm_apply_equals_fresh_rebuild` actually validate the warm-update path keeps the new fields consistent with a rebuild — without it that coverage is silently dropped. | None required; record the scope expansion. |
| INFO-2 | tests | INFO | crates/ariadne-daemon/src/domain/catalog.rs:281-431 | Warm test placed as in-module `#[cfg(test)]` unit test, not under `crates/ariadne-daemon/tests/` per `<files>`. Necessary: `WarmCatalog.{churn,co_change,symbol_churn}` are `pub(crate)` and unreachable from an external integration-test crate, so the assertion must live in-crate. | None required; correct placement given visibility. |
</findings>

<verdict>
PASS. Zero FAIL findings. All four exit criteria verified:
1. Both `Catalog` and `WarmCatalog` carry `complexity: u32` on `SymbolMeta`,
   set from `rec.complexity` in `from_record` (mcp catalog.rs:46,64; daemon
   catalog.rs:55,72). Asserted = 7/3 on both sides.
2. Both `build`s load `Vec<FileChurn>`/`Vec<CoChangePair>`/`Vec<SymbolChurn>`
   via `Storage::all_churn`/`all_co_change`/`all_symbol_churn` (mcp
   catalog.rs:137-141; daemon catalog.rs:144-150), each sorted by key (D3).
3. Both projections expose every field for the seeded fixture; cold and warm
   assert identical literal values (complexity 7/3; churn alpha 9/1-author,
   beta 4/2-authors; co_change count 3; symbol_churn sid1=5, sid2=2) — field-
   equal. Crate isolation forces two mirrored fixtures rather than one shared
   redb; both pin the same literals, so equality is checked, not assumed.
4. No new MCP tool / protocol variant / dependency; targeted nextest +
   architecture + clippy + fmt + doc all green (re-run this session).
</verdict>

<next_steps>
None. Tier-15a is the analytics substrate; tier-15b/15c consume `catalog.churn`
/ `.co_change` / `.symbol_churn` / `SymbolMeta.complexity` as pure in-RAM reads.
The staleness window for analytics after a live edit (vectors refresh only on a
full rebuild, R-B2) is documented and accepted — carry it forward to 15b/15c
golden design, not a defect here.
</next_steps>

<sources>
- Tier file: .claude/plans/post-v1-roadmap/tier-15a-analytics-catalog-projection.md
- Plan: .claude/plans/post-v1-roadmap/plan.md (D1/D3, RD8, R-B2, R1)
- Storage port methods: crates/ariadne-core/src/domain/ports.rs:59,69,75,122,129
- SymbolRecord.complexity: crates/ariadne-core/src/domain/records.rs:58
- Churn/CoChange/SymbolChurn records: crates/ariadne-core/src/domain/records.rs:67-153
- Google eng-practices (code health over perfection): https://google.github.io/eng-practices/review/reviewer/standard.html
- OWASP Top 10 (security category, N/A here): https://owasp.org/www-project-top-ten/
</sources>
