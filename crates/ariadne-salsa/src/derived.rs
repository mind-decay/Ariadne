//! Salsa `#[tracked]` derived queries (tier-04 step 4).
//!
//! Adaptations from the plan letter:
//!
//! * `parse_tree` was dropped as a separately tracked query because
//!   `tree_sitter::Tree` does not implement `salsa::Update` and a safe
//!   newtype wrapper is not possible — user-approved in this build session.
//! * Per the architecture invariant (ariadne-salsa may not depend on
//!   ariadne-parser/ariadne-scip; [src: tests/architecture.rs]),
//!   parsing and SCIP ingest are pushed into a driver layer that writes
//!   results back through the salsa input setters in later tiers. For
//!   tier-04 these queries return empty stubs — the cache-hit invariant
//!   and the per-revision query graph are what matter at this tier.
//!
//! All tracked-return types use only `Update`-friendly fields (`String`,
//! `u64`, `Vec<…>`, `[u8; N]`, derived structs with `salsa::Update`).

use std::sync::Arc;

use crate::inputs::{FileContentInput, ScipFactsInput, SyntacticFactsInput};

/// Syntactic facts pulled from a parsed file. Mirrors
/// `ariadne_parser::SyntacticFacts` but uses only `Update`-friendly types
/// — the driver layer converts at the boundary. `renders` + `hooks` carry the
/// component-graph sites (tier-07a) so the moved edge resolution emits the
/// same `Renders` / `UsesHook` edges as the CLI committer did
/// [src: crates/ariadne-parser/src/adapters/treesitter/facts.rs:144-156].
#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct SyntacticFactsRaw {
    /// Declarations in source order.
    pub decls: Vec<DeclRaw>,
    /// Imports in source order.
    pub imports: Vec<ImportRaw>,
    /// Call sites in source order.
    pub calls: Vec<CallRaw>,
    /// JSX/TSX render sites in source order.
    pub renders: Vec<RenderRaw>,
    /// Hook / reactive-primitive call sites in source order.
    pub hooks: Vec<HookRaw>,
}

/// Declaration record (driver-faced, `Update`-safe).
#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct DeclRaw {
    /// Free-form kind tag (mirrors `ariadne_parser::DeclKind` tag suffix).
    pub kind: String,
    /// Identifier text.
    pub name: String,
    /// `(byte_start, byte_end)` of the name node.
    pub name_byte_range: (u32, u32),
    /// `(byte_start, byte_end)` of the declaration node.
    pub def_byte_range: (u32, u32),
    /// Mirror of `ariadne_core::Visibility` as a single byte
    /// (`Visibility::to_byte`). The driver layer encodes at the boundary —
    /// `salsa::Update` has no auto-impl for non-`std` `Copy` types in
    /// salsa 0.26, so the mirror keeps the salsa node `Update`-safe
    /// [src: <https://docs.rs/salsa/0.26.2/salsa/trait.Update.html>].
    pub visibility_byte: u8,
    /// Attribute / annotation / decorator identifiers attached to the
    /// decl (e.g. `"test"` for Rust `#[test]`).
    pub attributes: Vec<String>,
    /// `McCabe` cyclomatic complexity for function-like decls; `0` otherwise.
    /// Carried verbatim from `ariadne_parser::Decl::complexity` at the
    /// composition-root boundary [src: post-v1-roadmap tier-12 step 6].
    pub complexity: u32,
}

/// Import record.
#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct ImportRaw {
    /// Raw module path text.
    pub path: String,
    /// `(byte_start, byte_end)` of the import-path node.
    pub byte_range: (u32, u32),
}

/// Call-site record.
#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct CallRaw {
    /// Callee identifier text.
    pub callee: String,
    /// Call shape as a single byte (`0=Free`, `1=Method`, `2=Path`), mapped
    /// from `ariadne_parser::CallKind` at the composition root. A fieldless
    /// enum has no `salsa::Update` auto-impl, so the salsa boundary uses a byte
    /// mirror like `DeclRaw::visibility_byte`; the resolver decodes it to gate
    /// the cross-crate fallback to `Free` calls
    /// [src: <https://docs.rs/salsa/0.26.2/salsa/trait.Update.html>; ADR-0024].
    pub kind_byte: u8,
    /// `(byte_start, byte_end)` of the callee identifier.
    pub byte_range: (u32, u32),
}

