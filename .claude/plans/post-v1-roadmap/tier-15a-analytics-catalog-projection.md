---
tier_id: tier-15a
title: Analytics catalog projection — load churn, co-change, symbol-churn, complexity warm + cold
deps: [tier-11b, tier-12]
exit_criteria:
  - Both the cold MCP `Catalog` and the warm daemon `WarmCatalog` carry `complexity: u32` on `SymbolMeta`, populated from `SymbolRecord.complexity`.
  - Both catalogs load `Vec<FileChurn>`, `Vec<CoChangePair>`, and `Vec<SymbolChurn>` at build time via the `Storage::all_churn` / `all_co_change` / `all_symbol_churn` port methods.
  - A test seeds a fixture redb with churn + co-change + symbol-churn + non-zero complexity and asserts both projections expose every field; the cold and warm projections are field-equal for the same fixture.
  - No new MCP tool, no daemon protocol variant, no new dependency; `cargo nextest run -p ariadne-mcp -p ariadne-daemon` + architecture + clippy + fmt all green.
status: completed
completed: 2026-06-02
---

<context>
The four Block-C analytics tools (tier-15b/15c) read data neither catalog holds. `SymbolMeta` drops `SymbolRecord.complexity` [src: crates/ariadne-mcp/src/catalog.rs:45-58; crates/ariadne-daemon/src/domain/catalog.rs:55-68], and the git-history tables (`CHURN`/`CO_CHANGE`/`SYMBOL_CHURN`, tier-11/11b) are never loaded into RAM. This tier extends the two read-only projections — the cold MCP `Catalog` and the warm `WarmCatalog` — to carry that data, so the 15b/15c tools are pure in-RAM reads on both the daemon and cold-fallback paths. Substrate only: no tool, protocol, or dependency change. Full context: plan.md; tier-13 D1.
</context>

<decisions>
- D1 — load churn/co-change/symbol-churn via the `Storage` port, not the read snapshot. `Storage::all_churn`/`all_co_change`/`all_symbol_churn` already return the full persisted vectors [src: crates/ariadne-core/src/domain/ports.rs:69,75,129]; `ReadSnapshot` only iterates files/symbols/edges [src: ports.rs:191-211]. Both builders already hold a `&S: Storage` [src: catalog.rs:88; daemon catalog.rs:97], so the three `all_*` calls slot into the existing build with no port change. *Rejected:* a new snapshot iterator (the port already exposes these; a new one duplicates surface).
- D2 — complexity rides `SymbolMeta`, not a side map. `complexity` is already a field on `SymbolRecord` [src: crates/ariadne-core/src/domain/records.rs:58]; threading it into the existing `SymbolMeta::from_record` keeps one per-symbol record and lets 15b build its file-Σ / per-symbol complexity maps straight from `catalog.symbols`. *Rejected:* a parallel `BTreeMap<SymbolId,u32>` (a second structure to keep in sync with the symbol set).
- D3 — store the analytics vectors as owned `Vec`s, sorted by key on load. They are small relative to the symbol set and are consumed wholesale by the use cases (`file_hotspots(&[FileChurn], …)` etc. [src: crates/ariadne-graph/src/hotspot.rs:102,126; co_change.rs:74]) — a `Vec` is the exact shape they take. Sort on load (path; `(a,b)`; `SymbolId`) so 15b output is deterministic with no re-sort.
</decisions>

<files>
- crates/ariadne-mcp/src/catalog.rs — modify: add `complexity` to `SymbolMeta` + `from_record`; add `churn: Vec<FileChurn>`, `co_change: Vec<CoChangePair>`, `symbol_churn: Vec<SymbolChurn>` to `Catalog`, loaded in `build` via the `Storage` port.
- crates/ariadne-daemon/src/domain/catalog.rs — modify: the identical changes to `WarmCatalog` + its `SymbolMeta`; `apply_changeset` leaves the analytics vectors untouched (a code-edit `Changeset` carries no git-history delta).
- crates/ariadne-daemon/src/domain/dump.rs — modify: thread `complexity` onto `MetaRow` and `churn`/`co_change`/`symbol_churn` onto `CatalogDump` so the divergence-0 `warm_apply_equals_fresh_rebuild` proptest validates the new fields stay rebuild-consistent on the warm-update path; without it that coverage is silently dropped [audit INFO-1; src: crates/ariadne-daemon/src/domain/dump.rs:38,68-70].
- crates/ariadne-mcp/tests/catalog_projection.rs — new: cold-projection test seeding churn/co-change/symbol-churn/complexity and asserting the cold `Catalog` exposes every field, sorted by key.
- crates/ariadne-daemon/src/domain/catalog.rs `#[cfg(test)]` — new: the warm-projection test lives in-module, not under `crates/ariadne-daemon/tests/`, because `WarmCatalog.{churn,co_change,symbol_churn}` are `pub(crate)` and unreachable from an external integration-test crate; it pins the same fixture literals as the cold test, so the two projections are checked field-equal [audit INFO-2].
- crates/ariadne-mcp/tests/support.rs — modify: extend a `seed_*` helper to persist churn/co-change/symbol-churn/complexity for the 15b/15c goldens [src: crates/ariadne-mcp/tests/support.rs:41-90].
</files>

<steps>
1. Failing test first: in `ariadne-mcp` tests, seed a fixture redb (`support.rs` helper) with two files carrying `FileChurn`, one `CoChangePair`, one `SymbolChurn`, and symbols with non-zero `complexity`; build a `Catalog` and assert `catalog.churn/.co_change/.symbol_churn` are non-empty and `catalog.symbols[..].complexity` is populated. Red — those fields do not exist [src: catalog.rs:60-79].
2. Extend `SymbolMeta` (both crates) with `complexity: u32`; set it in `from_record` from `rec.complexity` [src: records.rs:58].
3. Add `churn`/`co_change`/`symbol_churn` to `Catalog` and `WarmCatalog`; in each `build`, call `storage.all_churn()?` / `all_co_change()?` / `all_symbol_churn()?`, sort each by key, and store [src: ports.rs:69,75,129; catalog.rs:88-127; daemon catalog.rs:97-139].
4. In `WarmCatalog::apply_changeset`, leave the three analytics vectors unchanged — a code-edit `Changeset` carries no git-history delta [src: daemon catalog.rs:151-215]; document that a git-history refresh arrives as a full rebuild on the staleness handshake (plan R-B2).
5. Mirror the test on the daemon side (`WarmCatalog::build` from the same fixture) and assert cold-vs-warm field equality for churn/co-change/symbol-churn/complexity. Green.
6. Run the gate; confirm `tests/architecture.rs` is unaffected (no new dep edge); clippy/fmt/doc clean — new fields are documented to satisfy `#![deny(missing_docs)]` where it applies.
</steps>

<verification>
- `cargo nextest run -p ariadne-mcp -p ariadne-daemon` — the projection tests pass: both catalogs expose churn/co-change/symbol-churn/complexity, field-equal across cold and warm for one fixture.
- End-to-end (real, not stub): the test seeds a real redb via `ariadne-storage` and builds the real `Catalog`/`WarmCatalog`, asserting the loaded analytics match the seeded records (no mock storage).
- `cargo test --test architecture` (no new dep edge), `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo fmt --all --check`, `RUSTDOCFLAGS=-D warnings cargo doc -p ariadne-mcp -p ariadne-daemon --no-deps` — green.
</verification>

<rollback>
`git checkout -- crates/ariadne-mcp crates/ariadne-daemon`. Purely additive fields on two read-only catalogs; no on-disk format, protocol, or tool-surface change, so nothing downstream depends on this until 15b.
</rollback>
