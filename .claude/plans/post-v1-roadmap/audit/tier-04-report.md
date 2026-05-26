---
tier_id: tier-04
audited: 2026-05-26
verdict: PASS
commit: d363c2a27cd0b573da8bb871811fc535d0edf5cf
---

<scope>
Tier-04 `SymbolRecord` metadata enrichment — `visibility: Visibility` + `attributes: Vec<String>` threaded core → storage → parser → scip → cli → salsa behind a redb v2 → v3 migration step. Diff scoped to files listed in `.claude/plans/post-v1-roadmap/tier-04-symbol-metadata-enrichment.md` `<files>`, plus the new fixture `crates/ariadne-storage/fixtures/schema-v2.redb` and ADR-0014. Audit is against working-tree state (`HEAD = d363c2a`, tier-04 work not yet committed).
</scope>

<checks_run>
Verification commands re-run end-to-end:
- `cargo nextest run -p ariadne-storage` → 27/27 PASS (incl. `migration::v2_fixture_migrates_in_place_with_symbol_fields_preserved` + `migration::older_version_with_no_migration_path_returns_schema_mismatch`).
- `cargo nextest run -p ariadne-core -p ariadne-parser -p ariadne-scip` → 101/101 PASS.
- `cargo test --test architecture` → 1/1 PASS (hexagonal invariants hold).
- `cargo clippy --workspace --all-targets -- -D warnings` → clean.
- `cargo fmt --all --check` → clean.

