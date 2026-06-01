---
tier_id: tier-12
audited: 2026-06-01
verdict: PASS
commit: 5ba1fbcea0693a165bf6578d6c65259e55361fd1
---

<scope>
Tier-12 "Cyclomatic complexity — McCabe metric per function-like symbol from
the tree-sitter CST". Reviewed the working-tree diff scoped to the tier's
`<files>` plus the compile-forced cascade outside it. New files: parser
`complexity.rs`, parser `tests/complexity.rs`, `fixtures/schema-v6.redb`,
`docs/adr/0020-cyclomatic-complexity.md`. Modified: core `records.rs`; parser
`facts.rs`/`mod.rs` + 14 `facts_*.snap`; storage `migration.rs`/`tables.rs`/
`tests/migration.rs`; salsa `derived.rs`/`derive.rs`/`db.rs`/`memory.rs`; cli
`domain/mod.rs`. Out-of-scope-but-touched (compile-forced by the new struct
fields): daemon `facts.rs`/benches/tests, mcp benches/tests, graph tests,
salsa tests, storage `changeset`/`support`/golden snapshot.
Base: HEAD 5ba1fbc; tier-12 work is uncommitted in the working tree.
</scope>

<checks_run>
- Read every changed file end-to-end + both new source files and the ADR.
- `cargo nextest run -p ariadne-parser -p ariadne-storage -p ariadne-core -p ariadne-salsa -p ariadne-cli` → 145 passed, 3 skipped, 0 failed.
- `cargo nextest run -p ariadne-parser --test complexity` → 10/10 goldens pass (rust/python/js/ts/go/java/csharp/kotlin/c/cpp).
- `cargo nextest run -p ariadne-storage` → 47 passed (incl. v1/v2/v6 fixture migrations + v6→v7 & v1→v7 contiguity).
- `cargo test --test architecture` → 1 passed (hexagonal invariants hold).
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` → exit 0, zero warnings (proves the whole workspace, incl. all cascade sites, compiles clean).
- `cargo fmt --all --check` → exit 0.
- Hand-verified golden counts independently (rust branchy=4, boolean=3, outer/inner=2/2; go selector=3; java guarded=2; c selector=3; kotlin whenly=3) — goldens use explicit `assert_eq!` with hand-derived comments, not snapshot-accepted, so they are a real check.
- Spot-checked regenerated snapshots (rust/vue/svelte/astro): non-function decls 0, branchless fns 1, no SFC-template inflation.
- Migration chain traced: v3→v4/v4→v5/v5→v6 are purely additive (do not touch SYMBOLS), so the only SYMBOLS re-decode in the chain is v2→v3 (genuine 4-field source) and v6→v7 (`SymbolRecordV6` frozen 6-field prefix). The v2/v1 fixture tests read records back through the full chain and pass, confirming `postcard::from_bytes` tolerates the single trailing `complexity` byte a v2→v3 step now writes.
- Context7 query for postcard trailing-byte semantics returned "monthly quota exceeded"; substituted the empirical full-chain fixture tests as authoritative evidence.
</checks_run>

<findings>
| id | category | severity | location | problem | fix |
|----|----------|----------|----------|---------|-----|
| F1 | correctness | INFO | crates/ariadne-salsa/src/derive.rs:95-102 | The salsa-synthesized SFC component is `kind:"component"` with `complexity:0`; a literal read of exit-criterion 1 ("function-like symbols carry >=1") wants >=1, and empty-body McCabe is 1. Intentional + code-documented (tier-13 treats 0 as non-hotspot; the node has no measurable body and its script decls carry real complexity); no consumer impact. Outside the parser metric `attach_complexity` ever sees. | Optional: set to 1 for >=1 consistency, or add this synthetic-node case to ADR-0020's limitations list. |
| F2 | plan_adherence | INFO | crates/ariadne-daemon/src/domain/facts.rs:88 (+ ~12 test/bench/support sites) | Tier `<files>` under-enumerated the threading sites: the daemon's production `convert_facts` and the proptest/test/bench/snapshot construction sites are not listed. All are compile-forced by adding `complexity` to `SymbolRecord`/`DeclRaw`/`SymbolFactsRaw` and are correctly handled (daemon threads `d.complexity`; `arb_symbol_record` fuzzes via `any::<u32>()`). | None required; a plan-completeness nit, not a code defect. |
</findings>

<verdict>
PASS. Zero FAIL findings. Two INFO nits, neither gating.

All six exit criteria independently verified:
1. `SymbolRecord.complexity: u32` after `attributes`; parser function-like decls carry `decisions+1` (>=1), others 0 — `records.rs:55`, `complexity.rs:58-64`, goldens. (F1: synthetic SFC component=0, non-blocking.)
2. One CST `TreeCursor` walk (`complexity.rs:33-57`); `&&`/`||` counted (`is_boolean_operator`, operator-field detection avoids miscounting Rust `&&x` ref patterns); nested attribution via `innermost_containing_decl` — goldens prove parent not inflated.
3. `SCHEMA_VERSION=7` (`tables.rs:8`); `MigrationStep{from:6,to:7}` re-encodes SYMBOLS in place, `complexity=0`; v6 fixture migrates with first six fields byte-identical (`migration.rs` test asserts each).
4. Ten-language goldens assert hand-counted branchy + nested-function (rust/py/js/ts) + boolean cases — 10/10 green.
5. ADR-0020 records the metric, strict-McCabe (D2), decl-span boundary (D3), and arrow-as-variable limitation (D4), Status Accepted.
6. nextest (5 crates) + architecture + clippy + fmt all green.

The migration is the highest-risk surface and is sound: the frozen `SymbolRecordV6` captures the v6 byte prefix exactly, the collect-then-reinsert shape avoids redb's live-iterator-vs-insert hazard, the pass runs inside one `WriteTransaction` (crash-safe to v6), and the full v1→v7 / v2→v7 chains round-trip real records.
</verdict>

<next_steps>
None required to pass. Optional follow-ups (non-blocking):
- F1: decide 0 vs 1 for the synthesized SFC component and, if kept at 0, add it to ADR-0020's recorded-limitations list so the divergence from ">=1 for function-like" is documented where consumers look.
- F2: when authoring future field-threading tiers, enumerate the daemon `convert_facts` and proptest/support construction sites in `<files>` so the diff scope matches reality.
</next_steps>

<sources>
- [src: crates/ariadne-parser/src/adapters/treesitter/complexity.rs] — single-walk counter + per-Lang decision predicate.
- [src: crates/ariadne-storage/src/domain/migration.rs:236-289] — frozen `SymbolRecordV6` + `migrate_v6_to_v7`.
- [src: crates/ariadne-parser/tests/complexity.rs] — ten hand-counted goldens.
- [src: docs/adr/0020-cyclomatic-complexity.md] — metric + recorded limitations.
- [src: https://en.wikipedia.org/wiki/Cyclomatic_complexity ; McCabe, "A Complexity Measure", IEEE TSE 1976] — strict M = decisions + 1.
- [src: https://postcard.jamesmunns.com/wire-format] — non-self-describing codec (prefix-extension migration basis); trailing-byte tolerance confirmed empirically via fixture tests (Context7 quota-exhausted).
- [src: https://google.github.io/eng-practices/review/reviewer/standard.html] — code-health-over-perfection severity standard.
</sources>