/// JSX/TSX render-site record — one child-component element (`<Child/>`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct RenderRaw {
    /// Rendered component's tag-name identifier text.
    pub component: String,
    /// `(byte_start, byte_end)` of the tag-name identifier.
    pub byte_range: (u32, u32),
}

/// Hook / reactive-primitive call-site record (`useState`, `createSignal`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct HookRaw {
    /// Hook callee identifier text.
    pub callee: String,
    /// `(byte_start, byte_end)` of the callee identifier.
    pub byte_range: (u32, u32),
}

/// One SCIP occurrence (salsa-internal mirror of `ariadne_core::ScipOccurrence`).
/// The composition root extracts these with `ariadne-scip` and feeds them in via
/// [`ScipFactsInput`]; `ariadne-salsa` may not depend on `ariadne-scip`
/// [src: tests/architecture.rs lines 30-43], so this `Update`-safe mirror exists
/// for the salsa boundary, exactly as [`SyntacticFactsRaw`] mirrors the parser's
/// facts (scip-driven-edges plan D2).
#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct ScipOccurrenceRaw {
    /// Normalized SCIP symbol key (canonical-symbol id hex).
    pub symbol: String,
    /// `(byte_start, byte_end)` of the occurrence in the file's source bytes.
    pub byte_range: (u32, u32),
    /// SCIP `SymbolRole` bitset (`Definition = 0x1`, `Import = 0x2`, …).
    pub roles: u32,
}

/// One SCIP relationship (salsa-internal mirror of
/// `ariadne_core::ScipRelationship`). `from`/`to` are normalized symbol keys the
/// composition root resolves through the same global map as the occurrences;
/// the two flags select the edge kind in `resolve_scip_edges`
/// (scip-driven-edges plan D2, T3).
#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct ScipRelationshipRaw {
    /// Normalized key of the owning symbol (the `from` endpoint).
    pub from: String,
    /// Normalized key of the related symbol (the `to` endpoint).
    pub to: String,
    /// SCIP `is_implementation` (→ `EdgeKind::Implements`).
    pub is_implementation: bool,
    /// SCIP `is_type_definition` (→ `EdgeKind::TypeOf`).
    pub is_type_definition: bool,
}

/// All SCIP facts for one file (salsa-internal mirror of
/// `ariadne_core::ScipFacts`'s occurrence + relationship lists). The file's
/// indexed content hash rides alongside on [`ScipFactsInput`] rather than in
/// this struct so the coverage gate can read it without deep-cloning the
/// occurrence vector (scip-driven-edges plan D2, D4).
#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct ScipFactsRaw {
    /// Occurrences in extraction order.
    pub occurrences: Vec<ScipOccurrenceRaw>,
    /// Relationships declared on this file's symbols (scip-driven-edges T3).
    pub relationships: Vec<ScipRelationshipRaw>,
}

/// Symbol record. The salsa-internal mirror of `ariadne_core::SymbolRecord`,
/// using only `Update`-friendly fields.
#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct SymbolFactsRaw {
    /// Canonical symbol name.
    pub canonical_name: String,
    /// Free-form kind tag.
    pub kind: String,
    /// Defining file id, encoded as raw `u32`. Drivers convert to
    /// `ariadne_core::FileId` at the boundary.
    pub defining_file_raw: u32,
    /// `(byte_start, byte_end)` of the defining occurrence.
    pub defining_byte_range: (u32, u32),
    /// Mirror of `ariadne_core::Visibility` as a single byte
    /// (`Visibility::to_byte`); see `DeclRaw::visibility_byte` for why the
    /// salsa boundary uses a byte mirror.
    pub visibility_byte: u8,
    /// Attribute / annotation / decorator identifiers on the defining
    /// occurrence.
    pub attributes: Vec<String>,
    /// `McCabe` cyclomatic complexity for function-like symbols; `0` otherwise.
    /// The driver writes it into `SymbolRecord::complexity` at commit
    /// [src: post-v1-roadmap tier-12 step 6].
    pub complexity: u32,
}