Plan adherence reviewed end-to-end:
- `crates/ariadne-core/src/domain/types/visibility.rs:30-40` — `Visibility { Public=0, Restricted=1, Private=2, Unknown=3 }`, `#[non_exhaustive]`, `Default = Unknown` ✓.
- `crates/ariadne-core/src/domain/types/mod.rs:7,12` + `lib.rs:17` — façade re-export ✓.
- `crates/ariadne-core/src/domain/records.rs:42-48` — `visibility` + `attributes` appended after `defining_span`; postcard prefix-extension contract documented in-comment ✓.
- `crates/ariadne-storage/src/adapters/redb/tables.rs:8` — `SCHEMA_VERSION = 3` ✓.
- `crates/ariadne-storage/src/domain/migration.rs:107-159` — frozen `SymbolRecordV2 { canonical_name, kind, defining_file, defining_span }`, `migrate_v2_to_v3` collects then re-inserts, both fields defaulted; runs inside caller `WriteTransaction` ✓ [src: https://docs.rs/redb/4.1.0/redb/struct.WriteTransaction.html].
- `crates/ariadne-storage/src/domain/migration.rs:55-59` — `MigrationStep { from: 2, to: 3, .. }` registered in `MigrationRegistry::builtin` ✓.
- `crates/ariadne-storage/tests/migration.rs:347-392` — `v2_fixture_migrates_in_place_with_symbol_fields_preserved` asserts byte-prefix preservation + default fill on every committed v2 record; fixture regenerator `generate_v2_schema_fixture` `#[ignore]`'d so test runs never overwrite the binary ✓.
- `crates/ariadne-storage/fixtures/schema-v2.redb` — committed (1.1M) ✓.
- `crates/ariadne-parser/src/adapters/treesitter/queries/*.scm` — `@visibility` + `@attribute` captures added per language (Rust `visibility_modifier`/`attribute_item`; Java/Kotlin `modifiers` + `annotation`/`marker_annotation`; C# `modifier` + `attribute_list`; TS/JS/TSX `export_statement` + `accessibility_modifier` (TS only) + `decorator`; Python `decorator`; C `storage_class_specifier` + `attribute_declaration`; C++ adds `access_specifier`). Go has no `@visibility`/`@attribute` capture (delegated to leading-case rule) ✓.
- `crates/ariadne-parser/src/adapters/treesitter/facts.rs:401-402, 478-670` — `attach_visibility` (innermost-contains / largest-contained + Go leading-case fallback) and `attach_attributes` (innermost-contains / next-decl) ✓.
- `crates/ariadne-scip/src/indexer/mod.rs:63-71` — `symbol_visibility`: `local …` → Private, non-empty descriptor → Public, empty → Unknown ✓.
- `crates/ariadne-cli/src/domain/mod.rs:553-565` — `SymbolRecord` construction populates from `decl.visibility`/`decl.attributes`; synthesized SFC component is `Public`/empty ✓.
- `crates/ariadne-salsa/src/derived.rs:36-55, 77-95` — `DeclRaw` + `SymbolFactsRaw` carry `visibility_byte: u8` (single-byte mirror to keep `ariadne-core` dependency-free) + `attributes: Vec<String>`; deviation from plan-letter `Visibility` type is captured in ADR-0014 `<alternatives>` ✓.
- `docs/adr/0014-symbol-metadata-enrichment.md` — Status `Accepted`, dated 2026-05-26, cites plan RD10 + postcard wire-format spec + redb `WriteTransaction` docs + Go exported-identifier rule ✓.

Architecture / file-cap checks:
- `ariadne-core` deps unchanged (still zero in-workspace deps); `Visibility` carries no `salsa` impl — boundary is mirrored to `u8` on the salsa side (architecture invariant preserved).
- Authored tier + ADR file sizes: 62 / 126 lines (under 200 cap).

Exit-criteria reconciliation:
- EC1 (`visibility` + `attributes` on `SymbolRecord`, `Visibility` public): met.
- EC2 (`SCHEMA_VERSION = 3` + `MigrationStep { from: 2, to: 3 }`): met.
- EC3 (v2 file opens, migrates, first four fields byte-identical, new fields defaulted): met by `migration::v2_fixture_migrates_in_place_with_symbol_fields_preserved` over the committed `fixtures/schema-v2.redb`.
- EC4 (`.scm` queries + `Decl` carry visibility + attributes per language; SCIP best-effort): met for `visibility` (snapshots show `Public` / `Unknown` per fixture); `attributes` field present on every `Decl` but all snapshots emit `[]` because no fixture contains `#[test]`/`@Override`/decorator — INFO-1 below.
- EC5 (cli + salsa thread both fields): met.
- EC6 (ADR-0014 records the enrichment + postcard-migration approach): met.
- EC7 (nextest + architecture + clippy + fmt green): met (re-run above).
</checks_run>

<findings>

| id | category | severity | location | problem | fix | source |
|----|----------|----------|----------|---------|-----|--------|
| F1 | tests | INFO | `crates/ariadne-parser/fixtures/*` + `crates/ariadne-parser/tests/snapshots/*` | Every committed snapshot has `attributes: []`. No fixture contains a Rust `#[test]`, Java `@Override`/`@Test`, C# `[Attribute]`, Python `@decorator`, or TS decorator, so the `attach_attributes` path (innermost-contains + next-decl heuristic) and `attribute_name` extraction are entirely uncovered by automated tests. The plan's `<verification>` line states "parser fact tests assert `pub fn`/`#[test]`/exported decls carry the expected `Visibility` and `attributes`"; the visibility half is asserted, the attributes half relies on the manual `ariadne index` step. | Add one fixture per attribute-carrying grammar (e.g., a Rust source with `#[test] fn t() {}` and `#[derive(Debug)] struct X;`, a Java method with `@Override`, a TS class with a `@decorator`) and a snapshot that pins the resulting `attributes` vector. | `.claude/plans/post-v1-roadmap/tier-04-symbol-metadata-enrichment.md` `<verification>`; `crates/ariadne-parser/src/adapters/treesitter/facts.rs:521-542, 672-692` |
| F2 | correctness | INFO | `crates/ariadne-parser/src/adapters/treesitter/facts.rs:506-518` + `crates/ariadne-core/src/domain/types/visibility.rs:11-13` | A Rust item without `pub` (e.g., `mod inner`) resolves to `Visibility::Unknown` rather than `Visibility::Private`, because the `visibility_modifier` query fires only when a modifier token is present and `attach_visibility`'s fallback yields `Unknown` for non-Go grammars. The doc comment on `Visibility::Private` advertises "Rust default" as Private, so the actually-private items intentionally drop signal that the tier-05 dead-code classifier (RD4) is consuming. Behaviour matches the plan-letter step 7 ("Where a grammar exposes neither signal, emit `Visibility::Unknown`"), so it is not a tier-04 defect, but the doc/behaviour split will surprise the tier-05 implementer. | Either (a) document on the type that "default-private" languages still emit `Unknown` when no modifier token is captured, and remove "Rust default" from the `Private` doc comment; or (b) widen `attach_visibility`'s fallback to map `Lang::Rust` non-modifier decls to `Private`. Decide as part of tier-05's classifier design. | `crates/ariadne-parser/src/adapters/treesitter/queries/rust.scm:46`; `crates/ariadne-parser/fixtures/rust/sample.rs:33-37`; snapshot `facts_rust__facts_rust_sample.snap:107-118` (the `inner` mod). |
| F3 | maintainability | INFO | `crates/ariadne-core/src/domain/types/visibility.rs:25-40` | `Visibility` derives `PartialOrd`/`Ord` with `#[repr(u8)]` discriminants `Public=0 < Restricted=1 < Private=2 < Unknown=3`, so the derived comparison puts `Public` as the *least* visible variant. `attach_visibility`'s `visibility_max` uses the opposite intuition (`Public=3` strongest, `Unknown=0` weakest) via a private rank table. The two definitions agree only because no current code path uses derived `Ord`; a future consumer who writes `if vis >= Visibility::Restricted` will get the wrong half-plane. | Drop the `PartialOrd`/`Ord` derive on `Visibility` (leave `PartialEq`/`Eq`/`Hash`) and ship the lattice ordering as an inherent `fn rank(self) -> u8` mirroring the one in `visibility_max`, or reorder discriminants so derive matches the rank table. | `crates/ariadne-core/src/domain/types/visibility.rs:25-40`; `crates/ariadne-parser/src/adapters/treesitter/facts.rs:594-603`. |

No FAIL findings.

</findings>

<verdict>
PASS.

The tier ships the `Visibility` enum, threads `visibility` + `attributes` through `SymbolRecord` / `Decl` / `SymbolFactsRaw` / `DeclRaw`, bumps `SCHEMA_VERSION` to `3` with a registered `MigrationStep { 2 -> 3 }` that re-encodes the `SYMBOLS` table inside the existing single `WriteTransaction`, commits a `fixtures/schema-v2.redb` v2 binary, and pins the round-trip assertion that every v2 record's first four fields survive byte-identical with the new fields defaulted to `Unknown` / empty. ADR-0014 records the decision and the postcard limitation. All `<verification>` commands re-run green. The architecture invariant test still passes — `ariadne-core` is unchanged dependency-wise, and the salsa boundary mirrors `Visibility` as a single byte rather than depending on `salsa::Update` from the domain crate (deviation acknowledged in ADR-0014 `<alternatives>`).
</verdict>

<next_steps>
- F1 (tests): add `#[test]`/`@Override`/decorator fixtures in a follow-up to lift `attribute_name` + `attach_attributes` into the snapshot net. Track inside tier-05 (the consumer) or open a small follow-up tier.
- F2 (visibility default): pick one of the two resolutions before tier-05 lands the dead-code classifier — the classifier's roots logic should not depend on the Rust `mod inner` -> `Unknown` ambiguity.
- F3 (lattice ordering): either drop the derive or align the discriminants. Cheap; do it before any new consumer reads `Visibility` via `>=`.
</next_steps>

<sources>
- [redb WriteTransaction](https://docs.rs/redb/4.1.0/redb/struct.WriteTransaction.html)
- [postcard wire format](https://postcard.jamesmunns.com/wire-format)
- [Go exported identifiers](https://go.dev/ref/spec#Exported_identifiers)
- [tree-sitter query syntax](https://tree-sitter.github.io/tree-sitter/using-parsers/queries/1-syntax.html)
- `.claude/plans/post-v1-roadmap/plan.md` RD10
- `.claude/plans/post-v1-roadmap/tier-04-symbol-metadata-enrichment.md`
- `docs/adr/0014-symbol-metadata-enrichment.md`
</sources>
