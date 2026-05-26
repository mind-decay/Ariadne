# ADR-0014: SymbolRecord metadata enrichment — visibility + attributes

<status>
Accepted
Date: 2026-05-26
Decider: user / claude
</status>

<context>
v1's `SymbolRecord` carries only `canonical_name`, `kind`, `defining_file`,
and `defining_span` [src:
`crates/ariadne-core/src/domain/records.rs`]. The post-v1 dead-code
classifier (plan-RD4, landing in tier-05) needs per-language entry-point
signals — Rust `pub`/`#[test]`, JS/TS `export`, Java/Kotlin/C# annotations,
Python `__main__` — that no field on the v1 record exposes. The plan's RD10
adds those signals as `Visibility` + `attributes` on `SymbolRecord`,
threaded core → storage → parser → scip → cli → salsa.

The redb adapter persists `SymbolRecord` bodies through `postcard`, which is
non-self-describing — struct field count and names are not on the wire
[src: <https://postcard.jamesmunns.com/wire-format>]. Appending fields to
`SymbolRecord` therefore breaks decode of an existing v2 database; a
schema-format bump from v2 to v3 is required, plus a registered migration
step that re-encodes every `SYMBOLS` body in place. The migration framework
landed in tier-02 / ADR-0002 stays the carrier.

This ADR fixes both decisions for tier-04 and unblocks tier-05.
</context>

<decision>
Extend `ariadne-core::SymbolRecord` with two new fields appended after
`defining_span`: `visibility: Visibility` and `attributes: Vec<String>`.
`Visibility` is a public, `#[non_exhaustive]`, four-variant lattice
(`Public` / `Restricted` / `Private` / `Unknown`, default `Unknown`).
The on-disk schema bumps from v2 to v3; a registered
`MigrationStep { from: 2, to: 3, … }` re-encodes the `SYMBOLS` table
record-by-record via a frozen `SymbolRecordV2` decode followed by a v3
encode with the new fields defaulted.
</decision>

<rationale>
- **Maintainability.** The four-variant lattice spans the ten language
  visibility models Ariadne ingests in a single typed enum; consumers (the
  RD4 dead-code classifier, future LSP filters) read a closed set instead
  of re-parsing per-language modifier strings. The split between
  `Restricted` and `Private` keeps `pub(crate)` / `protected` /
  `internal` distinguishable from outright private symbols — important
  for diff-aware blast-radius (tier-14).
- **Reliability.** postcard's positional encoding means an in-place
  rewrite is the only safe migration. The v3 layout extends the v2 byte
  prefix unchanged; a frozen `SymbolRecordV2` struct (lives in
  `ariadne-storage::domain::migration`) drives the decode, and the
  whole pass runs inside the redb `WriteTransaction` already started by
  the migration runner — a mid-pass crash leaves the file at v2 (ACID)
  [src: `crates/ariadne-storage/src/domain/migration.rs`].
- **Efficiency.** The migration touches the `SYMBOLS` table exactly
  once; the `FILES` and `EDGES` tables are unchanged, so v2 → v3 cost
  is linear in `|symbols|` and reuses the existing postcard encode
  helpers — no second column store, no rebuild [src:
  `crates/ariadne-storage/src/adapters/codec.rs`].
- **Scalability.** Keeping `Visibility` `Copy` (single-byte repr) and
  `attributes: Vec<String>` short keeps the per-record overhead small
  even on the 100K-file workload SLO — the postcard tail is one varint
  tag plus the attribute vector length.

The SCIP `SymbolInformation` proto has no visibility or attribute fields
[src: `crates/ariadne-scip/proto/scip.proto`]; the SCIP ingest path
folds the `local <id>` / descriptor form distinction onto
`Visibility::Private` / `Visibility::Public` via the
`ariadne_scip::symbol_visibility` helper, leaves `attributes` empty,
and the limitation is noted in this ADR.
</rationale>

<alternatives>
- **Raw per-language modifier strings on `SymbolRecord`.** Rejected —
  every downstream consumer would re-parse, and the lattice would not
  be typed across languages. Defeats the maintainability case for
  the enrichment.
- **Rebuild-on-open on the format bump.** Rejected — discards SCIP
  ingest cost on every version change; this is the exact failure mode
  RD2 / ADR-0002 fixed.
- **Side-table for visibility / attributes keyed on `SymbolId`.**
  Rejected — doubles the read fan-out for every query that wants the
  new fields, and a per-`SymbolRecord` lookup is the hottest path.
- **Derive `salsa::Update` for `Visibility` in `ariadne-core`.**
  Rejected — `ariadne-core` is dependency-free per the architecture
  invariant; the salsa boundary mirrors `Visibility` as a single byte
  on `SymbolFactsRaw` / `DeclRaw`, matching the existing `kind_byte`
  pattern on `EdgeFactsRaw`.
</alternatives>

<consequences>
- `SCHEMA_VERSION` advances to `3`; the registered migration chain now
  spans `v1 → v2 → v3`. A binary built after this ADR refuses to open
  databases at versions outside `[1, 3]` with `SchemaMismatch`.
- The frozen `SymbolRecordV2` struct in
  `ariadne-storage::domain::migration` is permanent — it is the
  contract for any future v3+ migration to read v2 bodies. Future
  format bumps add their own frozen struct.
- The parser captures `@visibility` and `@attribute` from per-language
  `.scm` queries and attaches them to decls in a byte-range post-pass
  (innermost-containing / largest-contained / next-after). The
  attachment heuristic is captured in `attach_visibility` /
  `attach_attributes` and is the audit surface for future grammar
  drift.
- The SCIP visibility mapping is best-effort and explicitly documented
  as a known limitation; tier-05 RD4 consumes the syntactic-path
  signal first, then the SCIP fallback.
- Tier-05's dead-code classifier may now read `visibility` +
  `attributes` directly off `SymbolRecord` — the unlock RD10 was
  written to provide.
</consequences>

<sources>
- [src: <https://postcard.jamesmunns.com/wire-format>]
- [src: <https://docs.rs/redb/4.1.0/redb/struct.WriteTransaction.html>]
- [src: <https://go.dev/ref/spec#Exported_identifiers>]
- [src: <https://tree-sitter.github.io/tree-sitter/using-parsers/queries/1-syntax.html>]
- [src: `.claude/plans/post-v1-roadmap/plan.md` RD10]
- [src: `.claude/plans/post-v1-roadmap/tier-04-symbol-metadata-enrichment.md`]
- [src: `crates/ariadne-core/src/domain/records.rs`]
- [src: `crates/ariadne-storage/src/domain/migration.rs`]
- [src: `crates/ariadne-storage/src/adapters/codec.rs`]
- [src: `crates/ariadne-parser/src/adapters/treesitter/facts.rs`]
- [src: `crates/ariadne-scip/src/indexer/mod.rs`]
</sources>