/// Edge record (salsa-internal mirror).
#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct EdgeFactsRaw {
    /// Source symbol id (raw `u64`).
    pub src_raw: u64,
    /// Edge kind: 0=Defines, 1=References, 2=Imports — mirrors
    /// `ariadne_core::EdgeKind::to_byte`.
    pub kind_byte: u8,
    /// Destination symbol id (raw `u64`).
    pub dst_raw: u64,
    /// `(file_raw, byte_start, byte_end)` evidence span.
    pub evidence_span: (u32, u32, u32),
    /// Tier-07 coupling weight; tier-04 leaves it 0.
    pub weight: u32,
}

/// Syntactic facts for a file (tier-07a). The parsed facts enter salsa via
/// [`SyntacticFactsInput`] because `ariadne-salsa` may not depend on
/// `ariadne-parser` [src: tests/architecture.rs lines 30-33]; a composition
/// root (the CLI cold index, the daemon warm derive) parses and sets the
/// input. The query also touches [`FileContentInput::content`] so a watcher
/// content edit still invalidates it — the daemon re-parses and resets the
/// facts input on that same edit [src: post-v1-roadmap plan.md RD11].
#[salsa::tracked]
pub fn syntactic_facts(
    db: &dyn salsa::Database,
    file: FileContentInput,
    facts: SyntacticFactsInput,
) -> Arc<SyntacticFactsRaw> {
    let _ = file.content(db).len();
    let _ = file.path(db);
    Arc::new(facts.facts(db))
}

/// SCIP facts for a file. The composition root decodes the SCIP protobuf with
/// `ariadne-scip` and feeds the occurrences in through [`ScipFactsInput`] —
/// `ariadne-salsa` may not depend on `ariadne-scip` [src: tests/architecture.rs
/// lines 30-43], so extraction cannot live in salsa, mirroring how
/// [`syntactic_facts`] takes parsed facts. Empty until a root populates it;
/// [`crate::AriadneDb::commit_revision`] reads it (and [`ScipFactsInput::indexed_hash`])
/// for the covered-file SCIP edge pass (scip-driven-edges plan D2, D4). Symbols
/// stay tree-sitter — SCIP feeds edges only (plan D1) — so this drives no
/// `symbols_for_file` merge.
#[salsa::tracked]
pub fn scip_facts_for_file(db: &dyn salsa::Database, scip: ScipFactsInput) -> Arc<ScipFactsRaw> {
    Arc::new(scip.facts(db))
}

/// Per-file symbols: the parsed facts' decls become symbols (plus a synthesized
/// SFC `Component` symbol) via `crate::derive::build_symbols`. This is the
/// memoized per-file step the driver collects in
/// [`crate::AriadneDb::commit_revision`]. SCIP drives edges only (plan D1), so
/// symbols are tree-sitter-authoritative and this query does not depend on the
/// SCIP input [src: post-v1-roadmap plan.md RD11; scip-driven-edges plan D1].
#[salsa::tracked]
pub fn symbols_for_file(
    db: &dyn salsa::Database,
    file: FileContentInput,
    facts: SyntacticFactsInput,
) -> Arc<Vec<SymbolFactsRaw>> {
    let raw = syntactic_facts(db, file, facts);
    let rel_path = file.path(db);
    let file_len = u32::try_from(file.content(db).len()).unwrap_or(u32::MAX);
    Arc::new(crate::derive::build_symbols(&rel_path, file_len, &raw))
}

/// Per-file edges. Tier-04 stub returns empty; the real per-file edge signal is
/// resolved by the global driver pass in [`crate::AriadneDb::commit_revision`]
/// (tree-sitter `resolve_edges` and SCIP `resolve_scip_edges`), not memoized
/// here. Retained so the per-table memory probe lists the `edges_for_file`
/// table.
#[salsa::tracked]
pub fn edges_for_file(
    db: &dyn salsa::Database,
    file: FileContentInput,
    _scip: ScipFactsInput,
) -> Arc<Vec<EdgeFactsRaw>> {
    let _ = file.content(db).len();
    Arc::new(Vec::new())
}

/// Blast-radius stub. Real algorithm lands in tier-07; tier-04 needs the
/// query node so dependency tracking is in place. The `_depth` arg is held
/// as a value rather than a salsa input so callers can drive recursion
/// limits without an extra revision bump.
#[salsa::tracked]
pub fn blast_radius(db: &dyn salsa::Database, sym_raw: u64, _depth: u8) -> Arc<Vec<u64>> {
    let _ = db;
    let _ = sym_raw;
    Arc::new(Vec::new())
}
